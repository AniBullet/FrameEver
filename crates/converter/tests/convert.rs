use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use frameever_converter::{OldInstall, Source, Target, convert, find_installs};
use frameever_project::{
    InstantRecordingMeta, Platform, RecordingMeta, RecordingMetaInner, SingleSegment,
    StudioRecordingMeta, VideoMeta,
};
use frameever_utils::storage::{ProjectKind, Storage, StorageSettings, format_version};

struct Machine {
    root: tempfile::TempDir,
}

impl Machine {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    fn app_data_root(&self) -> PathBuf {
        self.path("AppData/Roaming")
    }

    fn videos(&self) -> PathBuf {
        self.path("Videos")
    }

    fn target(&self) -> Target {
        Target {
            settings_file: self.path("AppData/Roaming/io.github.anibullet.frameever/store"),
            default_storage_dir: self.path("Videos/FrameEver"),
        }
    }

    fn storage(&self) -> Storage {
        let target = self.target();
        Storage::new(
            target.default_storage_dir,
            StorageSettings::load(&target.settings_file),
        )
    }

    fn settings(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.target().settings_file).unwrap()).unwrap()
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn installs(&self) -> Vec<OldInstall> {
        find_installs(&self.app_data_root(), &self.videos())
    }
}

fn studio_project(dir: PathBuf, name: &str) -> PathBuf {
    std::fs::create_dir_all(&dir).unwrap();
    let meta = RecordingMeta {
        platform: Some(Platform::Windows),
        project_path: dir.clone(),
        pretty_name: name.into(),
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
    };
    meta.save_for_project().unwrap();
    dir
}

const AN_OLD_DAY: Duration = Duration::from_secs(1_700_000_000);

fn age(path: &Path) {
    let time = SystemTime::UNIX_EPOCH + AN_OLD_DAY;
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(time).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTimesExt;
        file.set_times(std::fs::FileTimes::new().set_created(time))
            .unwrap();
    }
}

fn date_of(path: &Path) -> SystemTime {
    let metadata = path.metadata().unwrap();
    metadata.created().or_else(|_| metadata.modified()).unwrap()
}

fn listed(storage: &Storage, kind: ProjectKind) -> Vec<(String, PathBuf)> {
    let mut names: Vec<_> = storage
        .projects(kind)
        .into_iter()
        .map(|project| {
            let meta = RecordingMeta::load_for_project(&project).unwrap();
            assert_eq!(format_version(&project), Some(1));
            (meta.pretty_name, project)
        })
        .collect();
    names.sort();
    names
}

#[test]
fn a_default_cap_anibullet_2_install_lands_in_the_storage_folder_with_compressed_copies_paired_and_dates_kept()
 {
    let machine = Machine::new();
    machine.write(
        "AppData/Roaming/so.cap.desktop.anibullet/store",
        br#"{"general_settings":{"recordingQuality":"high","language":"zh-CN","autoCompressInstant":true,"excludedWindows":[{"windowTitle":"Cap"},{"windowTitle":"Cap Camera"},{"windowTitle":"Notes"}]}}"#,
    );
    let base = "Videos/Cap Recordings";
    studio_project(
        machine.path(&format!("{base}/recordings/Studio.cap")),
        "Studio",
    );
    let instant = machine.write(&format!("{base}/exports/video/Instant.mp4"), b"original");
    age(&instant);
    machine.write(
        &format!("{base}/exports/video/Instant_compressed.mp4"),
        b"small",
    );
    machine.write(
        &format!("{base}/exports/video/Lonely_compressed.mp4"),
        b"only",
    );
    let shot = machine.write(&format!("{base}/exports/screenshot/Shot.png"), b"png");
    age(&shot);

    let installs = machine.installs();
    assert_eq!(
        installs,
        vec![OldInstall {
            source: Source::CapAniBullet2,
            app_data: machine.path("AppData/Roaming/so.cap.desktop.anibullet"),
            videos_dir: machine.videos(),
        }]
    );
    let report = convert(&installs[0], &machine.target());

    assert!(report.skipped.is_empty(), "{report:?}");
    assert_eq!(report.converted.len(), 4);
    let storage = machine.storage();
    let recordings = listed(&storage, ProjectKind::Recording);
    let names: Vec<_> = recordings.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Instant", "Lonely", "Studio"]);
    let instant = &recordings[0].1;
    assert_eq!(
        instant,
        &machine.path("Videos/FrameEver/Recordings/Instant.ever")
    );
    assert!(matches!(
        RecordingMeta::load_for_project(instant).unwrap().inner,
        RecordingMetaInner::Instant(InstantRecordingMeta::Complete { .. })
    ));
    assert_eq!(
        std::fs::read(instant.join("content/output.mp4")).unwrap(),
        b"original"
    );
    assert_eq!(
        std::fs::read(instant.join("content/output_compressed.mp4")).unwrap(),
        b"small"
    );
    assert_eq!(date_of(instant), SystemTime::UNIX_EPOCH + AN_OLD_DAY);
    assert_eq!(
        std::fs::read(recordings[1].1.join("content/output.mp4")).unwrap(),
        b"only"
    );
    assert!(
        !recordings[1]
            .1
            .join("content/output_compressed.mp4")
            .exists()
    );

    let screenshots = listed(&storage, ProjectKind::Screenshot);
    assert_eq!(screenshots.len(), 1);
    assert_eq!(screenshots[0].0, "Shot");
    let png = screenshots[0].1.join("original.png");
    assert_eq!(std::fs::read(&png).unwrap(), b"png");
    assert_eq!(date_of(&png), SystemTime::UNIX_EPOCH + AN_OLD_DAY);

    assert!(
        !machine
            .path(&format!("{base}/exports/video/Instant.mp4"))
            .exists()
    );
    assert!(
        !machine
            .path(&format!("{base}/recordings/Studio.cap"))
            .exists()
    );

    let general = &machine.settings()["general_settings"];
    assert_eq!(general["studioRecordingQuality"], "balanced");
    assert_eq!(general["language"], "zh-CN");
    assert_eq!(general["autoCompressInstant"], true);
    assert_eq!(
        general["excludedWindows"],
        serde_json::json!([
            {"windowTitle": "FrameEver"},
            {"windowTitle": "FrameEver Camera"},
            {"windowTitle": "Notes"}
        ])
    );
    assert!(general.get("storagePath").is_none());
}

#[test]
fn a_cap_anibullet_2_custom_folder_becomes_the_storage_folder_and_its_projects_stay_on_that_drive()
{
    let machine = Machine::new();
    let custom = machine.path("D/Captures");
    machine.write(
        "AppData/Roaming/so.cap.desktop.anibullet/store",
        serde_json::json!({ "general_settings": { "recordingsSavePath": custom, "recordingQuality": "ultra" } })
            .to_string()
            .as_bytes(),
    );
    studio_project(custom.join("recordings/Custom.cap"), "Custom");
    studio_project(
        machine.path("AppData/Roaming/so.cap.desktop.anibullet/recordings/Early.cap"),
        "Early",
    );
    studio_project(
        machine.path("AppData/Roaming/so.cap.desktop.anibullet/screenshots/Old shot.cap"),
        "Old shot",
    );
    machine.write("D/Captures/exports/video/Clip.mp4", b"clip");

    let report = convert(&machine.installs()[0], &machine.target());

    assert!(report.skipped.is_empty(), "{report:?}");
    let general = &machine.settings()["general_settings"];
    assert_eq!(general["storagePath"], custom.to_string_lossy().as_ref());
    assert_eq!(general["studioRecordingQuality"], "ultra");
    let storage = machine.storage();
    assert_eq!(storage.storage_dir(), custom);
    let recordings = listed(&storage, ProjectKind::Recording);
    assert_eq!(
        recordings,
        vec![
            ("Clip".to_string(), custom.join("Recordings/Clip.ever")),
            ("Custom".to_string(), custom.join("Recordings/Custom.ever")),
            ("Early".to_string(), custom.join("Recordings/Early.ever")),
        ]
    );
    assert_eq!(
        listed(&storage, ProjectKind::Screenshot),
        vec![(
            "Old shot".to_string(),
            custom.join("Screenshots/Old shot.ever")
        )]
    );
}

#[test]
fn an_official_cap_install_brings_its_projects_screenshots_and_rules_with_frameever_variables() {
    let machine = Machine::new();
    let custom = machine.path("E/Cap");
    let previous = machine.path("F/Older");
    machine.write(
        "AppData/Roaming/so.cap.desktop/store",
        serde_json::json!({
            "general_settings": {
                "recordingsPath": custom,
                "previousRecordingsPaths": [previous],
                "studioRecordingQuality": "compatibility",
                "excludedWindows": [{ "windowTitle": "Cap Settings" }]
            },
            "automations": { "version": 1, "rules": [{
                "id": "r1", "name": "Copy", "trigger": "studioRecordingFinished",
                "actions": [{ "type": "runCommand", "program": "copy.cmd",
                    "args": ["%CAP_PROJECT_PATH%", "$CAP_OUTPUT_PATH"],
                    "env": { "CAP_SHARE_LINK": "x" } }]
            }] }
        })
        .to_string()
        .as_bytes(),
    );
    machine.write(
        "AppData/Roaming/io.github.anibullet.frameever/store",
        br#"{"general_settings":{"language":"en"},"automations":{"version":1,"rules":[{"id":"mine","name":"Mine","trigger":"screenshotTaken"}]}}"#,
    );
    studio_project(custom.join("Current.cap"), "Current");
    studio_project(previous.join("Previous.cap"), "Previous");
    studio_project(
        machine.path("AppData/Roaming/so.cap.desktop/recordings/Default.cap"),
        "Default",
    );
    studio_project(
        machine.path("AppData/Roaming/so.cap.desktop/screenshots/Shot.cap"),
        "Shot",
    );

    let installs = machine.installs();
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].source, Source::Cap);
    let report = convert(&installs[0], &machine.target());

    assert!(report.skipped.is_empty(), "{report:?}");
    let storage = machine.storage();
    let names: Vec<_> = listed(&storage, ProjectKind::Recording)
        .into_iter()
        .map(|(name, project)| {
            assert!(project.starts_with(machine.path("Videos/FrameEver/Recordings")));
            name
        })
        .collect();
    assert_eq!(names, ["Current", "Default", "Previous"]);
    assert_eq!(listed(&storage, ProjectKind::Screenshot).len(), 1);

    let settings = machine.settings();
    let general = &settings["general_settings"];
    assert_eq!(general["language"], "en");
    assert_eq!(general["studioRecordingQuality"], "compatibility");
    assert_eq!(
        general["excludedWindows"],
        serde_json::json!([{ "windowTitle": "FrameEver Settings" }])
    );
    let rules = settings["automations"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0]["id"], "mine");
    assert_eq!(
        rules[1]["actions"][0]["args"],
        serde_json::json!(["%FRAMEEVER_PROJECT_PATH%", "$FRAMEEVER_OUTPUT_PATH"])
    );
    assert_eq!(
        rules[1]["actions"][0]["env"],
        serde_json::json!({ "FRAMEEVER_SHARE_LINK": "x" })
    );

    let again = convert(&installs[0], &machine.target());
    assert!(again.converted.is_empty() && again.skipped.is_empty());
    assert_eq!(machine.settings(), settings);
}

#[cfg(windows)]
#[test]
fn a_locked_file_is_reported_and_left_and_a_second_run_converts_only_what_is_left() {
    use std::os::windows::fs::OpenOptionsExt;

    let machine = Machine::new();
    let base = "Videos/Cap Recordings";
    let locked = machine.write(&format!("{base}/exports/video/Busy.mp4"), b"busy");
    machine.write(
        &format!("{base}/exports/video/Busy_compressed.mp4"),
        b"small",
    );
    machine.write(&format!("{base}/exports/video/Free.mp4"), b"free");
    let install = machine.installs().remove(0);

    let lock = std::fs::File::options()
        .read(true)
        .share_mode(0)
        .open(&locked)
        .unwrap();
    let first = convert(&install, &machine.target());
    assert!(
        machine
            .path(&format!("{base}/exports/video/Busy_compressed.mp4"))
            .is_file()
    );
    let second = convert(&install, &machine.target());
    drop(lock);
    let third = convert(&install, &machine.target());

    assert_eq!(first.converted.len(), 1);
    assert_eq!(
        first.skipped,
        vec![frameever_converter::Skipped {
            source: locked.clone(),
            reason: frameever_converter::SkipReason::InUse,
        }]
    );
    assert!(second.converted.is_empty());
    assert_eq!(second.skipped.len(), 1);
    assert_eq!(third.converted.len(), 1);
    assert!(third.skipped.is_empty());
    let names: Vec<_> = listed(&machine.storage(), ProjectKind::Recording)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["Busy", "Free"]);
    assert!(!locked.exists());
}

#[test]
fn a_cap_anibullet_2_instant_video_goes_back_into_the_project_it_was_moved_out_of() {
    let machine = Machine::new();
    let base = "Videos/Cap Recordings";
    let emptied = machine.path(&format!("{base}/recordings/Quick.cap"));
    std::fs::create_dir_all(&emptied).unwrap();
    RecordingMeta {
        platform: Some(Platform::Windows),
        project_path: emptied.clone(),
        pretty_name: "Quick take".into(),
        sharing: None,
        inner: RecordingMetaInner::Instant(InstantRecordingMeta::Complete {
            fps: 60,
            sample_rate: Some(48_000),
        }),
        upload: None,
    }
    .save_for_project()
    .unwrap();
    machine.write(&format!("{base}/exports/video/Quick.mp4"), b"quick");
    machine.write(&format!("{base}/exports/video/Quick_compressing.mp4"), b"");

    let report = convert(&machine.installs()[0], &machine.target());

    assert!(report.skipped.is_empty(), "{report:?}");
    assert_eq!(report.converted.len(), 1);
    let recordings = listed(&machine.storage(), ProjectKind::Recording);
    assert_eq!(recordings.len(), 1);
    let (name, project) = &recordings[0];
    assert_eq!(name, "Quick take");
    assert!(matches!(
        RecordingMeta::load_for_project(project).unwrap().inner,
        RecordingMetaInner::Instant(InstantRecordingMeta::Complete { fps: 60, .. })
    ));
    assert_eq!(
        std::fs::read(project.join("content/output.mp4")).unwrap(),
        b"quick"
    );
}

#[test]
fn a_cap_anibullet_2_jpeg_screenshot_becomes_a_png_screenshot_project_with_its_date() {
    let machine = Machine::new();
    let jpeg = machine.path("Videos/Cap Recordings/exports/screenshot/Photo.jpg");
    std::fs::create_dir_all(jpeg.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(4, 3, image::Rgb([200, 30, 30]))
        .save(&jpeg)
        .unwrap();
    age(&jpeg);
    machine.write("Videos/Cap Recordings/exports/screenshot/notes.txt", b"x");

    let report = convert(&machine.installs()[0], &machine.target());

    assert!(report.skipped.is_empty(), "{report:?}");
    let screenshots = listed(&machine.storage(), ProjectKind::Screenshot);
    assert_eq!(screenshots.len(), 1);
    let png = screenshots[0].1.join("original.png");
    let decoded = image::open(&png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (4, 3));
    assert_eq!(date_of(&png), SystemTime::UNIX_EPOCH + AN_OLD_DAY);
    assert!(!jpeg.exists());
}
