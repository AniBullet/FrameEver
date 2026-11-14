use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use frameever_installer_app::{
    command_line::{Launch, Mode},
    flow::{
        Action, AppState, Choice, Closed, EXIT_BUSY, EXIT_NO_SPACE, EXIT_NOT_INSTALLED, Effect,
        Flow, Frame, Holder, Installed, Job, Page, Setup, Silent, Style, System, quit_answer,
        silent, writes_first_language,
    },
    i18n::Language,
    instance_control::Reply,
};

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

#[derive(Clone)]
struct Machine {
    free: Rc<RefCell<Vec<(String, u64)>>>,
    app: Rc<RefCell<AppState>>,
    holders: Rc<RefCell<Vec<Holder>>>,
}

impl Machine {
    fn new() -> Self {
        Self {
            free: Rc::new(RefCell::new(vec![
                ("C:".into(), 58 * GB),
                ("D:".into(), 500 * GB),
            ])),
            app: Rc::new(RefCell::new(AppState::Absent)),
            holders: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn system(&self) -> System {
        let free = self.free.clone();
        let app = self.app.clone();
        let holders = self.holders.clone();
        System {
            volume: Box::new(|path: &Path| {
                let text = path.to_string_lossy();
                text.get(..2)
                    .filter(|it| it.ends_with(':'))
                    .map(str::to_uppercase)
            }),
            free_space: Box::new(move |root: &Path| {
                let root = root.to_string_lossy().trim_end_matches('\\').to_string();
                free.borrow()
                    .iter()
                    .find(|(volume, _)| *volume == root)
                    .map(|(_, bytes)| *bytes)
            }),
            holders: Box::new(move |_| holders.borrow().clone()),
            app: Box::new(move || *app.borrow()),
        }
    }

    fn set_app(&self, state: AppState) {
        *self.app.borrow_mut() = state;
    }

    fn hold(&self, holders: Vec<Holder>) {
        *self.holders.borrow_mut() = holders;
    }

    fn free_on(&self, volume: &str, bytes: u64) {
        for entry in self.free.borrow_mut().iter_mut() {
            if entry.0 == volume {
                entry.1 = bytes;
            }
        }
    }
}

fn frameever() -> Holder {
    Holder {
        name: "FrameEver".into(),
        frameever: true,
    }
}

fn setup(mode: Mode, installed: Option<(&str, Option<&str>)>) -> Setup {
    Setup {
        launch: Launch {
            mode,
            silent: false,
            dir: None,
            from_temp: false,
        },
        version: "1.0.0".into(),
        installed: installed.map(|(dir, version)| Installed {
            dir: PathBuf::from(dir),
            version: version.map(str::to_string),
        }),
        default_dir: PathBuf::from("C:\\Users\\me\\AppData\\Local\\FrameEver"),
        temp_dir: PathBuf::from("C:\\Users\\me\\AppData\\Local\\Temp"),
        storage_dir: PathBuf::from("D:\\Videos\\FrameEver"),
        installed_bytes: 380 * MB,
        payload_bytes: 140 * MB,
    }
}

fn flow(mode: Mode, installed: Option<(&str, Option<&str>)>, machine: &Machine) -> Flow {
    Flow::new(setup(mode, installed), machine.system(), Language::English)
}

fn run_job(effect: Option<Effect>) -> Job {
    match effect {
        Some(Effect::Run(job)) => job,
        other => panic!("expected a job, got {other:?}"),
    }
}

#[test]
fn a_fresh_install_starts_on_welcome_and_enter_installs_with_every_default() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    let screen = flow.screen();
    assert_eq!(flow.page(), Page::Welcome);
    assert_eq!(
        screen.frame,
        Frame::Main {
            title: "Install FrameEver".into(),
            tall: false
        }
    );
    assert!(screen.hero);
    assert_eq!(screen.heading, "FrameEver");
    assert_eq!(screen.links[0].label, "Options");
    assert_eq!(flow.primary(), Some(Action::Install));
    let job = run_job(flow.act(Action::Install));
    assert_eq!(
        job,
        Job::Install {
            dir: PathBuf::from("C:\\Users\\me\\AppData\\Local\\FrameEver"),
            arguments: vec![
                "/S".into(),
                "/D=C:\\Users\\me\\AppData\\Local\\FrameEver".into()
            ],
            fresh: true,
            desktop: true,
            start_menu: true,
            path: true,
        }
    );
    assert_eq!(flow.page(), Page::Installing);
    assert!(flow.locked());
    assert_eq!(flow.act(Action::Close), None);
    assert_eq!(flow.escape(), None);
}

#[test]
fn options_snap_the_window_tall_and_show_the_space_from_the_same_check() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    assert_eq!(flow.act(Action::ToggleOptions), None);
    let screen = flow.screen();
    assert_eq!(
        screen.frame,
        Frame::Main {
            title: "Install FrameEver".into(),
            tall: true
        }
    );
    let location = screen.location.unwrap();
    assert_eq!(location.path, "C:\\Users\\me\\AppData\\Local\\FrameEver");
    assert_eq!(location.space, "Needs 900 MB · 58 GB free");
    assert_eq!(
        screen.checks.iter().map(|it| it.on).collect::<Vec<_>>(),
        vec![true, true, true]
    );
    assert_eq!(flow.escape(), Some(Action::ToggleOptions));
    flow.act(Action::ToggleOptions);
    assert!(flow.screen().location.is_none());
}

#[test]
fn a_picked_folder_gets_the_app_folder_and_moves_the_space_check() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::ToggleOptions);
    assert_eq!(
        flow.act(Action::ChangeLocation),
        Some(Effect::PickLocation(PathBuf::from(
            "C:\\Users\\me\\AppData\\Local\\FrameEver"
        )))
    );
    flow.location_picked(Path::new("D:\\Apps"));
    assert_eq!(flow.location(), Path::new("D:\\Apps\\FrameEver"));
    assert_eq!(
        flow.screen().location.unwrap().space,
        "Needs 380 MB · 500 GB free"
    );
    flow.location_picked(Path::new("E:\\frameever"));
    assert_eq!(flow.location(), Path::new("E:\\frameever"));
}

#[test]
fn unchecked_shortcuts_and_path_reach_the_job() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::ToggleOptions);
    flow.act(Action::Toggle(Choice::Desktop));
    flow.act(Action::Toggle(Choice::Path));
    let Job::Install {
        arguments,
        desktop,
        start_menu,
        path,
        ..
    } = run_job(flow.act(Action::Install))
    else {
        panic!("install job expected");
    };
    assert_eq!(
        arguments,
        vec!["/S", "/D=C:\\Users\\me\\AppData\\Local\\FrameEver"]
    );
    assert!(!desktop && start_menu && !path);
}

#[test]
fn no_shortcuts_at_all_asks_nsis_for_none() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Toggle(Choice::Desktop));
    flow.act(Action::Toggle(Choice::StartMenu));
    let Job::Install { arguments, .. } = run_job(flow.act(Action::Install)) else {
        panic!("install job expected");
    };
    assert_eq!(
        arguments,
        vec!["/S", "/NS", "/D=C:\\Users\\me\\AppData\\Local\\FrameEver"]
    );
}

#[test]
fn a_full_disk_stops_before_anything_is_copied() {
    let machine = Machine::new();
    machine.free_on("C:", 120 * MB);
    let mut flow = flow(Mode::Install, None, &machine);
    assert_eq!(flow.act(Action::Install), None);
    assert_eq!(flow.page(), Page::NoSpace);
    let screen = flow.screen();
    assert_eq!(
        screen.sentence,
        "Drive C has 120 MB free and FrameEver needs 900 MB. Free up space or choose another drive."
    );
    assert_eq!(
        screen
            .buttons
            .iter()
            .map(|it| it.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Change location", "Try again"]
    );
    assert!(matches!(
        flow.act(Action::ChangeLocation),
        Some(Effect::PickLocation(_))
    ));
    assert_eq!(flow.page(), Page::Welcome);
    assert!(flow.screen().location.is_some());
    flow.location_picked(Path::new("D:\\"));
    assert_eq!(flow.act(Action::Install), None);
    assert_eq!(
        flow.screen().sentence,
        "Drive C has 120 MB free and FrameEver needs 520 MB. Free up space or choose another drive."
    );
    machine.free_on("C:", GB);
    assert!(matches!(flow.act(Action::TryAgain), Some(Effect::Run(_))));
}

#[test]
fn the_installer_never_closes_a_recording() {
    let machine = Machine::new();
    machine.set_app(AppState::Busy);
    machine.hold(vec![frameever()]);
    let mut flow = flow(Mode::Install, None, &machine);
    assert_eq!(flow.act(Action::Install), None);
    assert_eq!(flow.page(), Page::Recording);
    assert_eq!(flow.act(Action::CloseApp), None);
    assert_eq!(flow.primary(), Some(Action::TryAgain));
    assert_eq!(flow.act(Action::TryAgain), None);
    assert_eq!(flow.page(), Page::Recording);
    machine.set_app(AppState::Idle);
    assert_eq!(flow.act(Action::TryAgain), None);
    assert_eq!(flow.page(), Page::AppInUse);
    assert_eq!(flow.act(Action::Cancel), None);
    assert_eq!(flow.page(), Page::Welcome);
}

#[test]
fn a_quit_request_frameever_cannot_answer_counts_as_unreachable() {
    let down = || Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert_eq!(quit_answer(down(), true), Some(Closed::Unreachable));
    assert_eq!(quit_answer(down(), false), None);
    assert_eq!(quit_answer(Ok(Reply::Refused), true), Some(Closed::Refused));
    assert_eq!(quit_answer(Ok(Reply::Quitting), true), None);
}

#[test]
fn an_unreachable_frameever_asks_to_quit_it_from_the_tray() {
    let machine = Machine::new();
    machine.set_app(AppState::Idle);
    machine.hold(vec![frameever()]);
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    assert_eq!(flow.act(Action::CloseApp), Some(Effect::CloseApp));
    assert_eq!(flow.app_closed(Closed::Unreachable), None);
    assert_eq!(flow.page(), Page::AppInUse);
    assert_ne!(flow.primary(), Some(Action::CloseApp));
    assert!(flow.screen().sentence.contains("Quit it from the tray"));
}

#[test]
fn an_idle_frameever_is_closed_normally_then_the_install_goes_on() {
    let machine = Machine::new();
    machine.set_app(AppState::Idle);
    machine.hold(vec![frameever()]);
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    assert_eq!(flow.page(), Page::AppInUse);
    let screen = flow.screen();
    assert_eq!(screen.heading, "Close FrameEver to continue");
    assert_eq!(flow.primary(), Some(Action::CloseApp));
    assert_eq!(flow.act(Action::CloseApp), Some(Effect::CloseApp));
    assert_eq!(flow.app_closed(Closed::Refused), None);
    assert_eq!(flow.page(), Page::Recording);
    flow.act(Action::TryAgain);
    flow.act(Action::CloseApp);
    machine.set_app(AppState::Absent);
    machine.hold(Vec::new());
    assert!(matches!(
        flow.app_closed(Closed::Gone),
        Some(Effect::Run(_))
    ));
    assert_eq!(flow.page(), Page::Installing);
}

#[test]
fn other_programs_holding_files_are_named_and_never_closed_for_you() {
    let machine = Machine::new();
    machine.hold(vec![Holder {
        name: "frameever-cli".into(),
        frameever: false,
    }]);
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    let screen = flow.screen();
    assert_eq!(screen.heading, "Close the other program to continue");
    assert_eq!(
        screen.sentence,
        "frameever-cli is using FrameEver's files. Close it, then try again."
    );
    assert_eq!(flow.primary(), Some(Action::TryAgain));
    assert_eq!(flow.act(Action::CloseApp), None);
}

#[test]
fn a_frameever_that_does_not_answer_is_left_for_the_user_to_quit() {
    let machine = Machine::new();
    machine.set_app(AppState::Unreachable);
    machine.hold(vec![frameever()]);
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    assert_eq!(flow.primary(), Some(Action::TryAgain));
    assert!(flow.screen().sentence.contains("Quit it from the tray"));
}

#[test]
fn progress_is_one_real_bar_then_frameever_opens_and_the_window_goes() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    flow.progress(0.42);
    flow.progress(0.30);
    let progress = flow.screen().progress.unwrap();
    assert_eq!(
        (progress.label.as_str(), progress.percent),
        ("Installing…", 42)
    );
    assert_eq!(
        flow.finished(Ok(())),
        Some(Effect::Launch(PathBuf::from(
            "C:\\Users\\me\\AppData\\Local\\FrameEver\\FrameEver.exe"
        )))
    );
    let progress = flow.screen().progress.unwrap();
    assert_eq!(
        (progress.label.as_str(), progress.percent),
        ("Opening FrameEver…", 100)
    );
    assert_eq!(flow.launched(Ok(())), Some(Effect::Quit));
}

#[test]
fn frameever_that_will_not_open_says_so_with_try_again() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    flow.finished(Ok(()));
    assert_eq!(flow.launched(Err("error 2".into())), None);
    assert_eq!(flow.page(), Page::LaunchFailed);
    assert!(matches!(
        flow.act(Action::TryAgain),
        Some(Effect::Launch(_))
    ));
}

#[test]
fn a_failed_install_explains_with_details_and_logs() {
    let machine = Machine::new();
    let mut flow = flow(Mode::Install, None, &machine);
    flow.act(Action::Install);
    flow.finished(Err("NSIS exit code 2 · install.log".into()));
    assert_eq!(flow.page(), Page::Failed);
    let screen = flow.screen();
    assert!(screen.failed);
    assert_eq!(screen.details, None);
    flow.act(Action::ToggleDetails);
    let screen = flow.screen();
    assert_eq!(
        screen.details.as_deref(),
        Some("NSIS exit code 2 · install.log")
    );
    assert_eq!(
        screen.frame,
        Frame::Main {
            title: "Install FrameEver".into(),
            tall: true
        }
    );
    assert_eq!(flow.act(Action::ExportLogs), Some(Effect::ExportLogs));
    assert!(matches!(flow.act(Action::TryAgain), Some(Effect::Run(_))));
}

#[test]
fn an_older_install_updates_in_place_keeping_location_and_shortcuts() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Install,
        Some(("D:\\Apps\\FrameEver", Some("0.9.2"))),
        &machine,
    );
    assert_eq!(flow.page(), Page::Older);
    let screen = flow.screen();
    assert_eq!(
        screen.frame,
        Frame::Main {
            title: "Update FrameEver".into(),
            tall: false
        }
    );
    assert_eq!(screen.heading, "Update to 1.0.0");
    assert_eq!(
        screen.sentence,
        "Version 0.9.2 is installed. Location, shortcuts and settings stay as they are."
    );
    assert_eq!(screen.links[0].label, "Uninstall instead");
    assert_eq!(flow.act(Action::ToggleOptions), None);
    assert!(flow.screen().location.is_none());
    assert_eq!(
        run_job(flow.act(Action::Install)),
        Job::Install {
            dir: PathBuf::from("D:\\Apps\\FrameEver"),
            arguments: vec![
                "/S".into(),
                "/UPDATE".into(),
                "/D=D:\\Apps\\FrameEver".into()
            ],
            fresh: false,
            desktop: true,
            start_menu: true,
            path: false,
        }
    );
}

#[test]
fn an_install_without_a_readable_version_counts_as_older() {
    let machine = Machine::new();
    let flow = flow(Mode::Install, Some(("D:\\Apps\\FrameEver", None)), &machine);
    assert_eq!(flow.page(), Page::Older);
    assert!(flow.screen().sentence.starts_with("An earlier version"));
}

#[test]
fn the_same_version_offers_reinstall_or_uninstall() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Install,
        Some(("D:\\Apps\\FrameEver", Some("1.0.0"))),
        &machine,
    );
    assert_eq!(flow.page(), Page::Same);
    assert_eq!(flow.screen().heading, "FrameEver 1.0.0 is installed");
    assert_eq!(flow.primary(), Some(Action::Install));
    assert_eq!(flow.act(Action::Uninstall), None);
    assert_eq!(flow.page(), Page::Confirm);
    assert_eq!(
        flow.screen().frame,
        Frame::Main {
            title: "Uninstall FrameEver".into(),
            tall: false
        }
    );
    assert_eq!(flow.act(Action::Cancel), None);
    assert_eq!(flow.page(), Page::Same);
    assert_eq!(
        flow.screen().frame,
        Frame::Main {
            title: "Install FrameEver".into(),
            tall: false
        }
    );
}

#[test]
fn a_newer_install_is_kept_by_default_and_downgrading_stays_possible() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Install,
        Some(("D:\\Apps\\FrameEver", Some("1.1.0"))),
        &machine,
    );
    assert_eq!(flow.page(), Page::Newer);
    let screen = flow.screen();
    assert_eq!(
        screen
            .buttons
            .iter()
            .map(|it| (it.label.as_str(), it.style))
            .collect::<Vec<_>>(),
        vec![
            ("Install 1.0.0", Style::Secondary),
            ("Keep 1.1.0", Style::Primary)
        ]
    );
    assert_eq!(flow.primary(), Some(Action::Close));
    assert!(matches!(flow.act(Action::Install), Some(Effect::Run(_))));
}

#[test]
fn update_mode_runs_at_once_in_the_strip_and_relaunches() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Update,
        Some(("D:\\Apps\\FrameEver", Some("0.9.2"))),
        &machine,
    );
    assert_eq!(flow.screen().frame, Frame::Strip);
    let job = run_job(flow.start());
    assert!(matches!(job, Job::Install { fresh: false, .. }));
    let screen = flow.screen();
    assert_eq!(screen.meta.as_deref(), Some("0.9.2 → 1.0.0"));
    assert_eq!(screen.heading, "Updating FrameEver…");
    assert!(matches!(flow.finished(Ok(())), Some(Effect::Launch(_))));
    assert_eq!(flow.launched(Ok(())), Some(Effect::Quit));
}

#[test]
fn update_mode_waits_for_the_quitting_app_then_installs() {
    let machine = Machine::new();
    machine.hold(vec![frameever()]);
    let mut flow = flow(
        Mode::Update,
        Some(("D:\\Apps\\FrameEver", Some("0.9.2"))),
        &machine,
    );
    assert_eq!(flow.start(), Some(Effect::CloseApp));
    assert_eq!(flow.page(), Page::Updating);
    machine.hold(Vec::new());
    assert!(matches!(
        flow.app_closed(Closed::Gone),
        Some(Effect::Run(_))
    ));
}

#[test]
fn a_failed_update_stays_until_you_act() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Update,
        Some(("D:\\Apps\\FrameEver", Some("0.9.2"))),
        &machine,
    );
    flow.start();
    flow.finished(Err("NSIS exit code 2".into()));
    assert_eq!(flow.page(), Page::UpdateFailed);
    let screen = flow.screen();
    assert_eq!(screen.frame, Frame::Strip);
    assert!(screen.failed);
    assert!(matches!(
        flow.act(Action::OpenReleases),
        Some(Effect::OpenUrl(url)) if url == "https://github.com/AniBullet/FrameEver/releases"
    ));
    assert!(matches!(flow.act(Action::TryAgain), Some(Effect::Run(_))));
}

#[test]
fn update_mode_never_closes_a_busy_app() {
    let machine = Machine::new();
    machine.set_app(AppState::Busy);
    let mut flow = flow(
        Mode::Update,
        Some(("D:\\Apps\\FrameEver", Some("0.9.2"))),
        &machine,
    );
    assert_eq!(flow.start(), None);
    assert_eq!(flow.page(), Page::UpdateFailed);
    flow.act(Action::ToggleDetails);
    assert!(flow.screen().details.unwrap().contains("recording"));
}

#[test]
fn uninstall_keeps_the_storage_folder_and_settings_go_only_when_asked() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Uninstall,
        Some(("D:\\Apps\\FrameEver", Some("1.0.0"))),
        &machine,
    );
    assert_eq!(flow.page(), Page::Confirm);
    let screen = flow.screen();
    assert_eq!(
        screen.keep.as_deref(),
        Some("Recordings, screenshots and exports stay in D:\\Videos\\FrameEver.")
    );
    assert_eq!(screen.checks.len(), 1);
    assert!(!screen.checks[0].on);
    assert_eq!(screen.buttons[1].style, Style::Danger);
    assert_eq!(flow.primary(), Some(Action::Uninstall));
    assert_eq!(
        run_job(flow.act(Action::Uninstall)),
        Job::Uninstall {
            dir: PathBuf::from("D:\\Apps\\FrameEver"),
            delete_settings: false
        }
    );
    flow.progress(0.5);
    assert_eq!(flow.screen().progress.unwrap().label, "Uninstalling…");
    assert_eq!(flow.finished(Ok(())), None);
    assert_eq!(flow.page(), Page::Removed);
    assert_eq!(
        flow.screen().sentence,
        "Your recordings, screenshots and exports are still in D:\\Videos\\FrameEver."
    );
    assert_eq!(
        flow.act(Action::OpenFolder),
        Some(Effect::OpenFolder(PathBuf::from("D:\\Videos\\FrameEver")))
    );
    assert_eq!(flow.act(Action::Close), Some(Effect::Quit));
}

#[test]
fn also_delete_settings_reaches_the_uninstall_job() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Uninstall,
        Some(("D:\\Apps\\FrameEver", Some("1.0.0"))),
        &machine,
    );
    flow.act(Action::Toggle(Choice::DeleteSettings));
    assert!(flow.screen().checks[0].on);
    assert_eq!(
        run_job(flow.act(Action::Uninstall)),
        Job::Uninstall {
            dir: PathBuf::from("D:\\Apps\\FrameEver"),
            delete_settings: true
        }
    );
}

#[test]
fn cancelling_uninstall_mode_closes_the_window() {
    let machine = Machine::new();
    let mut flow = flow(
        Mode::Uninstall,
        Some(("D:\\Apps\\FrameEver", Some("1.0.0"))),
        &machine,
    );
    assert_eq!(flow.escape(), Some(Action::Cancel));
    assert_eq!(flow.act(Action::Cancel), Some(Effect::Quit));
}

#[test]
fn uninstall_never_closes_a_recording_either() {
    let machine = Machine::new();
    machine.set_app(AppState::Busy);
    let mut flow = flow(
        Mode::Uninstall,
        Some(("D:\\Apps\\FrameEver", Some("1.0.0"))),
        &machine,
    );
    assert_eq!(flow.act(Action::Uninstall), None);
    assert_eq!(flow.page(), Page::Recording);
    assert_eq!(
        flow.screen().frame,
        Frame::Main {
            title: "Uninstall FrameEver".into(),
            tall: false
        }
    );
    flow.act(Action::Cancel);
    assert_eq!(flow.page(), Page::Confirm);
}

#[test]
fn every_page_has_its_strings_in_chinese() {
    let machine = Machine::new();
    let mut flow = Flow::new(
        setup(Mode::Install, None),
        machine.system(),
        Language::SimplifiedChinese,
    );
    let screen = flow.screen();
    assert_eq!(screen.heading, "帧刻");
    assert_eq!(screen.buttons[0].label, "安装");
    flow.act(Action::ToggleOptions);
    assert_eq!(
        flow.screen().location.unwrap().space,
        "需要 900 MB · 可用 58 GB"
    );
}

fn silent_setup(mode: Mode, dir: Option<&str>, installed: Option<&str>) -> Setup {
    let mut setup = setup(mode, installed.map(|dir| (dir, Some("0.9.0"))));
    setup.launch.silent = true;
    setup.launch.dir = dir.map(PathBuf::from);
    setup
}

#[test]
fn silent_install_passes_s_and_d_through_and_opens_nothing() {
    let machine = Machine::new();
    assert_eq!(
        silent(
            &silent_setup(Mode::Install, Some("D:\\Tools\\FrameEver"), None),
            &machine.system()
        ),
        Silent::Run(Job::Install {
            dir: PathBuf::from("D:\\Tools\\FrameEver"),
            arguments: vec!["/S".into(), "/D=D:\\Tools\\FrameEver".into()],
            fresh: false,
            desktop: true,
            start_menu: true,
            path: true,
        })
    );
    let Silent::Run(Job::Install { dir, path, .. }) = silent(
        &silent_setup(Mode::Install, None, Some("E:\\FrameEver")),
        &machine.system(),
    ) else {
        panic!("install expected");
    };
    assert_eq!(dir, PathBuf::from("E:\\FrameEver"));
    assert!(!path);
}

#[test]
fn a_finished_install_writes_the_first_language_and_an_uninstall_does_not() {
    let machine = Machine::new();
    let Silent::Run(install) = silent(&silent_setup(Mode::Install, None, None), &machine.system())
    else {
        panic!("install expected");
    };
    assert!(writes_first_language(&install, &Ok(())));
    assert!(!writes_first_language(&install, &Err("failed".into())));
    let Silent::Run(uninstall) = silent(
        &silent_setup(Mode::Uninstall, None, Some("E:\\FrameEver")),
        &machine.system(),
    ) else {
        panic!("uninstall expected");
    };
    assert!(!writes_first_language(&uninstall, &Ok(())));
}

#[test]
fn silent_install_stops_for_a_recording_or_a_full_disk() {
    let machine = Machine::new();
    machine.set_app(AppState::Busy);
    let setup = silent_setup(Mode::Install, None, None);
    assert_eq!(silent(&setup, &machine.system()), Silent::Stop(EXIT_BUSY));
    machine.set_app(AppState::Idle);
    machine.hold(vec![frameever()]);
    assert!(matches!(
        silent(&setup, &machine.system()),
        Silent::CloseAppFirst(_)
    ));
    machine.free_on("C:", MB);
    assert_eq!(
        silent(&setup, &machine.system()),
        Silent::Stop(EXIT_NO_SPACE)
    );
}

#[test]
fn silent_uninstall_needs_an_install_and_keeps_settings() {
    let machine = Machine::new();
    assert_eq!(
        silent(
            &silent_setup(Mode::Uninstall, None, None),
            &machine.system()
        ),
        Silent::Stop(EXIT_NOT_INSTALLED)
    );
    assert_eq!(
        silent(
            &silent_setup(Mode::Uninstall, None, Some("E:\\FrameEver")),
            &machine.system()
        ),
        Silent::Run(Job::Uninstall {
            dir: PathBuf::from("E:\\FrameEver"),
            delete_settings: false
        })
    );
}
