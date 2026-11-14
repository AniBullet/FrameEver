use crate::i18n::{Language, n, translate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    General,
    Recording,
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
    DockIcon,
    MainWindowStart,
    AfterDelete,
    StudioQuality,
    InstantResolution,
    MaxFps,
    Countdown,
    ConfirmMicrophone,
    SeparateCursor,
    KeyPresses,
    CrashRecovery,
    NotchOverlay,
    ExcludedWindows,
    AfterScreenshot,
    AfterStudio,
    AfterInstant,
    StudioSound,
    AutoZoom,
    ZoomAmount,
    RememberedTerms,
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

pub const RECORDING_SHORTCUTS: [&str; 8] = [
    "openRecordingPickerArea",
    "openRecordingPickerWindow",
    "openRecordingPickerDisplay",
    "startStudioRecording",
    "startInstantRecording",
    "openRecordingPicker",
    "cycleRecordingMode",
    "focusCameraPreview",
];

pub const WHILE_RECORDING_SHORTCUTS: [&str; 4] = [
    "stopRecording",
    "togglePauseRecording",
    "restartRecording",
    "drawOnScreen",
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
        "drawOnScreen" => n!("Draw on screen"),
        _ => n!("Shortcut"),
    }
}

const GENERAL: &[Group] = &[
    Group {
        title: n!("Appearance"),
        rows: &[Row::Theme, Row::Language],
    },
    Group {
        title: n!("App"),
        rows: &[
            Row::Notifications,
            Row::DockIcon,
            Row::MainWindowStart,
            Row::AfterDelete,
        ],
    },
];

const RECORDING: &[Group] = &[
    Group {
        title: n!("Quality"),
        rows: &[Row::StudioQuality, Row::InstantResolution, Row::MaxFps],
    },
    Group {
        title: n!("Before you record"),
        rows: &[Row::Countdown, Row::ConfirmMicrophone],
    },
    Group {
        title: n!("Captured with the video"),
        rows: &[
            Row::SeparateCursor,
            Row::KeyPresses,
            Row::CrashRecovery,
            Row::NotchOverlay,
        ],
    },
    Group {
        title: n!("Hidden from recordings"),
        rows: &[Row::ExcludedWindows],
    },
    Group {
        title: n!("When it's done"),
        rows: &[Row::AfterScreenshot, Row::AfterStudio, Row::AfterInstant],
    },
];

const EDITING: &[Group] = &[
    Group {
        title: n!("New Studio projects"),
        rows: &[Row::StudioSound, Row::AutoZoom, Row::ZoomAmount],
    },
    Group {
        title: n!("Captions"),
        rows: &[Row::RememberedTerms],
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
    Group {
        title: n!("Compression"),
        rows: &[Row::CompressInstant, Row::KeepCompressed],
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
        ],
    },
    Group {
        title: n!("While recording"),
        rows: &[
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[0]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[1]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[2]),
            Row::Shortcut(WHILE_RECORDING_SHORTCUTS[3]),
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
    pub const ALL: [Page; 8] = [
        Page::General,
        Page::Recording,
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
            Page::Recording => n!("Quality, countdown, what's captured"),
            Page::Editing => n!("Defaults for new projects, captions"),
            Page::Storage => n!("Storage folder, names, compression"),
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

    pub fn groups(self) -> &'static [Group] {
        match self {
            Page::General => GENERAL,
            Page::Recording => RECORDING,
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

    pub fn label(self) -> &'static str {
        match self {
            Row::Theme => n!("Theme"),
            Row::Language => n!("Language"),
            Row::Notifications => n!("Notifications"),
            Row::DockIcon => n!("Always show dock icon"),
            Row::MainWindowStart => n!("When recording starts"),
            Row::AfterDelete => n!("After deleting a recording"),
            Row::StudioQuality => n!("Studio quality"),
            Row::InstantResolution => n!("Instant resolution"),
            Row::MaxFps => n!("Maximum frame rate"),
            Row::Countdown => n!("Countdown"),
            Row::ConfirmMicrophone => n!("Confirm without microphone"),
            Row::SeparateCursor => n!("Separate cursor"),
            Row::KeyPresses => n!("Key presses"),
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
            Row::DockIcon => n!("Keep FrameEver in the dock even when no windows are open."),
            Row::MainWindowStart => n!("What the main window does."),
            Row::InstantResolution => n!("The most Instant recordings are saved at."),
            Row::MaxFps => n!("Higher rates can drop frames on some PCs."),
            Row::ConfirmMicrophone => n!("Ask first if no microphone is selected or available."),
            Row::SeparateCursor => n!("Studio only. Adjust size and smoothing in the editor."),
            Row::KeyPresses => n!("Lets you add keyboard overlays in the editor."),
            Row::CrashRecovery => n!("Records in fragments, slightly larger while capturing."),
            Row::StudioSound => n!("Clean up microphone audio. Turn it off per project."),
            Row::AutoZoom => n!("Adds zoom segments around mouse clicks."),
            Row::ZoomAmount => n!("Also used for zoom segments you add yourself."),
            Row::RememberedTerms => n!("Names and jargon Whisper should spell your way."),
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
    fn the_sidebar_has_eight_pages_in_order() {
        let labels: Vec<_> = Page::ALL.iter().map(|page| page.label()).collect();
        assert_eq!(
            labels,
            [
                "General",
                "Recording",
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
                    "When recording starts",
                    "After deleting a recording",
                ],
            ),
            (
                Page::Recording,
                &[
                    "Studio quality",
                    "Instant resolution",
                    "Maximum frame rate",
                    "Countdown",
                    "Confirm without microphone",
                    "Separate cursor",
                    "Key presses",
                    "Crash recovery",
                    "Excluded windows",
                    "After a screenshot",
                    "After a Studio recording",
                    "After an Instant recording",
                ],
            ),
            (
                Page::Editing,
                &[
                    "Studio Sound",
                    "Auto zoom on clicks",
                    "Zoom amount",
                    "Remembered terms",
                ],
            ),
            (
                Page::Storage,
                &[
                    "Storage folder",
                    "Name template",
                    "Compress Instant recordings",
                    "Keep only the copy",
                ],
            ),
            (
                Page::Shortcuts,
                &[
                    "Screenshot an area",
                    "Screenshot a window",
                    "Screenshot the display",
                    "Record an area",
                    "Record a window",
                    "Record a display",
                    "Start Studio recording",
                    "Start Instant recording",
                    "Open recording picker",
                    "Cycle recording mode",
                    "Focus camera preview",
                    "Stop recording",
                    "Pause or resume",
                    "Restart recording",
                    "Draw on screen",
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
        assert!(zoom.iter().all(|place| place.page == Page::Editing));

        assert_eq!(labels(&search("倒计时")), ["Countdown"]);
        assert_eq!(labels(&search("  WHISPER ")), ["Remembered terms"]);
        assert_eq!(
            labels(&search("storage")),
            [
                "Storage folder",
                "Name template",
                "Compress Instant recordings",
                "Keep only the copy"
            ]
        );
        assert!(search("wxyz").is_empty());
        assert!(search("   ").is_empty());
    }

    #[test]
    fn old_page_slugs_land_on_the_page_that_now_holds_their_rows() {
        for page in Page::ALL {
            assert_eq!(Page::from_slug(page.slug()), Some(page));
        }
        assert_eq!(Page::from_slug("quality"), Some(Page::Recording));
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
        assert_eq!(place.page, Page::Recording);
        assert_eq!(place.group, "When it's done");
        assert_eq!(
            place_of(Row::Shortcut("stopRecording")).unwrap().page,
            Page::Shortcuts
        );
    }
}
