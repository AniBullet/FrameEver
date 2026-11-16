use std::path::Path;

use frameever_project::{ProjectConfiguration, RecordingMeta, input_display};
use gpui::{Context, Window};

use crate::{editor_edits::ProjectHistory, editor_window::EditorWindow, i18n::t, platform};

pub(crate) fn project_has_input_record(
    project_path: &Path,
    project: &ProjectConfiguration,
) -> bool {
    let typed_text = project
        .timeline
        .as_ref()
        .is_some_and(|timeline| !timeline.keyboard_segments.is_empty());
    typed_text
        || RecordingMeta::load_for_project(project_path)
            .is_ok_and(|meta| input_display::has_input_record(&meta))
}

pub(crate) fn clear_project_input_record(
    project_path: &Path,
    project: &mut ProjectConfiguration,
    history: &mut ProjectHistory,
) -> Result<(), String> {
    let mut meta = RecordingMeta::load_for_project(project_path).map_err(|e| e.to_string())?;
    input_display::clear_input_record(&mut meta)?;
    *project = std::mem::take(project).without_typed_text();
    history.forget_typed_text();
    Ok(())
}

impl EditorWindow {
    pub(crate) fn has_input_record(&self) -> bool {
        *self
            .input_record_present
            .get_or_init(|| project_has_input_record(&self.project_path, &self.project))
    }

    pub(crate) fn clear_input_record_clicked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(handle) = window.window_handle().downcast::<Self>() else {
            return;
        };
        cx.spawn(async move |_, cx| {
            if !platform::confirm_dialog(
                t!("Clear the input record?"),
                t!(
                    "The input display shows nothing pressed and this project's keyboard segments are removed. This can't be undone."
                ),
                t!("Clear input record"),
                t!("Cancel"),
                true,
            ) {
                return;
            }
            handle
                .update(cx, |view, window, cx| view.clear_input_record(window, cx))
                .ok();
        })
        .detach();
    }

    fn clear_input_record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.project_ready() {
            return;
        }
        self.set_selection(None, cx);
        let cleared =
            clear_project_input_record(&self.project_path, &mut self.project, &mut self.history);
        self.input_record_present = std::cell::OnceCell::new();
        self.project_changed(window, cx);
        if let Err(error) = cleared {
            tracing::warn!(%error, "The input record was not cleared");
            platform::alert_dialog(
                t!("Couldn't clear the input record"),
                t!("Close anything that has this project's files open, then try again."),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_project::{
        KeyPressEvent, KeyboardEvents,
        input_display::{
            INPUT_EVENTS_FILE_NAME, InputEvent, InputEventKind, InputEvents, InputRecord,
        },
    };

    const TYPED_TEXT: &str = "hunter2";

    fn project_with_input(directory: &Path) -> ProjectConfiguration {
        let take = directory.join("content/segments/segment-0");
        std::fs::create_dir_all(&take).unwrap();
        KeyboardEvents {
            presses: vec![KeyPressEvent {
                key: "w".to_string(),
                key_code: "W".to_string(),
                time_ms: 10.0,
                down: true,
            }],
        }
        .write_to_file(&take.join(frameever_project::KEYBOARD_EVENTS_FILE_NAME))
        .unwrap();
        InputEvents {
            events: vec![InputEvent {
                time_ms: 20.0,
                kind: InputEventKind::MouseButton {
                    button: 5,
                    down: true,
                },
            }],
        }
        .write_to_file(&take.join(INPUT_EVENTS_FILE_NAME))
        .unwrap();
        std::fs::write(take.join("display.mp4"), b"video").unwrap();
        std::fs::write(
            directory.join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "typed",
                "segments": [{
                    "display": {"path": "content/segments/segment-0/display.mp4"},
                    "keyboard": "content/segments/segment-0/keyboard.bin"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        serde_json::from_value(serde_json::json!({
            "inputDisplay": {"enabled": true},
            "timeline": {
                "segments": [{"recordingSegment": 0, "start": 0.0, "end": 4.0, "timescale": 1.0}],
                "zoomSegments": [],
                "keyboardSegments": [{
                    "id": "kb-1",
                    "start": 0.5,
                    "end": 1.5,
                    "displayText": TYPED_TEXT
                }]
            }
        }))
        .unwrap()
    }

    #[test]
    fn clearing_leaves_the_project_with_no_input_and_nothing_to_undo_back_to() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project_with_input(directory.path());
        let mut history = ProjectHistory::new(project.clone());
        project.input_display.as_mut().unwrap().size = 90.0;
        history.record(&project);
        assert!(project_has_input_record(directory.path(), &project));

        clear_project_input_record(directory.path(), &mut project, &mut history).unwrap();

        assert!(!project_has_input_record(directory.path(), &project));
        assert_eq!(project.shown_input_display().unwrap().size, 90.0);
        let meta = RecordingMeta::load_for_project(directory.path()).unwrap();
        assert!(
            InputRecord::for_take(&meta, 0)
                .state_at(1000.0)
                .held()
                .next()
                .is_none()
        );
        assert!(
            directory
                .path()
                .join("content/segments/segment-0/display.mp4")
                .exists()
        );
        let undone = history.undo().unwrap();
        assert!(!serde_json::to_string(undone).unwrap().contains(TYPED_TEXT));
        assert_eq!(undone.input_display.as_ref().unwrap().size, 60.0);
    }

    #[test]
    fn a_project_without_input_has_nothing_to_clear() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project_with_input(directory.path());
        let mut history = ProjectHistory::new(project.clone());
        clear_project_input_record(directory.path(), &mut project, &mut history).unwrap();

        assert!(!project_has_input_record(directory.path(), &project));
        assert!(!project_has_input_record(
            &directory.path().join("missing"),
            &ProjectConfiguration::default()
        ));
    }
}
