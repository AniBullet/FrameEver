use super::*;
use frameever_automation::{AutomationRule, MatchMode};

fn rule(id: &str, trigger: Trigger, enabled: bool, actions: Vec<Action>) -> AutomationRule {
    AutomationRule {
        id: id.to_string(),
        name: id.to_string(),
        enabled,
        trigger,
        match_mode: MatchMode::All,
        conditions: Vec::new(),
        actions,
    }
}

fn store(rules: Vec<AutomationRule>) -> AutomationsStore {
    AutomationsStore { version: 1, rules }
}

fn export_action() -> Action {
    Action::Export {
        profile: ExportProfile {
            format: ExportFormat::Mp4,
            fps: 30,
            resolution_base: frameever_project::XY { x: 1920, y: 1080 },
            compression: Some(AutomationExportCompression::Web),
            preset_name: None,
        },
        destination: ExportDestination::ProjectFolder,
    }
}

fn save_action(dir: &Path) -> Action {
    Action::SaveToLocation {
        dir: dir.to_string_lossy().to_string(),
        filename_template: None,
    }
}

const PLAYABLE_MP4: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/video-decode/tests/fixtures/h264-no-reordering.mp4"
));

fn succeeded(action: &Action) -> Option<PrecedingAction<'_>> {
    Some(PrecedingAction {
        action,
        succeeded: true,
    })
}

#[test]
fn rules_saved_by_the_settings_page_keep_their_enabled_state() {
    let dir = tempfile::tempdir().unwrap();
    let store_path = dir.path().join("store");
    std::fs::write(&store_path, "{}").unwrap();
    crate::store::set_test_tauri_store_path(Some(store_path));
    let saved: crate::store::AutomationsStore = serde_json::from_value(serde_json::json!({
        "version": 1,
        "rules": [
            { "id": "off", "name": "Off", "enabled": false, "trigger": "studioRecordingFinished",
              "actions": [{ "type": "deleteLocalFiles" }] },
            { "id": "on", "name": "On", "trigger": "screenshotTaken",
              "actions": [{ "type": "copyToClipboard", "source": "raw" }] }
        ]
    }))
    .unwrap();
    assert!(crate::store::set_automations(&saved));

    let parsed = parse_store(crate::store::automations_value().unwrap()).unwrap();
    crate::store::set_test_tauri_store_path(None);

    assert_eq!(parsed.rules.len(), 2);
    assert!(!parsed.rules[0].enabled);
    assert!(parsed.rules[1].enabled);
    assert!(!has_enabled_rule(&parsed, Trigger::StudioRecordingFinished));
    assert!(has_enabled_rule(&parsed, Trigger::ScreenshotTaken));
    assert!(
        frameever_automation::evaluate(
            &parsed,
            &Trigger::StudioRecordingFinished,
            &TriggerContext::new()
        )
        .is_empty()
    );
}

#[test]
fn rules_saved_by_the_previous_app_still_run() {
    let saved = serde_json::json!({
        "version": 1,
        "rules": [
            {
                "id": "6f0c5f0e-1d6b-4d0e-9a39-0d6f5d1f2a10",
                "name": "Compress and clean up",
                "enabled": true,
                "trigger": "studioRecordingFinished",
                "matchMode": "all",
                "conditions": [{ "type": "durationAtLeast", "secs": 2 }],
                "actions": [
                    {
                        "type": "export",
                        "profile": {
                            "format": "mp4",
                            "fps": 30,
                            "resolutionBase": { "x": 1280, "y": 720 },
                            "compression": "potato",
                            "presetName": null
                        },
                        "destination": { "customPath": { "dir": "/Users/someone/Compressed" } }
                    },
                    { "type": "deleteLocalFiles" }
                ]
            },
            {
                "id": "b8e1",
                "name": "",
                "enabled": false,
                "trigger": "screenshotTaken",
                "matchMode": "any",
                "conditions": [
                    { "type": "captureTargetIs", "target": "window" },
                    { "type": "windowTitleContains", "pattern": "Figma" }
                ],
                "actions": [
                    { "type": "saveToLocation", "dir": "/Users/someone/Shots", "filenameTemplate": "{date}-{window}" },
                    { "type": "upload", "organizationId": null, "copyLink": true, "openInBrowser": false },
                    { "type": "webhook", "url": "https://example.com/hook", "method": "POST", "headers": {}, "bodyTemplate": null },
                    { "type": "runCommand", "program": "echo", "args": ["hi"], "cwd": null, "env": {}, "useShell": false },
                    { "type": "notify", "titleTemplate": "FrameEver", "bodyTemplate": "" },
                    { "type": "copyToClipboard", "source": "rendered" },
                    { "type": "applyPreset", "name": "Square" },
                    { "type": "revealInFileManager" }, { "type": "openFile" }, { "type": "openEditor" },
                    { "type": "skipEditor" }, { "type": "recognizeTextToClipboard" }
                ]
            }
        ]
    });
    let parsed = parse_store(saved).unwrap();
    assert!(has_enabled_rule(&parsed, Trigger::StudioRecordingFinished));
    assert!(!has_enabled_rule(&parsed, Trigger::ScreenshotTaken));

    let ctx = TriggerContext::new()
        .with_project_path(PathBuf::from("/r/a.cap"))
        .with_recording_mode(AutomationRecordingMode::Studio)
        .with_duration(5.0);
    let matched = frameever_automation::evaluate(&parsed, &Trigger::StudioRecordingFinished, &ctx);
    assert_eq!(matched.len(), 1);
    let actions = &matched[0].1;
    match &actions[0] {
        Action::Export {
            profile,
            destination,
        } => {
            assert_eq!(profile.resolution_base.x, 1280);
            assert_eq!(
                profile.compression,
                Some(AutomationExportCompression::Potato)
            );
            assert_eq!(
                destination,
                &ExportDestination::CustomPath {
                    dir: "/Users/someone/Compressed".into()
                }
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(actions[1], Action::DeleteLocalFiles));
    assert!(
        frameever_automation::evaluate(
            &parsed,
            &Trigger::StudioRecordingFinished,
            &TriggerContext::new().with_duration(1.0)
        )
        .is_empty()
    );
    assert_eq!(parsed.rules[1].actions.len(), 10);

    assert!(parse_store(serde_json::json!({ "rules": "not a list" })).is_none());
}

#[test]
fn rules_that_upload_or_call_a_webhook_lose_those_actions_when_loaded() {
    let parsed = parse_store(serde_json::json!({
        "version": 1,
        "rules": [
            {
                "id": "uploaded",
                "name": "Upload then clean up",
                "trigger": "studioRecordingFinished",
                "actions": [
                    { "type": "upload", "organizationId": null, "copyLink": true, "openInBrowser": false },
                    { "type": "webhook", "url": "https://example.com/hook", "method": "POST", "headers": {}, "bodyTemplate": null },
                    { "type": "deleteLocalFiles" },
                    { "type": "openFile" }
                ]
            },
            { "id": "retired", "name": "After upload", "trigger": "uploadCompleted", "actions": [{ "type": "deleteLocalFiles" }] }
        ]
    }))
    .unwrap();
    assert_eq!(parsed.rules.len(), 1);
    assert_eq!(parsed.rules[0].actions.len(), 1);
    assert!(matches!(parsed.rules[0].actions[0], Action::OpenFile));
}

#[test]
fn no_rules_means_nothing_listens() {
    let empty = AutomationsStore::default();
    for trigger in [
        Trigger::ScreenshotTaken,
        Trigger::StudioRecordingFinished,
        Trigger::InstantRecordingFinished,
        Trigger::RecordingStarted,
        Trigger::VideoImported,
        Trigger::RecordingDeleted,
    ] {
        assert!(!has_enabled_rule(&empty, trigger));
    }
    let other = store(vec![rule(
        "a",
        Trigger::ScreenshotTaken,
        true,
        vec![Action::OpenFile],
    )]);
    assert!(!has_enabled_rule(&other, Trigger::StudioRecordingFinished));
}

#[test]
fn an_event_is_held_only_while_its_rules_run() {
    let events: &'static RunningEvents = Box::leak(Box::new(RunningEvents::new()));
    let shot = event_key(Path::new("/shots/a.cap/original.png"));

    let first = events
        .claim(Trigger::ScreenshotTaken, shot.clone())
        .unwrap();
    assert!(
        events
            .claim(Trigger::ScreenshotTaken, shot.clone())
            .is_none()
    );
    let other_trigger = events
        .claim(Trigger::StudioRecordingFinished, shot.clone())
        .unwrap();
    let other_path = events
        .claim(Trigger::ScreenshotTaken, "/shots/b.cap/original.png".into())
        .unwrap();

    drop(first);
    let reused_path = events.claim(Trigger::ScreenshotTaken, shot.clone());
    assert!(reused_path.is_some());
    drop((reused_path, other_trigger, other_path));
    assert!(events.lock().is_empty());
}

#[test]
fn skip_editor_and_open_editor_rules_override_the_setting() {
    use crate::store::PostStudioBehaviour;
    let project = Path::new("/recordings/a.cap");
    let skip = store(vec![rule(
        "skip",
        Trigger::StudioRecordingFinished,
        true,
        vec![Action::SkipEditor],
    )]);
    assert_eq!(
        studio_editor_behaviour_for(
            &skip,
            project,
            Duration::from_secs(4),
            PostStudioBehaviour::OpenEditor
        ),
        PostStudioBehaviour::DoNothing
    );

    let open = store(vec![rule(
        "open",
        Trigger::StudioRecordingFinished,
        true,
        vec![Action::OpenEditor],
    )]);
    assert_eq!(
        studio_editor_behaviour_for(
            &open,
            project,
            Duration::from_secs(4),
            PostStudioBehaviour::ShowOverlay
        ),
        PostStudioBehaviour::OpenEditor
    );

    let mut disabled = skip.clone();
    disabled.rules[0].enabled = false;
    assert_eq!(
        studio_editor_behaviour_for(
            &disabled,
            project,
            Duration::from_secs(4),
            PostStudioBehaviour::OpenEditor
        ),
        PostStudioBehaviour::OpenEditor
    );

    let mut long_only = skip;
    long_only.rules[0].conditions = vec![Condition::DurationAtLeast { secs: 60.0 }];
    assert_eq!(
        studio_editor_behaviour_for(
            &long_only,
            project,
            Duration::from_secs(4),
            PostStudioBehaviour::OpenEditor
        ),
        PostStudioBehaviour::OpenEditor
    );
    assert_eq!(
        studio_editor_behaviour_for(
            &long_only,
            project,
            Duration::from_secs(90),
            PostStudioBehaviour::OpenEditor
        ),
        PostStudioBehaviour::DoNothing
    );
}

#[test]
fn an_instant_recording_a_rule_deletes_gets_no_card_and_no_copy() {
    use crate::store::PostInstantBehaviour;
    let project = Path::new("/recordings/a.cap");
    let deleting = store(vec![rule(
        "delete",
        Trigger::InstantRecordingFinished,
        true,
        vec![
            Action::SaveToLocation {
                dir: "/saved".into(),
                filename_template: None,
            },
            Action::DeleteLocalFiles,
        ],
    )]);
    for setting in [
        PostInstantBehaviour::ShowCard,
        PostInstantBehaviour::CopyFile,
    ] {
        assert_eq!(
            instant_behaviour_for(&deleting, project, setting),
            PostInstantBehaviour::DoNothing
        );
    }
    let keeping = store(vec![rule(
        "save",
        Trigger::InstantRecordingFinished,
        true,
        vec![Action::SaveToLocation {
            dir: "/saved".into(),
            filename_template: None,
        }],
    )]);
    assert_eq!(
        instant_behaviour_for(&keeping, project, PostInstantBehaviour::ShowCard),
        PostInstantBehaviour::ShowCard
    );
}

#[test]
fn the_studio_run_sees_the_duration_the_editor_choice_saw() {
    let project = Path::new("/recordings/a.cap");
    let context = studio_context(project, Duration::from_secs(4));
    let short_only = store(vec![AutomationRule {
        conditions: vec![Condition::DurationAtMost { secs: 5.0 }],
        ..rule(
            "short",
            Trigger::StudioRecordingFinished,
            true,
            vec![Action::SkipEditor],
        )
    }]);
    assert_eq!(
        frameever_automation::evaluate(&short_only, &Trigger::StudioRecordingFinished, &context)
            .len(),
        1
    );
    assert_eq!(
        studio_editor_behaviour_for(
            &short_only,
            project,
            Duration::from_secs(4),
            crate::store::PostStudioBehaviour::OpenEditor
        ),
        crate::store::PostStudioBehaviour::DoNothing
    );
}

#[test]
fn rules_choose_the_screenshot_editor_and_skip_wins() {
    let ctx = TriggerContext::new().with_capture_target(CaptureTargetKind::Window);
    let open = rule(
        "open",
        Trigger::ScreenshotTaken,
        true,
        vec![Action::OpenEditor],
    );
    let skip = rule(
        "skip",
        Trigger::ScreenshotTaken,
        true,
        vec![Action::SkipEditor],
    );
    let mut disabled_skip = skip.clone();
    disabled_skip.enabled = false;

    assert_eq!(
        editor_choice(&store(vec![]), Trigger::ScreenshotTaken, &ctx),
        None
    );
    assert_eq!(
        editor_choice(&store(vec![open.clone()]), Trigger::ScreenshotTaken, &ctx),
        Some(true)
    );
    assert_eq!(
        editor_choice(
            &store(vec![open.clone(), skip]),
            Trigger::ScreenshotTaken,
            &ctx
        ),
        Some(false)
    );
    assert_eq!(
        editor_choice(
            &store(vec![open.clone(), disabled_skip]),
            Trigger::ScreenshotTaken,
            &ctx
        ),
        Some(true)
    );
    let studio = |actions: Vec<Action>| {
        editor_choice(
            &store(vec![rule(
                "r",
                Trigger::StudioRecordingFinished,
                true,
                actions,
            )]),
            Trigger::StudioRecordingFinished,
            &TriggerContext::new(),
        )
    };
    let compress = Action::Export {
        profile: ExportProfile {
            format: ExportFormat::Mp4,
            fps: 30,
            resolution_base: frameever_project::XY { x: 1280, y: 720 },
            compression: Some(AutomationExportCompression::Potato),
            preset_name: None,
        },
        destination: ExportDestination::CustomPath {
            dir: "/exports".into(),
        },
    };
    assert_eq!(
        studio(vec![compress, Action::DeleteLocalFiles]),
        Some(false)
    );
    assert_eq!(
        studio(vec![
            save_action(Path::new("/shots")),
            Action::DeleteLocalFiles
        ]),
        Some(false)
    );
    assert_eq!(studio(vec![Action::DeleteLocalFiles]), None);
    assert_eq!(
        studio(vec![export_action(), Action::DeleteLocalFiles]),
        None
    );
    assert_eq!(
        studio(vec![Action::DeleteLocalFiles, Action::OpenEditor]),
        Some(true)
    );
}

#[test]
fn only_a_screenshot_rule_that_can_delete_drops_the_overlay_card() {
    let ctx = TriggerContext::new().with_capture_target(CaptureTargetKind::Window);
    let deletes =
        |rules: Vec<AutomationRule>| rules_delete(&store(rules), Trigger::ScreenshotTaken, &ctx);
    let shot = |id: &str, enabled: bool, actions: Vec<Action>| {
        rule(id, Trigger::ScreenshotTaken, enabled, actions)
    };
    let save_then_delete = vec![save_action(Path::new("/shots")), Action::DeleteLocalFiles];

    assert!(!deletes(vec![]));
    assert!(deletes(vec![shot("save", true, save_then_delete.clone())]));
    assert!(!deletes(vec![shot("off", false, save_then_delete.clone())]));
    assert!(!deletes(vec![shot(
        "lone",
        true,
        vec![Action::DeleteLocalFiles]
    )]));
    assert!(!deletes(vec![shot("open", true, vec![Action::OpenEditor])]));
    assert!(!deletes(vec![rule(
        "studio",
        Trigger::StudioRecordingFinished,
        true,
        save_then_delete,
    )]));
}

#[tokio::test]
async fn open_editor_leaves_an_editor_choice_already_made_alone() {
    for trigger in [Trigger::StudioRecordingFinished, Trigger::ScreenshotTaken] {
        let host = GpuiAutomationHost::for_trigger(trigger);
        assert_eq!(
            host.open_editor(&TriggerContext::new().with_project_path("/r/a.cap".into()))
                .await,
            Ok(())
        );
    }
    let gone = tempfile::tempdir().unwrap().path().join("deleted.cap");
    let host = GpuiAutomationHost::for_trigger(Trigger::RecordingDeleted);
    assert_eq!(
        host.open_editor(&TriggerContext::new().with_project_path(gone))
            .await,
        Err("The recording no longer exists".to_string())
    );
}

#[test]
fn delete_is_refused_without_a_successful_verified_action_before_it() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("rec.cap");
    std::fs::create_dir(&project).unwrap();
    let outside = root.path().join("rec.mp4");
    std::fs::write(&outside, PLAYABLE_MP4).unwrap();
    let export = export_action();

    assert!(verify_before_delete(&project, None, Some(&outside), None, None).is_err());

    let notify = Action::Notify {
        title_template: "t".into(),
        body_template: "b".into(),
    };
    let error =
        verify_before_delete(&project, succeeded(&notify), Some(&outside), None, None).unwrap_err();
    assert!(error.contains("notify"), "{error}");

    let failed = Some(PrecedingAction {
        action: &export,
        succeeded: false,
    });
    let error = verify_before_delete(&project, failed, Some(&outside), None, None).unwrap_err();
    assert!(error.contains("did not succeed"), "{error}");

    assert!(verify_before_delete(&project, succeeded(&export), None, None, None).is_err());

    assert_eq!(
        verify_before_delete(&project, succeeded(&export), Some(&outside), None, None),
        Ok(())
    );
    let save = save_action(root.path());
    let written = file_fingerprint(&outside).unwrap();
    assert_eq!(
        verify_before_delete(
            &project,
            succeeded(&save),
            Some(&outside),
            Some(&outside),
            Some(&written)
        ),
        Ok(())
    );
}

#[test]
fn delete_is_refused_when_the_export_is_not_playable_or_the_copy_is_short() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("rec.cap");
    std::fs::create_dir(&project).unwrap();
    let export = export_action();

    let truncated = root.path().join("truncated.mp4");
    std::fs::write(&truncated, &PLAYABLE_MP4[..24]).unwrap();
    let error = verify_before_delete(&project, succeeded(&export), Some(&truncated), None, None)
        .unwrap_err();
    assert!(error.contains("not a playable video"), "{error}");

    let text = root.path().join("notes.mp4");
    std::fs::write(&text, b"not a video at all").unwrap();
    let error =
        verify_before_delete(&project, succeeded(&export), Some(&text), None, None).unwrap_err();
    assert!(error.contains("not a playable video"), "{error}");

    let source = root.path().join("original.png");
    std::fs::write(&source, b"0123456789").unwrap();
    let short = root.path().join("copy.png");
    std::fs::write(&short, b"01234").unwrap();
    let save = save_action(root.path());
    let written = file_fingerprint(&short).unwrap();
    let error = verify_before_delete(
        &project,
        succeeded(&save),
        Some(&short),
        Some(&source),
        Some(&written),
    )
    .unwrap_err();
    assert!(error.contains("5 bytes but the original is 10"), "{error}");
    assert!(
        verify_before_delete(
            &project,
            succeeded(&save),
            Some(&short),
            None,
            Some(&written)
        )
        .is_err()
    );
}

#[test]
fn delete_is_refused_when_the_output_is_missing_empty_or_inside_the_recording() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("rec.cap");
    std::fs::create_dir_all(project.join("output")).unwrap();
    let export = export_action();

    let missing = root.path().join("missing.mp4");
    let error =
        verify_before_delete(&project, succeeded(&export), Some(&missing), None, None).unwrap_err();
    assert!(error.contains("could not be checked"), "{error}");

    let empty = root.path().join("empty.mp4");
    std::fs::write(&empty, b"").unwrap();
    let error =
        verify_before_delete(&project, succeeded(&export), Some(&empty), None, None).unwrap_err();
    assert!(error.contains("missing or empty"), "{error}");

    let a_directory = root.path().join("folder.mp4");
    std::fs::create_dir(&a_directory).unwrap();
    assert!(
        verify_before_delete(&project, succeeded(&export), Some(&a_directory), None, None).is_err()
    );

    let inside = project.join("output").join("result.mp4");
    std::fs::write(&inside, PLAYABLE_MP4).unwrap();
    let error =
        verify_before_delete(&project, succeeded(&export), Some(&inside), None, None).unwrap_err();
    assert!(error.contains("inside the folder"), "{error}");
}

fn recording_project(root: &Path) -> PathBuf {
    let project = root.join("rec.ever");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("recording-meta.json"), b"{}").unwrap();
    project
}

fn on_the_media_stack(test: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(crate::MEDIA_THREAD_STACK_SIZE)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(test)
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_failed_export_keeps_the_recording() {
    on_the_media_stack(a_failed_export_keeps_the_recording_body());
}

async fn a_failed_export_keeps_the_recording_body() {
    let root = tempfile::tempdir().unwrap();
    let project = recording_project(root.path());
    let rules = store(vec![rule(
        "compress-then-delete",
        Trigger::StudioRecordingFinished,
        true,
        vec![export_action(), Action::DeleteLocalFiles],
    )]);
    let host = GpuiAutomationHost::default();
    let results = frameever_automation::run(
        &host,
        &rules,
        &Trigger::StudioRecordingFinished,
        &TriggerContext::new().with_project_path(project.clone()),
    )
    .await;
    let actions = &results[0].action_results;
    assert!(!actions[0].success);
    assert!(!actions[1].success);
    assert!(
        actions[1]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("did not succeed")),
        "{:?}",
        actions[1].error
    );
    assert!(project.join("recording-meta.json").is_file());
}

#[tokio::test]
async fn save_to_location_copies_the_capture_and_is_what_delete_checks() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("shot.cap");
    std::fs::create_dir(&bundle).unwrap();
    let image = bundle.join("original.png");
    std::fs::write(&image, b"png bytes").unwrap();
    let destination = root.path().join("saved");
    let ctx = TriggerContext::new()
        .with_project_path(bundle.clone())
        .with_image_path(image.clone())
        .with_window_title("Notes/Plan: Q4".to_string());

    let host = GpuiAutomationHost::default();
    host.save_to_location(&ctx, &destination.to_string_lossy(), Some("{window}.png"))
        .await
        .unwrap();
    let saved = destination.join("Notes_Plan_ Q4.png");
    assert_eq!(std::fs::read(&saved).unwrap(), b"png bytes");
    assert_eq!(host.last_output().as_deref(), Some(saved.as_path()));
    let save = save_action(&destination);
    assert_eq!(
        verify_before_delete(
            &bundle,
            succeeded(&save),
            host.last_output().as_deref(),
            Some(&image),
            host.output().and_then(|output| output.written).as_ref(),
        ),
        Ok(())
    );

    let into_bundle = GpuiAutomationHost::default();
    into_bundle
        .save_to_location(&ctx, &bundle.join("copies").to_string_lossy(), None)
        .await
        .unwrap();
    assert!(
        verify_before_delete(
            &bundle,
            succeeded(&save),
            into_bundle.last_output().as_deref(),
            Some(&image),
            into_bundle
                .output()
                .and_then(|output| output.written)
                .as_ref(),
        )
        .is_err()
    );

    let no_source = GpuiAutomationHost::default();
    assert!(
        no_source
            .save_to_location(
                &TriggerContext::new().with_project_path(bundle),
                &destination.to_string_lossy(),
                None
            )
            .await
            .is_err()
    );
    assert_eq!(no_source.last_output(), None);
}

#[tokio::test]
async fn saving_onto_the_capture_itself_never_truncates_it() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("shot.cap");
    std::fs::create_dir(&bundle).unwrap();
    let image = bundle.join("original.png");
    std::fs::write(&image, b"png bytes").unwrap();
    let ctx = TriggerContext::new()
        .with_project_path(bundle.clone())
        .with_image_path(image.clone());
    let host = GpuiAutomationHost::default();

    host.save_to_location(&ctx, &bundle.to_string_lossy(), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&image).unwrap(), b"png bytes");

    let aliases = root.path().join("aliases");
    std::fs::create_dir(&aliases).unwrap();
    std::fs::hard_link(&image, aliases.join("original.png")).unwrap();
    host.save_to_location(&ctx, &aliases.to_string_lossy(), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&image).unwrap(), b"png bytes");

    #[cfg(unix)]
    {
        let links = root.path().join("links");
        std::fs::create_dir(&links).unwrap();
        std::os::unix::fs::symlink(&image, links.join("original.png")).unwrap();
        host.save_to_location(&ctx, &links.to_string_lossy(), None)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&image).unwrap(), b"png bytes");
        let save = save_action(&links);
        assert!(
            verify_before_delete(
                &bundle,
                succeeded(&save),
                host.last_output().as_deref(),
                Some(&image),
                host.output().and_then(|output| output.written).as_ref(),
            )
            .is_err(),
            "a link back into the capture is not a copy"
        );
    }
}

#[tokio::test]
async fn a_second_save_never_overwrites_the_first() {
    let root = tempfile::tempdir().unwrap();
    let saved = root.path().join("saved");
    let mut copies = Vec::new();
    for (index, bytes) in [b"first capture".as_slice(), b"second"]
        .into_iter()
        .enumerate()
    {
        let bundle = root.path().join(format!("shot-{index}.cap"));
        std::fs::create_dir(&bundle).unwrap();
        let image = bundle.join("original.png");
        std::fs::write(&image, bytes).unwrap();
        let ctx = TriggerContext::new()
            .with_project_path(bundle.clone())
            .with_image_path(image.clone());
        let host = GpuiAutomationHost::default();
        host.save_to_location(&ctx, &saved.to_string_lossy(), None)
            .await
            .unwrap();
        let save = save_action(&saved);
        assert_eq!(
            verify_before_delete(
                &bundle,
                succeeded(&save),
                host.last_output().as_deref(),
                Some(&image),
                host.output().and_then(|output| output.written).as_ref(),
            ),
            Ok(())
        );
        copies.push(host.last_output().unwrap());
    }
    assert_eq!(
        copies,
        vec![saved.join("original.png"), saved.join("original (1).png")]
    );
    assert_eq!(std::fs::read(&copies[0]).unwrap(), b"first capture");
    assert_eq!(std::fs::read(&copies[1]).unwrap(), b"second");
}

#[tokio::test]
async fn delete_is_refused_when_the_saved_copy_is_not_the_one_this_run_wrote() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("shot.cap");
    std::fs::create_dir(&bundle).unwrap();
    let image = bundle.join("original.png");
    std::fs::write(&image, b"png bytes").unwrap();
    let saved = root.path().join("saved");
    let ctx = TriggerContext::new()
        .with_project_path(bundle.clone())
        .with_image_path(image.clone());
    let save = save_action(&saved);
    let check = |host: &GpuiAutomationHost| {
        verify_before_delete(
            &bundle,
            succeeded(&save),
            host.last_output().as_deref(),
            Some(&image),
            host.output().and_then(|output| output.written).as_ref(),
        )
    };

    let replaced = GpuiAutomationHost::default();
    replaced
        .save_to_location(&ctx, &saved.to_string_lossy(), None)
        .await
        .unwrap();
    let copy = replaced.last_output().unwrap();
    let other = root.path().join("other.png");
    std::fs::write(&other, b"png bytes").unwrap();
    let copy_modified = std::fs::metadata(&copy).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(&other)
        .unwrap()
        .set_modified(copy_modified)
        .unwrap();
    std::fs::rename(&other, &copy).unwrap();
    let error = check(&replaced).unwrap_err();
    assert!(
        error.contains("changed after this rule saved it"),
        "{error}"
    );

    let edited = GpuiAutomationHost::default();
    edited
        .save_to_location(&ctx, &saved.to_string_lossy(), None)
        .await
        .unwrap();
    std::fs::write(edited.last_output().unwrap(), b"png bytes and more").unwrap();
    assert!(check(&edited).is_err());

    let unwritten = GpuiAutomationHost::default();
    unwritten.remember_output(copy);
    let error = check(&unwritten).unwrap_err();
    assert!(error.contains("not written by this rule"), "{error}");
}

#[tokio::test]
async fn unused_names_count_up_past_files_folders_and_links() {
    let root = tempfile::tempdir().unwrap();
    let wanted = root.path().join("clip.mp4");
    std::fs::write(&wanted, b"earlier export").unwrap();
    std::fs::create_dir(root.path().join("clip (1).mp4")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.path().join("nowhere"),
        root.path().join("clip (2).mp4"),
    )
    .unwrap();
    let reserved = reserve_unused_path(&wanted).await.unwrap();
    #[cfg(unix)]
    assert_eq!(reserved, root.path().join("clip (3).mp4"));
    #[cfg(not(unix))]
    assert_eq!(reserved, root.path().join("clip (2).mp4"));
    assert_eq!(std::fs::read(&wanted).unwrap(), b"earlier export");
    assert_eq!(std::fs::metadata(&reserved).unwrap().len(), 0);
    assert_eq!(
        numbered_path(Path::new("/saved/README"), 2),
        PathBuf::from("/saved/README (2)")
    );
}

#[tokio::test]
async fn instant_rules_hold_the_folder_by_its_resolved_path() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("recordings");
    let project = real.join("instant.ever");
    std::fs::create_dir_all(&project).unwrap();
    #[cfg(unix)]
    let recorded_to = {
        let link = root.path().join("linked-recordings");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        link.join("instant.ever")
    };
    #[cfg(not(unix))]
    let recorded_to = project.clone();
    let resolved = project.canonicalize().unwrap();
    assert_ne!(recorded_to, resolved);

    let hold = InstantRulesHold::take(&recorded_to);
    let waiter = tokio::spawn({
        let resolved = resolved.clone();
        async move { instant_rules_finished(&resolved).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!waiter.is_finished());
    drop(hold);
    tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap();
    assert!(!instant_rule_holds().is_held(&hold_key(&resolved)));
}

#[tokio::test]
async fn instant_rules_hold_the_folder_until_every_hold_is_released() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("instant.ever");
    assert!(!instant_rule_holds().is_held(&hold_key(&project)));
    instant_rules_finished(&project).await;

    let first = InstantRulesHold::take(&project);
    let second = InstantRulesHold::take(&project);
    let waiter = tokio::spawn({
        let project = project.clone();
        async move { instant_rules_finished(&project).await }
    });
    drop(first);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!waiter.is_finished());
    drop(second);
    tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap();
    assert!(!instant_rule_holds().is_held(&hold_key(&project)));
}

#[tokio::test]
async fn a_folder_left_blank_or_relative_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let image = root.path().join("original.png");
    std::fs::write(&image, b"png bytes").unwrap();
    let ctx = TriggerContext::new().with_image_path(image);
    let host = GpuiAutomationHost::default();
    for dir in ["", "  ", "relative/folder"] {
        assert_eq!(
            host.save_to_location(&ctx, dir, None).await,
            Err("Choose a folder for this action".to_string())
        );
        assert_eq!(host.last_output(), None);
    }

    let project = root.path().join("rec.cap");
    std::fs::create_dir(&project).unwrap();
    let error = host
        .export(
            &TriggerContext::new().with_project_path(project),
            &ExportProfile {
                format: ExportFormat::Mp4,
                fps: 30,
                resolution_base: frameever_project::XY { x: 1920, y: 1080 },
                compression: None,
                preset_name: None,
            },
            &ExportDestination::CustomPath {
                dir: "exports".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error, "Choose a folder for this action");
}

#[test]
fn export_destination_and_compression_match_the_previous_app() {
    let project = Path::new("/recordings/My Recording.cap");
    assert_eq!(
        export_output_path(
            project,
            ExportFormat::Mp4,
            &ExportDestination::ProjectFolder
        ),
        None
    );
    assert_eq!(
        export_output_path(
            project,
            ExportFormat::Gif,
            &ExportDestination::CustomPath {
                dir: "/exports".into()
            }
        ),
        Some(PathBuf::from("/exports/My Recording.gif"))
    );
    assert_eq!(
        export_output_path(
            project,
            ExportFormat::Mov,
            &ExportDestination::CustomPath {
                dir: "/exports".into()
            }
        ),
        Some(PathBuf::from("/exports/My Recording.mov"))
    );
    assert!(matches!(
        export_compression(None),
        frameever_export::mp4::ExportCompression::Web
    ));
    assert!(matches!(
        export_compression(Some(AutomationExportCompression::Potato)),
        frameever_export::mp4::ExportCompression::Potato
    ));
    assert!(matches!(
        export_compression(Some(AutomationExportCompression::Maximum)),
        frameever_export::mp4::ExportCompression::Maximum
    ));
    assert!(matches!(
        export_compression(Some(AutomationExportCompression::Social)),
        frameever_export::mp4::ExportCompression::Social
    ));
}

#[test]
fn clipboard_copies_the_capture_and_refuses_an_unrendered_screenshot() {
    let screenshot = TriggerContext::new()
        .with_project_path(PathBuf::from("shot.cap"))
        .with_image_path(PathBuf::from("shot.cap/original.png"));
    assert_eq!(
        clipboard_image(&screenshot, ClipboardSource::Raw),
        Ok(PathBuf::from("shot.cap/original.png"))
    );
    assert!(clipboard_image(&screenshot, ClipboardSource::Rendered).is_err());

    let rendered = TriggerContext::new()
        .with_image_path(PathBuf::from("shot.cap/original.png"))
        .with_output_path(PathBuf::from("rendered.png"));
    assert_eq!(
        clipboard_image(&rendered, ClipboardSource::Rendered),
        Ok(PathBuf::from("rendered.png"))
    );
    assert!(clipboard_image(&TriggerContext::new(), ClipboardSource::Raw).is_err());
}

#[tokio::test]
async fn apply_preset_writes_the_named_preset_and_reports_a_missing_one() {
    let root = tempfile::tempdir().unwrap();
    let store_path = root.path().join("store");
    std::fs::write(
        &store_path,
        serde_json::to_vec(&serde_json::json!({
            "presets": { "presets": [ { "name": "Square", "config": serde_json::to_value(
                frameever_project::ProjectConfiguration {
                    aspect_ratio: Some(frameever_project::AspectRatio::Square),
                    ..Default::default()
                }
            ).unwrap() } ] }
        }))
        .unwrap(),
    )
    .unwrap();
    crate::store::set_test_tauri_store_path(Some(store_path));
    let project = root.path().join("rec.cap");
    std::fs::create_dir(&project).unwrap();
    let ctx = TriggerContext::new().with_project_path(project.clone());
    let host = GpuiAutomationHost::default();

    let missing = host.apply_preset(&ctx, "Vertical").await;
    let applied = write_preset(&project, &preset_config("Square").unwrap());
    let gone = host
        .apply_preset(
            &TriggerContext::new().with_project_path(root.path().join("deleted.cap")),
            "Square",
        )
        .await;
    crate::store::set_test_tauri_store_path(None);

    assert_eq!(missing, Err("Preset 'Vertical' not found".to_string()));
    assert_eq!(gone, Err("The recording no longer exists".to_string()));
    assert!(!root.path().join("deleted.cap").exists());
    applied.unwrap();
    let written = frameever_project::ProjectConfiguration::load(&project).unwrap();
    assert!(matches!(
        written.aspect_ratio,
        Some(frameever_project::AspectRatio::Square)
    ));
}

#[test]
fn the_host_runs_notifications_but_not_text_recognition_or_network_actions() {
    let capabilities = GpuiAutomationHost::default().capabilities().to_vec();
    assert!(capabilities.contains(&Capability::Notify));
    assert!(capabilities.contains(&Capability::DeleteLocalFiles));
    assert!(!capabilities.contains(&Capability::RecognizeText));
}

#[tokio::test]
async fn text_recognition_is_not_run_yet() {
    let host = GpuiAutomationHost::default();
    let ctx = TriggerContext::new().with_image_path(PathBuf::from("shot.png"));
    assert!(host.recognize_text_to_clipboard(&ctx).await.is_err());
}
