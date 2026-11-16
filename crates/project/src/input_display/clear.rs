use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::events::INPUT_EVENTS_FILE_NAME;
use crate::{
    KEYBOARD_EVENTS_FILE_NAME, LEGACY_KEYBOARD_EVENTS_FILE_NAME, ProjectConfiguration,
    RecordingMeta, RecordingMetaInner, StudioRecordingMeta,
};

static CLEARED: AtomicU64 = AtomicU64::new(0);

pub fn input_record_generation() -> u64 {
    CLEARED.load(Ordering::Relaxed)
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn take_files(meta: &RecordingMeta) -> Vec<PathBuf> {
    let Some(StudioRecordingMeta::MultipleSegments { inner }) = meta.studio_meta() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for segment in &inner.segments {
        files.extend(segment.keyboard.as_ref().map(|path| meta.path(path)));
        if let Some(directory) = segment.display.path.parent() {
            files.extend(
                [
                    KEYBOARD_EVENTS_FILE_NAME,
                    LEGACY_KEYBOARD_EVENTS_FILE_NAME,
                    INPUT_EVENTS_FILE_NAME,
                ]
                .map(|name| meta.path(&directory.join(name))),
            );
        }
    }
    files
}

pub fn has_input_record(meta: &RecordingMeta) -> bool {
    take_files(meta).iter().any(|path| path.exists())
}

pub fn clear_input_record(meta: &mut RecordingMeta) -> Result<(), String> {
    for path in take_files(meta) {
        remove_if_present(&path)?;
    }
    let mut declared = false;
    if let RecordingMetaInner::Studio(studio) = &mut meta.inner
        && let StudioRecordingMeta::MultipleSegments { inner } = studio.as_mut()
    {
        for segment in &mut inner.segments {
            declared |= segment.keyboard.take().is_some();
        }
    }
    if declared {
        meta.save_for_project()
            .map_err(|error| format!("{}: {error}", meta.project_path.display()))?;
    }
    CLEARED.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

impl ProjectConfiguration {
    pub fn without_typed_text(mut self) -> Self {
        if let Some(timeline) = &mut self.timeline {
            timeline.keyboard_segments.clear();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KeyPressEvent, KeyboardEvents,
        input_display::{InputEvent, InputEventKind, InputEvents, InputRecord},
    };

    fn project_with_input() -> (tempfile::TempDir, RecordingMeta) {
        let directory = tempfile::tempdir().unwrap();
        for take in 0..2 {
            let folder = directory
                .path()
                .join(format!("content/segments/segment-{take}"));
            std::fs::create_dir_all(&folder).unwrap();
            KeyboardEvents {
                presses: vec![KeyPressEvent {
                    key: "w".to_string(),
                    key_code: "W".to_string(),
                    time_ms: 10.0,
                    down: true,
                }],
            }
            .write_to_file(&folder.join(KEYBOARD_EVENTS_FILE_NAME))
            .unwrap();
            InputEvents {
                events: vec![InputEvent {
                    time_ms: 20.0,
                    kind: InputEventKind::MouseButton {
                        button: 1,
                        down: true,
                    },
                }],
            }
            .write_to_file(&folder.join(INPUT_EVENTS_FILE_NAME))
            .unwrap();
            std::fs::write(folder.join("cursor.json"), r#"{"clicks":[],"moves":[]}"#).unwrap();
        }
        let mut meta: RecordingMeta = serde_json::from_value(serde_json::json!({
            "pretty_name": "two takes",
            "segments": [
                {
                    "display": {"path": "content/segments/segment-0/display.mp4"},
                    "cursor": "content/segments/segment-0/cursor.json",
                    "keyboard": "content/segments/segment-0/keyboard.bin"
                },
                {"display": {"path": "content/segments/segment-1/display.mp4"}}
            ]
        }))
        .unwrap();
        meta.project_path = directory.path().to_path_buf();
        meta.save_for_project().unwrap();
        (directory, meta)
    }

    #[test]
    fn clearing_removes_the_record_from_every_take_and_keeps_the_rest_of_the_project() {
        let (directory, mut meta) = project_with_input();
        assert!(has_input_record(&meta));
        assert!(!InputRecord::for_take(&meta, 0).is_empty());
        let generation = input_record_generation();

        clear_input_record(&mut meta).unwrap();

        assert!(!has_input_record(&meta));
        assert!(input_record_generation() > generation);
        let saved = RecordingMeta::load_for_project(directory.path()).unwrap();
        for take in 0..2 {
            let folder = directory
                .path()
                .join(format!("content/segments/segment-{take}"));
            assert!(!folder.join(KEYBOARD_EVENTS_FILE_NAME).exists());
            assert!(!folder.join(INPUT_EVENTS_FILE_NAME).exists());
            assert!(folder.join("cursor.json").exists());
            assert!(InputRecord::for_take(&saved, take).is_empty());
            assert!(
                InputRecord::for_take(&saved, take)
                    .state_at(1000.0)
                    .held()
                    .next()
                    .is_none()
            );
        }
        let json = std::fs::read_to_string(directory.path().join("recording-meta.json")).unwrap();
        assert!(!json.contains("keyboard"));
        assert!(json.contains("cursor.json"));
    }

    #[test]
    fn clearing_a_project_without_a_record_changes_nothing() {
        let (directory, mut meta) = project_with_input();
        clear_input_record(&mut meta).unwrap();
        let before = std::fs::read(directory.path().join("recording-meta.json")).unwrap();

        clear_input_record(&mut meta).unwrap();

        assert_eq!(
            std::fs::read(directory.path().join("recording-meta.json")).unwrap(),
            before
        );
    }

    #[test]
    fn typed_text_is_dropped_from_a_configuration() {
        let project: ProjectConfiguration = serde_json::from_value(serde_json::json!({
            "timeline": {
                "segments": [{"recordingSegment": 0, "start": 0.0, "end": 4.0, "timescale": 1.0}],
                "zoomSegments": [],
                "keyboardSegments": [{
                    "id": "kb-1",
                    "start": 0.5,
                    "end": 1.5,
                    "displayText": "hunter2"
                }]
            }
        }))
        .unwrap();
        assert_eq!(
            project.timeline.as_ref().unwrap().keyboard_segments.len(),
            1
        );

        let json = serde_json::to_string(&project.without_typed_text()).unwrap();

        assert!(!json.contains("hunter2"));
        assert!(json.contains("\"segments\""));
    }
}
