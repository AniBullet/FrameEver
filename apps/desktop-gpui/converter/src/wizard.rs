use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use frameever_converter::{
    Item, OldInstall, Report, SkipReason, Source, Target, convert, destination,
};
use frameever_utils::storage::ProjectKind;

use crate::i18n::{self, Language};

pub type FindInstalls = Box<dyn Fn() -> Vec<OldInstall> + Send>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub recordings: usize,
    pub screenshots: usize,
}

impl Counts {
    fn of(items: &[Item]) -> Self {
        let mut counts = Self::default();
        for item in items {
            match item.kind() {
                ProjectKind::Recording => counts.recordings += 1,
                ProjectKind::Screenshot => counts.screenshots += 1,
            }
        }
        counts
    }

    pub fn total(self) -> usize {
        self.recordings + self.screenshots
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub install: OldInstall,
    pub counts: Counts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub install: OldInstall,
    pub counts: Counts,
    pub destination: PathBuf,
}

impl Preview {
    fn can_convert(&self) -> bool {
        self.counts.total() > 0 || self.install.source.carries_settings()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Find(Vec<Found>),
    Preview(Preview),
    Converting(Preview),
    Done(Preview, Report),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Choose(usize),
    PickFolder,
    Back,
    Convert,
    OpenDestination,
    StartOver,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub title: String,
    pub detail: String,
    pub action: Option<Action>,
    pub warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub primary: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screen {
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<Row>,
    pub notes: Vec<String>,
    pub buttons: Vec<Button>,
}

pub struct Job {
    install: OldInstall,
    target: Target,
}

impl Job {
    pub fn run(self) -> Report {
        convert(&self.install, &self.target)
    }
}

pub struct Wizard {
    target: Target,
    find_installs: FindInstalls,
    language: Language,
    step: Step,
}

impl Wizard {
    pub fn new(target: Target, find_installs: FindInstalls) -> Self {
        let step = Step::Find(found(&find_installs));
        Self {
            target,
            find_installs,
            language: i18n::current(),
            step,
        }
    }

    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    pub fn step(&self) -> &Step {
        &self.step
    }

    pub fn is_converting(&self) -> bool {
        matches!(self.step, Step::Converting(_))
    }

    pub fn destination(&self) -> Option<&Path> {
        match &self.step {
            Step::Find(_) => None,
            Step::Preview(preview) | Step::Converting(preview) | Step::Done(preview, _) => {
                Some(&preview.destination)
            }
        }
    }

    pub fn choose(&mut self, index: usize) {
        if let Step::Find(found) = &self.step
            && let Some(found) = found.get(index)
        {
            let install = found.install.clone();
            self.preview(install);
        }
    }

    pub fn choose_folder(&mut self, folder: &Path) {
        if matches!(self.step, Step::Find(_)) {
            self.preview(OldInstall::picked_folder(folder));
        }
    }

    fn preview(&mut self, install: OldInstall) {
        self.step = Step::Preview(Preview {
            counts: Counts::of(&install.items()),
            destination: destination(&install, &self.target),
            install,
        });
    }

    pub fn back(&mut self) {
        if !matches!(self.step, Step::Converting(_)) {
            self.step = Step::Find(found(&self.find_installs));
        }
    }

    pub fn convert(&mut self) -> Option<Job> {
        let Step::Preview(preview) = &self.step else {
            return None;
        };
        if !preview.can_convert() {
            return None;
        }
        let job = Job {
            install: preview.install.clone(),
            target: self.target.clone(),
        };
        self.step = Step::Converting(preview.clone());
        Some(job)
    }

    pub fn finish(&mut self, report: Report) {
        if let Step::Converting(preview) = &self.step {
            self.step = Step::Done(preview.clone(), report);
        }
    }

    fn tr(&self, english: &'static str) -> String {
        i18n::translate(english, self.language).to_string()
    }

    fn tr_fmt(&self, template: &'static str, args: &[(&str, &dyn Display)]) -> String {
        let mut out = i18n::translate(template, self.language).to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), &value.to_string());
        }
        out
    }

    fn button(&self, label: &'static str, action: Action, primary: bool) -> Button {
        Button {
            label: self.tr(label),
            action,
            primary,
        }
    }

    fn source_name(&self, install: &OldInstall) -> String {
        match install.source {
            Source::CapAniBullet2 => "Cap-aniBullet 2.x".to_string(),
            Source::Cap => "Cap".to_string(),
            Source::Folder => install.app_data.display().to_string(),
        }
    }

    fn counts_text(&self, counts: Counts) -> String {
        self.tr_fmt(
            "Recordings {recordings} · Screenshots {screenshots}",
            &[
                ("recordings", &counts.recordings),
                ("screenshots", &counts.screenshots),
            ],
        )
    }

    fn row(&self, title: String, detail: String) -> Row {
        Row {
            title,
            detail,
            action: None,
            warning: false,
        }
    }

    pub fn screen(&self) -> Screen {
        match &self.step {
            Step::Find(found) => self.find_screen(found),
            Step::Preview(preview) => self.preview_screen(preview),
            Step::Converting(preview) => Screen {
                title: self.tr("Converting…"),
                subtitle: self.tr_fmt(
                    "Moving {count} items. Keep this window open.",
                    &[("count", &preview.counts.total())],
                ),
                ..Screen::default()
            },
            Step::Done(_, report) => self.done_screen(report),
        }
    }

    fn find_screen(&self, found: &[Found]) -> Screen {
        Screen {
            title: self.tr("Bring your old recordings into FrameEver"),
            subtitle: if found.is_empty() {
                self.tr("No Cap or Cap-aniBullet 2.x data was found on this computer. Choose the folder that holds your old recordings.")
            } else {
                self.tr("Found on this computer. Choose one to see what will be converted.")
            },
            rows: found
                .iter()
                .enumerate()
                .map(|(index, found)| Row {
                    action: Some(Action::Choose(index)),
                    ..self.row(
                        self.source_name(&found.install),
                        self.counts_text(found.counts),
                    )
                })
                .collect(),
            notes: Vec::new(),
            buttons: vec![
                self.button("Choose a folder…", Action::PickFolder, found.is_empty()),
                self.button("Close", Action::Close, false),
            ],
        }
    }

    fn preview_screen(&self, preview: &Preview) -> Screen {
        let can_convert = preview.can_convert();
        let mut buttons = vec![self.button("Back", Action::Back, false)];
        if can_convert {
            buttons.push(self.button("Convert", Action::Convert, true));
        }
        Screen {
            title: if can_convert {
                self.tr("Ready to convert")
            } else {
                self.tr("Nothing to convert here")
            },
            subtitle: self.tr_fmt(
                "From {source}",
                &[("source", &self.source_name(&preview.install))],
            ),
            rows: vec![
                self.row(self.tr("Recordings"), preview.counts.recordings.to_string()),
                self.row(
                    self.tr("Screenshots"),
                    preview.counts.screenshots.to_string(),
                ),
                self.row(
                    self.tr("Settings"),
                    if preview.install.source.carries_settings() {
                        self.tr("Carried over where FrameEver has none yet")
                    } else {
                        self.tr("Left unchanged")
                    },
                ),
                self.row(
                    self.tr("Destination"),
                    preview.destination.display().to_string(),
                ),
            ],
            notes: if can_convert {
                vec![self.tr("Files are moved, not copied. Afterwards only FrameEver opens them.")]
            } else {
                Vec::new()
            },
            buttons,
        }
    }

    fn done_screen(&self, report: &Report) -> Screen {
        let converted = report.converted.len();
        let total = converted + report.skipped.len();
        Screen {
            title: if report.skipped.is_empty() {
                self.tr("Conversion finished")
            } else {
                self.tr("Some items were left behind")
            },
            subtitle: self.tr_fmt(
                "{converted} of {total} items are now in FrameEver.",
                &[("converted", &converted), ("total", &total)],
            ),
            rows: report
                .skipped
                .iter()
                .map(|skipped| Row {
                    warning: true,
                    ..self.row(
                        skipped
                            .source
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| skipped.source.display().to_string()),
                        match &skipped.reason {
                            SkipReason::InUse => self.tr(
                                "In use by another program. Close it and run the converter again.",
                            ),
                            SkipReason::Failed(error) => {
                                self.tr_fmt("Could not convert: {error}", &[("error", error)])
                            }
                        },
                    )
                })
                .collect(),
            notes: report
                .settings_error
                .iter()
                .map(|error| {
                    self.tr_fmt(
                        "Settings were not carried over: {error}",
                        &[("error", error)],
                    )
                })
                .collect(),
            buttons: vec![
                self.button("Open storage folder", Action::OpenDestination, false),
                self.button("Convert more", Action::StartOver, false),
                self.button("Close", Action::Close, true),
            ],
        }
    }
}

fn found(find_installs: &FindInstalls) -> Vec<Found> {
    find_installs()
        .into_iter()
        .map(|install| Found {
            counts: Counts::of(&install.items()),
            install,
        })
        .collect()
}
