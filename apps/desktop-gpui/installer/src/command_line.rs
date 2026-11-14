use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Update,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub mode: Mode,
    pub silent: bool,
    pub dir: Option<PathBuf>,
    pub from_temp: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallback {
    Installer(Vec<String>),
    Uninstaller(PathBuf),
}

pub const UPDATE_FLAG: &str = "--update";
pub const UNINSTALL_FLAG: &str = "--uninstall";
const FROM_TEMP_FLAG: &str = "--from-temp";

pub fn parse(args: impl IntoIterator<Item = String>) -> Launch {
    let args: Vec<String> = args.into_iter().collect();
    let mut launch = Launch {
        mode: Mode::Install,
        silent: false,
        dir: None,
        from_temp: false,
    };
    for (index, arg) in args.iter().enumerate() {
        let lower = arg.to_ascii_lowercase();
        if lower.starts_with("/d=") {
            let mut dir = arg[3..].to_string();
            for rest in &args[index + 1..] {
                dir.push(' ');
                dir.push_str(rest);
            }
            let dir = dir.trim();
            if !dir.is_empty() {
                launch.dir = Some(PathBuf::from(dir));
            }
            break;
        }
        match lower.as_str() {
            "/s" => launch.silent = true,
            "/update" | UPDATE_FLAG => launch.mode = Mode::Update,
            UNINSTALL_FLAG => launch.mode = Mode::Uninstall,
            FROM_TEMP_FLAG => launch.from_temp = true,
            _ => {}
        }
    }
    launch
}

fn folder_argument(prefix: &str, dir: &Path) -> String {
    format!("{prefix}{}", dir.display())
}

pub fn install_arguments(dir: &Path, update: bool, no_shortcuts: bool) -> Vec<String> {
    let mut arguments = vec!["/S".to_string()];
    if update {
        arguments.push("/UPDATE".to_string());
    }
    if no_shortcuts {
        arguments.push("/NS".to_string());
    }
    arguments.push(folder_argument("/D=", dir));
    arguments
}

pub fn uninstall_arguments(dir: &Path) -> Vec<String> {
    vec!["/S".to_string(), folder_argument("_?=", dir)]
}

pub fn relaunch_arguments(launch: &Launch, dir: &Path) -> Vec<String> {
    let mut arguments = vec![UNINSTALL_FLAG.to_string(), FROM_TEMP_FLAG.to_string()];
    if launch.silent {
        arguments.push("/S".to_string());
    }
    arguments.push(folder_argument("/D=", dir));
    arguments
}

pub fn uninstaller_passthrough(args: &[String]) -> Option<(PathBuf, Vec<String>)> {
    let at = args.iter().position(|arg| arg.starts_with("_?="))?;
    let dir = args[at..].join(" ");
    let dir = PathBuf::from(dir["_?=".len()..].trim());
    let mut rest: Vec<String> = args[..at]
        .iter()
        .filter(|arg| !arg.eq_ignore_ascii_case(UNINSTALL_FLAG))
        .cloned()
        .collect();
    rest.push(folder_argument("_?=", &dir));
    Some((dir.join("uninstall.exe"), rest))
}

pub fn fallback(launch: &Launch, installed: Option<&Path>) -> Option<Fallback> {
    match launch.mode {
        Mode::Install => Some(Fallback::Installer(
            launch
                .dir
                .iter()
                .map(|dir| folder_argument("/D=", dir))
                .collect(),
        )),
        Mode::Update => Some(Fallback::Installer(
            ["/P", "/R", "/UPDATE"].map(str::to_string).to_vec(),
        )),
        Mode::Uninstall => installed.map(|dir| Fallback::Uninstaller(dir.join("uninstall.exe"))),
    }
}
