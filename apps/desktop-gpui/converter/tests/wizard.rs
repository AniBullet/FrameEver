use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use frameever_converter::{Converted, Report, SkipReason, Skipped, Target, find_installs};
use frameever_converter_app::{
    i18n::Language,
    wizard::{Action, Effect, Job, Kind, Screen, StepState, System, Wizard},
};
use frameever_project::{
    Platform, RecordingMeta, RecordingMetaInner, SingleSegment, StudioRecordingMeta, VideoMeta,
};
use frameever_utils::storage::ProjectKind;

#[derive(Clone, Default)]
struct World {
    running: Arc<Mutex<Vec<String>>>,
    free: Arc<Mutex<Option<u64>>>,
    installed: Arc<Mutex<Option<PathBuf>>>,
}

struct Machine {
    root: tempfile::TempDir,
    world: World,
}

impl Machine {
    fn empty() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            world: World::default(),
        }
    }

    fn with_cap_anibullet_2() -> Self {
        let machine = Self::empty();
        machine.write(
            "AppData/Roaming/so.cap.desktop.anibullet/store",
            br#"{"general_settings":{"recordingQuality":"high"},"automations":{"version":1,"rules":[{"id":"a"},{"id":"b"}]}}"#,
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

    fn add_cap(&self) {
        self.write(
            "AppData/Roaming/so.cap.desktop/store",
            br#"{"general_settings":{}}"#,
        );
        studio_project(&self.path("AppData/Roaming/so.cap.desktop/recordings/Official.cap"));
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

    fn system(&self) -> System {
        let app_data_root = self.path("AppData/Roaming");
        let videos = self.path("Videos");
        let root = self.root.path().to_path_buf();
        let world = self.world.clone();
        let running = world.running.clone();
        let free = world.free.clone();
        let installed = world.installed.clone();
        System {
            find_installs: Box::new(move || find_installs(&app_data_root, &videos)),
            running: Box::new(move || running.lock().unwrap().clone()),
            volume: Box::new(move |path: &Path| {
                let relative = path.strip_prefix(&root).ok()?;
                let first = relative.components().next()?;
                Some(match first.as_os_str().to_str()? {
                    "E" => "E:".to_string(),
                    _ => "C:".to_string(),
                })
            }),
            free_space: Box::new(move |_: &Path| *free.lock().unwrap()),
            installed_app: Box::new(move || installed.lock().unwrap().clone()),
        }
    }

    fn wizard(&self, language: Language) -> Wizard {
        Wizard::new(self.target(), self.system()).with_language(language)
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
    texts.extend(screen.warnings.iter().map(|warning| warning.text.clone()));
    if let Some(empty) = &screen.empty {
        texts.push(empty.text.clone());
    }
    for row in screen.groups.iter().flatten() {
        texts.push(
            [&row.title, &row.detail, &row.trailing]
                .into_iter()
                .filter(|text| !text.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    if let Some(meter) = &screen.meter {
        texts.push(format!("{}% {} {}", meter.percent, meter.line, meter.left));
    }
    texts.extend(screen.notes.iter().cloned());
    texts.extend(
        screen
            .start
            .iter()
            .chain(&screen.end)
            .map(|button| format!("[{}]", button.label)),
    );
    texts
}

fn shows(screen: &Screen, text: &str) {
    let shown = texts(screen);
    assert!(
        shown.iter().any(|line| line == text),
        "{text:?} not in {shown:#?}"
    );
}

fn buttons(screen: &Screen) -> Vec<(Action, bool)> {
    screen
        .start
        .iter()
        .chain(&screen.end)
        .map(|button| (button.action, button.enabled))
        .collect()
}

fn job(effect: Option<Effect>) -> Job {
    match effect {
        Some(Effect::Run(job)) => job,
        _ => panic!("expected a conversion to start"),
    }
}

fn run(wizard: &mut Wizard, job: Job) {
    let mut seen = Vec::new();
    let report = job.run(|progress| seen.push(progress.clone()));
    for progress in seen {
        wizard.progress(progress);
    }
    wizard.finish(report);
}

struct Expected {
    language: Language,
    review: [&'static str; 6],
    convert: &'static str,
    converting: [&'static str; 2],
    done: [&'static str; 3],
    steps: [&'static str; 4],
}

fn run_end_to_end(expected: Expected) {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.installed.lock().unwrap() = Some(machine.path("FrameEver/FrameEver.exe"));
    let mut wizard = machine.wizard(expected.language);

    let review = wizard.screen();
    let labels: Vec<_> = review
        .steps
        .iter()
        .map(|(label, _)| label.as_str())
        .collect();
    assert_eq!(labels, expected.steps);
    assert_eq!(review.steps[1].1, StepState::Current);
    for text in expected.review {
        shows(&review, text);
    }
    shows(&review, &format!("[{}]", expected.convert));
    assert!(review.warnings.is_empty());
    assert_eq!(wizard.primary(), Some(Action::Convert));
    assert_eq!(wizard.escape(), Some(Action::Back));

    let job = job(wizard.act(Action::Convert));
    assert_eq!(wizard.escape(), None);
    let converting = wizard.screen();
    assert_eq!(converting.title, expected.converting[0]);
    assert_eq!(converting.steps[2].1, StepState::Current);
    assert_eq!(buttons(&converting), [(Action::Stop, true)]);
    let mut seen = Vec::new();
    let report = job.run(|progress| seen.push(progress.clone()));
    wizard.progress(seen[1].clone());
    let meter = wizard
        .screen()
        .meter
        .expect("a progress meter while converting");
    assert_eq!(meter.percent, 33);
    assert_eq!(meter.line, expected.converting[1]);
    wizard.finish(report);

    let done = wizard.screen();
    assert_eq!(
        [done.title.as_str(), done.subtitle.as_str()],
        expected.done[..2]
    );
    assert!(done.tick);
    shows(&done, expected.done[2]);
    assert_eq!(
        buttons(&done),
        [
            (Action::ShowFolder, true),
            (Action::Close, true),
            (Action::OpenFrameEver, true)
        ]
    );
    assert!(matches!(
        wizard.act(Action::OpenFrameEver),
        Some(Effect::Launch(path)) if path == machine.path("FrameEver/FrameEver.exe")
    ));
    assert!(matches!(
        wizard.act(Action::ShowFolder),
        Some(Effect::Open(path)) if path == machine.path("Videos/FrameEver")
    ));
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
}

#[test]
fn one_install_skips_choose_and_converts_end_to_end_in_english_and_chinese() {
    run_end_to_end(Expected {
        language: Language::English,
        review: [
            "Choose what comes over",
            "From Cap-aniBullet 2.x",
            "Recordings | .cap projects and loose MP4 exports | 2 · 1 KB",
            "Screenshots | PNG and JPEG, kept with their dates | 1 · 1 KB",
            "Settings and automations | Only what FrameEver doesn't have yet | 2 rules",
            "Files are moved, not copied. Afterwards only FrameEver opens them.",
        ],
        convert: "Convert 3 items",
        converting: ["Converting…", "Moving 2 of 3: Clip"],
        done: [
            "Conversion finished",
            "3 items are now in FrameEver.",
            "[Open FrameEver]",
        ],
        steps: ["Choose", "Review", "Convert", "Done"],
    });
    run_end_to_end(Expected {
        language: Language::SimplifiedChinese,
        review: [
            "选择要带上的内容",
            "来自 Cap-aniBullet 2.x",
            "录制 | .cap 项目和零散的 MP4 导出 | 2 个 · 1 KB",
            "截图 | PNG 和 JPEG，保留原来的日期 | 1 张 · 1 KB",
            "设置和自动化 | 只补上 FrameEver 还没有的 | 2 条规则",
            "文件会被移动而不是复制，之后只有 FrameEver 能打开它们。",
        ],
        convert: "转换 3 项",
        converting: ["正在转换…", "正在移动第 2 项，共 3 项：Clip"],
        done: ["转换完成", "3 项内容已进入 FrameEver。", "[打开 FrameEver]"],
        steps: ["选择", "确认", "转换", "完成"],
    });
}

#[test]
fn back_reaches_choose_and_a_picked_folder_goes_straight_to_review() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    assert_eq!(wizard.act(Action::Back).map(|_| ()), None);
    let choose = wizard.screen();
    assert_eq!(choose.steps[0].1, StepState::Current);
    shows(&choose, "Bring your recordings into FrameEver");
    shows(
        &choose,
        "Cap-aniBullet 2.x | 2 recordings · 1 screenshots · 1 KB",
    );
    assert_eq!(
        buttons(&choose),
        [(Action::PickFolder, true), (Action::Close, true)]
    );
    assert_eq!(choose.groups[0][0].action, Some(Action::Choose(0)));
    assert!(matches!(
        wizard.act(Action::PickFolder),
        Some(Effect::PickSource)
    ));

    let picked = machine.path("E/Old");
    std::fs::create_dir_all(&picked).unwrap();
    wizard.choose_folder(&picked);
    let empty = wizard.screen();
    let nothing = empty.empty.as_ref().expect("an empty state");
    assert_eq!(nothing.text, "There's nothing to convert in this folder.");
    assert_eq!(nothing.button.action, Action::PickFolder);
    assert_eq!(buttons(&empty), [(Action::Back, true)]);
    assert_eq!(wizard.primary(), None);

    studio_project(&picked.join("Kept.cap"));
    wizard.choose_folder(&picked);
    let review = wizard.screen();
    assert_eq!(review.subtitle, format!("From {}", picked.display()));
    assert!(
        texts(&review)
            .iter()
            .all(|text| !text.starts_with("Settings"))
    );
    shows(
        &review,
        "Files are moved to another drive: copied, checked, then the originals are deleted. Afterwards only FrameEver opens them.",
    );
}

#[test]
fn with_nothing_found_the_user_is_asked_for_a_folder() {
    let machine = Machine::empty();
    let mut wizard = machine.wizard(Language::English);
    let choose = wizard.screen();
    let empty = choose.empty.as_ref().expect("an empty state");
    assert_eq!(
        empty.text,
        "No Cap or Cap-aniBullet 2.x data on this PC. Choose the folder that holds your recordings."
    );
    assert_eq!(empty.button.action, Action::PickFolder);
    assert_eq!(wizard.primary(), Some(Action::PickFolder));
    assert!(matches!(wizard.act(Action::Close), Some(Effect::Quit)));
}

#[test]
fn unticking_a_kind_leaves_those_files_where_they_were() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    wizard.act(Action::Toggle(Kind::Screenshots));
    wizard.act(Action::Toggle(Kind::Settings));
    let review = wizard.screen();
    shows(&review, "[Convert 2 items]");
    let rows = &review.groups[0];
    assert_eq!(rows[1].check, Some((Kind::Screenshots, false)));
    assert!(rows[1].off);

    let started = job(wizard.act(Action::Convert));
    run(&mut wizard, started);

    assert!(
        machine
            .path("Videos/Cap Recordings/exports/screenshot/Shot.png")
            .is_file()
    );
    assert!(!machine.path("Videos/FrameEver/Screenshots").exists());
    assert!(!machine.target().settings_file.exists());
    shows(&wizard.screen(), "2 items are now in FrameEver.");
}

#[test]
fn nothing_ticked_keeps_convert_off() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    for kind in [Kind::Recordings, Kind::Screenshots, Kind::Settings] {
        wizard.act(Action::Toggle(kind));
    }
    assert_eq!(buttons(&wizard.screen())[1], (Action::Convert, false));
    assert!(wizard.act(Action::Convert).is_none());
    assert_eq!(wizard.primary(), None);
}

#[test]
fn not_enough_space_across_drives_is_caught_before_anything_moves() {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.free.lock().unwrap() = Some(10);
    let mut wizard = machine.wizard(Language::English);
    assert!(matches!(
        wizard.act(Action::Change),
        Some(Effect::PickDestination)
    ));
    std::fs::create_dir_all(machine.path("E")).unwrap();
    let elsewhere = machine.path("E/FrameEver");
    wizard.change_destination(&elsewhere);

    let review = wizard.screen();
    assert_eq!(
        review.warnings[0].text,
        "Not enough space on E:. This needs 1 KB and 1 KB is free. Change the folder or convert less."
    );
    assert_eq!(buttons(&review)[1], (Action::Convert, false));
    assert!(wizard.act(Action::Convert).is_none());
    assert!(
        machine
            .path("Videos/Cap Recordings/recordings/Studio.cap")
            .is_dir()
    );
    let destination = &review.groups[1][0];
    assert_eq!(
        destination.path.as_deref(),
        Some(elsewhere.display().to_string().as_str())
    );

    *machine.world.free.lock().unwrap() = Some(1 << 30);
    wizard.change_destination(&elsewhere);
    assert!(wizard.screen().warnings.is_empty());
    let started = job(wizard.act(Action::Convert));
    run(&mut wizard, started);
    assert!(elsewhere.join("Recordings/Studio.ever").is_dir());
}

#[test]
fn space_is_not_checked_on_the_same_drive() {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.free.lock().unwrap() = Some(0);
    let wizard = machine.wizard(Language::English);
    assert!(wizard.screen().warnings.is_empty());
    assert_eq!(wizard.primary(), Some(Action::Convert));
}

#[test]
fn a_running_old_app_or_frameever_is_a_warning_that_does_not_block() {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.running.lock().unwrap() =
        vec!["cap-anibullet.exe".into(), "FrameEver.exe".into()];
    let mut wizard = machine.wizard(Language::English);
    let review = wizard.screen();
    let warnings: Vec<_> = review.warnings.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(
        warnings,
        [
            "Cap-aniBullet 2.x is running. Close it first, or the projects it has open stay behind.",
            "FrameEver is running. Close it first, or it may overwrite the automations brought over.",
        ]
    );
    assert_eq!(
        review.warnings[0].button.as_ref().map(|b| b.action),
        Some(Action::Recheck)
    );
    assert_eq!(wizard.primary(), Some(Action::Convert));

    wizard.act(Action::Toggle(Kind::Settings));
    assert_eq!(wizard.screen().warnings.len(), 1);

    machine.world.running.lock().unwrap().clear();
    assert_eq!(wizard.screen().warnings.len(), 1);
    wizard.act(Action::Recheck);
    assert!(wizard.screen().warnings.is_empty());
}

#[test]
fn stop_and_the_close_button_finish_the_current_item_and_convert_the_rest_moves_only_what_is_left()
{
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    let first = job(wizard.act(Action::Convert));
    let mut seen = Vec::new();
    let report = first.run(|progress| {
        seen.push(progress.clone());
        if progress.done == 0 {
            assert!(wizard.act(Action::Close).is_none());
        }
    });
    let stopping = wizard.screen();
    assert_eq!(buttons(&stopping), [(Action::Stop, false)]);
    wizard.progress(seen[0].clone());
    assert_eq!(wizard.screen().meter.unwrap().line, "Finishing this item…");
    wizard.finish(report);

    let stopped = wizard.screen();
    assert_eq!(stopped.title, "Conversion stopped");
    assert_eq!(
        stopped.subtitle,
        "1 of 3 items are now in FrameEver. The rest are where they were."
    );
    assert!(!stopped.tick);
    assert_eq!(
        buttons(&stopped),
        [
            (Action::ShowFolder, true),
            (Action::Close, true),
            (Action::ConvertRest, true)
        ]
    );
    assert!(
        machine
            .path("Videos/Cap Recordings/exports/video/Clip.mp4")
            .is_file()
    );
    assert!(
        !machine
            .path("Videos/FrameEver/Recordings/Clip.ever")
            .exists()
    );

    let started = job(wizard.act(Action::ConvertRest));
    run(&mut wizard, started);
    let done = wizard.screen();
    assert_eq!(done.subtitle, "2 items are now in FrameEver.");
    assert!(
        machine
            .path("Videos/FrameEver/Recordings/Clip.ever")
            .is_dir()
    );
}

#[test]
fn left_behind_items_show_their_reason_and_details_and_try_again_converts_what_is_left() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    let _started = job(wizard.act(Action::Convert));
    let busy = machine.path("Videos/Cap Recordings/exports/video/Clip.mp4");
    let broken = machine.path("Videos/Cap Recordings/exports/screenshot/Shot.png");
    wizard.finish(Report {
        converted: vec![Converted {
            source: machine.path("Videos/Cap Recordings/recordings/Studio.cap"),
            project: machine.path("Videos/FrameEver/Recordings/Studio.ever"),
        }],
        skipped: vec![
            Skipped {
                source: busy,
                kind: ProjectKind::Recording,
                reason: SkipReason::InUse,
            },
            Skipped {
                source: broken,
                kind: ProjectKind::Screenshot,
                reason: SkipReason::Failed("disk on fire".into()),
            },
        ],
        settings_error: Some("Access is denied. (os error 5)".into()),
        stopped: false,
        left: 0,
    });

    let done = wizard.screen();
    assert_eq!(done.title, "Some items were left behind");
    assert_eq!(done.subtitle, "1 of 3 items are now in FrameEver.");
    assert!(!done.tick);
    assert_eq!(
        done.warnings[0].text,
        "Couldn't carry over your settings. Your recordings aren't affected."
    );
    shows(
        &done,
        "Clip.mp4 | In use by another program. Close it and try again.",
    );
    shows(
        &done,
        "Shot.png | Couldn't move this screenshot. It's still where it was.",
    );
    let row = &done.groups[0][1];
    let details = row.details.as_ref().expect("raw error behind Details");
    assert_eq!(details.raw, "disk on fire");
    assert!(!details.open);
    assert!(done.groups[0][0].details.is_none());
    wizard.act(details.toggle);
    let opened = wizard.screen();
    assert!(opened.groups[0][1].details.as_ref().unwrap().open);
    assert!(!opened.warnings[0].details.as_ref().unwrap().open);
    wizard.act(opened.warnings[0].details.as_ref().unwrap().toggle);
    wizard.act(details.toggle);
    let swapped = wizard.screen();
    assert!(!swapped.groups[0][1].details.as_ref().unwrap().open);
    assert!(swapped.warnings[0].details.as_ref().unwrap().open);
    assert_eq!(
        buttons(&done),
        [
            (Action::ShowFolder, true),
            (Action::Close, true),
            (Action::TryAgain, true)
        ]
    );

    let started = job(wizard.act(Action::TryAgain));
    run(&mut wizard, started);
    let again = wizard.screen();
    assert_eq!(again.title, "Conversion finished");
    assert_eq!(again.subtitle, "3 items are now in FrameEver.");
}

#[test]
fn convert_another_shows_only_while_another_install_remains_and_show_folder_stands_in_for_a_missing_frameever()
 {
    let machine = Machine::with_cap_anibullet_2();
    machine.add_cap();
    let mut wizard = machine.wizard(Language::English);
    assert!(wizard.screen().steps[0].1 == StepState::Current);
    wizard.act(Action::Choose(0));
    let started = job(wizard.act(Action::Convert));
    run(&mut wizard, started);
    let first = wizard.screen();
    assert_eq!(
        buttons(&first),
        [
            (Action::ConvertAnother, true),
            (Action::Close, true),
            (Action::ShowFolder, true)
        ]
    );

    wizard.act(Action::ConvertAnother);
    let choose = wizard.screen();
    assert_eq!(choose.groups[0][0].trailing, "Converted");
    wizard.act(Action::Choose(1));
    let started = job(wizard.act(Action::Convert));
    run(&mut wizard, started);
    assert_eq!(
        buttons(&wizard.screen()),
        [(Action::Close, true), (Action::ShowFolder, true)]
    );
}

#[test]
fn convert_another_also_shows_when_items_were_left_behind() {
    let machine = Machine::with_cap_anibullet_2();
    machine.add_cap();
    let mut wizard = machine.wizard(Language::English);
    wizard.act(Action::Choose(0));
    let _started = job(wizard.act(Action::Convert));
    wizard.finish(Report {
        skipped: vec![Skipped {
            source: machine.path("Videos/Cap Recordings/exports/video/Clip.mp4"),
            kind: ProjectKind::Recording,
            reason: SkipReason::InUse,
        }],
        ..Report::default()
    });
    assert_eq!(
        buttons(&wizard.screen()),
        [
            (Action::ShowFolder, true),
            (Action::ConvertAnother, true),
            (Action::Close, true),
            (Action::TryAgain, true)
        ]
    );
}

#[test]
fn a_storage_folder_that_cannot_be_saved_is_left_behind_and_not_finished() {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.free.lock().unwrap() = Some(1 << 30);
    let mut wizard = machine.wizard(Language::English);
    std::fs::create_dir_all(machine.path("E")).unwrap();
    wizard.change_destination(&machine.path("E/FrameEver"));
    machine.write(
        "AppData/Roaming/io.github.anibullet.frameever",
        b"a file, not a folder",
    );

    let started = job(wizard.act(Action::Convert));
    run(&mut wizard, started);

    let done = wizard.screen();
    assert_eq!(done.title, "Some items were left behind");
    assert_eq!(done.subtitle, "0 of 3 items are now in FrameEver.");
    assert!(!done.tick);
    assert_eq!(wizard.primary(), Some(Action::TryAgain));
    shows(
        &done,
        "Studio.cap | Couldn't move this recording. It's still where it was.",
    );
}

#[test]
fn an_unknown_free_space_across_drives_is_said_not_passed_silently() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    std::fs::create_dir_all(machine.path("E")).unwrap();
    wizard.change_destination(&machine.path("E/FrameEver"));

    let review = wizard.screen();
    assert_eq!(
        review.warnings[0].text,
        "Couldn't check the free space on E:. This needs 1 KB."
    );
    assert_eq!(wizard.primary(), Some(Action::Convert));
}

#[test]
fn convert_the_rest_skips_settings_already_carried_and_checks_space_again() {
    let machine = Machine::with_cap_anibullet_2();
    *machine.world.free.lock().unwrap() = Some(1 << 30);
    let mut wizard = machine.wizard(Language::English);
    std::fs::create_dir_all(machine.path("E")).unwrap();
    let elsewhere = machine.path("E/FrameEver");
    wizard.change_destination(&elsewhere);
    let first = job(wizard.act(Action::Convert));
    let report = first.run(|progress| {
        if progress.done == 0 {
            wizard.act(Action::Stop);
        }
    });
    wizard.finish(report);
    assert_eq!(wizard.screen().title, "Conversion stopped");

    let settings_file = machine.target().settings_file;
    let mut settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    assert!(
        settings
            .as_object_mut()
            .unwrap()
            .remove("automations")
            .is_some()
    );
    std::fs::write(&settings_file, settings.to_string()).unwrap();

    *machine.world.free.lock().unwrap() = Some(10);
    assert!(wizard.act(Action::ConvertRest).is_none());
    let review = wizard.screen();
    assert_eq!(review.title, "Choose what comes over");
    assert_eq!(
        review.warnings[0].text,
        "Not enough space on E:. This needs 1 KB and 1 KB is free. Change the folder or convert less."
    );
    shows(&review, "[Convert 2 items]");
    assert!(
        review.groups[0]
            .iter()
            .any(|row| row.check == Some((Kind::Settings, false)))
    );

    *machine.world.free.lock().unwrap() = Some(1 << 30);
    wizard.change_destination(&elsewhere);
    let rest = job(wizard.act(Action::Convert));
    run(&mut wizard, rest);
    assert_eq!(wizard.screen().subtitle, "2 items are now in FrameEver.");
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    assert!(settings.get("automations").is_none());
    assert!(elsewhere.join("Recordings/Clip.ever").is_dir());
}

#[test]
fn try_again_with_enough_space_runs_only_what_is_left_without_the_settings() {
    let machine = Machine::with_cap_anibullet_2();
    let mut wizard = machine.wizard(Language::English);
    let first = job(wizard.act(Action::Convert));
    let report = first.run(|progress| {
        if progress.done == 0 {
            wizard.act(Action::Stop);
        }
    });
    wizard.finish(report);
    let settings_file = machine.target().settings_file;
    let mut settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    settings.as_object_mut().unwrap().remove("automations");
    std::fs::write(&settings_file, settings.to_string()).unwrap();

    let rest = job(wizard.act(Action::ConvertRest));
    run(&mut wizard, rest);
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_file).unwrap()).unwrap();
    assert!(settings.get("automations").is_none());
    assert_eq!(wizard.screen().subtitle, "2 items are now in FrameEver.");
}
