use std::path::{Path, PathBuf};

use crate::i18n::t;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAction {
    OpenLogs,
    RevealRecording,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionNotice {
    pub title: String,
    pub recording: String,
    pub detail: String,
    pub actions: Vec<NoticeAction>,
    pub project_path: PathBuf,
}

pub fn compress_and_report(
    project_path: &Path,
    keep_only_compressed: bool,
    compress: impl FnOnce(&Path) -> Result<PathBuf, String>,
    notify: impl FnOnce(CompressionNotice),
) {
    if let Err(error) = compress(project_path) {
        tracing::warn!(path = %project_path.display(), "automatic compression failed: {error}");
        notify(CompressionNotice {
            title: t!("Couldn't compress the recording").to_string(),
            recording: recording_name(project_path),
            detail: error,
            actions: vec![NoticeAction::OpenLogs, NoticeAction::RevealRecording],
            project_path: project_path.to_path_buf(),
        });
        return;
    }
    tracing::info!(path = %project_path.display(), "compressed the instant recording");
    let percent = crate::library::video_versions(project_path)
        .and_then(|versions| versions.saving_percent())
        .unwrap_or(0);
    let kept_only_compressed = keep_only_compressed
        && match crate::library::delete_original(project_path) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(path = %project_path.display(), "could not remove the original after compressing: {error}");
                false
            }
        };
    let summary = if kept_only_compressed {
        t!("Compressed · {percent}% smaller", percent = percent)
    } else {
        t!(
            "Compressed copy ready · {percent}% smaller",
            percent = percent
        )
    };
    notify(CompressionNotice {
        title: t!("Compression finished").to_string(),
        recording: recording_name(project_path),
        detail: summary,
        actions: Vec::new(),
        project_path: project_path.to_path_buf(),
    });
}

fn recording_name(project_path: &Path) -> String {
    frameever_project::RecordingMeta::load_for_project(project_path)
        .map(|meta| meta.pretty_name)
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            project_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

impl NoticeAction {
    fn label(&self) -> &'static str {
        match self {
            NoticeAction::OpenLogs => t!("Open Logs Folder"),
            NoticeAction::RevealRecording => t!("Reveal in file manager"),
        }
    }

    fn argument(&self) -> &'static str {
        match self {
            NoticeAction::OpenLogs => "open-logs",
            NoticeAction::RevealRecording => "reveal-recording",
        }
    }

    fn from_argument(argument: &str) -> Option<Self> {
        match argument {
            "open-logs" => Some(NoticeAction::OpenLogs),
            "reveal-recording" => Some(NoticeAction::RevealRecording),
            _ => None,
        }
    }

    fn perform(&self, project_path: &Path) {
        match self {
            NoticeAction::OpenLogs => crate::library::open_path(&crate::diagnostics::logs_dir()),
            NoticeAction::RevealRecording => crate::library::reveal_in_folder(project_path),
        }
    }
}

#[cfg(windows)]
pub fn show(notice: CompressionNotice) {
    use tauri_winrt_notification::Toast;

    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID
    } else {
        crate::brand::APP_ID
    };
    let mut toast = Toast::new(app_id)
        .title(&notice.title)
        .text1(&notice.recording)
        .text2(&notice.detail);
    if !notice.actions.is_empty() {
        toast = toast.duration(tauri_winrt_notification::Duration::Long);
    }
    for action in &notice.actions {
        toast = toast.add_button(action.label(), action.argument());
    }
    let project_path = notice.project_path;
    let toast = toast.on_activated(move |argument| {
        if let Some(action) = argument.as_deref().and_then(NoticeAction::from_argument) {
            action.perform(&project_path);
        }
        Ok(())
    });
    match toast.show() {
        Ok(()) => tracing::info!(title = %notice.title, "showed the compression notification"),
        Err(error) => tracing::warn!("showing the compression notification failed: {error}"),
    }
}

#[cfg(not(windows))]
pub fn show(notice: CompressionNotice) {
    tracing::info!(title = %notice.title, "compression notifications are Windows-only");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(
        project_path: &Path,
        keep_only_compressed: bool,
        compress: impl FnOnce(&Path) -> Result<PathBuf, String>,
    ) -> Vec<CompressionNotice> {
        let mut notices = Vec::new();
        compress_and_report(project_path, keep_only_compressed, compress, |notice| {
            notices.push(notice)
        });
        notices
    }

    #[test]
    fn a_failed_compression_says_why_and_offers_the_logs_and_the_recording() {
        let empty = tempfile::tempdir().unwrap();
        let notices = capture(empty.path(), false, |path| {
            crate::library::compress_video(path, |_, _| true)
        });
        let [notice] = notices.as_slice() else {
            panic!("expected one notice, got {notices:?}");
        };
        assert_eq!(notice.title, "Couldn't compress the recording");
        assert_eq!(
            notice.detail,
            "This recording already has a compressed version"
        );
        assert_eq!(
            notice.actions,
            vec![NoticeAction::OpenLogs, NoticeAction::RevealRecording]
        );
        assert_eq!(notice.project_path, empty.path());
    }

    fn recorded_bundle() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("content")).unwrap();
        std::fs::write(dir.path().join("content/output.mp4"), [0u8; 200]).unwrap();
        dir
    }

    fn compress_to_half(bundle: &Path) -> Result<PathBuf, String> {
        let compressed = bundle.join("content/output_compressed.mp4");
        std::fs::write(&compressed, [0u8; 100]).unwrap();
        Ok(compressed)
    }

    #[test]
    fn a_finished_compression_says_how_much_smaller_the_copy_is() {
        let bundle = recorded_bundle();
        let notices = capture(bundle.path(), false, compress_to_half);
        let [notice] = notices.as_slice() else {
            panic!("expected one notice, got {notices:?}");
        };
        assert_eq!(notice.title, "Compression finished");
        assert_eq!(notice.detail, "Compressed copy ready · 50% smaller");
        assert!(notice.actions.is_empty());
    }

    #[test]
    fn keeping_only_the_compressed_version_removes_the_original_and_says_so() {
        let bundle = recorded_bundle();
        let notices = capture(bundle.path(), true, compress_to_half);
        let [notice] = notices.as_slice() else {
            panic!("expected one notice, got {notices:?}");
        };
        assert_eq!(notice.detail, "Compressed · 50% smaller");
        assert_eq!(
            std::fs::metadata(bundle.path().join("content/output.mp4"))
                .unwrap()
                .len(),
            100
        );
        assert!(!bundle.path().join("content/output_compressed.mp4").exists());
    }

    #[test]
    fn each_button_argument_leads_back_to_its_action() {
        for action in [NoticeAction::OpenLogs, NoticeAction::RevealRecording] {
            assert_eq!(NoticeAction::from_argument(action.argument()), Some(action));
        }
        assert_eq!(NoticeAction::from_argument(""), None);
    }
}
