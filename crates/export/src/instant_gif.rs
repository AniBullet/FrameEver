use std::path::Path;

use crate::ExportRange;

pub const FPS: u32 = 15;

const LONG_SIDE_LIMIT: f64 = 1080.0;
const ESTIMATED_BYTES_PER_PIXEL: f64 = 0.06;
const SMALLEST_ESTIMATE: f64 = 100_000.0;
const LARGE_BYTES: u64 = 20_000_000;

pub fn frame_count(range: ExportRange) -> u32 {
    ((range.end - range.start) * f64::from(FPS))
        .round()
        .max(1.0) as u32
}

pub fn estimated_bytes((width, height): (u32, u32), seconds: f64) -> u64 {
    (f64::from(width) * f64::from(height) * f64::from(FPS) * seconds * ESTIMATED_BYTES_PER_PIXEL)
        .max(SMALLEST_ESTIMATE) as u64
}

pub fn is_large(bytes: u64) -> bool {
    bytes > LARGE_BYTES
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GifFailure {
    Cancelled,
    VideoMissing,
    VideoUnreadable(String),
    FolderNotWritable(String),
}

fn unreadable(what: &str) -> impl Fn(ffmpeg::Error) -> GifFailure + '_ {
    move |error| GifFailure::VideoUnreadable(format!("{what}: {error}"))
}

fn not_writable<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> GifFailure + '_ {
    move |error| GifFailure::FolderNotWritable(format!("{what}: {error}"))
}

const TIME_TOLERANCE: f64 = 1e-4;

pub fn save_gif(
    video: &Path,
    output: &Path,
    range: ExportRange,
    size: GifSize,
    mut progress: impl FnMut(u32, u32) -> bool,
) -> Result<(), GifFailure> {
    if !video.is_file() {
        return Err(GifFailure::VideoMissing);
    }
    let mut input = ffmpeg::format::input(video).map_err(unreadable("open the video"))?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| GifFailure::VideoUnreadable("the file has no video".to_string()))?;
    let stream_index = stream.index();
    let time_base = f64::from(stream.time_base());
    let stream_start = match stream.start_time() {
        ffmpeg::ffi::AV_NOPTS_VALUE => 0,
        start => start,
    };
    let seconds = input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE);
    if seconds > 0.0 && range.start >= seconds {
        return Err(GifFailure::VideoUnreadable(
            "the stretch starts after the end of the video".to_string(),
        ));
    }
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().video())
        .map_err(unreadable("decode the video"))?;
    let (width, height) = size.pixels((decoder.width(), decoder.height()));
    let total = frame_count(range);

    if range.start > 0.0 {
        let position = ((range.start + stream_start as f64 * time_base)
            * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64;
        input
            .seek(position, ..position)
            .map_err(unreadable("find the start of the stretch"))?;
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(not_writable("create the folder"))?;
    }
    let staged = crate::prepare_new_output(output).map_err(not_writable("stage the GIF"))?;
    let encoder = frameever_enc_gif::GifEncoderWrapper::new(
        staged.as_ref().map_or(output, AsRef::as_ref),
        width,
        height,
        FPS,
    )
    .map_err(not_writable("start the GIF"))?;

    let mut frames = StretchFrames {
        encoder,
        scaler: None,
        scaled: ffmpeg::frame::Video::empty(),
        scaled_is_held: false,
        held: None,
        size: (width, height),
        range,
        total,
        written: 0,
    };
    let mut incoming = ffmpeg::frame::Video::empty();
    let mut last_time = range.start;
    let mut receive = |decoder: &mut ffmpeg::decoder::Video,
                       frames: &mut StretchFrames,
                       progress: &mut dyn FnMut(u32, u32) -> bool|
     -> Result<(), GifFailure> {
        while !frames.is_full() && decoder.receive_frame(&mut incoming).is_ok() {
            let time = incoming
                .timestamp()
                .map_or(last_time, |pts| (pts - stream_start) as f64 * time_base);
            last_time = time;
            frames.take(&mut incoming, time, progress)?;
        }
        Ok(())
    };

    if !progress(0, total) {
        return Err(GifFailure::Cancelled);
    }
    for (stream, packet) in input.packets() {
        if frames.is_full() {
            break;
        }
        if stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(unreadable("decode the video"))?;
        receive(&mut decoder, &mut frames, &mut progress)?;
    }
    decoder.send_eof().ok();
    receive(&mut decoder, &mut frames, &mut progress)?;
    frames.fill(&mut progress)?;

    frames
        .encoder
        .finish()
        .map_err(not_writable("finish the GIF"))?;
    if let Some(staged) = staged {
        staged
            .persist_noclobber(output)
            .map_err(not_writable("move the GIF into place"))?;
    }
    progress(total, total);
    Ok(())
}

struct StretchFrames {
    encoder: frameever_enc_gif::GifEncoderWrapper,
    scaler: Option<ffmpeg::software::scaling::Context>,
    scaled: ffmpeg::frame::Video,
    scaled_is_held: bool,
    held: Option<ffmpeg::frame::Video>,
    size: (u32, u32),
    range: ExportRange,
    total: u32,
    written: u32,
}

impl StretchFrames {
    fn is_full(&self) -> bool {
        self.written >= self.total
    }

    fn next_time(&self) -> f64 {
        self.range.start + f64::from(self.written) / f64::from(FPS)
    }

    fn take(
        &mut self,
        incoming: &mut ffmpeg::frame::Video,
        time: f64,
        progress: &mut dyn FnMut(u32, u32) -> bool,
    ) -> Result<(), GifFailure> {
        while self.held.is_some() && !self.is_full() && self.next_time() < time - TIME_TOLERANCE {
            self.write_held(progress)?;
        }
        let held = self.held.get_or_insert_with(ffmpeg::frame::Video::empty);
        std::mem::swap(held, incoming);
        self.scaled_is_held = false;
        Ok(())
    }

    fn fill(&mut self, progress: &mut dyn FnMut(u32, u32) -> bool) -> Result<(), GifFailure> {
        if self.held.is_none() {
            return Err(GifFailure::VideoUnreadable(
                "the video has no frames in that stretch".to_string(),
            ));
        }
        while !self.is_full() {
            self.write_held(progress)?;
        }
        Ok(())
    }

    fn write_held(&mut self, progress: &mut dyn FnMut(u32, u32) -> bool) -> Result<(), GifFailure> {
        let Some(held) = &self.held else {
            return Ok(());
        };
        if !self.scaled_is_held {
            let (width, height) = self.size;
            if self.scaler.is_none() {
                self.scaler = Some(
                    ffmpeg::software::scaling::Context::get(
                        held.format(),
                        held.width(),
                        held.height(),
                        ffmpeg::format::Pixel::RGBA,
                        width,
                        height,
                        ffmpeg::software::scaling::Flags::BICUBIC,
                    )
                    .map_err(unreadable("resize the video"))?,
                );
            }
            if let Some(scaler) = &mut self.scaler {
                scaler
                    .run(held, &mut self.scaled)
                    .map_err(unreadable("resize a frame"))?;
            }
            self.scaled_is_held = true;
        }
        self.encoder
            .add_frame(self.scaled.data(0), self.scaled.stride(0))
            .map_err(not_writable("add a frame to the GIF"))?;
        self.written += 1;
        if !progress(self.written.min(self.total - 1), self.total) {
            return Err(GifFailure::Cancelled);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifSize {
    Large,
    Medium,
    Small,
}

impl GifSize {
    pub const ALL: [Self; 3] = [Self::Large, Self::Medium, Self::Small];

    fn divisor(self) -> f64 {
        match self {
            Self::Large => 1.0,
            Self::Medium => 2.0,
            Self::Small => 4.0,
        }
    }

    pub fn pixels(self, (width, height): (u32, u32)) -> (u32, u32) {
        let fit = (f64::from(width.max(height)) / LONG_SIDE_LIMIT).max(1.0);
        let even = |side: u32| {
            let half = (f64::from(side) / fit / self.divisor() / 2.0).round();
            (half.max(1.0) as u32) * 2
        };
        (even(width), even(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frameever_enc_ffmpeg::{h264::H264Encoder, mp4::MP4File};
    use frameever_media_info::VideoInfo;
    use image::AnimationDecoder;
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    const SOURCE_FPS: u64 = 30;
    const SOURCE_FRAMES: u64 = 60;

    fn instant_recording(directory: &Path) -> PathBuf {
        ffmpeg::init().ok();
        let project = directory.join("Instant.ever");
        std::fs::create_dir_all(project.join("content")).unwrap();
        std::fs::write(
            project.join("recording-meta.json"),
            r#"{"pretty_name":"Instant","sharing":null,"fps":30,"sample_rate":48000}"#,
        )
        .unwrap();
        let info = VideoInfo {
            pixel_format: frameever_media_info::Pixel::NV12,
            width: 320,
            height: 240,
            time_base: ffmpeg::Rational(1, 1_000_000),
            frame_rate: ffmpeg::Rational(SOURCE_FPS as i32, 1),
        };
        let mut file = MP4File::init(
            "source",
            project.join("content/output.mp4"),
            false,
            |output| H264Encoder::builder(info).build(output),
            |_| None,
        )
        .unwrap();
        for index in 0..SOURCE_FRAMES {
            let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, 320, 240);
            frame.data_mut(0).fill(40 + (index * 3) as u8);
            frame.data_mut(1).fill(128);
            file.queue_video_frame(
                frame,
                Duration::from_nanos(index * 1_000_000_000 / SOURCE_FPS),
            )
            .unwrap();
        }
        file.finish().unwrap();
        project
    }

    fn video_size(video: &Path) -> Option<(u32, u32)> {
        let input = ffmpeg::format::input(video).ok()?;
        let stream = input.streams().best(ffmpeg::media::Type::Video)?;
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .ok()?
            .decoder()
            .video()
            .ok()?;
        Some((decoder.width(), decoder.height()))
    }

    fn video(project: &Path) -> PathBuf {
        project.join("content/output.mp4")
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

    fn brightness(frame: &image::Frame) -> i64 {
        i64::from(frame.buffer().get_pixel(0, 0).0[0])
    }

    const WHOLE: ExportRange = ExportRange {
        start: 0.0,
        end: 2.0,
    };

    #[tokio::test]
    async fn the_export_pipeline_cannot_open_an_instant_recording() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());

        let refused = crate::ExporterBase::builder(project)
            .with_config(Default::default())
            .with_range(Some(WHOLE))
            .build_video_only()
            .await;

        assert!(matches!(
            refused,
            Err(crate::ExporterBuildError::NotStudioRecording)
        ));
    }

    #[test]
    fn a_gif_holds_the_chosen_stretch_at_fifteen_frames_a_second() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());
        let whole = directory.path().join("whole.gif");
        let stretch = directory.path().join("stretch.gif");

        let mut reports = Vec::new();
        save_gif(
            &video(&project),
            &whole,
            WHOLE,
            GifSize::Large,
            |done, total| {
                reports.push((done, total));
                true
            },
        )
        .unwrap();
        save_gif(
            &video(&project),
            &stretch,
            ExportRange {
                start: 0.4,
                end: 1.4,
            },
            GifSize::Large,
            |_, _| true,
        )
        .unwrap();

        let whole = gif_frames(&whole);
        let stretch = gif_frames(&stretch);
        assert_eq!(whole.len(), 30);
        assert_eq!(stretch.len(), 15);
        let seconds: f64 = stretch
            .iter()
            .map(|frame| Duration::from(frame.delay()).as_secs_f64())
            .sum();
        assert!((seconds - 1.0).abs() < 0.08, "{seconds}");
        assert!(
            whole
                .windows(2)
                .all(|pair| brightness(&pair[0]) < brightness(&pair[1]))
        );
        for (index, frame) in stretch.iter().enumerate() {
            let nearest = (0..whole.len())
                .min_by_key(|other| (brightness(&whole[*other]) - brightness(frame)).abs())
                .unwrap();
            assert_eq!(nearest, index + 6);
        }
        assert_eq!(reports.first(), Some(&(0, 30)));
        assert_eq!(reports.last(), Some(&(30, 30)));
    }

    #[test]
    fn each_size_gives_its_even_pixel_size() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());
        let short = ExportRange {
            start: 0.0,
            end: 0.2,
        };

        let sizes = GifSize::ALL.map(|size| {
            let output = directory.path().join(format!("{size:?}.gif"));
            save_gif(&video(&project), &output, short, size, |_, _| true).unwrap();
            let frames = gif_frames(&output);
            assert_eq!(frames.len(), 3);
            frames[0].buffer().dimensions()
        });

        assert_eq!(sizes, [(320, 240), (160, 120), (80, 60)]);
    }

    #[test]
    fn a_cancelled_gif_leaves_no_file_behind() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());
        let exports = directory.path().join("Exports");
        std::fs::create_dir_all(&exports).unwrap();

        let error = save_gif(
            &video(&project),
            &exports.join("cancelled.gif"),
            WHOLE,
            GifSize::Small,
            |done, _| done < 2,
        )
        .unwrap_err();

        assert_eq!(error, GifFailure::Cancelled);
        assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 0);
    }

    #[test]
    fn a_gif_comes_from_the_compressed_copy_when_it_is_the_only_video() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());
        let compressed = project.join("content/output_compressed.mp4");
        crate::compress::compress_to_hevc(&video(&project), &compressed, |_, _| true).unwrap();
        std::fs::rename(&compressed, video(&project)).unwrap();
        let before = std::fs::read(video(&project)).unwrap();
        let output = directory.path().join("compressed.gif");

        save_gif(
            &video(&project),
            &output,
            ExportRange {
                start: 1.0,
                end: 2.0,
            },
            GifSize::Medium,
            |_, _| true,
        )
        .unwrap();

        let frames = gif_frames(&output);
        assert_eq!(frames.len(), 15);
        assert_eq!(frames[0].buffer().dimensions(), (160, 120));
        assert!(brightness(&frames[0]) < brightness(&frames[14]));
        assert_eq!(std::fs::read(video(&project)).unwrap(), before);
    }

    #[test]
    #[ignore = "benchmark: set FRAMEEVER_GIF_SOURCE to a video"]
    fn benchmark_save_gif() {
        ffmpeg::init().ok();
        let source = PathBuf::from(std::env::var("FRAMEEVER_GIF_SOURCE").unwrap());
        let seconds = std::env::var("FRAMEEVER_GIF_SECONDS")
            .ok()
            .and_then(|seconds| seconds.parse().ok())
            .unwrap_or(5.0);
        let directory = tempfile::tempdir().unwrap();
        let pixels = video_size(&source).unwrap();
        for size in GifSize::ALL {
            let output = directory.path().join(format!("{size:?}.gif"));
            let started = std::time::Instant::now();
            save_gif(
                &source,
                &output,
                ExportRange {
                    start: 0.0,
                    end: seconds,
                },
                size,
                |_, _| true,
            )
            .unwrap();
            println!(
                "{size:?} {:?} {seconds} s: {:.2} s, {} bytes, estimate {} bytes",
                size.pixels(pixels),
                started.elapsed().as_secs_f64(),
                std::fs::metadata(&output).unwrap().len(),
                estimated_bytes(size.pixels(pixels), seconds),
            );
        }
    }

    #[test]
    fn a_failure_says_whether_the_video_or_the_folder_is_the_problem_and_writes_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let project = instant_recording(directory.path());
        let exports = directory.path().join("Exports");
        std::fs::create_dir_all(&exports).unwrap();
        let damaged = project.join("content/damaged.mp4");
        std::fs::write(&damaged, b"not a video").unwrap();
        let blocked = directory.path().join("blocked");
        std::fs::write(&blocked, b"a file where the folder should be").unwrap();
        let save = |video: &Path, output: &Path, range| {
            save_gif(video, output, range, GifSize::Small, |_, _| true)
        };

        assert_eq!(
            save(
                &project.join("content/gone.mp4"),
                &exports.join("gone.gif"),
                WHOLE
            ),
            Err(GifFailure::VideoMissing)
        );
        assert!(matches!(
            save(&damaged, &exports.join("damaged.gif"), WHOLE),
            Err(GifFailure::VideoUnreadable(_))
        ));
        assert!(matches!(
            save(
                &video(&project),
                &exports.join("past.gif"),
                ExportRange {
                    start: 5.0,
                    end: 6.0,
                },
            ),
            Err(GifFailure::VideoUnreadable(_))
        ));
        assert!(matches!(
            save(&video(&project), &blocked.join("blocked.gif"), WHOLE),
            Err(GifFailure::FolderNotWritable(_))
        ));
        assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 0);
    }

    #[test]
    fn large_keeps_the_recording_up_to_1080_on_the_long_side_and_the_others_halve_it() {
        let sizes = |source| GifSize::ALL.map(|size| size.pixels(source));

        assert_eq!(sizes((800, 600)), [(800, 600), (400, 300), (200, 150)]);
        assert_eq!(sizes((1920, 1080)), [(1080, 608), (540, 304), (270, 152)]);
        assert_eq!(sizes((1080, 1920)), [(608, 1080), (304, 540), (152, 270)]);
        assert_eq!(sizes((2560, 1440)), [(1080, 608), (540, 304), (270, 152)]);
        assert_eq!(sizes((1366, 768)), [(1080, 608), (540, 304), (270, 152)]);
        assert_eq!(sizes((6, 4)), [(6, 4), (4, 2), (2, 2)]);
    }

    #[test]
    fn the_estimate_grows_with_pixels_and_seconds_and_warns_above_20_mb() {
        assert_eq!(estimated_bytes((540, 304), 10.0), 1_477_440);
        assert_eq!(estimated_bytes((1080, 608), 10.0), 5_909_760);
        assert_eq!(estimated_bytes((1080, 608), 40.0), 23_639_040);
        assert_eq!(estimated_bytes((270, 152), 5.0), 184_680);
        assert_eq!(estimated_bytes((2, 2), 0.1), 100_000);

        assert!(!is_large(5_909_760));
        assert!(!is_large(20_000_000));
        assert!(is_large(23_639_040));
    }

    #[test]
    fn a_stretch_holds_fifteen_frames_a_second_and_never_none() {
        let frames = |start, end| frame_count(ExportRange { start, end });

        assert_eq!(frames(0.0, 2.0), 30);
        assert_eq!(frames(0.5, 1.5), 15);
        assert_eq!(frames(3.0, 3.5), 8);
        assert_eq!(frames(1.0, 1.01), 1);
        assert_eq!(frames(2.0, 1.0), 1);
    }
}
