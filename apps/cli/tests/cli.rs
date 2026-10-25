use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use serde_json::Value;

fn frameever() -> Command {
    Command::new(env!("CARGO_BIN_EXE_frameever"))
}

fn run(args: &[&str]) -> Output {
    frameever()
        .args(args)
        .output()
        .expect("failed to spawn frameever binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn parse_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout was not valid JSON: {e}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            stdout(output),
            stderr(output)
        )
    })
}

#[test]
fn help_succeeds_and_lists_commands() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    for command in [
        "export",
        "export-preview",
        "project",
        "record",
        "screenshot",
        "recordings",
        "update",
        "mcp",
        "agents",
        "targets",
        "doctor",
        "selftest",
        "version",
        "desktop",
        "guide",
        "automations",
        "completions",
    ] {
        assert!(text.contains(command), "help missing '{command}':\n{text}");
    }
}

#[test]
fn no_args_prints_branded_intro() {
    let output = run(&[]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stderr(&output).is_empty(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("██████████████████"), "stdout: {text}");
    assert!(text.contains("FrameEver"), "stdout: {text}");
    assert!(!text.contains("/ ___|__ _ _ __"), "stdout: {text}");
    assert!(text.contains("frameever record start --screen <id> --detach"));
    assert!(text.contains("frameever --help"));
}

#[test]
fn no_args_json_is_parseable() {
    let output = run(&["--json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["name"], "frameever");
    assert!(json["commands"].is_array());
}

#[test]
fn subcommand_help_succeeds() {
    for command in [
        "export",
        "export-preview",
        "project",
        "record",
        "screenshot",
        "recordings",
        "update",
        "mcp",
        "agents",
        "targets",
        "doctor",
        "selftest",
        "version",
        "desktop",
        "guide",
        "automations",
        "completions",
    ] {
        let output = run(&[command, "--help"]);
        assert!(
            output.status.success(),
            "`frameever {command} --help` failed: {}",
            stderr(&output)
        );
    }
}

#[test]
fn unknown_command_fails() {
    let output = run(&["definitely-not-a-command"]);
    assert!(!output.status.success());
}

#[test]
fn version_json_is_parseable() {
    let output = run(&["version", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["name"], "frameever");
    assert!(json["version"].is_string());
    assert!(json["platform"]["os"].is_string());
    assert!(json["distribution"].is_string());
}

#[test]
fn doctor_json_is_parseable() {
    let output = run(&["doctor", "--format", "json"]);
    // doctor may report failing checks; it should still emit valid JSON and exit 0.
    let json = parse_json(&output);
    assert_eq!(json["version"]["name"], "frameever");
    assert!(json["checks"].is_array());
    assert!(json["ok"].is_boolean());
    assert!(json["permissions"].is_object());
}

#[test]
fn completions_generate_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = run(&["completions", shell]);
        assert!(
            output.status.success(),
            "completions for {shell} failed: {}",
            stderr(&output)
        );
        assert!(!output.stdout.is_empty(), "completions for {shell} empty");
    }
}

#[test]
fn targets_screens_json_is_parseable() {
    let output = run(&["targets", "screens", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json.is_array(), "expected a JSON array of screens");
}

#[test]
fn targets_all_json_is_parseable() {
    let output = run(&["targets", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json["screens"].is_array());
    assert!(json["windows"].is_array());
    assert!(json["cameras"].is_array());
}

#[test]
fn targets_format_before_subcommand_is_rejected() {
    // `--format` before the subcommand must not be silently ignored; clap rejects the conflict.
    let output = run(&["targets", "--format", "json", "screens"]);
    assert!(!output.status.success());
}

#[test]
fn record_requires_duration_when_non_interactive() {
    // The test harness gives the child a non-TTY stdin, so recording without --duration must error
    // rather than stop instantly on EOF.
    let output = run(&["record"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--duration") || stderr(&output).contains("interactive"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn version_text_reports_distribution() {
    let output = run(&["version"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("distribution:"));
}

#[test]
fn desktop_status_json_is_parseable() {
    let output = run(&["desktop", "status", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json["installed"].is_boolean());
    assert!(json["onPath"].is_boolean());
    assert!(json["shimPath"].is_string());
}

#[test]
fn project_inspect_missing_fails() {
    let output = run(&[
        "project",
        "inspect",
        "/this/path/does/not/exist.ever",
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).to_lowercase().contains("failed to load"));
}

#[test]
fn project_validate_missing_meta_reports_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(&[
        "project",
        "validate",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["valid"], false);
    assert!(json["error"].is_string());
}

#[test]
fn project_validate_complete_project_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    let content = project.join("content");
    std::fs::create_dir_all(&content).unwrap();
    std::fs::write(content.join("display.mp4"), b"fake").unwrap();
    write_single_segment_meta(&project);

    let output = run(&[
        "project",
        "validate",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["valid"], true);
    assert_eq!(json["recordingType"], "studio");
}

#[test]
fn project_validate_detects_missing_media() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    std::fs::create_dir_all(&project).unwrap();
    write_single_segment_meta(&project);

    let output = run(&[
        "project",
        "validate",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["valid"], false);
    let missing = json["missing"].as_array().unwrap();
    assert!(!missing.is_empty(), "expected missing media files");
}

#[test]
fn project_validate_rejects_in_progress_zero_segment_studio_project() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    std::fs::create_dir_all(&project).unwrap();
    let meta = serde_json::json!({
        "platform": "Linux",
        "pretty_name": "Broken Linux Recording",
        "sharing": null,
        "segments": [],
        "status": { "status": "InProgress" }
    });
    std::fs::write(
        project.join("recording-meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();

    let output = run(&[
        "project",
        "validate",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["valid"], false);
    assert!(json["error"].is_string());
    let problems = json["problems"].as_array().unwrap();
    assert!(
        problems.iter().any(|problem| problem
            .as_str()
            .is_some_and(|value| value.contains("no segments"))),
        "expected no-segments validation problem: {json}"
    );
}

#[test]
fn project_inspect_complete_project_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    let content = project.join("content");
    std::fs::create_dir_all(&content).unwrap();
    std::fs::write(content.join("display.mp4"), b"fake").unwrap();
    write_single_segment_meta(&project);

    let output = run(&[
        "project",
        "inspect",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["meta"]["pretty_name"], "Test Project");
}

#[test]
fn record_rejects_non_positive_duration() {
    let output = run(&["record", "--duration", "0"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Duration") || stdout(&output).contains("Duration"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn record_rejects_zero_fps() {
    let output = run(&["record", "--fps", "0", "--duration", "1"]);
    assert!(!output.status.success());
}

#[test]
fn record_validation_failures_emit_json_error_events() {
    for command in [
        &["record"][..],
        &["record", "start"][..],
        &["record", "start", "--detach"][..],
    ] {
        for json_flag in [&["--json"][..], &["--format", "json"][..]] {
            for (params, expected_error) in [
                (&["--duration", "0"][..], "Duration"),
                (&["--duration", "NaN"][..], "Duration"),
                (&["--duration", "inf"][..], "Duration"),
                (&["--fps", "0", "--duration", "1"][..], "--fps"),
                (&["--duration", "1"][..], "No target specified"),
            ] {
                let mut args = command.to_vec();
                args.extend_from_slice(json_flag);
                args.extend_from_slice(params);
                let output = run(&args);
                assert_eq!(output.status.code(), Some(1), "args: {args:?}");
                let json = parse_json(&output);
                assert_eq!(json["type"], "error", "args: {args:?}");
                let error = json["error"].as_str().expect("expected an error message");
                assert!(error.contains(expected_error), "args: {args:?}: {error}");
                assert!(stderr(&output).contains(error), "args: {args:?}");
            }
        }
    }
}

#[test]
fn export_rejects_settings_json_with_flags() {
    let output = run(&[
        "export",
        "/tmp/whatever.ever",
        "--settings-json",
        "{\"format\":\"Mp4\"}",
        "--fps",
        "30",
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cannot be combined"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn export_rejects_quality_with_gif() {
    let output = run(&[
        "export",
        "/tmp/whatever.ever",
        "--format",
        "gif",
        "--quality",
        "web",
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("only supported for"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn export_rejects_bad_resolution() {
    let output = run(&[
        "export",
        "/tmp/whatever.ever",
        "--resolution",
        "not-a-resolution",
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).to_lowercase().contains("resolution"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn export_rejects_duplicate_output() {
    let output = run(&[
        "export",
        "/tmp/whatever.ever",
        "/tmp/out1.mp4",
        "--output",
        "/tmp/out2.mp4",
    ]);
    assert!(!output.status.success());
}

#[test]
fn targets_mics_json_is_parseable() {
    let output = run(&["targets", "mics", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json.is_array(), "expected a JSON array of mics");
}

#[test]
fn targets_all_json_includes_mics() {
    let output = run(&["targets", "--format", "json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json["mics"].is_array());
}

#[test]
fn recordings_list_empty_dir_is_empty_json() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(&[
        "recordings",
        "list",
        "--dir",
        dir.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json.as_array().map(|a| a.len()), Some(0));
}

#[test]
fn recordings_list_includes_the_custom_and_previous_folders_the_app_scans() {
    let root = tempfile::tempdir().unwrap();
    let app_data = root.path().join("app");
    let custom = root.path().join("custom");
    let previous = root.path().join("previous");
    write_named_meta(&app_data.join("recordings/Default.ever"), "Default");
    write_named_meta(&custom.join("Custom.ever"), "Custom");
    write_named_meta(&previous.join("Previous.ever"), "Previous");
    write_named_meta(&previous.join("Older.cap"), "Older");
    write_named_meta(&previous.join("Not a project"), "Not a project");
    std::fs::write(
        app_data.join("store"),
        serde_json::to_vec(&serde_json::json!({
            "general_settings": {
                "recordingsPath": custom,
                "previousRecordingsPaths": [previous]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let output = frameever()
        .args(["recordings", "list", "--format", "json"])
        .env("FRAMEEVER_GPUI_APP_DATA_DIR", &app_data)
        .env_remove("FRAMEEVER_GPUI_TAURI_STORE")
        .env_remove("FRAMEEVER_GPUI_RECORDINGS_DIR")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let mut names: Vec<String> = parse_json(&output)
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["Custom", "Default", "Previous"]);
}

#[test]
fn project_config_set_then_get_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    write_single_segment_meta(&project);

    let set = run(&[
        "project",
        "config",
        "set",
        project.to_str().unwrap(),
        "--settings-json",
        "{}",
    ]);
    assert!(set.status.success(), "stderr: {}", stderr(&set));
    assert!(project.join("project-config.json").exists());

    let get = run(&[
        "project",
        "config",
        "get",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(get.status.success(), "stderr: {}", stderr(&get));
    let json = parse_json(&get);
    assert!(json.is_object(), "expected a project config object");
}

#[test]
fn project_config_get_without_file_returns_default() {
    // Instant / un-edited projects have no project-config.json; `config get` should still succeed
    // with the effective default config rather than erroring.
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("recording.ever");
    write_single_segment_meta(&project);

    let output = run(&[
        "project",
        "config",
        "get",
        project.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json.is_object(), "expected a default project config object");
}

#[test]
fn export_missing_project_emits_json_error_event() {
    let output = run(&[
        "export",
        "/this/path/does/not/exist.ever",
        "--progress-json",
    ]);
    assert!(!output.status.success());
    // The NDJSON stream must end with a machine-readable terminal error rather than just stopping.
    // The `error` field is uniform across every JSON-emitting command (the `type` tag stays Error).
    let json = parse_json(&output);
    assert_eq!(json["type"], "Error");
    assert!(json["error"].is_string());
}

#[test]
fn export_global_json_implies_progress_stream() {
    // `--json` (global) must behave like --progress-json/--completion-json so the agent's universal
    // reflex works on export too; failure still ends with a terminal Error event.
    let output = run(&["export", "/this/path/does/not/exist.ever", "--json"]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["type"], "Error");
    assert!(json["error"].is_string());
}

#[test]
fn export_preview_missing_project_emits_json_error() {
    let output = run(&[
        "export-preview",
        "/this/path/does/not/exist.ever",
        "--frame-time",
        "0",
        "--settings-json",
        "{}",
    ]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert!(json["error"].is_string());
}

#[test]
fn record_no_target_json_emits_error_event() {
    // --duration satisfies the non-interactive guard, so this fails on target resolution and must
    // report that failure as a JSON event on stdout.
    let output = run(&["record", "--duration", "1", "--format", "json"]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["type"], "error");
    assert!(json["error"].is_string());
}

#[test]
fn global_json_flag_works_before_subcommand() {
    // The order-insensitive global --json is the headline agent ergonomic: it must parse in front of
    // the verb and force JSON output the same as a trailing `--format json`.
    let output = run(&["--json", "version"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["name"], "frameever");
}

#[test]
fn guide_json_is_parseable_and_self_describing() {
    let output = run(&["guide", "--json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["binary"], "frameever");
    assert_eq!(json["schemaVersion"], 3);
    assert!(json["commands"].is_array());
    assert!(json["env"].is_array());
    assert!(json["outputConvention"].is_object());
    let commands = serde_json::to_string(&json["commands"]).unwrap();
    for command in [
        "record start",
        "record stop",
        "export",
        "screenshot",
        "mcp serve",
        "agents install",
        "recordings list",
        "project validate",
        "automations list",
    ] {
        assert!(
            commands.contains(&format!("\"{command}\"")),
            "guide missing {command}"
        );
    }
    for removed in [
        "caps",
        "account",
        "organizations",
        "developers",
        "upload",
        "auth",
    ] {
        assert!(
            !commands.contains(&format!("\"{removed}")),
            "guide still lists {removed}"
        );
    }
}

#[test]
fn agent_installer_dry_run_is_machine_readable() {
    let home = tempfile::tempdir().unwrap();
    let codex_home = home.path().join(".codex");
    let output = frameever()
        .args([
            "agents",
            "install",
            "--target",
            "codex",
            "--component",
            "all",
            "--dry-run",
            "--yes",
            "--format",
            "json",
        ])
        .env("CODEX_HOME", &codex_home)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert_eq!(json["target"], "codex");
    assert_eq!(json["dryRun"], true);
    assert_eq!(json["applied"], false);
    let changes = json["changes"].as_array().unwrap();
    let count = |component: &str| {
        changes
            .iter()
            .filter(|change| change["component"] == component)
            .count()
    };
    assert_eq!(count("mcp"), 1);
    assert!(count("skill") >= 1);
    assert_eq!(count("mcp") + count("skill"), changes.len());
    assert!(!codex_home.exists());
}

#[test]
fn agent_installer_emits_json_error_without_confirmation() {
    let home = tempfile::tempdir().unwrap();
    let installer_output = frameever()
        .args([
            "agents",
            "install",
            "--target",
            "codex",
            "--component",
            "skill",
            "--format",
            "json",
        ])
        .env("HOME", home.path())
        .env("CODEX_HOME", home.path().join(".codex"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(installer_output.status.code(), Some(1));
    assert!(parse_json(&installer_output)["error"].is_string());
}

#[test]
fn mcp_stdout_is_protocol_only_and_exposes_expected_tools() {
    let mut child = frameever()
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start MCP server");
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut stdin = child.stdin.take().unwrap();
    for message in [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "frameever-test", "version": "1" }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    ] {
        writeln!(stdin, "{message}").unwrap();
    }
    stdin.flush().unwrap();
    let initialize = receiver.recv_timeout(Duration::from_secs(10));
    let tools = receiver.recv_timeout(Duration::from_secs(10));
    let _ = child.kill();
    let _ = child.wait();

    let initialize: Value = serde_json::from_str(&initialize.unwrap().unwrap()).unwrap();
    assert_eq!(initialize["id"], 1);
    assert_eq!(initialize["result"]["serverInfo"]["name"], "frameever");
    let tools: Value = serde_json::from_str(&tools.unwrap().unwrap()).unwrap();
    assert_eq!(tools["id"], 2);
    let mut names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "doctor",
            "export",
            "project_validate",
            "record_start",
            "record_status",
            "record_stop",
            "recordings_list",
            "screenshot",
            "targets",
        ]
    );
}

#[test]
fn record_status_empty_is_json_array() {
    let output = run(&["record", "status", "--json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json.is_array(), "expected a JSON array of sessions");
}

#[test]
fn record_stop_unknown_id_fails_with_json_error() {
    let output = run(&["record", "stop", "--id", "does-not-exist", "--json"]);
    assert!(!output.status.success());
    let json = parse_json(&output);
    assert_eq!(json["type"], "error");
    assert!(json["error"].is_string());
}

#[test]
fn doctor_exits_zero_even_when_checks_fail() {
    // doctor is a report, not a gate: agents branch on `ok`/`captureReady`, so it must exit 0.
    let output = run(&["doctor", "--json"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json = parse_json(&output);
    assert!(json["ok"].is_boolean());
    assert!(json["captureReady"].is_boolean());
}

#[test]
fn doctor_check_ids_are_the_pinned_vocabulary() {
    let output = run(&["doctor", "--json"]);
    let json = parse_json(&output);
    let ids: Vec<&str> = json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    #[cfg(target_os = "macos")]
    assert_eq!(
        ids,
        [
            "ffmpeg",
            "screenRecordingPermission",
            "screenCaptureKit",
            "cliInstall"
        ]
    );

    #[cfg(not(target_os = "macos"))]
    assert_eq!(ids, ["ffmpeg", "screenRecordingPermission", "cliInstall"]);
}

#[test]
fn clean_error_has_no_debug_quotes() {
    // main() must print `error: <message>` (not the default `Error: "debug-quoted"`); a leading
    // `Error: "` would mean agents scraping stderr get stray quotes/escapes.
    let output = run(&["record", "--duration", "1"]);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("error:"), "stderr: {err}");
    assert!(!err.contains("Error: \""), "stderr had debug quotes: {err}");
}

fn write_single_segment_meta(project: &Path) {
    std::fs::create_dir_all(project).unwrap();
    let meta = serde_json::json!({
        "pretty_name": "Test Project",
        "display": { "path": "content/display.mp4", "fps": 30 }
    });
    std::fs::write(
        project.join("recording-meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();
}

fn write_named_meta(project: &Path, name: &str) {
    std::fs::create_dir_all(project).unwrap();
    let meta = serde_json::json!({
        "pretty_name": name,
        "display": { "path": "content/display.mp4", "fps": 30 }
    });
    std::fs::write(
        project.join("recording-meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();
}
