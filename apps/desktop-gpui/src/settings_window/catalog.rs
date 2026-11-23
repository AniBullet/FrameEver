use crate::i18n::{Language, n, translate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    General,
    Recording,
    Studio,
    Instant,
    Editing,
    Storage,
    Shortcuts,
    Automations,
    Advanced,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Row {
    Theme,
    Language,
    Notifications,
    RecordingSounds,
    ChapterMarkSound,
    KeepDrawings,
    DockIcon,
    MainWindowStart,
    AfterDelete,
    AfterEditorClose,
    AfterCancel,
    StudioQuality,
    InstantResolution,
    MaxFps,
    Countdown,
    ConfirmMicrophone,
    AutoStop,
    SeparateCursor,
    RecordInput,
    CrashRecovery,
    ClickHighlight,
    ShowInput,
    InputDevice,
    InputPreset,
    InputGamepadPreset,
    InputSize,
    InputOpacity,
    InputPosition,
    WindowStyle,
    FocusCenter,
    NotchOverlay,
    ExcludedWindows,
    AfterScreenshot,
    AfterStudio,
    AfterInstant,
    StudioSound,
    VoiceIsolation,
    AutoZoom,
    ZoomAmount,
    RememberedTerms,
    InputPresets,
    StorageFolder,
    NameTemplate,
    CompressInstant,
    KeepCompressed,
    Shortcut(&'static str),
    Rules,
    Cli,
    Muxer,
    Diagnostic,
    Logs,
    Version,
    UpdateDownloads,
    UpdateChannel,
    ReportProblem,
    Changelog,
    Licences,
}

pub struct Group {
    pub title: &'static str,
    pub rows: &'static [Row],
}

pub const SCREENSHOT_SHORTCUTS: [&str; 3] =
    ["screenshotArea", "screenshotWindow", "screenshotDisplay"];

pub const RECORDING_SHORTCUTS: [&str; 10] = [
    "openRecordingPickerArea",
    "openRecordingPickerWindow",
    "openRecordingPickerDisplay",
    "startRecording",
    "startStudioRecording",
    "startInstantRecording",
    "openRecordingPicker",
    "cycleRecordingMode",
    "focusCameraPreview",
    "toggleTeleprompterPlayback",
];

pub const WHILE_RECORDING_SHORTCUTS: [&str; 9] = [
    "stopRecording",
    "togglePauseRecording",
    "restartRecording",
    "drawOnScreen",
    "focusRecordingControls",
    "toggleRecordingControls",
    "addChapterMarker",
    "toggleMicrophoneMute",
    "pushToTalk",
];

pub fn all_shortcuts() -> impl Iterator<Item = &'static str> {
    SCREENSHOT_SHORTCUTS
        .into_iter()
        .chain(RECORDING_SHORTCUTS)
        .chain(WHILE_RECORDING_SHORTCUTS)
}

pub fn shortcut_label(action: &str) -> &'static str {
    match action {
        "screenshotArea" => n!("Screenshot an area"),
        "screenshotWindow" => n!("Screenshot a window"),
        "screenshotDisplay" => n!("Screenshot the display"),
        "startRecording" => n!("Start in selected mode"),
        "startStudioRecording" => n!("Start Studio recording"),
        "startInstantRecording" => n!("Start Instant recording"),
        "openRecordingPickerArea" => n!("Record an area"),
        "openRecordingPickerWindow" => n!("Record a window"),
        "openRecordingPickerDisplay" => n!("Record a display"),
        "openRecordingPicker" => n!("Open recording picker"),
        "cycleRecordingMode" => n!("Cycle recording mode"),
        "stopRecording" => n!("Stop recording"),
        "togglePauseRecording" => n!("Pause or resume"),
        "restartRecording" => n!("Restart recording"),
        "focusCameraPreview" => n!("Focus camera preview"),
        "toggleTeleprompterPlayback" => n!("Play or pause the teleprompter"),
        "drawOnScreen" => n!("Draw on screen"),
        "focusRecordingControls" => n!("Focus recording controls"),
        "toggleRecordingControls" => n!("Hide or show controls"),
        "addChapterMarker" => n!("Add chapter mark"),
        "toggleMicrophoneMute" => n!("Mute"),
        "pushToTalk" => n!("Push to talk"),
        _ => n!("Shortcut"),
    }
}

const GENERAL: &[Group] = &[
    Group {
        title: n!("Appearance"),
        rows: &[Row::Theme, Row::Language, Row::DockIcon],
    },
    Group {
        title: n!("Notifications and sounds"),
        rows: &[
            Row::Notifications,
            Row::RecordingSounds,
            Row::ChapterMarkSound,
        ],
    },
    Group {
        title: n!("Recording window"),
        rows: &[
            Row::MainWindowStart,
            Row::AfterEditorClose,
            Row::AfterCancel,
            Row::AfterDelete,
        ],
    },
];

const RECORDING: &[Group] = &[
    Group {
        title: n!("Before you record"),
        rows: &[Row::Countdown, Row::ConfirmMicrophone],
    },
    Group {
        title: n!("While recording"),
        rows: &[
            Row::KeepDrawings,
            Row::MaxFps,
            Row::AutoStop,
            Row::NotchOverlay,
        ],
    },
    Group {
        title: n!("Window recording"),
        rows: &[Row::WindowStyle, Row::FocusCenter],
    },
    Group {
        title: n!("Hidden from recordings"),
        rows: &[Row::ExcludedWindows],
    },
    Group {
        title: n!("Screenshots"),
        rows: &[Row::AfterScreenshot],
    },
];

const STUDIO: &[Group] = &[
    Group {
        title: n!("Recording"),
        rows: &[
            Row::StudioQuality,
            Row::SeparateCursor,
            Row::RecordInput,
            Row::CrashRecovery,
        ],
    },
    Group {
        title: n!("New projects"),
        rows: &[
            Row::StudioSound,
            Row::VoiceIsolation,
            Row::AutoZoom,
            Row::ZoomAmount,
        ],
    },
    Group {
        title: n!("When it's done"),
        rows: &[Row::AfterStudio],
    },
];

const INSTANT: &[Group] = &[
    Group {
        title: n!("Recording"),
        rows: &[Row::InstantResolution],
    },
    Group {
        title: n!("Drawn into the video"),
        rows: &[
            Row::ClickHighlight,
            Row::ShowInput,
            Row::InputDevice,
            Row::InputPreset,
            Row::InputGamepadPreset,
            Row::InputSize,
            Row::InputOpacity,
            Row::InputPosition,
        ],
    },
    Group {
        title: n!("When it's done"),
        rows: &[Row::AfterInstant, Row::CompressInstant, Row::KeepCompressed],
    },
];

const EDITING: &[Group] = &[
    Group {
        title: n!("Captions"),
        rows: &[Row::RememberedTerms],
    },
    Group {
        title: n!("Input display"),
        rows: &[Row::InputPresets],
    },
];

const STORAGE: &[Group] = &[
    Group {
        title: n!("Storage folder"),
        rows: &[Row::StorageFolder],
    },
    Group {
        title: n!("Default project name"),
        rows: &[Row::NameTemplate],
    },
];

const SHORTCUTS: &[Group] = &[
    Group {
        title: n!("Screenshots"),
        rows: &[
            Row::Shortcut(SCREENSHOT_SHORTCUTS[0]),
            Row::Shortcut(SCREENSHOT_SHORTCUTS[1]),
            Row::Shortcut(SCREENSHOT_SHORTCUTS[2]),
        ],
    },
    Group {
        title: n!("Start recording"),
        rows: &[
            Row::Shortcut(RECORDING_SHORTCUTS[0]),
            Row::Shortcut(RECORDING_SHORTCUTS[1]),
            Row::Shortcut(RECORDING_SHORTCUTS[2]),
            Row::Shortcut(RECORDING_SHORTCUTS[3]),
            Row::Shortcut(RECORDING_SHORTCUTS[4]),
            Row::Shortcut(RECORDING_SHORTCUTS[5]),
            Row::Shortcut(RECORDING_SHORTCUTS[6]),
            Row::Shortcut(RECORDING_SHORTCUTS[7]),
            Row::Shortcut(RECORDING_SHORTCUTS[8]),
            Row::Shortcut(RECORDING_SHORTCUTS[9]),
        ],
    },
    Group {
        title: n!("While recording"),
        rows: &[
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[0]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[1]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[2]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[3]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[4]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[5]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[6]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[7]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[8]),
        ],
    },
];

const AUTOMATIONS: &[Group] = &[Group {
    title: n!("Rules"),
    rows: &[Row::Rules],
}];

const ADVANCED: &[Group] = &[
    Group {
        title: n!("Command line"),
        rows: &[Row::Cli],
    },
    Group {
        title: n!("Reliability"),
        rows: &[Row::Muxer],
    },
    Group {
        title: n!("Troubleshooting"),
        rows: &[Row::Diagnostic, Row::Logs],
    },
];

const ABOUT: &[Group] = &[
    Group {
        title: "",
        rows: &[Row::Version],
    },
    Group {
        title: n!("Updates"),
        rows: &[Row::UpdateDownloads, Row::UpdateChannel, Row::ReportProblem],
    },
    Group {
        title: n!("What's new"),
        rows: &[Row::Changelog],
    },
    Group {
        title: n!("Licences"),
        rows: &[Row::Licences],
    },
];

impl Page {
    pub const ALL: [Page; 10] = [
        Page::General,
        Page::Recording,
        Page::Studio,
        Page::Instant,
        Page::Editing,
        Page::Storage,
        Page::Shortcuts,
        Page::Automations,
        Page::Advanced,
        Page::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::General => n!("General"),
            Page::Recording => n!("Recording"),
            Page::Studio => n!("Studio"),
            Page::Instant => n!("Instant"),
            Page::Editing => n!("Editing"),
            Page::Storage => n!("Storage"),
            Page::Shortcuts => n!("Shortcuts"),
            Page::Automations => n!("Automations"),
            Page::Advanced => n!("Advanced"),
            Page::About => n!("About"),
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Page::General => n!("Theme, language, notifications"),
            Page::Recording => n!("What every recording shares"),
            Page::Studio => n!("Recordings you edit afterwards"),
            Page::Instant => n!("Recordings that are finished when you stop"),
            Page::Editing => n!("Captions, input presets"),
            Page::Storage => n!("Storage folder, names"),
            Page::Shortcuts => n!("Work from any app"),
            Page::Automations => n!("Rules that run only on this PC"),
            Page::Advanced => n!("CLI, diagnostics, logs"),
            Page::About => n!("Version, updates, licences"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Page::General => "icons/settings.svg",
            Page::Recording => "icons/video.svg",
            Page::Studio => "icons/clapperboard.svg",
            Page::Instant => "icons/zap.svg",
            Page::Editing => "icons/pencil.svg",
            Page::Storage => "icons/folder.svg",
            Page::Shortcuts => "icons/keyboard.svg",
            Page::Automations => "icons/workflow.svg",
            Page::Advanced => "icons/sliders-horizontal.svg",
            Page::About => "icons/info.svg",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Page::General => "general",
            Page::Recording => "recording",
            Page::Studio => "studio",
            Page::Instant => "instant",
            Page::Editing => "editing",
            Page::Storage => "storage",
            Page::Shortcuts => "hotkeys",
            Page::Automations => "automations",
            Page::Advanced => "advanced",
            Page::About => "about",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        if let Some(page) = Self::ALL.into_iter().find(|page| page.slug() == slug) {
            return Some(page);
        }
        match slug {
            "shortcuts" => Some(Page::Shortcuts),
            "quality" => Some(Page::Recording),
            "transcription" => Some(Page::Editing),
            "cli" | "experimental" | "feedback" => Some(Page::Advanced),
            "changelog" => Some(Page::About),
            "recordings" | "screenshots" => Some(Page::General),
            _ => None,
        }
    }

    pub fn starts_cluster(self) -> bool {
        matches!(
            self,
            Page::Recording | Page::Editing | Page::Shortcuts | Page::Advanced
        )
    }

    pub fn groups(self) -> &'static [Group] {
        match self {
            Page::General => GENERAL,
            Page::Recording => RECORDING,
            Page::Studio => STUDIO,
            Page::Instant => INSTANT,
            Page::Editing => EDITING,
            Page::Storage => STORAGE,
            Page::Shortcuts => SHORTCUTS,
            Page::Automations => AUTOMATIONS,
            Page::Advanced => ADVANCED,
            Page::About => ABOUT,
        }
    }
}

impl Row {
    pub fn is_shown(self) -> bool {
        cfg!(target_os = "macos") || !matches!(self, Row::DockIcon | Row::NotchOverlay)
    }

    pub fn follows(self) -> Option<Row> {
        match self {
            Row::VoiceIsolation => Some(Row::StudioSound),
            Row::InputDevice
            | Row::InputPreset
            | Row::InputGamepadPreset
            | Row::InputSize
            | Row::InputOpacity
            | Row::InputPosition => Some(Row::ShowInput),
            Row::FocusCenter => Some(Row::WindowStyle),
            Row::KeepCompressed => Some(Row::CompressInstant),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Row::Theme => n!("Theme"),
            Row::Language => n!("Language"),
            Row::Notifications => n!("Notifications"),
            Row::RecordingSounds => n!("Recording sounds"),
            Row::ChapterMarkSound => n!("Chapter mark sound"),
            Row::KeepDrawings => n!("Keep drawings on screen"),
            Row::DockIcon => n!("Always show dock icon"),
            Row::MainWindowStart => n!("When recording starts"),
            Row::AfterDelete => n!("After deleting a recording"),
            Row::AfterEditorClose => n!("When you close the editor"),
            Row::AfterCancel => n!("When you cancel"),
            Row::StudioQuality => n!("Studio quality"),
            Row::InstantResolution => n!("Instant resolution"),
            Row::MaxFps => n!("Maximum frame rate"),
            Row::Countdown => n!("Countdown"),
            Row::ConfirmMicrophone => n!("Confirm without microphone"),
            Row::AutoStop => n!("Auto-stop"),
            Row::ClickHighlight => n!("Click highlight"),
            Row::ShowInput => n!("Show input"),
            Row::InputDevice => n!("Input device"),
            Row::InputPreset => n!("Input preset"),
            Row::InputGamepadPreset => n!("Gamepad preset"),
            Row::InputSize => n!("Input display size"),
            Row::InputOpacity => n!("Input display opacity"),
            Row::InputPosition => n!("Input display position"),
            Row::WindowStyle => n!("Record a window as"),
            Row::FocusCenter => n!("Center the window on screen"),
            Row::VoiceIsolation => n!("Voice isolation"),
            Row::SeparateCursor => n!("Separate cursor"),
            Row::RecordInput => n!("Record input"),
            Row::CrashRecovery => n!("Crash recovery"),
            Row::NotchOverlay => n!("Draw the MacBook notch on screen recordings"),
            Row::ExcludedWindows => n!("Excluded windows"),
            Row::AfterScreenshot => n!("After a screenshot"),
            Row::AfterStudio => n!("After a Studio recording"),
            Row::AfterInstant => n!("After an Instant recording"),
            Row::StudioSound => n!("Studio Sound"),
            Row::AutoZoom => n!("Auto zoom on clicks"),
            Row::ZoomAmount => n!("Zoom amount"),
            Row::RememberedTerms => n!("Remembered terms"),
            Row::InputPresets => n!("Input presets"),
            Row::StorageFolder => n!("Storage folder"),
            Row::NameTemplate => n!("Name template"),
            Row::CompressInstant => n!("Compress Instant recordings"),
            Row::KeepCompressed => n!("Keep only the copy"),
            Row::Shortcut(action) => shortcut_label(action),
            Row::Rules => n!("Rules"),
            Row::Cli => n!("frameever command"),
            Row::Muxer => n!("Out-of-process muxer"),
            Row::Diagnostic => n!("Diagnostic report"),
            Row::Logs => n!("Logs"),
            Row::Version => n!("Version"),
            Row::UpdateDownloads => n!("Update downloads"),
            Row::UpdateChannel => n!("Update channel"),
            Row::ReportProblem => n!("Report a problem"),
            Row::Changelog => n!("Changelog"),
            Row::Licences => n!("Licences"),
        }
    }

    pub fn description(self) -> Option<&'static str> {
        Some(match self {
            Row::Language => n!("The tray menu and window titles switch after a restart."),
            Row::Notifications => n!("Saved files, clipboard copies and finished compression."),
            Row::RecordingSounds => {
                n!("Plays when a recording starts or stops and when you take a screenshot.")
            }
            Row::ChapterMarkSound => {
                n!(
                    "Plays when you add a chapter mark while recording. Your microphone can pick it up."
                )
            }
            Row::KeepDrawings => {
                n!("When off, each drawing fades a few seconds after you draw it.")
            }
            Row::DockIcon => n!("Keep FrameEver in the dock even when no windows are open."),
            Row::MainWindowStart => n!("What the main window does."),
            Row::AfterEditorClose => n!("Applies once the last editor window closes."),
            Row::AfterCancel => n!("Applies when you press Esc or close the picker."),
            Row::InstantResolution => n!("The most Instant recordings are saved at."),
            Row::MaxFps => n!("Higher rates can drop frames on some PCs."),
            Row::ConfirmMicrophone => n!("Ask first if no microphone is selected or available."),
            Row::SeparateCursor => n!("Adjust size and smoothing in the editor."),
            Row::ShowInput => n!("Draws your keys, mouse or gamepad into the video."),
            Row::InputDevice => n!("Automatic draws the one you are using."),
            Row::InputPreset => n!("The keys drawn for keyboard and mouse."),
            Row::InputGamepadPreset => n!("The pad drawn for a gamepad."),
            Row::VoiceIsolation => n!("What Studio Sound starts with in a new project."),
            Row::CrashRecovery => n!("Records in fragments, slightly larger while capturing."),
            Row::StudioSound => n!("Clean up microphone audio. Turn it off per project."),
            Row::AutoZoom => n!("Adds zoom segments around mouse clicks."),
            Row::ZoomAmount => n!("Also used for zoom segments you add yourself."),
            Row::RememberedTerms => n!("Names and jargon Whisper should spell your way."),
            Row::InputPresets => {
                n!("Layouts for the input display. Studio and Instant recordings share them.")
            }
            Row::CompressInstant => {
                n!("Makes an HEVC copy about half the size. The original stays.")
            }
            Row::KeepCompressed => n!("Deletes the original to save space."),
            Row::Cli => n!("For terminals, scripts and AI agents."),
            Row::Muxer => n!("Writes video in a separate process so a crash loses less."),
            Row::Diagnostic => n!("A short test that flashes and beeps. Stays on this PC."),
            Row::Logs => n!("Stay on this PC until you attach them yourself."),
            Row::UpdateDownloads => n!("Choose whether FrameEver downloads updates automatically."),
            Row::UpdateChannel => n!("Nightly builds come first and may be unstable."),
            Row::ReportProblem => {
                n!("Opens GitHub in your browser. Nothing is sent until you submit.")
            }
            _ => return None,
        })
    }

    pub fn id(self) -> String {
        match self {
            Row::Shortcut(action) => format!("shortcut-{action}"),
            row => format!("{row:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub page: Page,
    pub group: &'static str,
    pub row: Row,
}

pub fn places() -> impl Iterator<Item = Place> {
    Page::ALL.into_iter().flat_map(|page| {
        page.groups().iter().flat_map(move |group| {
            group
                .rows
                .iter()
                .copied()
                .filter(|row| row.is_shown())
                .map(move |row| Place {
                    page,
                    group: group.title,
                    row,
                })
        })
    })
}

pub fn place_of(row: Row) -> Option<Place> {
    places().find(|place| place.row == row)
}

pub fn search(query: &str) -> Vec<Place> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let matches = |english: &'static str| {
        Language::ALL
            .into_iter()
            .any(|language| translate(english, language).to_lowercase().contains(&query))
    };
    places()
        .filter(|place| {
            matches(place.row.label())
                || place.row.description().is_some_and(matches)
                || matches(place.page.label())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(places: &[Place]) -> Vec<&'static str> {
        places.iter().map(|place| place.row.label()).collect()
    }

    #[test]
    fn the_sidebar_has_ten_pages_in_five_clusters() {
        let labels: Vec<_> = Page::ALL.iter().map(|page| page.label()).collect();
        let clusters: Vec<_> = Page::ALL
            .iter()
            .filter(|page| page.starts_cluster())
            .map(|page| page.label())
            .collect();
        assert_eq!(clusters, ["Recording", "Editing", "Shortcuts", "Advanced"]);
        assert_eq!(
            labels,
            [
                "General",
                "Recording",
                "Studio",
                "Instant",
                "Editing",
                "Storage",
                "Shortcuts",
                "Automations",
                "Advanced",
                "About"
            ]
        );
    }

    #[test]
    fn every_inventory_row_has_its_place_on_windows() {
        let inventory: &[(Page, &[&str])] = &[
            (
                Page::General,
                &[
                    "Theme",
                    "Language",
                    "Notifications",
                    "Recording sounds",
                    "Chapter mark sound",
                    "When recording starts",
                    "When you close the editor",
                    "When you cancel",
                    "After deleting a recording",
                ],
            ),
            (
                Page::Recording,
                &[
                    "Countdown",
                    "Confirm without microphone",
                    "Keep drawings on screen",
                    "Maximum frame rate",
                    "Auto-stop",
                    "Record a window as",
                    "Center the window on screen",
                    "Excluded windows",
                    "After a screenshot",
                ],
            ),
            (
                Page::Studio,
                &[
                    "Studio quality",
                    "Separate cursor",
                    "Record input",
                    "Crash recovery",
                    "Studio Sound",
                    "Voice isolation",
                    "Auto zoom on clicks",
                    "Zoom amount",
                    "After a Studio recording",
                ],
            ),
            (
                Page::Instant,
                &[
                    "Instant resolution",
                    "Click highlight",
                    "Show input",
                    "Input device",
                    "Input preset",
                    "Gamepad preset",
                    "Input display size",
                    "Input display opacity",
                    "Input display position",
                    "After an Instant recording",
                    "Compress Instant recordings",
                    "Keep only the copy",
                ],
            ),
            (Page::Editing, &["Remembered terms", "Input presets"]),
            (Page::Storage, &["Storage folder", "Name template"]),
            (
                Page::Shortcuts,
                &[
                    "Screenshot an area",
                    "Screenshot a window",
                    "Screenshot the display",
                    "Record an area",
                    "Record a window",
                    "Record a display",
                    "Start in selected mode",
                    "Start Studio recording",
                    "Start Instant recording",
                    "Open recording picker",
                    "Cycle recording mode",
                    "Focus camera preview",
                    "Play or pause the teleprompter",
                    "Stop recording",
                    "Pause or resume",
                    "Restart recording",
                    "Draw on screen",
                    "Focus recording controls",
                    "Hide or show controls",
                    "Add chapter mark",
                    "Mute",
                    "Push to talk",
                ],
            ),
            (Page::Automations, &["Rules"]),
            (
                Page::Advanced,
                &[
                    "frameever command",
                    "Out-of-process muxer",
                    "Diagnostic report",
                    "Logs",
                ],
            ),
            (
                Page::About,
                &[
                    "Version",
                    "Update downloads",
                    "Update channel",
                    "Report a problem",
                    "Changelog",
                    "Licences",
                ],
            ),
        ];
        if cfg!(target_os = "macos") {
            return;
        }
        for (page, expected) in inventory {
            let found: Vec<_> = places().filter(|place| place.page == *page).collect();
            assert_eq!(labels(&found), *expected, "{page:?}");
        }
    }

    #[test]
    fn every_hotkey_action_has_one_shortcut_row() {
        for action in all_shortcuts() {
            let rows = places()
                .filter(|place| place.row == Row::Shortcut(action))
                .count();
            assert_eq!(rows, 1, "{action}");
            assert_ne!(shortcut_label(action), "Shortcut");
        }
    }

    #[test]
    fn search_matches_labels_descriptions_and_page_names_in_both_languages() {
        let zoom = search("zoom");
        assert_eq!(labels(&zoom), ["Auto zoom on clicks", "Zoom amount"]);
        assert!(zoom.iter().all(|place| place.page == Page::Studio));

        assert_eq!(labels(&search("倒计时")), ["Countdown"]);
        assert_eq!(labels(&search("  WHISPER ")), ["Remembered terms"]);
        assert_eq!(
            labels(&search("storage")),
            ["Storage folder", "Name template"]
        );
        assert!(search("wxyz").is_empty());
        assert!(search("   ").is_empty());
    }

    #[test]
    fn search_finds_what_used_to_live_only_beside_the_recording() {
        assert_eq!(labels(&search("自动停止")), ["Auto-stop"]);
        assert_eq!(labels(&search("click highlight")), ["Click highlight"]);
        assert_eq!(labels(&search("显示操作")), ["Show input"]);
        assert_eq!(
            labels(&search("input preset")),
            ["Input preset", "Input presets"]
        );
        assert_eq!(labels(&search("窗口录制方式")), ["Record a window as"]);
        assert_eq!(labels(&search("人声隔离")), ["Voice isolation"]);
    }

    #[test]
    fn options_of_one_recording_mode_sit_on_the_page_of_that_mode() {
        let page = |row| place_of(row).unwrap().page;
        for row in [
            Row::StudioQuality,
            Row::SeparateCursor,
            Row::RecordInput,
            Row::CrashRecovery,
            Row::StudioSound,
            Row::VoiceIsolation,
            Row::AutoZoom,
            Row::ZoomAmount,
            Row::AfterStudio,
        ] {
            assert_eq!(page(row), Page::Studio, "{row:?}");
        }
        for row in [
            Row::InstantResolution,
            Row::ClickHighlight,
            Row::ShowInput,
            Row::InputDevice,
            Row::InputPreset,
            Row::InputGamepadPreset,
            Row::InputSize,
            Row::InputOpacity,
            Row::InputPosition,
            Row::AfterInstant,
            Row::CompressInstant,
            Row::KeepCompressed,
        ] {
            assert_eq!(page(row), Page::Instant, "{row:?}");
        }
        for row in [
            Row::Countdown,
            Row::MaxFps,
            Row::AutoStop,
            Row::WindowStyle,
            Row::AfterScreenshot,
        ] {
            assert_eq!(page(row), Page::Recording, "{row:?}");
        }
    }

    #[test]
    fn a_row_that_follows_a_switch_sits_right_after_it_in_the_same_group() {
        let followers: Vec<Row> = places()
            .map(|place| place.row)
            .filter(|row| row.follows().is_some())
            .collect();
        assert_eq!(
            followers,
            [
                Row::FocusCenter,
                Row::VoiceIsolation,
                Row::InputDevice,
                Row::InputPreset,
                Row::InputGamepadPreset,
                Row::InputSize,
                Row::InputOpacity,
                Row::InputPosition,
                Row::KeepCompressed
            ]
        );
        for group in Page::ALL.iter().flat_map(|page| page.groups()) {
            for (index, row) in group.rows.iter().enumerate() {
                let Some(leader) = row.follows() else {
                    continue;
                };
                let before = &group.rows[..index];
                let last_leader = before.iter().rposition(|row| row.follows().is_none());
                assert_eq!(
                    last_leader.map(|at| before[at]),
                    Some(leader),
                    "{row:?} is not under {leader:?}"
                );
            }
        }
    }

    fn keys_the_settings_store_reads() -> Vec<String> {
        let source = include_str!("../store.rs").replace("\r\n", "\n");
        let start = source.find("fn from_sections(").unwrap();
        let end = start + source[start..].find("\n    }\n").unwrap();
        let body: String = source[start..end]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let mut keys = vec!["storage".to_string()];
        for section in ["general", "safety", "audio_enhancement"] {
            for opening in [format!("_at({section},"), format!("{section}.get(")] {
                for (at, _) in body.match_indices(&opening) {
                    let rest = &body[at + opening.len()..];
                    let quoted = rest.strip_prefix('"');
                    let name = quoted.unwrap_or(rest);
                    let length = name
                        .find(|character: char| character != '_' && !character.is_alphanumeric())
                        .unwrap();
                    keys.push(name[..length].to_string());
                }
            }
        }
        keys.sort();
        keys.dedup();
        keys
    }

    fn row_of_stored_key(key: &str) -> Option<Option<Row>> {
        Some(Some(match key {
            "theme" => Row::Theme,
            "language" => Row::Language,
            "hideDockIcon" => Row::DockIcon,
            "enableNotifications" => Row::Notifications,
            "RECORDING_SOUNDS_KEY" => Row::RecordingSounds,
            "CHAPTER_MARK_SOUND_KEY" => Row::ChapterMarkSound,
            "KEEP_DRAWINGS_KEY" => Row::KeepDrawings,
            "mainWindowRecordingStartBehaviour" => Row::MainWindowStart,
            "postDeletionBehaviour" => Row::AfterDelete,
            "keepMainHiddenAfterEditor" => Row::AfterEditorClose,
            "keepMainHiddenOnCancel" => Row::AfterCancel,
            "studioRecordingQuality" => Row::StudioQuality,
            "instantModeMaxResolution" => Row::InstantResolution,
            "maxFps" => Row::MaxFps,
            "recordingCountdown" => Row::Countdown,
            "confirmBeforeRecordingWithoutMicrophone" => Row::ConfirmMicrophone,
            "recordingAutoStopMinutes" => Row::AutoStop,
            "custom_cursor_capture2" => Row::SeparateCursor,
            "captureKeyboardEvents" => Row::RecordInput,
            "crashRecoveryRecording" => Row::CrashRecovery,
            "instantClickHighlight" => Row::ClickHighlight,
            "instantShowInput" => Row::ShowInput,
            "instantInputDevice" => Row::InputDevice,
            "instantInputPreset" => Row::InputPreset,
            "instantInputGamepadPreset" => Row::InputGamepadPreset,
            "instantInputSize" => Row::InputSize,
            "instantInputOpacity" => Row::InputOpacity,
            "instantInputPosition" | "instantInputCorner" => Row::InputPosition,
            "windowCaptureStyle" => Row::WindowStyle,
            "focusCenter" => Row::FocusCenter,
            "macbookNotchOverlay" => Row::NotchOverlay,
            "excludedWindows" => Row::ExcludedWindows,
            "postScreenshotBehaviour" => Row::AfterScreenshot,
            "postStudioRecordingBehaviour" => Row::AfterStudio,
            "postInstantRecordingBehaviour" => Row::AfterInstant,
            "STUDIO_SOUND_BY_DEFAULT_KEY" => Row::StudioSound,
            "isolation" => Row::VoiceIsolation,
            "autoZoomOnClicks" => Row::AutoZoom,
            "defaultZoomAmount" => Row::ZoomAmount,
            "storage" => Row::StorageFolder,
            "defaultProjectNameTemplate" => Row::NameTemplate,
            "autoCompressInstant" => Row::CompressInstant,
            "autoCompressDeleteOriginal" => Row::KeepCompressed,
            "outOfProcessMuxer" => Row::Muxer,
            "updateDownloadPolicy" => Row::UpdateDownloads,
            "updateChannel" => Row::UpdateChannel,
            "editorPreviewQuality"
            | "EDITOR_PREVIEW_QUALITY_CHOSEN_KEY"
            | "editorSidebarWidth"
            | "EDITOR_PREVIEW_SPEED_KEY"
            | "deleteInstantRecordingsAfterUpload"
            | "enableNativeCameraPreview" => return Some(None),
            _ => return None,
        }))
    }

    #[test]
    fn every_stored_setting_has_a_row_or_is_only_remembered() {
        let keys = keys_the_settings_store_reads();
        assert!(
            keys.len() >= 45,
            "the store's keys were not found: {keys:?}"
        );
        for key in keys {
            match row_of_stored_key(&key) {
                Some(Some(row)) => assert!(
                    Page::ALL
                        .iter()
                        .flat_map(|page| page.groups())
                        .any(|group| group.rows.contains(&row)),
                    "{key} names {row:?}, which is on no page"
                ),
                Some(None) => {}
                None => panic!(
                    "{key} is read from the settings store but has no settings row. Give it one, \
                     or list it with the values the app only remembers \
                     (docs/design/README.md, Where a setting lives)."
                ),
            }
        }
    }

    #[test]
    fn old_page_slugs_land_on_the_page_that_now_holds_their_rows() {
        for page in Page::ALL {
            assert_eq!(Page::from_slug(page.slug()), Some(page));
        }
        assert_eq!(Page::from_slug("quality"), Some(Page::Recording));
        assert_eq!(Page::from_slug("studio"), Some(Page::Studio));
        assert_eq!(Page::from_slug("instant"), Some(Page::Instant));
        assert_eq!(Page::from_slug("transcription"), Some(Page::Editing));
        assert_eq!(Page::from_slug("cli"), Some(Page::Advanced));
        assert_eq!(Page::from_slug("feedback"), Some(Page::Advanced));
        assert_eq!(Page::from_slug("changelog"), Some(Page::About));
        assert_eq!(Page::from_slug("recordings"), Some(Page::General));
        assert_eq!(Page::from_slug("nope"), None);
    }

    #[test]
    fn a_row_knows_where_it_lives() {
        let place = place_of(Row::AfterInstant).unwrap();
        assert_eq!(place.page, Page::Instant);
        assert_eq!(place.group, "When it's done");
        assert_eq!(
            place_of(Row::Shortcut("stopRecording")).unwrap().page,
            Page::Shortcuts
        );
    }
}
