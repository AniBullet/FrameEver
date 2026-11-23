use std::path::{Path, PathBuf};

use frameever_export::ExportRange;
pub use frameever_export::gif::GifQualityPreset;
pub use frameever_export::instant_gif::{FPS, GifFailure, GifSize, estimated_bytes, is_large};

use crate::i18n::tc;

pub fn quality_label(quality: GifQualityPreset) -> &'static str {
    match quality {
        GifQualityPreset::Low => tc!("gif quality", "Low"),
        GifQualityPreset::Medium => tc!("gif quality", "Medium"),
        GifQualityPreset::High => tc!("gif quality", "High"),
    }
}

pub fn can_save_as_gif(entry: &super::view::Entry) -> bool {
    entry.kind == super::MediaKind::Instant
        && entry.duration.is_some()
        && super::instant_output_path(&entry.path).is_file()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GifSource {
    pub pixels: (u32, u32),
    pub seconds: f64,
}

pub fn source(project: &Path) -> Result<GifSource, GifFailure> {
    let video = super::instant_output_path(project);
    if !video.is_file() {
        return Err(GifFailure::VideoMissing);
    }
    let unreadable = |what: &str| GifFailure::VideoUnreadable(what.to_string());
    let (pixels, _) =
        super::video_shape(&video).ok_or_else(|| unreadable("the video has no picture size"))?;
    let seconds = super::media_duration(&video)
        .filter(|seconds| *seconds > 0.0)
        .ok_or_else(|| unreadable("the video has no length"))?;
    Ok(GifSource { pixels, seconds })
}

pub fn save_as_gif(
    project: &Path,
    name: &str,
    exports: &Path,
    range: ExportRange,
    size: GifSize,
    quality: GifQualityPreset,
    progress: impl FnMut(u32, u32) -> bool,
) -> Result<PathBuf, GifFailure> {
    let output =
        super::unique_export_path(exports, name, "gif").map_err(GifFailure::FolderNotWritable)?;
    frameever_export::instant_gif::save_gif(
        &super::instant_output_path(project),
        &output,
        range,
        size,
        quality,
        progress,
    )?;
    super::record_export(
        project,
        &super::export_record(output.clone(), "gif", None),
        false,
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_enc_ffmpeg::{h264::H264Encoder, mp4::MP4File};
    use std::time::Duration;

    fn instant_recording(recordings: &Path, name: &str) -> PathBuf {
        ffmpeg::init().ok();
        let project = recordings.join(format!("{name}.ever"));
        std::fs::create_dir_all(project.join("content")).unwrap();
        std::fs::write(
            project.join("recording-meta.json"),
            format!(r#"{{"pretty_name":"{name}","sharing":null,"fps":30,"sample_rate":48000}}"#),
        )
        .unwrap();
        let info = frameever_media_info::VideoInfo {
            pixel_format: frameever_media_info::Pixel::NV12,
            width: 320,
            height: 240,
            time_base: ffmpeg::Rational(1, 1_000_000),
            frame_rate: ffmpeg::Rational(30, 1),
        };
        let mut file = MP4File::init(
            "source",
            project.join("content/output.mp4"),
            false,
            |output| H264Encoder::builder(info).build(output),
            |_| None,
        )
        .unwrap();
        for index in 0..30_u64 {
            let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, 320, 240);
            frame.data_mut(0).fill(40 + (index * 3) as u8);
            frame.data_mut(1).fill(128);
            file.queue_video_frame(frame, Duration::from_nanos(index * 1_000_000_000 / 30))
                .unwrap();
        }
        file.finish().unwrap();
        project
    }

    const HALF_A_SECOND: ExportRange = ExportRange {
        start: 0.2,
        end: 0.7,
    };

    #[test]
    fn a_saved_gif_lands_in_the_exports_folder_and_joins_the_exports_of_its_recording() {
        let directory = tempfile::tempdir().unwrap();
        let recordings = directory.path().join("Recordings");
        let exports = directory.path().join("Exports");
        let project = instant_recording(&recordings, "Demo");
        let video = std::fs::read(project.join("content/output.mp4")).unwrap();

        let first = save_as_gif(
            &project,
            "Demo",
            &exports,
            HALF_A_SECOND,
            GifSize::Small,
            GifQualityPreset::High,
            |_, _| true,
        )
        .unwrap();
        let second = save_as_gif(
            &project,
            "Demo",
            &exports,
            HALF_A_SECOND,
            GifSize::Small,
            GifQualityPreset::High,
            |_, _| true,
        )
        .unwrap();

        assert_eq!(first, exports.join("Demo.gif"));
        assert_eq!(second, exports.join("Demo (1).gif"));
        let records = frameever_utils::storage::exports(&project);
        assert_eq!(
            records
                .iter()
                .map(|record| (
                    record.path.clone(),
                    record.format.as_str(),
                    record.codec.clone()
                ))
                .collect::<Vec<_>>(),
            vec![(first.clone(), "gif", None), (second.clone(), "gif", None)]
        );
        let entries = crate::library::entries_in(&[recordings], &[]);
        assert_eq!(
            entries[0]
                .exports
                .iter()
                .map(|export| (export.name.as_str(), export.size))
                .collect::<Vec<_>>(),
            vec![
                ("Demo.gif", Some(std::fs::metadata(&first).unwrap().len())),
                (
                    "Demo (1).gif",
                    Some(std::fs::metadata(&second).unwrap().len())
                ),
            ]
        );
        assert!(std::fs::metadata(&first).unwrap().len() > 0);
        assert_eq!(
            std::fs::read(project.join("content/output.mp4")).unwrap(),
            video
        );
    }

    #[test]
    fn only_a_finished_instant_recording_with_its_video_can_be_saved_as_a_gif() {
        let directory = tempfile::tempdir().unwrap();
        let recordings = directory.path().join("Recordings");
        let instant = instant_recording(&recordings, "Instant");
        let lost = recordings.join("Lost.ever");
        let studio = recordings.join("Studio.ever");
        for (project, meta) in [
            (
                &lost,
                r#"{"pretty_name":"Lost","sharing":null,"fps":30,"sample_rate":48000}"#,
            ),
            (
                &studio,
                r#"{"pretty_name":"Studio","sharing":null,"segments":[{"display":{"path":"content/segments/segment-0/display.mp4","fps":30}}]}"#,
            ),
        ] {
            std::fs::create_dir_all(project).unwrap();
            std::fs::write(project.join("recording-meta.json"), meta).unwrap();
        }

        let entries = crate::library::entries_in(&[recordings], &[]);
        let offered =
            |name: &str| can_save_as_gif(entries.iter().find(|entry| entry.name == name).unwrap());

        assert!(offered("Instant"));
        assert!(!offered("Lost"));
        assert!(!offered("Studio"));
        assert_eq!(
            source(&instant),
            Ok(GifSource {
                pixels: (320, 240),
                seconds: 1.0,
            })
        );
        assert_eq!(source(&lost), Err(GifFailure::VideoMissing));
    }

    #[test]
    fn a_cancelled_or_failed_gif_is_neither_left_in_the_folder_nor_recorded() {
        let directory = tempfile::tempdir().unwrap();
        let recordings = directory.path().join("Recordings");
        let exports = directory.path().join("Exports");
        let project = instant_recording(&recordings, "Demo");

        let cancelled = save_as_gif(
            &project,
            "Demo",
            &exports,
            HALF_A_SECOND,
            GifSize::Small,
            GifQualityPreset::High,
            |done, _| done < 2,
        );
        std::fs::remove_file(project.join("content/output.mp4")).unwrap();
        let failed = save_as_gif(
            &project,
            "Demo",
            &exports,
            HALF_A_SECOND,
            GifSize::Small,
            GifQualityPreset::High,
            |_, _| true,
        );

        assert_eq!(cancelled, Err(GifFailure::Cancelled));
        assert_eq!(failed, Err(GifFailure::VideoMissing));
        assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 0);
        assert!(frameever_utils::storage::exports(&project).is_empty());
    }

    #[test]
    fn a_failed_gif_says_whether_the_video_or_the_exports_folder_is_the_problem() {
        let directory = tempfile::tempdir().unwrap();
        let recordings = directory.path().join("Recordings");
        let project = instant_recording(&recordings, "Demo");
        let blocked = directory.path().join("Exports");
        std::fs::write(&blocked, b"a file where the folder should be").unwrap();
        let save = |exports: &Path| {
            save_as_gif(
                &project,
                "Demo",
                exports,
                HALF_A_SECOND,
                GifSize::Small,
                GifQualityPreset::High,
                |_, _| true,
            )
        };

        assert!(matches!(
            save(&blocked),
            Err(GifFailure::FolderNotWritable(_))
        ));
        std::fs::write(project.join("content/output.mp4"), b"not a video").unwrap();
        assert!(matches!(
            save(&directory.path().join("Elsewhere")),
            Err(GifFailure::VideoUnreadable(_))
        ));
        assert!(frameever_utils::storage::exports(&project).is_empty());
    }

    #[test]
    fn the_gif_is_named_after_the_recording_with_characters_a_file_cannot_have_removed() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            super::super::unique_export_path(directory.path(), "Demo: take 2?", "gif"),
            Ok(directory.path().join("Demo take 2.gif"))
        );
        assert_eq!(
            super::super::unique_export_path(directory.path(), "///", "gif"),
            Ok(directory.path().join("Recording.gif"))
        );
    }
}
