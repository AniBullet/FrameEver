use std::path::{Path, PathBuf};

use frameever_installer_app::command_line::{
    Fallback, Launch, Mode, fallback, install_arguments, parse, relaunch_arguments,
    uninstall_arguments, uninstaller_passthrough,
};

fn launch(args: &[&str]) -> Launch {
    parse(args.iter().map(|arg| arg.to_string()))
}

#[test]
fn no_arguments_is_an_interactive_install() {
    assert_eq!(
        launch(&[]),
        Launch {
            mode: Mode::Install,
            silent: false,
            dir: None,
            from_temp: false,
        }
    );
}

#[test]
fn silent_install_keeps_the_nsis_switches_scripts_and_winget_use() {
    let parsed = launch(&["/S", "/D=C:\\Tools\\FrameEver"]);
    assert_eq!(parsed.mode, Mode::Install);
    assert!(parsed.silent);
    assert_eq!(parsed.dir, Some(PathBuf::from("C:\\Tools\\FrameEver")));
}

#[test]
fn the_install_folder_takes_the_rest_of_the_line_as_nsis_does() {
    let parsed = launch(&["/S", "/D=C:\\Program", "Files\\My", "Apps\\FrameEver"]);
    assert_eq!(
        parsed.dir,
        Some(PathBuf::from("C:\\Program Files\\My Apps\\FrameEver"))
    );
}

#[test]
fn an_empty_install_folder_is_ignored() {
    assert_eq!(launch(&["/D="]).dir, None);
}

#[test]
fn the_updater_starts_update_mode_and_older_updaters_still_get_it() {
    assert_eq!(launch(&["--update"]).mode, Mode::Update);
    assert_eq!(launch(&["/P", "/R", "/UPDATE"]).mode, Mode::Update);
    assert!(!launch(&["/P", "/R", "/UPDATE"]).silent);
}

#[test]
fn the_uninstall_entry_starts_uninstall_mode() {
    let parsed = launch(&["--uninstall"]);
    assert_eq!(parsed.mode, Mode::Uninstall);
    assert!(!parsed.from_temp);
    let relaunched = launch(&["--uninstall", "--from-temp"]);
    assert!(relaunched.from_temp);
    assert!(launch(&["--uninstall", "/S"]).silent);
}

#[test]
fn switches_are_case_insensitive_like_nsis() {
    assert!(launch(&["/s"]).silent);
    assert_eq!(launch(&["/update"]).mode, Mode::Update);
    assert_eq!(
        launch(&["/d=D:\\FrameEver"]).dir,
        Some(PathBuf::from("D:\\FrameEver"))
    );
}

#[test]
fn nsis_runs_silent_with_the_folder_last() {
    let dir = Path::new("D:\\Apps\\FrameEver");
    assert_eq!(
        install_arguments(dir, false, false),
        vec!["/S", "/D=D:\\Apps\\FrameEver"]
    );
    assert_eq!(
        install_arguments(dir, true, false),
        vec!["/S", "/UPDATE", "/D=D:\\Apps\\FrameEver"]
    );
    assert_eq!(
        install_arguments(dir, false, true),
        vec!["/S", "/NS", "/D=D:\\Apps\\FrameEver"]
    );
}

#[test]
fn the_nsis_uninstaller_runs_in_place_so_it_can_be_waited_for() {
    assert_eq!(
        uninstall_arguments(Path::new("D:\\Apps\\FrameEver")),
        vec!["/S", "_?=D:\\Apps\\FrameEver"]
    );
}

#[test]
fn the_temp_copy_is_relaunched_in_uninstall_mode() {
    let dir = Path::new("D:\\My Apps\\FrameEver");
    let silent = launch(&["--uninstall", "/S"]);
    assert_eq!(
        relaunch_arguments(&silent, dir),
        vec![
            "--uninstall",
            "--from-temp",
            "/S",
            "/D=D:\\My Apps\\FrameEver"
        ]
    );
    let relaunched = relaunch_arguments(&launch(&["--uninstall"]), dir);
    assert_eq!(
        relaunched,
        vec!["--uninstall", "--from-temp", "/D=D:\\My Apps\\FrameEver"]
    );
    let parsed = parse(relaunched);
    assert!(parsed.from_temp);
    assert_eq!(parsed.dir.as_deref(), Some(dir));
}

#[test]
fn the_nsis_reinstall_page_still_reaches_the_nsis_uninstaller_through_the_entry() {
    let args: Vec<String> = ["--uninstall", "/UPDATE", "/P", "_?=D:\\Apps\\FrameEver"]
        .map(str::to_string)
        .to_vec();
    assert_eq!(
        uninstaller_passthrough(&args),
        Some((
            PathBuf::from("D:\\Apps\\FrameEver\\uninstall.exe"),
            vec![
                "/UPDATE".to_string(),
                "/P".to_string(),
                "_?=D:\\Apps\\FrameEver".to_string()
            ]
        ))
    );
    assert_eq!(uninstaller_passthrough(&["--uninstall".to_string()]), None);
    let spaced: Vec<String> = ["--uninstall", "_?=D:\\My", "Apps\\FrameEver"]
        .map(str::to_string)
        .to_vec();
    assert_eq!(
        uninstaller_passthrough(&spaced),
        Some((
            PathBuf::from("D:\\My Apps\\FrameEver\\uninstall.exe"),
            vec!["_?=D:\\My Apps\\FrameEver".to_string()]
        ))
    );
}

#[test]
fn without_gpui_each_mode_falls_back_to_nsis_with_its_own_ui() {
    let installed = Path::new("D:\\Apps\\FrameEver");
    assert_eq!(
        fallback(&launch(&[]), Some(installed)),
        Some(Fallback::Installer(Vec::new()))
    );
    assert_eq!(
        fallback(&launch(&["/D=E:\\FrameEver"]), None),
        Some(Fallback::Installer(vec!["/D=E:\\FrameEver".to_string()]))
    );
    assert_eq!(
        fallback(&launch(&["--update"]), Some(installed)),
        Some(Fallback::Installer(vec![
            "/P".to_string(),
            "/R".to_string(),
            "/UPDATE".to_string()
        ]))
    );
    assert_eq!(
        fallback(&launch(&["--uninstall"]), Some(installed)),
        Some(Fallback::Uninstaller(installed.join("uninstall.exe")))
    );
    assert_eq!(fallback(&launch(&["--uninstall"]), None), None);
}
