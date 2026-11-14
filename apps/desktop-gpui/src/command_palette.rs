use std::path::PathBuf;

use crate::i18n::{Language, n, translate};
use crate::settings_window::catalog::{self, Row};

pub mod view;

pub const RECENT_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Command,
    Project,
    Setting,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Command, Scope::Project, Scope::Setting];

    pub fn prefix(self) -> &'static str {
        match self {
            Scope::Command => ">",
            Scope::Project => "@",
            Scope::Setting => "$",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::Command => n!("Commands"),
            Scope::Project => n!("Projects"),
            Scope::Setting => n!("Settings"),
        }
    }

    fn from_prefix(typed: &str) -> Option<(Scope, &str)> {
        Scope::ALL.into_iter().find_map(|scope| {
            typed
                .strip_prefix(scope.prefix())
                .map(|rest| (scope, rest.trim_start()))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Recorder,
    Library,
    Editor,
    ScreenshotEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    StartRecording,
    TakeScreenshot,
    ShowLibrary,
    BackToRecording,
    ExportSelection,
    GridView,
    ListView,
    RefreshLibrary,
    PlayPause,
    ExportProject,
    CopyScreenshot,
    SaveScreenshot,
    Undo,
    Redo,
    OpenStorageFolder,
    OpenSettings,
    StopRecording,
    PauseRecording,
    RestartRecording,
}

impl Command {
    pub const ALL: [Command; 19] = [
        Command::StartRecording,
        Command::TakeScreenshot,
        Command::ShowLibrary,
        Command::BackToRecording,
        Command::ExportSelection,
        Command::GridView,
        Command::ListView,
        Command::RefreshLibrary,
        Command::PlayPause,
        Command::ExportProject,
        Command::CopyScreenshot,
        Command::SaveScreenshot,
        Command::Undo,
        Command::Redo,
        Command::OpenStorageFolder,
        Command::OpenSettings,
        Command::StopRecording,
        Command::PauseRecording,
        Command::RestartRecording,
    ];

    pub const WHILE_RECORDING: [Command; 3] = [
        Command::StopRecording,
        Command::PauseRecording,
        Command::RestartRecording,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Command::StartRecording => n!("Start recording"),
            Command::TakeScreenshot => n!("Take screenshot"),
            Command::ShowLibrary => n!("Show library"),
            Command::BackToRecording => n!("Back to recording"),
            Command::ExportSelection | Command::ExportProject => n!("Export…"),
            Command::GridView => n!("Grid view"),
            Command::ListView => n!("List view"),
            Command::RefreshLibrary => n!("Refresh library"),
            Command::PlayPause => n!("Play or pause"),
            Command::CopyScreenshot => n!("Copy to clipboard"),
            Command::SaveScreenshot => n!("Save"),
            Command::Undo => n!("Undo"),
            Command::Redo => n!("Redo"),
            Command::OpenStorageFolder => n!("Open storage folder"),
            Command::OpenSettings => n!("Open settings"),
            Command::StopRecording => n!("Stop recording"),
            Command::PauseRecording => n!("Pause"),
            Command::RestartRecording => n!("Restart"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Command::StartRecording => "icons/circle.svg",
            Command::TakeScreenshot => "icons/image.svg",
            Command::ShowLibrary => "icons/chevron-right.svg",
            Command::BackToRecording => "icons/chevron-left.svg",
            Command::ExportSelection | Command::ExportProject => "icons/download.svg",
            Command::GridView => "icons/layout-grid.svg",
            Command::ListView => "icons/list.svg",
            Command::RefreshLibrary => "icons/refresh-cw.svg",
            Command::PlayPause => "icons/play.svg",
            Command::CopyScreenshot => "icons/copy.svg",
            Command::SaveScreenshot => "icons/save.svg",
            Command::Undo => "icons/undo-2.svg",
            Command::Redo => "icons/redo-2.svg",
            Command::OpenStorageFolder => "icons/folder-open.svg",
            Command::OpenSettings => "icons/settings.svg",
            Command::StopRecording => "icons/square.svg",
            Command::PauseRecording => "icons/pause.svg",
            Command::RestartRecording => "icons/rotate-ccw.svg",
        }
    }

    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Command::ShowLibrary | Command::BackToRecording => &["Ctrl", "L"],
            Command::GridView => &["Ctrl", "1"],
            Command::ListView => &["Ctrl", "2"],
            Command::PlayPause => &["Space"],
            Command::Undo => &["Ctrl", "Z"],
            Command::Redo => &["Ctrl", "Y"],
            _ => &[],
        }
    }

    pub fn id(self) -> String {
        format!("{self:?}")
    }

    fn offered_on(self, surface: Surface) -> bool {
        use Surface::{Editor, Library, Recorder, ScreenshotEditor};
        match self {
            Command::StartRecording | Command::TakeScreenshot => {
                matches!(surface, Recorder | Library)
            }
            Command::ShowLibrary => surface != Library,
            Command::BackToRecording
            | Command::ExportSelection
            | Command::GridView
            | Command::ListView
            | Command::RefreshLibrary => surface == Library,
            Command::PlayPause | Command::ExportProject => surface == Editor,
            Command::CopyScreenshot | Command::SaveScreenshot => surface == ScreenshotEditor,
            Command::Undo | Command::Redo => matches!(surface, Editor | ScreenshotEditor),
            Command::OpenStorageFolder | Command::OpenSettings => true,
            Command::StopRecording | Command::PauseRecording | Command::RestartRecording => false,
        }
    }

    pub fn offered(surface: Surface, recording: bool) -> Vec<Command> {
        if recording {
            return Command::WHILE_RECORDING.to_vec();
        }
        Command::ALL
            .into_iter()
            .filter(|command| command.offered_on(surface))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Command(Command),
    Project(PathBuf),
    Setting(Row),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub target: Target,
    pub name: String,
    pub other_names: Vec<String>,
    pub detail: Option<String>,
    pub icon: &'static str,
    pub keys: &'static [&'static str],
}

impl Entry {
    pub fn command(command: Command, language: Language) -> Self {
        let (name, other_names) = names(command.label(), language);
        Self {
            target: Target::Command(command),
            name,
            other_names,
            detail: None,
            icon: command.icon(),
            keys: command.keys(),
        }
    }

    pub fn setting(place: catalog::Place, language: Language) -> Self {
        let (name, other_names) = names(place.row.label(), language);
        Self {
            target: Target::Setting(place.row),
            name,
            other_names,
            detail: Some(translate(place.page.label(), language).to_string()),
            icon: place.page.icon(),
            keys: &[],
        }
    }

    pub fn project(path: PathBuf, name: String, detail: String, icon: &'static str) -> Self {
        Self {
            target: Target::Project(path),
            name,
            other_names: Vec::new(),
            detail: Some(detail),
            icon,
            keys: &[],
        }
    }

    fn score(&self, query: &str) -> Option<u32> {
        std::iter::once(&self.name)
            .chain(&self.other_names)
            .filter_map(|name| fuzzy_score(query, name))
            .max()
    }
}

fn names(english: &'static str, language: Language) -> (String, Vec<String>) {
    let name = translate(english, language);
    let other_names = Language::ALL
        .into_iter()
        .map(|other| translate(english, other))
        .filter(|other| *other != name)
        .map(str::to_string)
        .collect();
    (name.to_string(), other_names)
}

pub fn fuzzy_score(query: &str, text: &str) -> Option<u32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let text: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0u32;
    let mut wanted = 0;
    let mut previous_hit: Option<usize> = None;
    for (at, c) in text.iter().enumerate() {
        if wanted == query.len() {
            break;
        }
        if *c != query[wanted] {
            continue;
        }
        let word_start = at == 0 || !text[at - 1].is_alphanumeric();
        let run = at > 0 && previous_hit == Some(at - 1);
        score += 1;
        if run {
            score += 8;
        }
        if word_start {
            score += 6;
        }
        if at == 0 {
            score += 4;
        }
        previous_hit = Some(at);
        wanted += 1;
    }
    (wanted == query.len()).then_some(score)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recording {
    Off,
    Running,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Recents(Vec<String>);

impl Recents {
    pub fn from_ids(ids: Vec<String>) -> Self {
        let mut recents = Self::default();
        for id in ids.into_iter().rev() {
            recents.note_id(id);
        }
        recents
    }

    pub fn ids(&self) -> &[String] {
        &self.0
    }

    pub fn note(&mut self, command: Command) {
        if Command::WHILE_RECORDING.contains(&command) {
            return;
        }
        self.note_id(command.id());
    }

    fn note_id(&mut self, id: String) {
        self.0.retain(|known| *known != id);
        self.0.insert(0, id);
        self.0.truncate(RECENT_LIMIT);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub commands: Vec<Entry>,
    pub projects: Vec<Entry>,
    pub settings: Vec<Entry>,
    pub recents: Recents,
    pub recording: bool,
}

impl Source {
    pub fn new(
        surface: Surface,
        recording: Recording,
        language: Language,
        projects: Vec<Entry>,
        recents: Recents,
    ) -> Self {
        let paused = recording == Recording::Paused;
        let recording = recording != Recording::Off;
        let commands = Command::offered(surface, recording)
            .into_iter()
            .map(|command| {
                let mut entry = Entry::command(command, language);
                if paused && command == Command::PauseRecording {
                    (entry.name, entry.other_names) = names(n!("Resume"), language);
                    entry.icon = "icons/play.svg";
                }
                entry
            })
            .collect();
        if recording {
            return Self {
                commands,
                projects: Vec::new(),
                settings: Vec::new(),
                recents: Recents::default(),
                recording,
            };
        }
        Self {
            commands,
            projects,
            settings: catalog::places()
                .map(|place| Entry::setting(place, language))
                .collect(),
            recents,
            recording,
        }
    }

    fn of(&self, scope: Scope) -> &[Entry] {
        match scope {
            Scope::Command => &self.commands,
            Scope::Project => &self.projects,
            Scope::Setting => &self.settings,
        }
    }

    fn recent_commands(&self) -> (Heading, Vec<&Entry>) {
        let recent: Vec<&Entry> = self
            .recents
            .ids()
            .iter()
            .filter_map(|id| {
                self.commands.iter().find(
                    |entry| matches!(&entry.target, Target::Command(command) if command.id() == *id),
                )
            })
            .collect();
        if recent.is_empty() {
            (
                Heading::Scope(Scope::Command),
                self.commands.iter().take(RECENT_LIMIT).collect(),
            )
        } else {
            (Heading::RecentCommands, recent)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    Recording,
    RecentCommands,
    RecentProjects,
    Scope(Scope),
}

impl Heading {
    pub fn label(self) -> &'static str {
        match self {
            Heading::Recording => n!("Recording"),
            Heading::RecentCommands => n!("Recent commands"),
            Heading::RecentProjects => n!("Recent projects"),
            Heading::Scope(scope) => scope.label(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group<'a> {
    pub heading: Heading,
    pub entries: Vec<&'a Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Run(Command),
    OpenProject(PathBuf),
    ShowInLibrary(PathBuf),
    OpenSetting(Row),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Palette {
    query: String,
    scope: Option<Scope>,
    index: usize,
}

impl Palette {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn scope(&self) -> Option<Scope> {
        self.scope
    }

    pub fn typed(&mut self, text: &str, recording: bool) -> bool {
        self.index = 0;
        if self.scope.is_none()
            && !recording
            && let Some((scope, rest)) = Scope::from_prefix(text)
        {
            self.scope = Some(scope);
            self.query = rest.to_string();
            return true;
        }
        self.query = text.to_string();
        false
    }

    pub fn backspace_on_empty(&mut self) -> bool {
        if self.query.is_empty() && self.scope.is_some() {
            self.scope = None;
            self.index = 0;
            return true;
        }
        false
    }

    pub fn shows_prefix_hints(&self, source: &Source) -> bool {
        self.query.trim().is_empty() && self.scope.is_none() && !source.recording
    }

    pub fn groups<'a>(&self, source: &'a Source) -> Vec<Group<'a>> {
        let query = self.query.trim();
        if source.recording {
            return non_empty(vec![Group {
                heading: Heading::Recording,
                entries: ranked(&source.commands, query),
            }]);
        }
        if query.is_empty() && self.scope.is_none() {
            let (heading, entries) = source.recent_commands();
            return non_empty(vec![
                Group { heading, entries },
                Group {
                    heading: Heading::RecentProjects,
                    entries: source.projects.iter().take(RECENT_LIMIT).collect(),
                },
            ]);
        }
        non_empty(
            Scope::ALL
                .into_iter()
                .filter(|scope| self.scope.is_none_or(|only| only == *scope))
                .map(|scope| Group {
                    heading: Heading::Scope(scope),
                    entries: ranked(source.of(scope), query),
                })
                .collect(),
        )
    }

    pub fn count(&self, source: &Source) -> usize {
        self.groups(source)
            .iter()
            .map(|group| group.entries.len())
            .sum()
    }

    pub fn index(&self, source: &Source) -> usize {
        self.index.min(self.count(source).saturating_sub(1))
    }

    pub fn select(&mut self, index: usize) {
        self.index = index;
    }

    pub fn step(&mut self, forward: bool, source: &Source) {
        let count = self.count(source);
        if count == 0 {
            self.index = 0;
            return;
        }
        let index = self.index(source);
        self.index = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
    }

    pub fn chosen<'a>(&self, source: &'a Source) -> Option<&'a Entry> {
        self.groups(source)
            .into_iter()
            .flat_map(|group| group.entries)
            .nth(self.index(source))
    }

    pub fn confirm(&self, source: &Source, shift: bool) -> Option<Outcome> {
        Some(match &self.chosen(source)?.target {
            Target::Command(command) => Outcome::Run(*command),
            Target::Project(path) if shift => Outcome::ShowInLibrary(path.clone()),
            Target::Project(path) => Outcome::OpenProject(path.clone()),
            Target::Setting(row) => Outcome::OpenSetting(*row),
        })
    }
}

fn non_empty(groups: Vec<Group<'_>>) -> Vec<Group<'_>> {
    groups
        .into_iter()
        .filter(|group| !group.entries.is_empty())
        .collect()
}

fn ranked<'a>(entries: &'a [Entry], query: &str) -> Vec<&'a Entry> {
    let mut hits: Vec<(u32, &Entry)> = entries
        .iter()
        .filter_map(|entry| entry.score(query).map(|score| (score, entry)))
        .collect();
    hits.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    hits.into_iter().map(|(_, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> Entry {
        Entry::project(
            PathBuf::from(format!("C:/storage/{name}.ever")),
            name.to_string(),
            "Studio".to_string(),
            "icons/clapperboard.svg",
        )
    }

    fn source(surface: Surface, language: Language) -> Source {
        Source::new(
            surface,
            Recording::Off,
            language,
            [
                "Product demo",
                "产品演示",
                "Weekly sync",
                "a",
                "b",
                "c",
                "d",
            ]
            .into_iter()
            .map(project)
            .collect(),
            Recents::default(),
        )
    }

    fn names_in(palette: &Palette, source: &Source) -> Vec<String> {
        palette
            .groups(source)
            .into_iter()
            .flat_map(|group| group.entries)
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn headings(palette: &Palette, source: &Source) -> Vec<Heading> {
        palette
            .groups(source)
            .iter()
            .map(|group| group.heading)
            .collect()
    }

    fn typed(text: &str) -> Palette {
        let mut palette = Palette::default();
        palette.typed(text, false);
        palette
    }

    #[test]
    fn letters_match_in_order_without_being_adjacent() {
        assert!(fuzzy_score("opst", "Open settings").is_some());
        assert!(fuzzy_score("OPEN SET", "open settings").is_some());
        assert_eq!(fuzzy_score("tso", "Open settings"), None);
        assert_eq!(fuzzy_score("zzz", "Open settings"), None);
    }

    #[test]
    fn a_run_at_a_word_start_outranks_scattered_letters() {
        let whole = fuzzy_score("exp", "Export…").unwrap();
        let scattered = fuzzy_score("exp", "Take screenshot now, please").unwrap_or(0);
        let inner = fuzzy_score("exp", "Reexport").unwrap();
        assert!(whole > inner);
        assert!(inner > scattered);
    }

    #[test]
    fn chinese_names_match_by_their_characters() {
        assert!(fuzzy_score("导出", "导出…").is_some());
        assert!(fuzzy_score("资库", "显示资源库").is_some());
        assert_eq!(fuzzy_score("库资", "显示资源库"), None);
    }

    #[test]
    fn the_chinese_ui_finds_a_command_by_either_name() {
        let source = source(Surface::Library, Language::SimplifiedChinese);
        assert_eq!(
            names_in(&typed(">刷新"), &source),
            [translate("Refresh library", Language::SimplifiedChinese)]
        );
        assert_eq!(
            names_in(&typed(">refresh"), &source),
            [translate("Refresh library", Language::SimplifiedChinese)]
        );
    }

    #[test]
    fn pinyin_finds_nothing() {
        let source = source(Surface::Library, Language::SimplifiedChinese);
        assert_eq!(names_in(&typed("chanpinyanshi"), &source), [""; 0]);
        assert_eq!(names_in(&typed(">shuaxin"), &source), [""; 0]);
    }

    #[test]
    fn a_prefix_becomes_the_scope_and_leaves_the_query_empty() {
        for (prefix, scope) in [
            (">", Scope::Command),
            ("@", Scope::Project),
            ("$", Scope::Setting),
        ] {
            let mut palette = Palette::default();
            assert!(palette.typed(prefix, false));
            assert_eq!(palette.scope(), Some(scope));
            assert_eq!(palette.query(), "");
        }
        let mut palette = typed("@");
        assert!(!palette.typed("$", false));
        assert_eq!(palette.scope(), Some(Scope::Project));
        assert_eq!(palette.query(), "$");
    }

    #[test]
    fn backspace_on_an_empty_query_removes_the_scope() {
        let mut palette = typed("@demo");
        assert!(!palette.backspace_on_empty());
        palette.typed("", false);
        assert!(palette.backspace_on_empty());
        assert_eq!(palette.scope(), None);
        assert!(!palette.backspace_on_empty());
    }

    #[test]
    fn a_scope_lists_only_its_own_kind() {
        let source = source(Surface::Library, Language::English);
        assert_eq!(
            headings(&typed("@"), &source),
            [Heading::Scope(Scope::Project)]
        );
        assert_eq!(names_in(&typed("@"), &source).len(), 7);
        assert_eq!(
            headings(&typed("$the"), &source),
            [Heading::Scope(Scope::Setting)]
        );
        assert!(names_in(&typed("$the"), &source).contains(&"Theme".to_string()));
    }

    #[test]
    fn a_plain_query_searches_commands_projects_and_settings() {
        let source = source(Surface::Library, Language::English);
        let palette = typed("o");
        assert_eq!(
            headings(&palette, &source),
            [
                Heading::Scope(Scope::Command),
                Heading::Scope(Scope::Project),
                Heading::Scope(Scope::Setting)
            ]
        );
        assert_eq!(headings(&typed("zzzz"), &source), []);
    }

    #[test]
    fn commands_follow_the_window_they_are_opened_in() {
        let has = |surface, command| Command::offered(surface, false).contains(&command);
        assert!(has(Surface::Recorder, Command::ShowLibrary));
        assert!(!has(Surface::Recorder, Command::GridView));
        assert!(has(Surface::Library, Command::GridView));
        assert!(has(Surface::Library, Command::BackToRecording));
        assert!(!has(Surface::Library, Command::ShowLibrary));
        assert!(has(Surface::Editor, Command::ExportProject));
        assert!(!has(Surface::Editor, Command::StartRecording));
        assert!(has(Surface::ScreenshotEditor, Command::CopyScreenshot));
        assert!(!has(Surface::ScreenshotEditor, Command::PlayPause));
        for surface in [
            Surface::Recorder,
            Surface::Library,
            Surface::Editor,
            Surface::ScreenshotEditor,
        ] {
            assert!(has(surface, Command::OpenSettings));
            assert!(!has(surface, Command::StopRecording));
        }
    }

    #[test]
    fn while_recording_it_offers_only_stop_pause_and_restart() {
        for surface in [Surface::Recorder, Surface::Library, Surface::Editor] {
            let source = Source::new(
                surface,
                Recording::Running,
                Language::English,
                vec![project("Product demo")],
                Recents::from_ids(vec![Command::OpenSettings.id()]),
            );
            let expected = ["Stop recording", "Pause", "Restart"];
            assert_eq!(names_in(&Palette::default(), &source), expected);
            assert_eq!(headings(&Palette::default(), &source), [Heading::Recording]);
            let recording = |text: &str| {
                let mut palette = Palette::default();
                assert!(!palette.typed(text, true));
                palette
            };
            assert_eq!(names_in(&recording("@"), &source), [""; 0]);
            assert_eq!(names_in(&recording("$"), &source), [""; 0]);
            assert_eq!(names_in(&recording("product"), &source), [""; 0]);
            assert_eq!(names_in(&recording("stop"), &source), ["Stop recording"]);
            assert!(!Palette::default().shows_prefix_hints(&source));
        }
    }

    #[test]
    fn an_empty_palette_shows_up_to_five_recent_commands_and_projects() {
        let mut source = source(Surface::Library, Language::English);
        for command in [
            Command::GridView,
            Command::ListView,
            Command::OpenSettings,
            Command::RefreshLibrary,
            Command::StartRecording,
            Command::TakeScreenshot,
            Command::ListView,
        ] {
            source.recents.note(command);
        }
        let palette = Palette::default();
        let groups = palette.groups(&source);
        assert_eq!(
            headings(&palette, &source),
            [Heading::RecentCommands, Heading::RecentProjects]
        );
        let commands: Vec<_> = groups[0].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            commands,
            [
                "List view",
                "Take screenshot",
                "Start recording",
                "Refresh library",
                "Open settings"
            ]
        );
        let projects: Vec<_> = groups[1].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            projects,
            ["Product demo", "产品演示", "Weekly sync", "a", "b"]
        );
        assert!(palette.shows_prefix_hints(&source));
    }

    #[test]
    fn while_paused_the_pause_row_reads_resume() {
        let source = Source::new(
            Surface::Recorder,
            Recording::Paused,
            Language::English,
            Vec::new(),
            Recents::default(),
        );
        let expected = ["Stop recording", "Resume", "Restart"];
        assert_eq!(names_in(&Palette::default(), &source), expected);
        let mut palette = Palette::default();
        palette.typed("resume", true);
        assert_eq!(
            palette.confirm(&source, false),
            Some(Outcome::Run(Command::PauseRecording))
        );
        let chinese = Source::new(
            Surface::Recorder,
            Recording::Paused,
            Language::SimplifiedChinese,
            Vec::new(),
            Recents::default(),
        );
        assert!(names_in(&Palette::default(), &chinese).contains(&"继续".to_string()));
    }

    #[test]
    fn recording_commands_are_not_remembered_as_recent() {
        let mut recents = Recents::default();
        recents.note(Command::OpenSettings);
        for command in Command::WHILE_RECORDING {
            recents.note(command);
        }
        assert_eq!(recents.ids(), [Command::OpenSettings.id()]);
    }

    #[test]
    fn a_recent_command_from_another_window_is_left_out() {
        let mut source = source(Surface::Recorder, Language::English);
        source.recents.note(Command::GridView);
        source.recents.note(Command::OpenSettings);
        let palette = Palette::default();
        let groups = palette.groups(&source);
        let commands: Vec<_> = groups[0].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(commands, ["Open settings"]);
    }

    #[test]
    fn with_no_recent_commands_the_first_commands_stand_in() {
        let source = source(Surface::Recorder, Language::English);
        let palette = Palette::default();
        let groups = palette.groups(&source);
        assert_eq!(groups[0].heading, Heading::Scope(Scope::Command));
        assert_eq!(groups[0].entries[0].name, "Start recording");
        assert!(groups[0].entries.len() <= RECENT_LIMIT);
    }

    #[test]
    fn recents_survive_a_round_trip_in_order() {
        let mut recents = Recents::default();
        recents.note(Command::GridView);
        recents.note(Command::OpenSettings);
        assert_eq!(Recents::from_ids(recents.ids().to_vec()), recents);
    }

    #[test]
    fn arrows_wrap_around_the_results() {
        let source = source(Surface::Library, Language::English);
        let mut palette = typed("@");
        palette.step(false, &source);
        assert_eq!(palette.index(&source), 6);
        palette.step(true, &source);
        assert_eq!(palette.index(&source), 0);
        palette.step(true, &source);
        assert_eq!(palette.chosen(&source).unwrap().name, "产品演示");
        palette.typed("zzzz", false);
        palette.step(true, &source);
        assert_eq!(palette.chosen(&source), None);
    }

    #[test]
    fn enter_acts_and_shift_enter_shows_a_project_in_the_library() {
        let source = source(Surface::Library, Language::English);
        let path = PathBuf::from("C:/storage/Weekly sync.ever");
        let palette = typed("@weekly");
        assert_eq!(
            palette.confirm(&source, false),
            Some(Outcome::OpenProject(path.clone()))
        );
        assert_eq!(
            palette.confirm(&source, true),
            Some(Outcome::ShowInLibrary(path))
        );
        let palette = typed(">grid");
        for shift in [false, true] {
            assert_eq!(
                palette.confirm(&source, shift),
                Some(Outcome::Run(Command::GridView))
            );
        }
        assert_eq!(
            typed("$theme").confirm(&source, false),
            Some(Outcome::OpenSetting(Row::Theme))
        );
        assert_eq!(typed("zzzz").confirm(&source, false), None);
    }

    #[test]
    fn every_command_icon_is_bundled() {
        use gpui::AssetSource;
        for command in Command::ALL {
            assert!(
                crate::assets::Assets
                    .load(command.icon())
                    .unwrap()
                    .is_some(),
                "{command:?}"
            );
        }
    }
}
