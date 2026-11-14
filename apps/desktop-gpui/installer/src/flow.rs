use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use frameever_utils::brand::{APP_NAME, RELEASES_REPO};

use crate::{
    command_line::{Launch, Mode, install_arguments},
    i18n::{self, Language},
    instance_control::Reply,
    measure::{Bar, Need, format_size, needs},
    version::{Existing, compare},
};

pub const EXIT_FAILED: i32 = 2;
pub const EXIT_BUSY: i32 = 5;
pub const EXIT_NO_SPACE: i32 = 6;
pub const EXIT_NOT_INSTALLED: i32 = 7;

pub type PathProbe<T> = Box<dyn Fn(&Path) -> Option<T>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub dir: PathBuf,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    pub launch: Launch,
    pub version: String,
    pub installed: Option<Installed>,
    pub default_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub storage_dir: PathBuf,
    pub installed_bytes: u64,
    pub payload_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub name: String,
    pub frameever: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Absent,
    Idle,
    Busy,
    Unreachable,
}

pub struct System {
    pub volume: PathProbe<String>,
    pub free_space: PathProbe<u64>,
    pub holders: Box<dyn Fn(&Path) -> Vec<Holder>>,
    pub app: Box<dyn Fn() -> AppState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Desktop,
    StartMenu,
    Path,
    DeleteSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    ToggleOptions,
    Toggle(Choice),
    ChangeLocation,
    UninstallInstead,
    Uninstall,
    CloseApp,
    Cancel,
    TryAgain,
    ToggleDetails,
    ExportLogs,
    OpenReleases,
    OpenFolder,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Install {
        dir: PathBuf,
        arguments: Vec<String>,
        fresh: bool,
        desktop: bool,
        start_menu: bool,
        path: bool,
    },
    Uninstall {
        dir: PathBuf,
        delete_settings: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    PickLocation(PathBuf),
    Run(Job),
    CloseApp,
    Launch(PathBuf),
    OpenUrl(String),
    OpenFolder(PathBuf),
    ExportLogs,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    Gone,
    Refused,
    StillRunning,
    Unreachable,
}

pub fn quit_answer(reply: std::io::Result<Reply>, running: bool) -> Option<Closed> {
    match reply {
        Ok(Reply::Refused) => Some(Closed::Refused),
        Err(_) if running => Some(Closed::Unreachable),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Silent {
    Run(Job),
    CloseAppFirst(Job),
    Stop(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub action: Action,
    pub open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub choice: Choice,
    pub label: String,
    pub note: Option<String>,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub label: String,
    pub path: String,
    pub space: String,
    pub change: Button,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub label: String,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Main { title: String, tall: bool },
    Strip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub frame: Frame,
    pub hero: bool,
    pub heading: String,
    pub sentence: String,
    pub meta: Option<String>,
    pub location: Option<Location>,
    pub keep: Option<String>,
    pub checks: Vec<Check>,
    pub details: Option<String>,
    pub progress: Option<Progress>,
    pub links: Vec<Link>,
    pub buttons: Vec<Button>,
    pub locked: bool,
    pub failed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    Older,
    Same,
    Newer,
    Installing,
    AppInUse,
    Recording,
    NoSpace,
    Failed,
    LaunchFailed,
    Updating,
    UpdateFailed,
    Confirm,
    Uninstalling,
    UninstallFailed,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    Install,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shortfall {
    drive: String,
    needed: u64,
    free: u64,
}

pub struct Flow {
    setup: Setup,
    system: System,
    language: Language,
    page: Page,
    home: Page,
    intent: Intent,
    options_open: bool,
    location: PathBuf,
    desktop: bool,
    start_menu: bool,
    path: bool,
    delete_settings: bool,
    details_open: bool,
    failure: String,
    holders: Vec<Holder>,
    unreachable: bool,
    shortfall: Option<Shortfall>,
    bar: Bar,
}

fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    args.iter()
        .fold(template.to_string(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), &value.to_string())
        })
}

fn volume_root(volume: &str) -> PathBuf {
    PathBuf::from(format!("{volume}\\"))
}

fn first_shortfall(needs: &[Need], system: &System) -> Option<Shortfall> {
    needs.iter().find_map(|need| {
        let free = (system.free_space)(&volume_root(&need.volume))?;
        (free < need.bytes).then(|| Shortfall {
            drive: need.volume.trim_end_matches(':').to_string(),
            needed: need.bytes,
            free,
        })
    })
}

fn install_location(picked: &Path) -> PathBuf {
    let named = picked
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(APP_NAME));
    if named {
        picked.to_path_buf()
    } else {
        picked.join(APP_NAME)
    }
}

pub fn writes_first_language(job: &Job, outcome: &Result<(), String>) -> bool {
    matches!(job, Job::Install { .. }) && outcome.is_ok()
}

pub fn silent(setup: &Setup, system: &System) -> Silent {
    let job = match setup.launch.mode {
        Mode::Uninstall => {
            let Some(installed) = &setup.installed else {
                return Silent::Stop(EXIT_NOT_INSTALLED);
            };
            Job::Uninstall {
                dir: installed.dir.clone(),
                delete_settings: false,
            }
        }
        Mode::Install | Mode::Update => {
            let dir = setup
                .launch
                .dir
                .clone()
                .or_else(|| setup.installed.as_ref().map(|it| it.dir.clone()))
                .unwrap_or_else(|| setup.default_dir.clone());
            let needed = needs(
                &dir,
                &setup.temp_dir,
                setup.installed_bytes,
                setup.payload_bytes,
                &system.volume,
            );
            if first_shortfall(&needed, system).is_some() {
                return Silent::Stop(EXIT_NO_SPACE);
            }
            Job::Install {
                arguments: install_arguments(&dir, setup.launch.mode == Mode::Update, false),
                dir,
                fresh: false,
                desktop: true,
                start_menu: true,
                path: setup.installed.is_none(),
            }
        }
    };
    match (system.app)() {
        AppState::Busy | AppState::Unreachable => Silent::Stop(EXIT_BUSY),
        AppState::Idle => Silent::CloseAppFirst(job),
        AppState::Absent => Silent::Run(job),
    }
}

impl Flow {
    pub fn new(setup: Setup, system: System, language: Language) -> Self {
        let existing = setup
            .installed
            .as_ref()
            .map(|it| compare(&setup.version, it.version.as_deref()));
        let (page, intent) = match setup.launch.mode {
            Mode::Uninstall => (Page::Confirm, Intent::Uninstall),
            Mode::Update => (Page::Updating, Intent::Install),
            Mode::Install => (
                match existing {
                    None => Page::Welcome,
                    Some(Existing::Older(_) | Existing::Unknown) => Page::Older,
                    Some(Existing::Same) => Page::Same,
                    Some(Existing::Newer(_)) => Page::Newer,
                },
                Intent::Install,
            ),
        };
        let location = setup
            .installed
            .as_ref()
            .map(|it| it.dir.clone())
            .or_else(|| setup.launch.dir.clone())
            .unwrap_or_else(|| setup.default_dir.clone());
        Self {
            setup,
            system,
            language,
            page,
            home: page,
            intent,
            options_open: false,
            location,
            desktop: true,
            start_menu: true,
            path: true,
            delete_settings: false,
            details_open: false,
            failure: String::new(),
            holders: Vec::new(),
            unreachable: false,
            shortfall: None,
            bar: Bar::default(),
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn location(&self) -> &Path {
        &self.location
    }

    pub fn start(&mut self) -> Option<Effect> {
        (self.setup.launch.mode == Mode::Update)
            .then(|| self.gate())
            .flatten()
    }

    pub fn locked(&self) -> bool {
        matches!(
            self.page,
            Page::Installing | Page::Uninstalling | Page::Updating
        )
    }

    fn updating(&self) -> bool {
        self.setup.launch.mode == Mode::Update
    }

    fn fresh(&self) -> bool {
        self.setup.installed.is_none()
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.details_open = false;
    }

    fn go_home(&mut self) -> Option<Effect> {
        if self.home == Page::Confirm && self.page == Page::Confirm {
            return Some(Effect::Quit);
        }
        if self.home != Page::Confirm {
            self.intent = Intent::Install;
        }
        self.go(self.home);
        None
    }

    fn stop_update(&mut self, reason: String) {
        self.failure = reason;
        self.go(Page::UpdateFailed);
    }

    fn shortfall_sentence(&self, shortfall: &Shortfall) -> String {
        self.tr_fmt(
            "Drive {drive} has {free} free and FrameEver needs {needed}. Free up space or choose another drive.",
            &[
                ("drive", &shortfall.drive),
                ("free", &format_size(shortfall.free)),
                ("needed", &format_size(shortfall.needed)),
            ],
        )
    }

    fn recording_sentence(&self) -> String {
        self.tr(
            "FrameEver is recording. The installer won't close it. Stop the recording, then try again.",
        )
    }

    fn needs(&self) -> Vec<Need> {
        needs(
            &self.location,
            &self.setup.temp_dir,
            self.setup.installed_bytes,
            self.setup.payload_bytes,
            &self.system.volume,
        )
    }

    fn job(&self) -> Job {
        match self.intent {
            Intent::Uninstall => Job::Uninstall {
                dir: self.location.clone(),
                delete_settings: self.delete_settings,
            },
            Intent::Install => {
                let fresh = self.fresh();
                Job::Install {
                    arguments: install_arguments(
                        &self.location,
                        !fresh,
                        fresh && !self.desktop && !self.start_menu,
                    ),
                    dir: self.location.clone(),
                    fresh,
                    desktop: self.desktop,
                    start_menu: self.start_menu,
                    path: fresh && self.path,
                }
            }
        }
    }

    fn working_page(&self) -> Page {
        match (self.intent, self.updating()) {
            (Intent::Uninstall, _) => Page::Uninstalling,
            (Intent::Install, true) => Page::Updating,
            (Intent::Install, false) => Page::Installing,
        }
    }

    fn failed_page(&self) -> Page {
        match (self.intent, self.updating()) {
            (Intent::Uninstall, _) => Page::UninstallFailed,
            (Intent::Install, true) => Page::UpdateFailed,
            (Intent::Install, false) => Page::Failed,
        }
    }

    fn gate(&mut self) -> Option<Effect> {
        if self.intent == Intent::Install {
            self.shortfall = first_shortfall(&self.needs(), &self.system);
            if let Some(shortfall) = self.shortfall.clone() {
                if self.updating() {
                    self.stop_update(self.shortfall_sentence(&shortfall));
                } else {
                    self.go(Page::NoSpace);
                }
                return None;
            }
        }
        let app = (self.system.app)();
        if app == AppState::Busy {
            if self.updating() {
                self.stop_update(self.recording_sentence());
            } else {
                self.go(Page::Recording);
            }
            return None;
        }
        self.holders = (self.system.holders)(&self.location);
        self.unreachable = app == AppState::Unreachable;
        if app != AppState::Absent && !self.holders.iter().any(|it| it.frameever) {
            self.holders.insert(
                0,
                Holder {
                    name: APP_NAME.to_string(),
                    frameever: true,
                },
            );
        }
        if !self.holders.is_empty() {
            if self.updating() {
                return Some(Effect::CloseApp);
            }
            self.go(Page::AppInUse);
            return None;
        }
        self.bar = Bar::default();
        self.go(self.working_page());
        Some(Effect::Run(self.job()))
    }

    fn can_close_app(&self) -> bool {
        !self.unreachable && self.holders.iter().any(|it| it.frameever)
    }

    pub fn act(&mut self, action: Action) -> Option<Effect> {
        if self.locked() {
            return None;
        }
        match action {
            Action::Install => {
                self.intent = Intent::Install;
                self.options_open = false;
                self.gate()
            }
            Action::ToggleOptions if self.page == Page::Welcome => {
                self.options_open = !self.options_open;
                None
            }
            Action::Toggle(choice) => {
                let value = match choice {
                    Choice::Desktop => &mut self.desktop,
                    Choice::StartMenu => &mut self.start_menu,
                    Choice::Path => &mut self.path,
                    Choice::DeleteSettings => &mut self.delete_settings,
                };
                *value = !*value;
                None
            }
            Action::ChangeLocation if self.fresh() => {
                self.go(Page::Welcome);
                self.options_open = true;
                Some(Effect::PickLocation(self.location.clone()))
            }
            Action::UninstallInstead | Action::Uninstall if self.page != Page::Confirm => {
                self.intent = Intent::Uninstall;
                self.delete_settings = false;
                self.go(Page::Confirm);
                None
            }
            Action::Uninstall => {
                self.intent = Intent::Uninstall;
                self.gate()
            }
            Action::CloseApp if self.page == Page::AppInUse && self.can_close_app() => {
                Some(Effect::CloseApp)
            }
            Action::Cancel => self.go_home(),
            Action::TryAgain => match self.page {
                Page::LaunchFailed => Some(Effect::Launch(self.executable())),
                _ => self.gate(),
            },
            Action::ToggleDetails => {
                self.details_open = !self.details_open;
                None
            }
            Action::ExportLogs => Some(Effect::ExportLogs),
            Action::OpenReleases => Some(Effect::OpenUrl(format!(
                "https://github.com/{RELEASES_REPO}/releases"
            ))),
            Action::OpenFolder => Some(Effect::OpenFolder(self.setup.storage_dir.clone())),
            Action::Close => Some(Effect::Quit),
            _ => None,
        }
    }

    pub fn location_picked(&mut self, picked: &Path) {
        if self.fresh() {
            self.location = install_location(picked);
        }
    }

    pub fn app_closed(&mut self, closed: Closed) -> Option<Effect> {
        match closed {
            Closed::Refused if self.updating() => {
                self.stop_update(self.recording_sentence());
                None
            }
            Closed::Refused => {
                self.go(Page::Recording);
                None
            }
            Closed::StillRunning if self.updating() => {
                self.stop_update(self.tr("FrameEver is open, so its files can't be replaced."));
                None
            }
            Closed::Unreachable if self.updating() => {
                self.stop_update(self.tr(
                    "FrameEver is open, so its files can't be replaced. Quit it from the tray, then try again.",
                ));
                None
            }
            Closed::Unreachable => {
                self.unreachable = true;
                self.go(Page::AppInUse);
                None
            }
            Closed::Gone | Closed::StillRunning => self.gate(),
        }
    }

    pub fn progress(&mut self, fraction: f64) {
        if self.locked() {
            self.bar.advance(fraction);
        }
    }

    pub fn percent(&self) -> u8 {
        self.bar.percent()
    }

    pub fn executable(&self) -> PathBuf {
        self.location.join(format!("{APP_NAME}.exe"))
    }

    pub fn finished(&mut self, outcome: Result<(), String>) -> Option<Effect> {
        if !self.locked() {
            return None;
        }
        match outcome {
            Ok(()) => {
                self.bar.complete();
                match self.intent {
                    Intent::Install => Some(Effect::Launch(self.executable())),
                    Intent::Uninstall => {
                        self.go(Page::Removed);
                        None
                    }
                }
            }
            Err(details) => {
                self.failure = details;
                self.shortfall = None;
                self.go(self.failed_page());
                None
            }
        }
    }

    pub fn launched(&mut self, outcome: Result<(), String>) -> Option<Effect> {
        match outcome {
            Ok(()) => Some(Effect::Quit),
            Err(details) => {
                self.failure = details;
                self.go(if self.updating() {
                    Page::UpdateFailed
                } else {
                    Page::LaunchFailed
                });
                None
            }
        }
    }

    pub fn primary(&self) -> Option<Action> {
        if self.locked() {
            return None;
        }
        self.screen()
            .buttons
            .iter()
            .find(|button| matches!(button.style, Style::Primary | Style::Danger))
            .map(|button| button.action)
    }

    pub fn escape(&self) -> Option<Action> {
        match self.page {
            _ if self.locked() => None,
            Page::Welcome if self.options_open => Some(Action::ToggleOptions),
            Page::AppInUse | Page::Recording | Page::NoSpace | Page::Confirm => {
                Some(Action::Cancel)
            }
            _ => Some(Action::Close),
        }
    }

    fn tr(&self, english: &'static str) -> String {
        i18n::translate(english, self.language).to_string()
    }

    fn tr_fmt(&self, template: &'static str, args: &[(&str, &dyn Display)]) -> String {
        fill(i18n::translate(template, self.language), args)
    }

    fn button(&self, label: &'static str, action: Action, style: Style) -> Button {
        Button {
            label: self.tr(label),
            action,
            style,
        }
    }

    fn name(&self) -> String {
        self.tr("FrameEver")
    }

    fn tagline(&self) -> String {
        self.tr("Record, edit and export, all on your own computer.")
    }

    fn installed_version(&self) -> String {
        self.setup
            .installed
            .as_ref()
            .and_then(|it| it.version.clone())
            .unwrap_or_default()
    }

    fn storage_folder(&self) -> String {
        self.setup.storage_dir.display().to_string()
    }

    fn space_line(&self) -> String {
        let needs = self.needs();
        let Some(need) = needs.first() else {
            return String::new();
        };
        let needed = format_size(need.bytes);
        match (self.system.free_space)(&volume_root(&need.volume)) {
            Some(free) => self.tr_fmt(
                "Needs {needed} · {free} free",
                &[("needed", &needed), ("free", &format_size(free))],
            ),
            None => self.tr_fmt("Needs {needed}", &[("needed", &needed)]),
        }
    }

    fn title(&self) -> String {
        match (self.intent, self.page) {
            (Intent::Uninstall, _) => self.tr("Uninstall FrameEver"),
            (_, Page::Older) => self.tr("Update FrameEver"),
            _ => self.tr("Install FrameEver"),
        }
    }

    fn blank(&self) -> Screen {
        Screen {
            frame: Frame::Main {
                title: self.title(),
                tall: false,
            },
            hero: false,
            heading: String::new(),
            sentence: String::new(),
            meta: None,
            location: None,
            keep: None,
            checks: Vec::new(),
            details: None,
            progress: None,
            links: Vec::new(),
            buttons: Vec::new(),
            locked: self.locked(),
            failed: false,
        }
    }

    fn check(&self, choice: Choice, label: String, note: Option<String>) -> Check {
        let on = match choice {
            Choice::Desktop => self.desktop,
            Choice::StartMenu => self.start_menu,
            Choice::Path => self.path,
            Choice::DeleteSettings => self.delete_settings,
        };
        Check {
            choice,
            label,
            note,
            on,
        }
    }

    fn details_link(&self, screen: &mut Screen) {
        screen.links.push(Link {
            label: self.tr("Details"),
            action: Action::ToggleDetails,
            open: Some(self.details_open),
        });
        if self.details_open {
            screen.details = Some(self.failure.clone());
            if let Frame::Main { tall, .. } = &mut screen.frame {
                *tall = true;
            }
        }
    }

    fn holder_sentence(&self) -> String {
        if self.holders.iter().any(|it| it.frameever) {
            if self.unreachable {
                return self.tr(
                    "FrameEver is open, so its files can't be replaced. Quit it from the tray, then try again.",
                );
            }
            return self.tr("FrameEver is open, so its files can't be replaced.");
        }
        let mut names: Vec<&str> = self.holders.iter().map(|it| it.name.as_str()).collect();
        names.dedup();
        self.tr_fmt(
            "{apps} is using FrameEver's files. Close it, then try again.",
            &[("apps", &names.join(", "))],
        )
    }

    pub fn screen(&self) -> Screen {
        let mut screen = self.blank();
        let version = self.setup.version.clone();
        match self.page {
            Page::Welcome => {
                screen.hero = true;
                screen.heading = self.name();
                screen.sentence = self.tagline();
                screen.buttons = vec![self.button("Install", Action::Install, Style::Primary)];
                screen.links.push(Link {
                    label: self.tr("Options"),
                    action: Action::ToggleOptions,
                    open: Some(self.options_open),
                });
                if self.options_open {
                    screen.frame = Frame::Main {
                        title: self.title(),
                        tall: true,
                    };
                    screen.location = Some(Location {
                        label: self.tr("Install location"),
                        path: self.location.display().to_string(),
                        space: self.space_line(),
                        change: self.button("Change…", Action::ChangeLocation, Style::Secondary),
                    });
                    screen.checks = vec![
                        self.check(Choice::Desktop, self.tr("Desktop shortcut"), None),
                        self.check(Choice::StartMenu, self.tr("Start menu shortcut"), None),
                        self.check(
                            Choice::Path,
                            self.tr("Add frameever to PATH"),
                            Some(self.tr("Run the CLI from any terminal.")),
                        ),
                    ];
                }
            }
            Page::Older => {
                screen.hero = true;
                screen.heading = self.tr_fmt("Update to {version}", &[("version", &version)]);
                let installed = self.installed_version();
                screen.sentence = if compare(&version, Some(&installed)) == Existing::Unknown {
                    self.tr("An earlier version is installed. Location, shortcuts and settings stay as they are.")
                } else {
                    self.tr_fmt(
                        "Version {version} is installed. Location, shortcuts and settings stay as they are.",
                        &[("version", &installed)],
                    )
                };
                screen.links.push(Link {
                    label: self.tr("Uninstall instead"),
                    action: Action::UninstallInstead,
                    open: None,
                });
                screen.buttons = vec![self.button("Update", Action::Install, Style::Primary)];
            }
            Page::Same => {
                screen.hero = true;
                screen.heading =
                    self.tr_fmt("FrameEver {version} is installed", &[("version", &version)]);
                screen.sentence = self.tr("Reinstall it to repair missing files. Settings stay.");
                screen.links.push(Link {
                    label: self.tr("Uninstall"),
                    action: Action::Uninstall,
                    open: None,
                });
                screen.buttons = vec![self.button("Reinstall", Action::Install, Style::Primary)];
            }
            Page::Newer => {
                let installed = self.installed_version();
                screen.hero = true;
                screen.heading = self.tr("A newer version is installed");
                screen.sentence = self.tr_fmt(
                    "FrameEver {installed} is on this computer. This installer is {version}.",
                    &[("installed", &installed), ("version", &version)],
                );
                screen.buttons = vec![
                    Button {
                        label: self.tr_fmt("Install {version}", &[("version", &version)]),
                        action: Action::Install,
                        style: Style::Secondary,
                    },
                    Button {
                        label: self.tr_fmt("Keep {installed}", &[("installed", &installed)]),
                        action: Action::Close,
                        style: Style::Primary,
                    },
                ];
            }
            Page::Installing => {
                screen.hero = true;
                screen.heading = self.name();
                screen.sentence = self.tagline();
                let label = if self.bar.percent() >= 100 {
                    self.tr("Opening FrameEver…")
                } else {
                    self.tr("Installing…")
                };
                screen.progress = Some(Progress {
                    label,
                    percent: self.bar.percent(),
                });
            }
            Page::AppInUse => {
                screen.heading = if self.holders.iter().any(|it| it.frameever) {
                    self.tr("Close FrameEver to continue")
                } else {
                    self.tr("Close the other program to continue")
                };
                screen.sentence = self.holder_sentence();
                screen.buttons = vec![self.button("Cancel", Action::Cancel, Style::Secondary)];
                screen.buttons.push(if self.can_close_app() {
                    self.button("Close FrameEver", Action::CloseApp, Style::Primary)
                } else {
                    self.button("Try again", Action::TryAgain, Style::Primary)
                });
            }
            Page::Recording => {
                screen.heading = self.tr("Finish your recording first");
                screen.sentence = self.recording_sentence();
                screen.buttons = vec![
                    self.button("Cancel", Action::Cancel, Style::Secondary),
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::NoSpace => {
                screen.heading = self.tr("Couldn't install FrameEver");
                if let Some(shortfall) = &self.shortfall {
                    screen.sentence = self.shortfall_sentence(shortfall);
                }
                screen.buttons = vec![
                    if self.fresh() {
                        self.button("Change location", Action::ChangeLocation, Style::Secondary)
                    } else {
                        self.button("Cancel", Action::Cancel, Style::Secondary)
                    },
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::Failed => {
                screen.failed = true;
                screen.heading = self.tr("Couldn't install FrameEver");
                screen.sentence = self.tr("Setup stopped before it finished. Try again.");
                self.details_link(&mut screen);
                screen.buttons = vec![
                    self.button("Export logs", Action::ExportLogs, Style::Secondary),
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::LaunchFailed => {
                screen.failed = true;
                screen.heading = self.tr("Couldn't open FrameEver");
                screen.sentence = self.tr("FrameEver is installed but didn't start. Try again.");
                self.details_link(&mut screen);
                screen.buttons = vec![
                    self.button("Close", Action::Close, Style::Secondary),
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::Updating => {
                screen.frame = Frame::Strip;
                screen.heading = self.tr("Updating FrameEver…");
                let from = self.installed_version();
                screen.meta = Some(if from.is_empty() {
                    version.clone()
                } else {
                    format!("{from} → {version}")
                });
                screen.progress = Some(Progress {
                    label: screen.heading.clone(),
                    percent: self.bar.percent(),
                });
            }
            Page::UpdateFailed => {
                screen.frame = Frame::Strip;
                screen.failed = true;
                screen.heading = self.tr("Couldn't update FrameEver");
                screen.sentence = self.tr("Try again, or get the installer from GitHub Releases.");
                self.details_link(&mut screen);
                screen.buttons = vec![
                    self.button("Open Releases", Action::OpenReleases, Style::Secondary),
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::Confirm => {
                screen.heading = self.tr("Uninstall FrameEver?");
                screen.sentence =
                    self.tr("This removes the app, its shortcuts and its PATH entry.");
                screen.keep = Some(self.tr_fmt(
                    "Recordings, screenshots and exports stay in {folder}.",
                    &[("folder", &self.storage_folder())],
                ));
                screen.checks = vec![self.check(
                    Choice::DeleteSettings,
                    self.tr("Also delete settings"),
                    Some(self.tr("Settings and logs in the app data folder.")),
                )];
                screen.buttons = vec![
                    self.button("Cancel", Action::Cancel, Style::Secondary),
                    self.button("Uninstall", Action::Uninstall, Style::Danger),
                ];
            }
            Page::Uninstalling => {
                screen.heading = self.tr("Uninstalling FrameEver");
                screen.sentence = self.tr("Your recordings stay where they are.");
                screen.progress = Some(Progress {
                    label: self.tr("Uninstalling…"),
                    percent: self.bar.percent(),
                });
            }
            Page::UninstallFailed => {
                screen.failed = true;
                screen.heading = self.tr("Couldn't uninstall FrameEver");
                screen.sentence = self.tr("Uninstall stopped before it finished. Try again.");
                self.details_link(&mut screen);
                screen.buttons = vec![
                    self.button("Export logs", Action::ExportLogs, Style::Secondary),
                    self.button("Try again", Action::TryAgain, Style::Primary),
                ];
            }
            Page::Removed => {
                screen.heading = self.tr("FrameEver is uninstalled");
                screen.sentence = self.tr_fmt(
                    "Your recordings, screenshots and exports are still in {folder}.",
                    &[("folder", &self.storage_folder())],
                );
                screen.buttons = vec![
                    self.button("Open folder", Action::OpenFolder, Style::Secondary),
                    self.button("Close", Action::Close, Style::Primary),
                ];
            }
        }
        screen
    }
}
