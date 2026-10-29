use std::path::{Path, PathBuf};

use frameever_converter::{
    Converted, OldInstall, Report, SkipReason, Skipped, Target, find_installs,
};
use frameever_converter_app::{
    i18n::Language,
    wizard::{Action, Screen, Step, Wizard},
};
use frameever_project::{
    Platform, RecordingMeta, RecordingMetaInner, SingleSegment, StudioRecordingMeta, VideoMeta,
};

struct Machine {
    root: tempfile::TempDir,
}

impl Machine {
    fn empty() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn with_cap_anibullet_2() -> Self {
        let machine = Self::empty();
        machine.write(
            "AppData/Roaming/so.cap.desktop.anibullet/store",
            br#"{"general_settings":{"recordingQuality":"high"}}"#,
        );
        studio_project(&machine.path("Videos/Cap Recordings/recordings/Studio.cap"));
        machine.write("Videos/Cap Recordings/exports/video/Clip.mp4", b"clip");
        machine.write(
            "Videos/Cap Recordings/exports/video/Clip_compressed.mp4",
            b"small",
        );
        machine.write("Videos/Cap Recordings/exports/screenshot/Shot.png", b"png");
        machine
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn target(&self) -> Target {
        Target {
            settings_file: self.path("AppData/Roaming/io.github.anibullet.frameever/store"),
            default_storage_dir: self.path("Videos/FrameEver"),
        }
    }

    fn wizard(&self) -> Wizard {
        let app_data_root = self.path("AppData/Roaming");
        let videos = self.path("Videos");
        Wizard::new(
            self.target(),
            Box::new(move || find_installs(&app_data_root, &videos)),
        )
        .with_language(Language::English)
    }
}

fn studio_project(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    RecordingMeta {
        platform: Some(Platform::Windows),
        project_path: dir.to_path_buf(),
        pretty_name: "Studio".into(),
        sharing: None,
        inner: RecordingMetaInner::Studio(Box::new(StudioRecordingMeta::SingleSegment {
            segment: SingleSegment {
                display: VideoMeta {
                    path: "content/display.mp4".into(),
                    fps: 30,
                    start_time: Some(0.0),
                    device_id: None,
                },
                camera: None,
                audio: None,
                cursor: None,
            },
        })),
        upload: None,
    }
    .save_for_project()
    .unwrap();
}

fn texts(screen: &Screen) -> Vec<String> {
    let mut texts = vec![screen.title.clone(), screen.subtitle.clone()];
    for row in &screen.rows {
        texts.push(format!("{}: {}", row.title, row.detail));
    }
    texts.extend(screen.notes.iter().cloned());
    texts.extend(
        screen
            .buttons
            .iter()
            .map(|button| format!("[{}]", button.label)),
    );
    texts
}

fn actions(screen: &Screen) -> Vec<Action> {
    screen
        .rows
        .iter()
        .filter_map(|row| row.action)
        .chain(screen.buttons.iter().map(|button| button.action))
        .collect()
}

struct Expected {
    language: Language,
    find: [&'static str; 4],
    preview_title: &'static str,
    from: &'static str,
    destination: &'static str,
    converting: &'static str,
    done: [&'static str; 2],
}

fn run_end_to_end(expected: Expected) {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard().with_language(expected.language);

    let find = wizard.screen();
    let shown = texts(&find);
    for text in expected.find {
        assert!(
            shown.iter().any(|line| line == text),
            "{text:?} not in {shown:#?}"
        );
    }
    assert_eq!(
        actions(&find),
        [Action::Choose(0), Action::PickFolder, Action::Close]
    );

    wizard.choose(0);
    let preview = wizard.screen();
    let shown = texts(&preview);
    assert_eq!(preview.title, expected.preview_title);
    assert_eq!(preview.subtitle, expected.from);
    let destination = format!(
        "{}: {}",
        expected.destination,
        machine.path("Videos/FrameEver").display()
    );
    assert!(
        shown.contains(&destination),
        "{destination:?} not in {shown:#?}"
    );
    assert_eq!(actions(&preview), [Action::Back, Action::Convert]);

    assert!(!wizard.is_converting());
    let job = wizard.convert().expect("a preview with items converts");
    assert!(wizard.is_converting());
    assert_eq!(wizard.screen().title, expected.converting);
    assert!(wizard.screen().buttons.is_empty());
    wizard.finish(job.run());
    assert!(!wizard.is_converting());

    let done = wizard.screen();
    assert_eq!([done.title.as_str(), done.subtitle.as_str()], expected.done);
    assert!(done.rows.is_empty());
    assert_eq!(
        actions(&done),
        [Action::OpenDestination, Action::StartOver, Action::Close]
    );
    assert_eq!(
        wizard.destination(),
        Some(machine.path("Videos/FrameEver").as_path())
    );
    assert!(
        machine
            .path("Videos/FrameEver/Recordings/Studio.ever")
            .is_dir()
    );
    assert!(
        machine
            .path("Videos/FrameEver/Recordings/Clip.ever/content/output_compressed.mp4")
            .is_file()
    );
    assert!(
        machine
            .path("Videos/FrameEver/Screenshots/Shot.ever")
            .is_dir()
    );

    wizard.back();
    assert!(matches!(wizard.step(), Step::Find(found) if found.len() == 1));
}

#[test]
fn the_wizard_converts_a_cap_anibullet_2_install_end_to_end_in_english_and_chinese() {
    run_end_to_end(Expected {
        language: Language::English,
        find: [
            "Bring your old recordings into FrameEver",
            "Found on this computer. Choose one to see what will be converted.",
            "Cap-aniBullet 2.x: Recordings 2 · Screenshots 1",
            "[Choose a folder…]",
        ],
        preview_title: "Ready to convert",
        from: "From Cap-aniBullet 2.x",
        destination: "Destination",
        converting: "Converting…",
        done: ["Conversion finished", "3 of 3 items are now in FrameEver."],
    });
    run_end_to_end(Expected {
        language: Language::SimplifiedChinese,
        find: [
            "把旧的录制带进 FrameEver",
            "在这台电脑上找到了以下数据。选择一项，查看会转换哪些内容。",
            "Cap-aniBullet 2.x: 录制文件 2 · 截图 1",
            "[选择文件夹…]",
        ],
        preview_title: "准备转换",
        from: "来自 Cap-aniBullet 2.x",
        destination: "目标位置",
        converting: "正在转换…",
        done: ["转换完成", "3 项中的 3 项已进入 FrameEver。"],
    });
}

#[test]
fn with_nothing_found_the_user_picks_a_folder_and_left_behind_items_are_listed() {
    let machine = Machine::empty();
    let mut wizard = machine.wizard();
    let find = wizard.screen();
    assert!(find.rows.is_empty());
    assert!(
        find.subtitle
            .starts_with("No Cap or Cap-aniBullet 2.x data")
    );
    assert_eq!(actions(&find), [Action::PickFolder, Action::Close]);

    let picked = machine.path("E/Old");
    std::fs::create_dir_all(&picked).unwrap();
    wizard.choose_folder(&picked);
    let preview = wizard.screen();
    assert_eq!(preview.title, "Nothing to convert here");
    assert_eq!(actions(&preview), [Action::Back]);
    assert!(wizard.convert().is_none());

    studio_project(&picked.join("Kept.cap"));
    machine.write("E/Old/exports/video/Busy.mp4", b"busy");
    wizard.back();
    wizard.choose_folder(&picked);
    let shown = texts(&wizard.screen());
    assert!(
        shown.contains(&"Settings: Left unchanged".to_string()),
        "{shown:#?}"
    );
    assert!(wizard.convert().is_some());
    wizard.finish(Report {
        converted: vec![Converted {
            source: picked.join("Kept.cap"),
            project: machine.path("Videos/FrameEver/Recordings/Kept.ever"),
        }],
        skipped: vec![Skipped {
            source: picked.join("exports/video/Busy.mp4"),
            reason: SkipReason::InUse,
        }],
        settings_error: Some("disk full".into()),
    });

    let done = wizard.screen();
    assert_eq!(done.title, "Some items were left behind");
    assert_eq!(done.subtitle, "1 of 2 items are now in FrameEver.");
    assert_eq!(done.rows.len(), 1);
    assert_eq!(done.rows[0].title, "Busy.mp4");
    assert_eq!(
        done.rows[0].detail,
        "In use by another program. Close it and run the converter again."
    );
    assert!(done.rows[0].warning);
    assert_eq!(done.notes, ["Settings were not carried over: disk full"]);
}

#[test]
fn a_picked_folder_is_shown_with_its_path() {
    let machine = Machine::empty();
    let picked = machine.path("E/Old");
    studio_project(&picked.join("One.cap"));
    let mut wizard = machine.wizard();
    wizard.choose_folder(&picked);
    assert_eq!(
        wizard.screen().subtitle,
        format!("From {}", picked.display())
    );
    assert!(
        matches!(wizard.step(), Step::Preview(preview) if preview.install == OldInstall::picked_folder(&picked))
    );
}
