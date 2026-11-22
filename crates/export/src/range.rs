use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct ExportRange {
    pub start: f64,
    pub end: f64,
}

const FRAME_TOLERANCE: f64 = 1e-6;

impl ExportRange {
    pub fn frames(self, fps: u32, total_frames: u32) -> std::ops::Range<u32> {
        let frame_at = |seconds: f64| {
            (!seconds.is_nan()).then(|| {
                (seconds * f64::from(fps) - FRAME_TOLERANCE)
                    .ceil()
                    .clamp(0.0, f64::from(total_frames)) as u32
            })
        };
        match (frame_at(self.start), frame_at(self.end)) {
            (Some(start), Some(end)) if start < end => start..end,
            _ => 0..0,
        }
    }

    pub fn overlaps(self, timeline_seconds: f64) -> bool {
        self.start.max(0.0) < self.end.min(timeline_seconds)
    }

    pub fn frame_count(self, fps: u32, total_frames: u32) -> u32 {
        let frames = self.frames(fps, total_frames);
        frames.end - frames.start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExporterBase, gif::GifExportSettings, mp4::Mp4ExportSettings};
    use frameever_project::{ProjectConfiguration, XY};
    use image::AnimationDecoder;
    use std::{
        path::{Path, PathBuf},
        process::Command,
    };

    const FPS: u32 = 10;
    const CLIP: ExportRange = ExportRange {
        start: 0.5,
        end: 1.5,
    };

    fn ffmpeg(arguments: &[&str], output: &Path) {
        let result = Command::new("ffmpeg")
            .args(["-v", "error"])
            .args(arguments)
            .arg(output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    fn two_second_recording(path: &Path) {
        std::fs::create_dir_all(path).unwrap();
        ffmpeg(
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=10:duration=2",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "1",
                "-threads",
                "1",
            ],
            &path.join("display.mp4"),
        );
        ffmpeg(
            &[
                "-f",
                "lavfi",
                "-i",
                r"aevalsrc=if(between(t\,0.5\,1.5)\,sin(2*PI*440*t)\,0):s=48000:d=2",
                "-c:a",
                "libopus",
            ],
            &path.join("audio.ogg"),
        );
        std::fs::write(
            path.join("recording-meta.json"),
            serde_json::to_vec(&serde_json::json!({
                "pretty_name": "Clip range",
                "display": { "path": "display.mp4", "fps": 10 },
                "audio": { "path": "audio.ogg" }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    async fn export_gif(project: &Path, output: &Path, range: Option<ExportRange>) -> PathBuf {
        let base = Box::pin(
            ExporterBase::builder(project.to_path_buf())
                .with_config(ProjectConfiguration::default())
                .with_output_path(output.to_path_buf())
                .with_range(range)
                .build_video_only(),
        )
        .await
        .unwrap();
        Box::pin(
            GifExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                quality: None,
            }
            .export(base, |_| true),
        )
        .await
        .unwrap()
    }

    fn gif_frames(path: &Path) -> Vec<image::Frame> {
        image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            std::fs::File::open(path).unwrap(),
        ))
        .unwrap()
        .into_frames()
        .collect_frames()
        .unwrap()
    }

    fn gif_seconds(frames: &[image::Frame]) -> f64 {
        frames
            .iter()
            .map(|frame| std::time::Duration::from(frame.delay()).as_secs_f64())
            .sum()
    }

    fn difference(left: &image::Frame, right: &image::Frame) -> u64 {
        left.buffer()
            .as_raw()
            .iter()
            .zip(right.buffer().as_raw())
            .map(|(left, right)| u64::from(left.abs_diff(*right)))
            .sum()
    }

    #[derive(Debug, PartialEq)]
    struct Mp4Streams {
        video_frames: u64,
        first_video_frame: f64,
        video_seconds: f64,
        audio_seconds: f64,
    }

    fn probe(path: &Path, stream: &str, entries: &str) -> serde_json::Value {
        let result = Command::new("ffprobe")
            .args(["-v", "error", "-count_frames", "-select_streams", stream])
            .args(["-show_entries", entries, "-of", "json"])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["streams"][0].clone()
    }

    fn number(value: &serde_json::Value) -> f64 {
        value.as_str().unwrap().parse().unwrap()
    }

    fn mp4_streams(path: &Path) -> Mp4Streams {
        let video = probe(path, "v:0", "stream=nb_read_frames,start_time,duration");
        let audio = probe(path, "a:0", "stream=duration");
        Mp4Streams {
            video_frames: number(&video["nb_read_frames"]) as u64,
            first_video_frame: number(&video["start_time"]),
            video_seconds: number(&video["duration"]),
            audio_seconds: number(&audio["duration"]),
        }
    }

    fn loudness(path: &Path, from_seconds: f64, to_seconds: f64) -> f64 {
        let result = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-f", "f32le", "-ac", "1", "-ar", "48000", "-"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let samples: Vec<f64> = result
            .stdout
            .chunks_exact(4)
            .map(|bytes| f64::from(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])))
            .collect();
        let stretch =
            &samples[(from_seconds * 48_000.0) as usize..(to_seconds * 48_000.0) as usize];
        (stretch.iter().map(|sample| sample * sample).sum::<f64>() / stretch.len() as f64).sqrt()
    }

    async fn export_mp4(
        project: &Path,
        output: &Path,
        range: Option<ExportRange>,
    ) -> (u32, PathBuf) {
        let base = Box::pin(
            ExporterBase::builder(project.to_path_buf())
                .with_config(ProjectConfiguration::default())
                .with_output_path(output.to_path_buf())
                .with_range(range)
                .build_for_mp4(Default::default()),
        )
        .await
        .unwrap();
        let expected = base.total_frames(FPS);
        let path = Box::pin(mp4_settings().export_prepared(base, |_| true))
            .await
            .unwrap();
        (expected, path)
    }

    fn mp4_settings() -> Mp4ExportSettings {
        Mp4ExportSettings {
            fps: FPS,
            resolution_base: XY::new(64, 64),
            compression: crate::mp4::ExportCompression::Maximum,
            custom_bpp: None,
            force_ffmpeg_decoder: false,
            optimize_filesize: false,
            codec: crate::mp4::ExportCodec::H264,
        }
    }

    static WATCHING_MEDIA_ACTIVITY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn watched_progress() -> (
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        impl FnMut(u32) -> bool + Send + 'static,
    ) {
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let report = seen.clone();
        let progress = move |_| {
            report.fetch_or(
                frameever_editor::MediaActivity::any_running(),
                std::sync::atomic::Ordering::Relaxed,
            );
            true
        };
        (seen, progress)
    }

    async fn video_base(project: &Path, output: PathBuf) -> ExporterBase {
        Box::pin(
            ExporterBase::builder(project.to_path_buf())
                .with_config(ProjectConfiguration::default())
                .with_output_path(output)
                .build_video_only(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn preparing_an_export_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let base = Box::pin(video_base(&project, directory.path().join("all.mp4"))).await;
        assert!(frameever_editor::MediaActivity::any_running());
        drop(base);
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_gif_export_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let base = Box::pin(video_base(&project, directory.path().join("all.gif"))).await;
        let (seen, progress) = watched_progress();
        Box::pin(
            GifExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                quality: None,
            }
            .export(base, progress),
        )
        .await
        .unwrap();
        assert!(seen.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_mov_export_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let base = Box::pin(video_base(&project, directory.path().join("all.mov"))).await;
        let (seen, progress) = watched_progress();
        Box::pin(
            crate::mov::MovExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                cursor_only: false,
            }
            .export(base, progress),
        )
        .await
        .unwrap();
        assert!(seen.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn an_mp4_export_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let base = Box::pin(
            ExporterBase::builder(project)
                .with_config(ProjectConfiguration::default())
                .with_output_path(directory.path().join("all.mp4"))
                .build_for_mp4(Default::default()),
        )
        .await
        .unwrap();
        let (seen, progress) = watched_progress();
        Box::pin(mp4_settings().export_prepared(base, progress))
            .await
            .unwrap();
        assert!(seen.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_tracks_export_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let mut seen = false;
        Box::pin(crate::tracks::export_tracks(
            project,
            Some(ProjectConfiguration::default()),
            crate::tracks::TrackExportSettings {
                tracks: vec![crate::tracks::TrackKind::Screen],
                fps: FPS,
                force_ffmpeg_decoder: true,
            },
            directory.path().join("tracks"),
            Default::default(),
            |_| seen |= frameever_editor::MediaActivity::any_running(),
        ))
        .await
        .unwrap();
        assert!(seen);
    }

    #[test]
    #[ignore = "requires ffmpeg"]
    fn a_gif_of_an_instant_recording_keeps_background_analysis_waiting() {
        let _alone = WATCHING_MEDIA_ACTIVITY.blocking_lock();
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let mut seen = false;
        crate::instant_gif::save_gif(
            &project.join("display.mp4"),
            &directory.path().join("instant.gif"),
            CLIP,
            crate::instant_gif::GifSize::Small,
            crate::gif::GifQualityPreset::Low,
            |_, _| {
                seen |= frameever_editor::MediaActivity::any_running();
                true
            },
        )
        .unwrap();
        assert!(seen);
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_cancelled_mp4_of_a_clip_leaves_no_file() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let output = directory.path().join("clip.mp4");
        let base = Box::pin(
            ExporterBase::builder(project)
                .with_config(ProjectConfiguration::default())
                .with_output_path(output.clone())
                .with_range(Some(CLIP))
                .build_for_mp4(Default::default()),
        )
        .await
        .unwrap();

        let result = Box::pin(mp4_settings().export_prepared(base, |frame| frame < 5)).await;

        assert!(result.is_err(), "{result:?}");
        assert!(!output.exists());
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_gif_of_a_clip_holds_the_frames_of_that_clip() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);

        let whole =
            gif_frames(&export_gif(&project, &directory.path().join("whole.gif"), None).await);
        let clip =
            gif_frames(&export_gif(&project, &directory.path().join("clip.gif"), Some(CLIP)).await);

        assert_eq!(whole.len(), 21);
        assert!((gif_seconds(&whole) - 2.1).abs() < 0.05);
        assert_eq!(clip.len(), 10);
        assert!((gif_seconds(&clip) - 1.0).abs() < 0.05);
        for (offset, frame) in clip.iter().enumerate() {
            let nearest = (0..whole.len())
                .min_by_key(|index| difference(frame, &whole[*index]))
                .unwrap();
            assert_eq!(nearest, 5 + offset);
        }
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn an_mp4_of_a_clip_holds_the_frames_and_sound_of_that_clip() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);

        let (whole_frames, whole) =
            export_mp4(&project, &directory.path().join("whole.mp4"), None).await;
        let (clip_frames, clip) =
            export_mp4(&project, &directory.path().join("clip.mp4"), Some(CLIP)).await;

        assert_eq!(whole_frames, 21);
        assert_eq!(clip_frames, 10);
        let whole = mp4_streams(&whole);
        assert_eq!(whole.video_frames, 21);
        assert!((whole.video_seconds - 2.1).abs() < 0.01, "{whole:?}");
        assert!((whole.audio_seconds - 2.0).abs() < 0.05, "{whole:?}");
        let clip = mp4_streams(&clip);
        assert_eq!(clip.video_frames, 10);
        assert_eq!(clip.first_video_frame, 0.0);
        assert!((clip.video_seconds - 1.0).abs() < 0.01, "{clip:?}");
        assert!((clip.audio_seconds - 1.0).abs() < 0.05, "{clip:?}");
        let whole_path = directory.path().join("whole.mp4");
        let clip_path = directory.path().join("clip.mp4");
        assert!(loudness(&whole_path, 0.1, 0.4) < 0.01);
        assert!(loudness(&whole_path, 0.6, 0.9) > 0.1);
        assert!(loudness(&clip_path, 0.1, 0.4) > 0.1);
        assert!(loudness(&clip_path, 0.6, 0.9) > 0.1);
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_mov_of_a_clip_holds_the_frames_of_that_clip() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let base = Box::pin(
            ExporterBase::builder(project)
                .with_config(ProjectConfiguration::default())
                .with_output_path(directory.path().join("clip.mov"))
                .with_range(Some(CLIP))
                .build_video_only(),
        )
        .await
        .unwrap();

        let path = Box::pin(
            crate::mov::MovExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                cursor_only: false,
            }
            .export(base, |_| true),
        )
        .await
        .unwrap();

        let video = probe(&path, "v:0", "stream=nb_read_frames,start_time,duration");
        assert_eq!(number(&video["nb_read_frames"]), 10.0);
        assert_eq!(number(&video["start_time"]), 0.0);
        assert!((number(&video["duration"]) - 1.0).abs() < 0.01, "{video}");
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_range_outside_the_timeline_is_nothing_to_export() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);

        let result = Box::pin(
            ExporterBase::builder(project)
                .with_config(ProjectConfiguration::default())
                .with_range(Some(ExportRange {
                    start: 5.0,
                    end: 6.0,
                }))
                .build_video_only(),
        )
        .await;

        assert!(matches!(
            result,
            Err(crate::ExporterBuildError::NothingToExport)
        ));
    }

    #[tokio::test]
    #[ignore = "requires ffmpeg and a wgpu adapter"]
    async fn a_clip_shorter_than_one_frame_is_nothing_to_export() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project.ever");
        two_second_recording(&project);
        let between_frames = Some(ExportRange {
            start: 0.31,
            end: 0.39,
        });
        let builder = |name: &str| {
            ExporterBase::builder(project.clone())
                .with_config(ProjectConfiguration::default())
                .with_output_path(directory.path().join(name))
                .with_range(between_frames)
        };
        let nothing = Err(crate::ExporterBuildError::NothingToExport.to_string());

        let gif = Box::pin(
            GifExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                quality: None,
            }
            .export(
                Box::pin(builder("short.gif").build_video_only())
                    .await
                    .unwrap(),
                |_| true,
            ),
        )
        .await;
        let mov = Box::pin(
            crate::mov::MovExportSettings {
                fps: FPS,
                resolution_base: XY::new(64, 64),
                cursor_only: false,
            }
            .export(
                Box::pin(builder("short.mov").build_video_only())
                    .await
                    .unwrap(),
                |_| true,
            ),
        )
        .await;
        let mp4 = Box::pin(
            mp4_settings().export_prepared(
                Box::pin(builder("short.mp4").build_for_mp4(Default::default()))
                    .await
                    .unwrap(),
                |_| true,
            ),
        )
        .await;

        assert_eq!(gif, nothing);
        assert_eq!(mov, nothing);
        assert_eq!(mp4, nothing);
        for name in ["short.gif", "short.mov", "short.mp4"] {
            assert!(!directory.path().join(name).exists(), "{name}");
        }
    }

    #[test]
    fn a_range_covers_the_frames_whose_time_falls_inside_it() {
        let clip = ExportRange {
            start: 2.0,
            end: 5.0,
        };
        assert_eq!(clip.frames(30, 300), 60..150);
        assert_eq!(clip.frames(10, 100), 20..50);
        let off_grid = ExportRange {
            start: 0.25,
            end: 0.75,
        };
        assert_eq!(off_grid.frames(10, 100), 3..8);
    }

    #[test]
    fn neighbouring_ranges_share_no_frame_and_leave_none_out() {
        let bounds = [0.0, 0.1 + 0.2, 1.0 / 3.0 * 4.0, 2.7, 4.0];
        for fps in [10, 15, 24, 30, 60] {
            let total = 4 * fps;
            let mut next = 0;
            for pair in bounds.windows(2) {
                let frames = ExportRange {
                    start: pair[0],
                    end: pair[1],
                }
                .frames(fps, total);
                assert_eq!(frames.start, next, "{fps} fps, {pair:?}");
                next = frames.end;
            }
            assert_eq!(next, total, "{fps} fps");
        }
    }

    #[test]
    fn a_range_is_cut_to_the_timeline_and_may_be_empty() {
        let past_the_end = ExportRange {
            start: 8.0,
            end: 12.0,
        };
        assert_eq!(past_the_end.frames(10, 100), 80..100);
        for nothing in [
            ExportRange {
                start: 12.0,
                end: 14.0,
            },
            ExportRange {
                start: 3.0,
                end: 3.0,
            },
            ExportRange {
                start: 5.0,
                end: 2.0,
            },
            ExportRange {
                start: f64::NAN,
                end: 2.0,
            },
        ] {
            assert!(nothing.frames(10, 100).is_empty(), "{nothing:?}");
        }
        assert_eq!(
            ExportRange {
                start: -1.0,
                end: 0.5
            }
            .frames(10, 100),
            0..5
        );
    }
}
