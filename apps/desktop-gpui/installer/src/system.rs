use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use frameever_installer_app::{
    flow::{AppState, Closed, Holder, Installed, System},
    instance_control::{Control, ENDPOINT_EXTENSION, PIDFILE_NAME, Reply},
    measure::drive_letter,
};
use frameever_utils::brand::{APP_ID, APP_NAME};

const IMAGE_NAMES: [&str; 2] = [
    concat!(frameever_utils::app_name!(), ".exe"),
    "frameever-gpui.exe",
];
const ASK_TIMEOUT: Duration = Duration::from_secs(3);
const QUIT_WAIT: Duration = Duration::from_secs(30);
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\FrameEver";
const PRODUCT_KEY: &str = "Software\\aniBullet\\FrameEver";
const INSTALLED_ENV: &str = "FRAMEEVER_INSTALLER_INSTALLED";

pub fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().chain(std::iter::once(0)).collect()
}

fn from_wide(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

pub fn this_machine(app_data: PathBuf) -> System {
    let for_app = app_data.clone();
    System {
        volume: Box::new(|path: &Path| drive_letter(path).map(|letter| format!("{letter}:"))),
        free_space: Box::new(free_space),
        holders: Box::new(holders),
        app: Box::new(move || app_state(&for_app)),
    }
}

fn free_space(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let path = wide(path.as_os_str());
    let mut available = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

pub fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(path),
                Ok(_) => files.push(path),
                Err(_) => {}
            }
        }
    }
    files
}

fn image_path(pid: u32) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    };
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut image = [0_u16; 1024];
    let mut len = image.len() as u32;
    let found =
        unsafe { QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut len) } != 0;
    unsafe { CloseHandle(process) };
    found.then(|| PathBuf::from(std::ffi::OsString::from_wide(&image[..len as usize])))
}

fn is_frameever_image(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy())
        .is_some_and(|name| {
            IMAGE_NAMES
                .iter()
                .any(|image| name.eq_ignore_ascii_case(image))
        })
}

fn holders(dir: &Path) -> Vec<Holder> {
    use windows_sys::Win32::{
        Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS},
        System::RestartManager::{
            CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
            RmStartSession,
        },
    };
    let files: Vec<Vec<u16>> = files_in(dir)
        .iter()
        .map(|path| wide(path.as_os_str()))
        .collect();
    if files.is_empty() {
        return Vec::new();
    }
    let pointers: Vec<*const u16> = files.iter().map(|file| file.as_ptr()).collect();
    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    if unsafe { RmStartSession(&mut session, 0, key.as_mut_ptr()) } != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut found = Vec::new();
    let registered = unsafe {
        RmRegisterResources(
            session,
            pointers.len() as u32,
            pointers.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        )
    };
    if registered == ERROR_SUCCESS {
        let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
        for _ in 0..3 {
            let mut needed = 0u32;
            let mut count = infos.len() as u32;
            let mut reasons = 0u32;
            let status = unsafe {
                RmGetList(
                    session,
                    &mut needed,
                    &mut count,
                    if infos.is_empty() {
                        std::ptr::null_mut()
                    } else {
                        infos.as_mut_ptr()
                    },
                    &mut reasons,
                )
            };
            if status == ERROR_MORE_DATA {
                infos = vec![unsafe { std::mem::zeroed() }; needed as usize];
                continue;
            }
            if status == ERROR_SUCCESS {
                infos.truncate(count as usize);
                let own = std::process::id();
                for info in &infos {
                    let pid = info.Process.dwProcessId;
                    if pid == own {
                        continue;
                    }
                    let image = image_path(pid);
                    let frameever = image.as_deref().is_some_and(is_frameever_image);
                    let name = from_wide(&info.strAppName);
                    let name = if name.is_empty() {
                        image
                            .as_ref()
                            .and_then(|path| path.file_stem())
                            .map(|stem| stem.to_string_lossy().into_owned())
                            .unwrap_or_else(|| pid.to_string())
                    } else {
                        name
                    };
                    found.push(Holder {
                        name: if frameever {
                            APP_NAME.to_string()
                        } else {
                            name
                        },
                        frameever,
                    });
                }
            }
            break;
        }
    }
    unsafe { RmEndSession(session) };
    found
}

struct Endpoint {
    pid: u32,
    port: u16,
    secret: u64,
}

fn endpoint(app_data: &Path) -> Option<Endpoint> {
    let path = app_data
        .join(PIDFILE_NAME)
        .with_extension(ENDPOINT_EXTENSION);
    let contents = std::fs::read_to_string(path).ok()?;
    let mut parts = contents.trim().split(':');
    let pid = parts.next()?.parse().ok()?;
    let port = parts.next()?.parse().ok()?;
    let secret = u64::from_str_radix(parts.next()?, 16).ok()?;
    (pid != 0 && port != 0 && parts.next().is_none()).then_some(Endpoint { pid, port, secret })
}

fn frameever_running() -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };
    let own = std::process::id();
    let mut running = false;
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more && !running {
            let name = from_wide(&entry.szExeFile);
            running = entry.th32ProcessID != own
                && IMAGE_NAMES
                    .iter()
                    .any(|image| name.eq_ignore_ascii_case(image));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    running
}

pub fn ask(app_data: &Path, control: Control) -> std::io::Result<Reply> {
    let endpoint = endpoint(app_data).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "FrameEver has no endpoint")
    })?;
    if !image_path(endpoint.pid)
        .as_deref()
        .is_some_and(is_frameever_image)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the endpoint belongs to no running FrameEver",
        ));
    }
    let mut stream = std::net::TcpStream::connect_timeout(
        &(std::net::Ipv4Addr::LOCALHOST, endpoint.port).into(),
        ASK_TIMEOUT,
    )?;
    stream.set_read_timeout(Some(ASK_TIMEOUT))?;
    stream.set_write_timeout(Some(ASK_TIMEOUT))?;
    stream.write_all(&endpoint.secret.to_be_bytes())?;
    stream.write_all(&control.size().to_be_bytes())?;
    let mut reply = [0u8; 1];
    stream.read_exact(&mut reply)?;
    Reply::from_byte(reply[0]).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unknown reply from FrameEver",
        )
    })
}

pub fn app_state(app_data: &Path) -> AppState {
    match ask(app_data, Control::Busy) {
        Ok(Reply::Busy) => AppState::Busy,
        Ok(_) => AppState::Idle,
        Err(_) if frameever_running() => AppState::Unreachable,
        Err(_) => AppState::Absent,
    }
}

pub fn close_app(app_data: &Path, dir: &Path) -> Closed {
    if let Ok(Reply::Refused) = ask(app_data, Control::Quit) {
        return Closed::Refused;
    }
    let started = Instant::now();
    while started.elapsed() < QUIT_WAIT {
        if !frameever_running() && !holders(dir).iter().any(|it| it.frameever) {
            return Closed::Gone;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Closed::StillRunning
}

fn read_string(key: &str, value: Option<&str>) -> Option<String> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW},
    };
    let key = wide(std::ffi::OsStr::new(key));
    let value = value.map(|value| wide(std::ffi::OsStr::new(value)));
    let mut buffer = vec![0u16; 2048];
    let mut bytes = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    (status == ERROR_SUCCESS).then(|| from_wide(&buffer))
}

pub fn installed() -> Option<Installed> {
    if let Some(value) = std::env::var_os(INSTALLED_ENV) {
        let value = value.to_string_lossy().into_owned();
        let (dir, version) = value.split_once('|')?;
        return Some(Installed {
            dir: PathBuf::from(dir),
            version: Some(version.to_string()).filter(|it| !it.is_empty()),
        });
    }
    let location = read_string(UNINSTALL_KEY, Some("InstallLocation"))
        .or_else(|| read_string(PRODUCT_KEY, None))
        .map(|value| PathBuf::from(value.trim().trim_matches('"')))
        .filter(|dir| dir.join(format!("{APP_NAME}.exe")).is_file())?;
    Some(Installed {
        dir: location,
        version: read_string(UNINSTALL_KEY, Some("DisplayVersion")),
    })
}

pub fn forget_install_location() {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
    let key = wide(std::ffi::OsStr::new(PRODUCT_KEY));
    unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
}

pub fn local_app_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

pub fn default_dir() -> PathBuf {
    local_app_data().join(APP_NAME)
}

pub fn local_settings_dir() -> PathBuf {
    local_app_data().join(APP_ID)
}

pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut animates = 1;
    let read = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animates).cast(), 0)
    };
    read != 0 && animates == 0
}
