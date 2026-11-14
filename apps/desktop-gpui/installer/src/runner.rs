use std::{
    fmt::Display,
    io::Write,
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use frameever_installer_app::{
    command_line::uninstall_arguments,
    flow::Job,
    measure::{install_fraction, removal_fraction},
    payload,
    settings::{removable_settings, with_first_language},
};
use frameever_utils::{
    brand::{APP_NAME, CLI_COMMAND},
    storage,
};

use crate::system;

const POLL: Duration = Duration::from_millis(100);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CHUNK: usize = 8 * 1024 * 1024;
const CLI_EXE: &str = "frameever-cli.exe";
const SETUP_EXE: &str = "frameever-setup.exe";
const UNINSTALLER_EXE: &str = "uninstall.exe";

pub fn work_dir() -> PathBuf {
    std::env::temp_dir().join("frameever-setup")
}

pub fn log_file() -> PathBuf {
    work_dir().join("install.log")
}

pub fn log(line: impl Display) {
    let dir = work_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|it| it.as_secs())
        .unwrap_or_default();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file())
    {
        let written = writeln!(file, "{seconds} {line}");
        if written.is_err() {
            return;
        }
    }
}

fn nsis_failure(code: Option<i32>) -> String {
    let code = code.map_or_else(|| "?".to_string(), |code| code.to_string());
    format!("NSIS exit code {code} · {}", log_file().display())
}

fn written_bytes(child: &Child) -> u64 {
    use windows_sys::Win32::System::Threading::{GetProcessIoCounters, IO_COUNTERS};
    let mut counters: IO_COUNTERS = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetProcessIoCounters(child.as_raw_handle(), &mut counters) };
    if ok == 0 {
        0
    } else {
        counters.WriteTransferCount
    }
}

fn spawn(program: &Path, arguments: &[String]) -> Result<Child, String> {
    let mut command = Command::new(program);
    for argument in arguments {
        command.raw_arg(argument);
    }
    log(format_args!(
        "run {} {}",
        program.display(),
        arguments.join(" ")
    ));
    command
        .spawn()
        .map_err(|error| format!("{}: {error}", program.display()))
}

fn wait(mut child: Child, mut tick: impl FnMut(&Child)) -> Result<(), String> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                log(format_args!("exit {status}"));
                return if status.success() {
                    Ok(())
                } else {
                    Err(nsis_failure(status.code()))
                };
            }
            Ok(None) => tick(&child),
            Err(error) => return Err(error.to_string()),
        }
        std::thread::sleep(POLL);
    }
}

pub fn extract(on_bytes: &mut dyn FnMut(u64)) -> Result<PathBuf, String> {
    if payload::NSIS.is_empty() {
        return Err("This copy of the installer has no FrameEver inside it.".to_string());
    }
    let dir = work_dir();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let target = dir.join(format!("{APP_NAME}-{}-nsis.exe", env!("CARGO_PKG_VERSION")));
    let mut file = std::fs::File::create(&target).map_err(|error| error.to_string())?;
    let mut written = 0u64;
    for chunk in payload::NSIS.chunks(CHUNK) {
        file.write_all(chunk).map_err(|error| error.to_string())?;
        written += chunk.len() as u64;
        on_bytes(written);
    }
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(target)
}

pub fn spawn_and_wait(program: &Path, arguments: &[String]) -> Result<(), String> {
    spawn(program, arguments).and_then(|child| wait(child, |_| {}))
}

pub fn run_nsis(arguments: &[String]) -> Result<(), String> {
    let nsis = extract(&mut |_| {})?;
    let outcome = spawn(&nsis, arguments).and_then(|child| wait(child, |_| {}));
    std::fs::remove_file(&nsis).ok();
    outcome
}

fn cli(dir: &Path, command: &str) {
    let exe = dir.join(CLI_EXE);
    if !exe.is_file() {
        return;
    }
    let status = Command::new(&exe)
        .args(["desktop", command])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    log(format_args!("{CLI_COMMAND} desktop {command}: {status:?}"));
}

fn known_folder(id: &windows_sys::core::GUID) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    let mut raw = std::ptr::null_mut();
    let status =
        unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut raw) };
    if status != 0 || raw.is_null() {
        return None;
    }
    let length = (0..)
        .take_while(|index| unsafe { *raw.add(*index) } != 0)
        .count();
    let path = PathBuf::from(std::ffi::OsString::from_wide(unsafe {
        std::slice::from_raw_parts(raw, length)
    }));
    unsafe { CoTaskMemFree(raw.cast()) };
    Some(path)
}

fn remove_shortcuts(desktop: bool, start_menu: bool) {
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs};
    let name = format!("{APP_NAME}.lnk");
    for (keep, folder) in [
        (desktop, &FOLDERID_Desktop),
        (start_menu, &FOLDERID_Programs),
    ] {
        if keep {
            continue;
        }
        if let Some(folder) = known_folder(folder) {
            let removed = std::fs::remove_file(folder.join(&name));
            log(format_args!(
                "remove shortcut {}: {removed:?}",
                folder.display()
            ));
        }
    }
}

pub fn write_first_language(code: &str) {
    let file = storage::settings_file();
    let current = std::fs::read_to_string(&file).ok();
    let Some(document) = with_first_language(current.as_deref(), code) else {
        return;
    };
    let Ok(text) = serde_json::to_string_pretty(&document) else {
        return;
    };
    if let Some(parent) = file.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let temporary = file.with_extension("frameever-setup.tmp");
    let written =
        std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, &file));
    log(format_args!("first language {code}: {written:?}"));
}

pub fn install(
    job: &Job,
    installed_bytes: u64,
    progress: &mut dyn FnMut(f64),
) -> Result<(), String> {
    let Job::Install {
        dir,
        arguments,
        fresh,
        desktop,
        start_menu,
        path,
    } = job
    else {
        return Err("not an install".to_string());
    };
    let payload_bytes = payload::NSIS.len() as u64;
    let nsis = extract(&mut |written| {
        progress(install_fraction(written, 0, payload_bytes, installed_bytes))
    })?;
    let outcome = spawn(&nsis, arguments).and_then(|child| {
        wait(child, |child| {
            progress(install_fraction(
                payload_bytes,
                written_bytes(child),
                payload_bytes,
                installed_bytes,
            ))
        })
    });
    std::fs::remove_file(&nsis).ok();
    outcome?;
    if *fresh {
        remove_shortcuts(*desktop, *start_menu);
    }
    if *path {
        cli(dir, "install-cli");
    }
    Ok(())
}

fn protected_folders() -> Vec<PathBuf> {
    let settings = storage::StorageSettings::load(&storage::settings_file());
    let mut kept = vec![storage::default_storage_dir()];
    kept.extend(settings.storage_path);
    kept.extend(settings.previous_storage_paths);
    kept
}

pub fn storage_folder() -> PathBuf {
    storage::StorageSettings::load(&storage::settings_file())
        .storage_path
        .unwrap_or_else(storage::default_storage_dir)
}

fn delete_settings() {
    let roaming = storage::app_data_dir();
    let local = system::local_settings_dir();
    let kept = protected_folders();
    let kept: Vec<&Path> = kept.iter().map(PathBuf::as_path).collect();
    for root in removable_settings(&[&roaming, &local], &kept) {
        let removed = std::fs::remove_dir_all(&root);
        log(format_args!(
            "delete settings {}: {removed:?}",
            root.display()
        ));
    }
    system::forget_install_location();
}

pub fn uninstall(job: &Job, progress: &mut dyn FnMut(f64)) -> Result<(), String> {
    let Job::Uninstall {
        dir,
        delete_settings: settings,
    } = job
    else {
        return Err("not an uninstall".to_string());
    };
    let uninstaller = dir.join(UNINSTALLER_EXE);
    if !uninstaller.is_file() {
        return Err(format!("{} is missing", uninstaller.display()));
    }
    cli(dir, "uninstall-cli");
    let at_start = system::files_in(dir).len();
    spawn(&uninstaller, &uninstall_arguments(dir)).and_then(|child| {
        wait(child, |_| {
            progress(removal_fraction(at_start, system::files_in(dir).len()))
        })
    })?;
    for leftover in [UNINSTALLER_EXE, SETUP_EXE] {
        std::fs::remove_file(dir.join(leftover)).ok();
    }
    std::fs::remove_dir(dir).ok();
    if *settings {
        delete_settings();
    }
    Ok(())
}

pub fn launch(executable: &Path) -> Result<(), String> {
    let dir = executable.parent().unwrap_or(Path::new("."));
    Command::new(executable)
        .current_dir(dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", executable.display()))
}

pub fn relaunch_from_temp(arguments: &[String]) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let dir = std::env::temp_dir().join(format!("frameever-uninstall-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let copy = dir.join(SETUP_EXE);
    std::fs::copy(&current, &copy).map_err(|error| error.to_string())?;
    Command::new(&copy)
        .args(arguments)
        .current_dir(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn remove_temp_copy() {
    let Ok(current) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = current.parent() else {
        return;
    };
    let named = dir
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("frameever-uninstall-"));
    if !named || !dir.starts_with(std::env::temp_dir()) {
        return;
    }
    let script = format!(
        "ping 127.0.0.1 -n 3 >NUL & rmdir /s /q \"{}\"",
        dir.display()
    );
    let spawned = Command::new("cmd.exe")
        .arg("/C")
        .raw_arg(script)
        .current_dir(std::env::temp_dir())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if spawned.is_err() {
        log("could not schedule removing the temp copy");
    }
}

pub fn export_log(target: &Path) -> Result<(), String> {
    std::fs::copy(log_file(), target)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
