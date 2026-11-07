use std::{
    collections::HashSet,
    fmt::Display,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use frameever_converter::{
    Item, OldApp, OldInstall, Plan, Progress, Report, Selection, SkipReason, Source, Target,
    convert_watched, destination,
};
use frameever_utils::{brand::APP_NAME, storage::ProjectKind};

use crate::i18n::{self, Language};

pub type PathProbe<T> = Box<dyn Fn(&Path) -> Option<T>>;

pub struct System {
    pub find_installs: Box<dyn Fn() -> Vec<OldInstall>>,
    pub running: Box<dyn Fn() -> Vec<String>>,
    pub volume: PathProbe<String>,
    pub free_space: PathProbe<u64>,
    pub installed_app: Box<dyn Fn() -> Option<PathBuf>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Recordings,
    Screenshots,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Choose(usize),
    PickFolder,
    Back,
    Toggle(Kind),
    Change,
    Recheck,
    Convert,
    Stop,
    ConvertRest,
    TryAgain,
    OpenFrameEver,
    ShowFolder,
    ConvertAnother,
    Details(Disclosure),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Disclosure {
    Settings,
    Item(usize),
}

pub enum Effect {
    PickSource,
    PickDestination,
    Run(Job),
    Open(PathBuf),
    Launch(PathBuf),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Folder,
    FolderOpen,
    Video,
    Image,
    Settings,
    Alert,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Primary,
    Secondary,
    Ghost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub style: Style,
    pub icon: Option<Icon>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Details {
    pub label: String,
    pub raw: String,
    pub open: bool,
    pub toggle: Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub text: String,
    pub details: Option<Details>,
    pub button: Option<Button>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Empty {
    pub text: String,
    pub button: Button,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub icon: Option<Icon>,
    pub title: String,
    pub detail: String,
    pub trailing: String,
    pub path: Option<String>,
    pub check: Option<(Kind, bool)>,
    pub action: Option<Action>,
    pub button: Option<Button>,
    pub details: Option<Details>,
    pub off: bool,
    pub warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meter {
    pub percent: u8,
    pub line: String,
    pub left: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Current,
    Ahead,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screen {
    pub steps: Vec<(String, StepState)>,
    pub title: String,
    pub subtitle: String,
    pub tick: bool,
    pub warnings: Vec<Warning>,
    pub empty: Option<Empty>,
    pub groups: Vec<Vec<Row>>,
    pub meter: Option<Meter>,
    pub notes: Vec<String>,
    pub start: Vec<Button>,
    pub end: Vec<Button>,
}

pub struct Job {
    install: OldInstall,
    target: Target,
    plan: Plan,
    stop: Arc<AtomicBool>,
}

impl Job {
    pub fn run(self, on_progress: impl FnMut(&Progress)) -> Report {
        convert_watched(
            &self.install,
            &self.target,
            &self.plan,
            &self.stop,
            on_progress,
        )
    }
}

struct Found {
    install: OldInstall,
    recordings: usize,
    screenshots: usize,
    bytes: u64,
}

struct Measured {
    kind: ProjectKind,
    bytes: u64,
    crosses: bool,
}

struct Review {
    install: OldInstall,
    items: Vec<Item>,
    measured: Vec<Measured>,
    rules: usize,
    selection: Selection,
    destination: PathBuf,
    drive: String,
    free: Option<u64>,
    running: Vec<String>,
}

impl Review {
    fn count(&self, kind: ProjectKind) -> usize {
        self.measured
            .iter()
            .filter(|item| item.kind == kind)
            .count()
    }

    fn bytes(&self, kind: ProjectKind) -> u64 {
        self.measured
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| item.bytes)
            .sum()
    }

    fn selected(&self) -> usize {
        self.measured
            .iter()
            .filter(|item| self.selection.takes(item.kind))
            .count()
    }

    fn carries_settings(&self) -> bool {
        self.install.has_settings() && self.selection.settings
    }

    fn crosses(&self) -> bool {
        self.measured.iter().any(|item| item.crosses)
    }

    fn needed(&self) -> u64 {
        self.measured
            .iter()
            .filter(|item| item.crosses && self.selection.takes(item.kind))
            .map(|item| item.bytes)
            .sum()
    }

    fn lacks_space(&self) -> bool {
        let needed = self.needed();
        needed > 0 && self.free.is_some_and(|free| free < needed)
    }

    fn space_unknown(&self) -> bool {
        self.needed() > 0 && self.free.is_none()
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty() && !self.install.has_settings()
    }

    fn can_convert(&self) -> bool {
        (self.selected() > 0 || self.carries_settings()) && !self.lacks_space()
    }

    fn running_app(&self, app: &str) -> bool {
        self.running
            .iter()
            .any(|process| process.eq_ignore_ascii_case(app))
    }

    fn plan(&self) -> Plan {
        Plan {
            selection: self.selection,
            destination: Some(self.destination.clone()),
        }
    }
}

enum Step {
    Choose,
    Review,
    Converting { progress: Option<Progress> },
    Done(Report),
}

pub struct Wizard {
    target: Target,
    system: System,
    language: Language,
    found: Vec<Found>,
    finished: Vec<OldInstall>,
    review: Option<Review>,
    step: Step,
    stop: Arc<AtomicBool>,
    details_open: HashSet<Disclosure>,
}

impl Wizard {
    pub fn new(target: Target, system: System) -> Self {
        let mut wizard = Self {
            target,
            system,
            language: i18n::current(),
            found: Vec::new(),
            finished: Vec::new(),
            review: None,
            step: Step::Choose,
            stop: Arc::new(AtomicBool::new(false)),
            details_open: HashSet::new(),
        };
        wizard.find();
        if wizard.found.len() == 1 {
            wizard.choose(0);
        }
        wizard
    }

    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    pub fn is_converting(&self) -> bool {
        matches!(self.step, Step::Converting { .. })
    }

    fn find(&mut self) {
        self.found = (self.system.find_installs)()
            .into_iter()
            .map(|install| {
                let items = install.items();
                let count = |kind| items.iter().filter(|item| item.kind() == kind).count();
                Found {
                    recordings: count(ProjectKind::Recording),
                    screenshots: count(ProjectKind::Screenshot),
                    bytes: items.iter().map(Item::size).sum(),
                    install,
                }
            })
            .collect();
    }

    fn choose(&mut self, index: usize) {
        if let Some(found) = self.found.get(index) {
            let install = found.install.clone();
            self.open_review(install);
        }
    }

    pub fn choose_folder(&mut self, folder: &Path) {
        if matches!(self.step, Step::Choose | Step::Review) {
            self.open_review(OldInstall::picked_folder(folder));
        }
    }

    pub fn change_destination(&mut self, folder: &Path) {
        if !matches!(self.step, Step::Review) {
            return;
        }
        if let Some(mut review) = self.review.take() {
            review.destination = folder.to_path_buf();
            self.measure(&mut review);
            self.review = Some(review);
        }
    }

    fn open_review(&mut self, install: OldInstall) {
        let items = install.items();
        let mut review = Review {
            destination: destination(&install, &self.target),
            rules: install.rule_count(),
            install,
            items,
            measured: Vec::new(),
            selection: Selection::default(),
            drive: String::new(),
            free: None,
            running: (self.system.running)(),
        };
        self.measure(&mut review);
        self.review = Some(review);
        self.details_open.clear();
        self.step = Step::Review;
    }

    fn measure(&self, review: &mut Review) {
        let target = nearest_existing(&review.destination);
        let drive = (self.system.volume)(&target);
        review.measured = review
            .items
            .iter()
            .map(|item| Measured {
                kind: item.kind(),
                bytes: item.size(),
                crosses: drive.is_some() && (self.system.volume)(item.source()) != drive,
            })
            .collect();
        review.drive = drive.unwrap_or_default();
        review.free = (self.system.free_space)(&target);
    }

    fn start(&mut self) -> Option<Effect> {
        let review = self.review.as_ref()?;
        self.stop = Arc::new(AtomicBool::new(false));
        let job = Job {
            install: review.install.clone(),
            target: self.target.clone(),
            plan: review.plan(),
            stop: self.stop.clone(),
        };
        self.details_open.clear();
        self.step = Step::Converting { progress: None };
        Some(Effect::Run(job))
    }

    pub fn progress(&mut self, progress: Progress) {
        if let Step::Converting { progress: current } = &mut self.step {
            *current = Some(progress);
        }
    }

    pub fn finish(&mut self, report: Report) {
        if !self.is_converting() {
            return;
        }
        if report.skipped.is_empty()
            && report.settings_error.is_none()
            && !report.stopped
            && let Some(review) = &self.review
            && !self.finished.contains(&review.install)
        {
            self.finished.push(review.install.clone());
        }
        self.step = Step::Done(report);
    }

    fn retry(&mut self) -> Option<Effect> {
        let Step::Done(report) = &self.step else {
            return None;
        };
        let settings_carried = report.settings_error.is_none();
        let mut review = self.review.take()?;
        if settings_carried && review.carries_settings() {
            review.selection.settings = false;
        }
        review.items = review.install.items();
        review.running = (self.system.running)();
        self.measure(&mut review);
        let ready = review.can_convert();
        self.review = Some(review);
        if ready {
            self.start()
        } else {
            self.details_open.clear();
            self.step = Step::Review;
            None
        }
    }

    fn installed_app(&self) -> Option<PathBuf> {
        (self.system.installed_app)()
    }

    fn remaining_install(&self) -> bool {
        let current = self.review.as_ref().map(|review| &review.install);
        self.found
            .iter()
            .any(|found| Some(&found.install) != current && !self.finished.contains(&found.install))
    }

    pub fn act(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::Choose(index) => {
                if matches!(self.step, Step::Choose) {
                    self.choose(index);
                }
                None
            }
            Action::PickFolder => Some(Effect::PickSource),
            Action::Back => {
                if matches!(self.step, Step::Review) {
                    self.find();
                    self.step = Step::Choose;
                }
                None
            }
            Action::ConvertAnother => {
                self.find();
                self.step = Step::Choose;
                None
            }
            Action::Toggle(kind) => {
                if let (Step::Review, Some(review)) = (&self.step, &mut self.review) {
                    let selection = &mut review.selection;
                    match kind {
                        Kind::Recordings => selection.recordings = !selection.recordings,
                        Kind::Screenshots => selection.screenshots = !selection.screenshots,
                        Kind::Settings => selection.settings = !selection.settings,
                    }
                }
                None
            }
            Action::Change => Some(Effect::PickDestination),
            Action::Recheck => {
                let running = (self.system.running)();
                if let Some(review) = &mut self.review {
                    review.running = running;
                }
                None
            }
            Action::Convert => {
                let ready = matches!(self.step, Step::Review)
                    && self.review.as_ref().is_some_and(Review::can_convert);
                if ready { self.start() } else { None }
            }
            Action::ConvertRest | Action::TryAgain => self.retry(),
            Action::Stop | Action::Close if self.is_converting() => {
                self.stop.store(true, Ordering::SeqCst);
                None
            }
            Action::Stop => None,
            Action::Close => Some(Effect::Quit),
            Action::OpenFrameEver => self.installed_app().map(Effect::Launch),
            Action::ShowFolder => self
                .review
                .as_ref()
                .map(|review| Effect::Open(review.destination.clone())),
            Action::Details(disclosure) => {
                if !self.details_open.remove(&disclosure) {
                    self.details_open.insert(disclosure);
                }
                None
            }
        }
    }

    pub fn primary(&self) -> Option<Action> {
        let screen = self.screen();
        screen
            .start
            .iter()
            .chain(&screen.end)
            .chain(screen.empty.as_ref().map(|empty| &empty.button))
            .find(|button| button.style == Style::Primary && button.enabled)
            .map(|button| button.action)
    }

    pub fn escape(&self) -> Option<Action> {
        matches!(self.step, Step::Review).then_some(Action::Back)
    }

    fn tr(&self, english: &'static str) -> String {
        i18n::translate(english, self.language).to_string()
    }

    fn tr_fmt(&self, template: &'static str, args: &[(&str, &dyn Display)]) -> String {
        fill(i18n::translate(template, self.language), args)
    }

    fn tr_fmt_in(
        &self,
        context: &'static str,
        template: &'static str,
        args: &[(&str, &dyn Display)],
    ) -> String {
        fill(i18n::translate_in(context, template, self.language), args)
    }

    fn button(&self, label: &'static str, action: Action, style: Style) -> Button {
        Button {
            label: self.tr(label),
            action,
            style,
            icon: None,
            enabled: true,
        }
    }

    fn source_name(&self, install: &OldInstall) -> String {
        match install.source {
            Source::CapAniBullet2 | Source::Cap => install.source.old_apps()[0].name.to_string(),
            Source::Folder => install.app_data.display().to_string(),
        }
    }

    fn steps(&self) -> Vec<(String, StepState)> {
        let at = match self.step {
            Step::Choose => 0,
            Step::Review => 1,
            Step::Converting { .. } => 2,
            Step::Done(_) => 3,
        };
        [
            self.tr("Choose"),
            self.tr("Review"),
            self.tr("Convert"),
            self.tr("Done"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let state = match index.cmp(&at) {
                std::cmp::Ordering::Less => StepState::Done,
                std::cmp::Ordering::Equal => StepState::Current,
                std::cmp::Ordering::Greater => StepState::Ahead,
            };
            (label, state)
        })
        .collect()
    }

    pub fn screen(&self) -> Screen {
        let screen = match (&self.step, &self.review) {
            (Step::Review, Some(review)) => self.review_screen(review),
            (Step::Converting { progress }, Some(review)) => {
                self.converting_screen(review, progress.as_ref())
            }
            (Step::Done(report), Some(_)) => self.done_screen(report),
            _ => self.choose_screen(),
        };
        Screen {
            steps: self.steps(),
            ..screen
        }
    }

    fn choose_screen(&self) -> Screen {
        let close = self.button("Close", Action::Close, Style::Secondary);
        if self.found.is_empty() {
            return Screen {
                empty: Some(Empty {
                    text: self.tr("No Cap or Cap-aniBullet 2.x data on this PC. Choose the folder that holds your recordings."),
                    button: self.button("Choose a folder…", Action::PickFolder, Style::Primary),
                }),
                end: vec![close],
                ..Screen::default()
            };
        }
        Screen {
            title: self.tr("Bring your recordings into FrameEver"),
            subtitle: self.tr("Found on this PC. Choose one to see what comes over."),
            groups: vec![
                self.found
                    .iter()
                    .enumerate()
                    .map(|(index, found)| Row {
                        icon: Some(Icon::Folder),
                        title: self.source_name(&found.install),
                        detail: self.tr_fmt(
                            "{recordings} recordings · {screenshots} screenshots · {size}",
                            &[
                                ("recordings", &found.recordings),
                                ("screenshots", &found.screenshots),
                                ("size", &size_text(found.bytes)),
                            ],
                        ),
                        trailing: if self.finished.contains(&found.install) {
                            self.tr("Converted")
                        } else {
                            String::new()
                        },
                        action: Some(Action::Choose(index)),
                        ..Row::default()
                    })
                    .collect(),
            ],
            start: vec![self.button("Choose a folder…", Action::PickFolder, Style::Ghost)],
            end: vec![close],
            ..Screen::default()
        }
    }

    fn back(&self) -> Button {
        Button {
            icon: Some(Icon::Back),
            ..self.button("Back", Action::Back, Style::Ghost)
        }
    }

    fn review_screen(&self, review: &Review) -> Screen {
        if review.is_empty() {
            return Screen {
                empty: Some(Empty {
                    text: self.tr("There's nothing to convert in this folder."),
                    button: self.button("Choose another…", Action::PickFolder, Style::Secondary),
                }),
                start: vec![self.back()],
                ..Screen::default()
            };
        }
        let selection = review.selection;
        let mut kinds = Vec::new();
        let recordings = review.count(ProjectKind::Recording);
        if recordings > 0 {
            kinds.push(Row {
                icon: Some(Icon::Video),
                title: self.tr("Recordings"),
                detail: self.tr(".cap projects and loose MP4 exports"),
                trailing: self.tr_fmt_in(
                    "recordings",
                    "{count} · {size}",
                    &[
                        ("count", &recordings),
                        ("size", &size_text(review.bytes(ProjectKind::Recording))),
                    ],
                ),
                check: Some((Kind::Recordings, selection.recordings)),
                off: !selection.recordings,
                ..Row::default()
            });
        }
        let screenshots = review.count(ProjectKind::Screenshot);
        if screenshots > 0 {
            kinds.push(Row {
                icon: Some(Icon::Image),
                title: self.tr("Screenshots"),
                detail: self.tr("PNG and JPEG, kept with their dates"),
                trailing: self.tr_fmt_in(
                    "screenshots",
                    "{count} · {size}",
                    &[
                        ("count", &screenshots),
                        ("size", &size_text(review.bytes(ProjectKind::Screenshot))),
                    ],
                ),
                check: Some((Kind::Screenshots, selection.screenshots)),
                off: !selection.screenshots,
                ..Row::default()
            });
        }
        if review.install.has_settings() {
            kinds.push(Row {
                icon: Some(Icon::Settings),
                title: self.tr("Settings and automations"),
                detail: self.tr("Only what FrameEver doesn't have yet"),
                trailing: if review.rules > 0 {
                    self.tr_fmt("{count} rules", &[("count", &review.rules)])
                } else {
                    String::new()
                },
                check: Some((Kind::Settings, selection.settings)),
                off: !selection.settings,
                ..Row::default()
            });
        }
        let destination = Row {
            title: self.tr("Destination"),
            path: Some(review.destination.display().to_string()),
            button: Some(self.button("Change…", Action::Change, Style::Secondary)),
            ..Row::default()
        };
        let selected = review.selected();
        let convert = Button {
            enabled: review.can_convert(),
            ..if selected > 0 {
                Button {
                    label: self.tr_fmt("Convert {count} items", &[("count", &selected)]),
                    ..self.button("Convert", Action::Convert, Style::Primary)
                }
            } else {
                self.button("Convert", Action::Convert, Style::Primary)
            }
        };
        Screen {
            title: self.tr("Choose what comes over"),
            subtitle: self.tr_fmt(
                "From {source}",
                &[("source", &self.source_name(&review.install))],
            ),
            warnings: self.review_warnings(review),
            groups: vec![kinds, vec![destination]],
            notes: vec![if review.crosses() {
                self.tr("Files are moved to another drive: copied, checked, then the originals are deleted. Afterwards only FrameEver opens them.")
            } else {
                self.tr("Files are moved, not copied. Afterwards only FrameEver opens them.")
            }],
            start: vec![self.back()],
            end: vec![convert],
            ..Screen::default()
        }
    }

    fn review_warnings(&self, review: &Review) -> Vec<Warning> {
        let mut warnings = Vec::new();
        if review.space_unknown() {
            warnings.push(Warning {
                text: self.tr_fmt(
                    "Couldn't check the free space on {drive}. This needs {needed}.",
                    &[
                        ("drive", &review.drive),
                        ("needed", &size_text(review.needed())),
                    ],
                ),
                details: None,
                button: None,
            });
        }
        if review.lacks_space() {
            warnings.push(Warning {
                text: self.tr_fmt(
                    "Not enough space on {drive}. This needs {needed} and {free} is free. Change the folder or convert less.",
                    &[
                        ("drive", &review.drive),
                        ("needed", &size_text(review.needed())),
                        ("free", &size_text(review.free.unwrap_or_default())),
                    ],
                ),
                details: None,
                button: None,
            });
        }
        let check_again = || Some(self.button("Check again", Action::Recheck, Style::Ghost));
        let running_old: Vec<&OldApp> = review
            .install
            .source
            .old_apps()
            .iter()
            .filter(|app| review.running_app(app.process))
            .collect();
        for app in running_old {
            warnings.push(Warning {
                text: self.tr_fmt(
                    "{app} is running. Close it first, or the projects it has open stay behind.",
                    &[("app", &app.name)],
                ),
                details: None,
                button: check_again(),
            });
        }
        if review.carries_settings() && review.running_app(&format!("{APP_NAME}.exe")) {
            warnings.push(Warning {
                text: self.tr(
                    "FrameEver is running. Close it first, or it may overwrite the automations brought over.",
                ),
                details: None,
                button: check_again(),
            });
        }
        warnings
    }

    fn converting_screen(&self, review: &Review, progress: Option<&Progress>) -> Screen {
        let stopping = self.stop.load(Ordering::SeqCst);
        let (done, total) = progress.map_or((0, 0), |progress| (progress.done, progress.total));
        let line = match progress.and_then(|progress| progress.current.as_ref()) {
            _ if stopping => self.tr("Finishing this item…"),
            Some(name) => self.tr_fmt(
                "Moving {at} of {total}: {name}",
                &[("at", &(done + 1)), ("total", &total), ("name", name)],
            ),
            None => String::new(),
        };
        let left = match progress {
            Some(progress) if review.crosses() => {
                self.tr_fmt("{size} left", &[("size", &size_text(progress.bytes_left))])
            }
            _ => String::new(),
        };
        Screen {
            title: self.tr("Converting…"),
            subtitle: self.tr("Keep this window open."),
            meter: Some(Meter {
                percent: percent(done, total),
                line,
                left,
            }),
            notes: vec![self.tr("Items not moved yet stay where they are if you stop.")],
            end: vec![Button {
                enabled: !stopping,
                ..self.button("Stop", Action::Stop, Style::Secondary)
            }],
            ..Screen::default()
        }
    }

    fn done_screen(&self, report: &Report) -> Screen {
        let total = report.total();
        let converted = report.converted.len();
        let details = |disclosure: Disclosure, raw: &str| Details {
            label: self.tr("Details"),
            raw: raw.to_string(),
            open: self.details_open.contains(&disclosure),
            toggle: Action::Details(disclosure),
        };
        let warnings: Vec<Warning> = report
            .settings_error
            .iter()
            .map(|error| Warning {
                text: self
                    .tr("Couldn't carry over your settings. Your recordings aren't affected."),
                details: Some(details(Disclosure::Settings, error)),
                button: None,
            })
            .collect();
        let left: Vec<Row> = report
            .skipped
            .iter()
            .enumerate()
            .map(|(index, skipped)| {
                let details = |raw: &str| details(Disclosure::Item(index), raw);
                let (detail, raw) = match (&skipped.reason, skipped.kind) {
                    (SkipReason::InUse, _) => (
                        self.tr("In use by another program. Close it and try again."),
                        None,
                    ),
                    (SkipReason::Failed(error), ProjectKind::Recording) => (
                        self.tr("Couldn't move this recording. It's still where it was."),
                        Some(details(error)),
                    ),
                    (SkipReason::Failed(error), ProjectKind::Screenshot) => (
                        self.tr("Couldn't move this screenshot. It's still where it was."),
                        Some(details(error)),
                    ),
                };
                Row {
                    icon: Some(Icon::Alert),
                    title: skipped
                        .source
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| skipped.source.display().to_string()),
                    detail,
                    details: raw,
                    warning: true,
                    ..Row::default()
                }
            })
            .collect();
        let show_folder = Button {
            icon: Some(Icon::FolderOpen),
            ..self.button("Show folder", Action::ShowFolder, Style::Ghost)
        };
        let close = self.button("Close", Action::Close, Style::Secondary);
        let (title, subtitle, primary) = if report.stopped && report.left > 0 {
            (
                self.tr("Conversion stopped"),
                self.tr_fmt(
                    "{converted} of {total} items are now in FrameEver. The rest are where they were.",
                    &[("converted", &converted), ("total", &total)],
                ),
                self.button("Convert the rest", Action::ConvertRest, Style::Primary),
            )
        } else if !left.is_empty() {
            (
                self.tr("Some items were left behind"),
                self.tr_fmt(
                    "{converted} of {total} items are now in FrameEver.",
                    &[("converted", &converted), ("total", &total)],
                ),
                self.button("Try again", Action::TryAgain, Style::Primary),
            )
        } else {
            let primary = if self.installed_app().is_some() {
                self.button("Open FrameEver", Action::OpenFrameEver, Style::Primary)
            } else {
                Button {
                    style: Style::Primary,
                    ..show_folder.clone()
                }
            };
            (
                self.tr("Conversion finished"),
                self.tr_fmt("{total} items are now in FrameEver.", &[("total", &total)]),
                primary,
            )
        };
        let all_moved =
            primary.action == Action::OpenFrameEver || primary.action == Action::ShowFolder;
        let start = if primary.action == Action::ShowFolder {
            Vec::new()
        } else {
            vec![show_folder]
        };
        let mut end = Vec::new();
        if self.remaining_install() {
            end.push(self.button("Convert another", Action::ConvertAnother, Style::Secondary));
        }
        end.extend([close, primary]);
        Screen {
            title,
            subtitle,
            tick: all_moved && warnings.is_empty(),
            groups: if left.is_empty() {
                Vec::new()
            } else {
                vec![left]
            },
            warnings,
            start,
            end,
            ..Screen::default()
        }
    }
}

fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

fn nearest_existing(path: &Path) -> PathBuf {
    path.ancestors()
        .find(|ancestor| ancestor.exists())
        .unwrap_or(path)
        .to_path_buf()
}

fn percent(done: usize, total: usize) -> u8 {
    if total == 0 {
        return 0;
    }
    ((done * 100) / total).min(100) as u8
}

pub fn size_text(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.0} MB", value / MB)
    } else if bytes == 0 {
        "0 KB".to_string()
    } else {
        format!("{:.0} KB", (value / KB).max(1.))
    }
}
