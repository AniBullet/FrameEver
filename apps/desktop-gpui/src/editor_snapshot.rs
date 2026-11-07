use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use frameever_export::preview::{StillFrame, render_still_with_config, render_still_with_editor};
use gpui::{Context, Window};

use crate::editor_window::{EDITOR_PREVIEW_FPS, EditorWindow};
use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    Save,
    Copy,
}

impl FrameAction {
    pub const ALL: [FrameAction; 2] = [FrameAction::Save, FrameAction::Copy];

    pub fn label(self) -> &'static str {
        match self {
            FrameAction::Save => t!("Save to library"),
            FrameAction::Copy => t!("Copy to clipboard"),
        }
    }

    pub fn done(self) -> &'static str {
        match self {
            FrameAction::Save => t!("Frame saved to the library"),
            FrameAction::Copy => t!("Frame copied"),
        }
    }

    pub fn failed(self) -> &'static str {
        match self {
            FrameAction::Save => t!(
                "Couldn't save the frame. Check that the storage folder is available and has free space, then try again."
            ),
            FrameAction::Copy => t!("Couldn't copy the frame. Try again."),
        }
    }
}

pub fn frame_name(project_name: &str, time: f64) -> String {
    let centis = (time.max(0.0) * 100.0).floor() as u64;
    let minutes = centis / 6000;
    let seconds = centis / 100 % 60;
    let fraction = centis % 100;
    format!("{project_name} ({minutes}:{seconds:02}.{fraction:02})")
}

pub fn encode_still_png(still: &StillFrame) -> anyhow::Result<Vec<u8>> {
    crate::screenshot_export::encode_rgba_png(&still.rgba, still.width, still.height)
        .map_err(|error| anyhow!("encoding the frame PNG: {error}"))
}

pub fn save_still_in(dir: &Path, pretty_name: &str, still: &StillFrame) -> anyhow::Result<PathBuf> {
    let png_bytes = encode_still_png(still)?;
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating screenshots dir {}", dir.display()))?;
    let bundle = crate::screenshot::create_screenshot_project(dir, pretty_name.to_string())?;
    let png = bundle.join("original.png");
    std::fs::write(&png, png_bytes).with_context(|| format!("writing {}", png.display()))?;
    Ok(png)
}

impl EditorWindow {
    pub(crate) fn export_current_frame(
        &mut self,
        action: FrameAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let time = self.playhead_time();
        let project = self.project.clone();
        let instance = self.instance.clone();
        let path = self.project_path.clone();
        let force = cfg!(target_os = "windows");
        let name = frame_name(
            self.summary()
                .map_or("FrameEver", |summary| summary.pretty_name.as_str()),
            time,
        );
        let screenshots = crate::library::screenshots_dir();
        let task = gpui_tokio::Tokio::spawn_result(cx, async move {
            let still = match instance {
                Some(instance) => {
                    render_still_with_editor(&instance, project, time, EDITOR_PREVIEW_FPS).await
                }
                None => {
                    render_still_with_config(path, project, time, EDITOR_PREVIEW_FPS, force).await
                }
            }
            .map_err(|error| anyhow!("rendering the frame: {error}"))?;
            match action {
                FrameAction::Save => tokio::task::spawn_blocking(move || {
                    save_still_in(&screenshots, &name, &still).map(|_| None)
                })
                .await
                .context("the frame save task died")?,
                FrameAction::Copy => {
                    tokio::task::spawn_blocking(move || encode_still_png(&still).map(Some))
                        .await
                        .context("the frame encode task died")?
                }
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(Some(png)) => {
                    let copied = cx.update(|_, cx| crate::platform::copy_png_to_clipboard(png, cx));
                    match copied {
                        Ok(copied) => copied.await.map_err(|error| anyhow!(error)),
                        Err(error) => Err(error),
                    }
                }
                Ok(None) => Ok(()),
                Err(error) => Err(error),
            };
            this.update_in(cx, |this, window, cx| match result {
                Ok(()) => {
                    if action == FrameAction::Save {
                        crate::app_windows::screenshot_saved(cx);
                    }
                    this.show_notice(action.done(), window, cx);
                }
                Err(error) => {
                    tracing::error!("exporting the current frame failed: {error:#}");
                    this.show_notice(action.failed(), window, cx);
                }
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still() -> StillFrame {
        StillFrame {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255,
            ],
        }
    }

    #[test]
    fn a_saved_frame_is_a_screenshot_in_the_library_with_the_rendered_pixels() {
        let temp = tempfile::tempdir().unwrap();
        let screenshots = temp.path().join("Screenshots");

        let png = save_still_in(&screenshots, "Demo (0:12.40)", &still()).unwrap();

        let listed = crate::library::list_screenshots_in(&screenshots);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].path, png);
        assert_eq!(listed[0].pretty_name, "Demo (0:12.40)");
        let decoded = image::open(&png).unwrap().into_rgba8();
        assert_eq!((decoded.width(), decoded.height()), (2, 2));
        assert_eq!(
            decoded.into_raw(),
            vec![
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255
            ]
        );
    }

    #[test]
    fn saving_the_same_frame_twice_keeps_both() {
        let temp = tempfile::tempdir().unwrap();

        let first = save_still_in(temp.path(), "Demo (0:01.00)", &still()).unwrap();
        let second = save_still_in(temp.path(), "Demo (0:01.00)", &still()).unwrap();

        assert_ne!(first, second);
        assert_eq!(crate::library::list_screenshots_in(temp.path()).len(), 2);
    }

    #[test]
    fn a_save_into_an_unwritable_place_fails_without_leaving_a_screenshot() {
        let temp = tempfile::tempdir().unwrap();
        let blocker = temp.path().join("Screenshots");
        std::fs::write(&blocker, b"a file, not a folder").unwrap();

        assert!(save_still_in(&blocker, "Demo (0:01.00)", &still()).is_err());
        assert!(crate::library::list_screenshots_in(temp.path()).is_empty());
    }

    #[test]
    fn a_frame_is_named_after_the_project_and_the_playhead() {
        assert_eq!(frame_name("Demo", 72.405), "Demo (1:12.40)");
        assert_eq!(frame_name("Demo", 0.0), "Demo (0:00.00)");
    }
}
