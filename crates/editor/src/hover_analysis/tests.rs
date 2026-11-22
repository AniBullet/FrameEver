use std::{cell::Cell, path::Path, sync::OnceLock, time::Duration};

use frameever_enc_ffmpeg::segmented_stream::{SegmentedVideoEncoder, SegmentedVideoEncoderConfig};
use frameever_project::{CursorClickEvent, CursorEvents, CursorMoveEvent};

use super::*;

const W: usize = 120;
const H: usize = 80;

pub(crate) const HIGHLIGHT_AT_MS: f64 = 1000.0;

pub(crate) const RECORDING_SECS: u32 = 4;

pub(crate) fn highlight_recording(display: &Path) {
    static ENCODED_ONCE_FOR_ALL_TESTS: OnceLock<Vec<u8>> = OnceLock::new();
    std::fs::write(
        display,
        ENCODED_ONCE_FOR_ALL_TESTS.get_or_init(encode_highlight_recording),
    )
    .unwrap();
}

fn encode_highlight_recording() -> Vec<u8> {
    const FPS: u32 = 30;
    const WIDTH: usize = 320;
    const HEIGHT: usize = 240;
    ffmpeg::init().unwrap();
    let parts = tempfile::tempdir().unwrap();
    let mut encoder = SegmentedVideoEncoder::init(
        parts.path().to_path_buf(),
        frameever_media_info::VideoInfo {
            pixel_format: frameever_media_info::Pixel::NV12,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            time_base: ffmpeg::Rational(1, 1_000_000),
            frame_rate: ffmpeg::Rational(FPS as i32, 1),
        },
        SegmentedVideoEncoderConfig {
            segment_duration: Duration::from_secs(1),
            bpp: 2.0,
            ..Default::default()
        },
    )
    .unwrap();
    for index in 0..RECORDING_SECS * FPS {
        let time_ms = f64::from(index) * 1000.0 / f64::from(FPS);
        let mut frame =
            ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, WIDTH as u32, HEIGHT as u32);
        frame.data_mut(0).fill(220);
        frame.data_mut(1).fill(128);
        if time_ms >= HIGHLIGHT_AT_MS {
            let stride = frame.stride(0);
            let luma = frame.data_mut(0);
            for row in 100..140 {
                luma[row * stride + 120..row * stride + 200].fill(60);
            }
        }
        encoder
            .queue_frame(frame, Duration::from_secs_f64(time_ms / 1000.0))
            .unwrap();
    }
    encoder.finish().unwrap();
    let mut media = std::fs::read_dir(parts.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "m4s"))
        .collect::<Vec<_>>();
    media.sort();
    let joined = parts.path().join("joined.mp4");
    frameever_enc_ffmpeg::remux::concatenate_m4s_segments_with_init(
        &parts.path().join("init.mp4"),
        &media,
        &joined,
    )
    .unwrap();
    std::fs::read(joined).unwrap()
}

pub(crate) fn pointer_in_the_middle(seconds: u32) -> CursorEvents {
    CursorEvents {
        moves: (0..=seconds * 10)
            .map(|step| CursorMoveEvent {
                active_modifiers: Vec::new(),
                cursor_id: "arrow".into(),
                time_ms: f64::from(step) * 100.0,
                x: 0.5,
                y: 0.5,
            })
            .collect(),
        clicks: Vec::new(),
    }
}

fn blank() -> Vec<u8> {
    vec![240; W * H]
}

fn with_rect(mut luma: Vec<u8>, x0: usize, y0: usize, w: usize, h: usize, value: u8) -> Vec<u8> {
    for y in y0..y0 + h {
        luma[y * W + x0..y * W + x0 + w].fill(value);
    }
    luma
}

fn button(value: u8) -> Vec<u8> {
    with_rect(blank(), 40, 30, 40, 16, value)
}

fn detect(frames: impl IntoIterator<Item = (Vec<u8>, Option<(usize, usize)>, bool)>) -> Vec<f64> {
    let mut detector = HoverDetector::new(W, H);
    for (index, (luma, pointer, busy)) in frames.into_iter().enumerate() {
        detector.push(index as f64 * 33.0, luma, pointer, busy);
    }
    detector.finish()
}

const ON_THE_BUTTON: Option<(usize, usize)> = Some((60, 38));

#[test]
fn a_control_lighting_up_under_the_pointer_is_found() {
    let frames = (0..10).map(|i| {
        let frame = if i < 5 { blank() } else { button(60) };
        (frame, ON_THE_BUTTON, false)
    });
    assert_eq!(detect(frames), vec![5.0 * 33.0]);
}

#[test]
fn a_control_that_flashes_for_one_frame_is_found() {
    let frames = (0..12).map(|i| {
        let frame = if i == 5 { button(60) } else { blank() };
        (frame, ON_THE_BUTTON, false)
    });
    assert_eq!(detect(frames), vec![5.0 * 33.0]);
}

#[test]
fn a_highlight_at_the_very_end_is_found() {
    let frames = (0..10).map(|i| {
        let frame = if i < 9 { blank() } else { button(60) };
        (frame, ON_THE_BUTTON, false)
    });
    assert_eq!(detect(frames), vec![9.0 * 33.0]);
}

#[test]
fn nothing_changing_finds_nothing() {
    let frames = (0..20).map(|i| (blank(), Some((10 + i, 38)), false));
    assert!(detect(frames).is_empty());
}

#[test]
fn a_change_away_from_the_pointer_is_ignored() {
    let frames = (0..10).map(|i| {
        let frame = if i < 5 { blank() } else { button(60) };
        (frame, Some((5, 5)), false)
    });
    assert!(detect(frames).is_empty());
}

#[test]
fn a_change_of_most_of_the_screen_is_ignored() {
    let frames = (0..10).map(|i| {
        let frame = if i < 5 { blank() } else { vec![30; W * H] };
        (frame, ON_THE_BUTTON, false)
    });
    assert!(detect(frames).is_empty());
}

#[test]
fn changes_while_the_button_is_held_are_ignored() {
    let frames = (0..10).map(|i| {
        let frame = if i < 5 { blank() } else { button(60) };
        (frame, ON_THE_BUTTON, true)
    });
    assert!(detect(frames).is_empty());
}

#[test]
fn continuous_change_under_the_pointer_is_ignored() {
    let frames = (0..30).map(|i| {
        let value = if i % 2 == 0 { 40 } else { 200 };
        (button(value), ON_THE_BUTTON, false)
    });
    assert!(detect(frames).is_empty());
}

#[test]
fn an_animation_with_repeated_frames_is_ignored() {
    let frames = (0..40).map(|i| {
        let value = if (i / 2) % 2 == 0 { 40 } else { 200 };
        (button(value), ON_THE_BUTTON, false)
    });
    assert!(detect(frames).is_empty());
}

#[test]
fn content_scrolling_under_a_still_pointer_is_ignored() {
    let page = |offset: usize| {
        (0..12).fold(blank(), |luma, line| {
            let y = (line * 9 + 80 - offset % 80) % 76;
            with_rect(luma, 20, y, 80, 3, 30)
        })
    };
    let frames = (0..12).map(|i| (page(if i < 6 { 0 } else { 6 }), Some((60, 40)), false));
    assert!(detect(frames).is_empty());
}

fn buttons(events: &[(u8, bool, f64)]) -> PointerTrack {
    PointerTrack::new(&CursorEvents {
        clicks: events
            .iter()
            .map(|&(button, down, time_ms)| CursorClickEvent {
                active_modifiers: Vec::new(),
                cursor_num: button,
                cursor_id: "arrow".into(),
                time_ms,
                down,
            })
            .collect(),
        moves: Vec::new(),
    })
}

#[test]
fn releasing_one_button_keeps_the_guard_while_another_is_held() {
    let left_held_longest = buttons(&[
        (0, true, 1000.0),
        (1, true, 1500.0),
        (1, false, 1600.0),
        (0, false, 3000.0),
    ]);
    assert!(left_held_longest.busy(2000.0));
    assert!(left_held_longest.busy(2800.0));
    assert!(!left_held_longest.busy(3400.0));

    let right_held_longest = buttons(&[
        (0, true, 1000.0),
        (1, true, 1500.0),
        (0, false, 1600.0),
        (1, false, 3000.0),
    ]);
    assert!(right_held_longest.busy(2000.0));
    assert!(!right_held_longest.busy(3400.0));
}

#[test]
fn a_release_without_its_press_does_not_end_a_held_button() {
    let track = buttons(&[(0, true, 1000.0), (1, false, 1500.0), (0, false, 3000.0)]);
    assert!(track.busy(2000.0));
    assert!(!track.busy(3400.0));
}

#[test]
fn every_held_stretch_is_guarded() {
    let track = buttons(&[
        (0, true, 1000.0),
        (0, false, 2000.0),
        (0, true, 5000.0),
        (0, false, 6000.0),
        (0, true, 9000.0),
    ]);
    for time in [1500.0, 5500.0, 20_000.0] {
        assert!(track.busy(time), "{time}");
    }
    for time in [500.0, 3000.0, 8000.0] {
        assert!(!track.busy(time), "{time}");
    }
}

#[test]
fn a_recording_is_analysed_in_cursor_time_and_remembered() {
    let folder = tempfile::tempdir().unwrap();
    let display = folder.path().join("display.mp4");
    highlight_recording(&display);

    let changes = analyze(
        &display,
        &pointer_in_the_middle(RECORDING_SECS + 1),
        250.0,
        &|| Schedule::Run,
    )
    .unwrap()
    .expect("analysis should finish");

    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(
        (changes[0] - (HIGHLIGHT_AT_MS + 250.0)).abs() < 2.0,
        "{changes:?}"
    );
    assert_eq!(cached_changes(&display), Some(changes));

    let mut video = std::fs::read(&display).unwrap();
    video.extend_from_slice(&[0; 8]);
    std::fs::write(&display, video).unwrap();
    assert_eq!(cached_changes(&display), None);
}

#[test]
fn an_older_detectors_result_is_not_used() {
    let folder = tempfile::tempdir().unwrap();
    let display = folder.path().join("display.mp4");
    std::fs::write(&display, [1, 2, 3]).unwrap();
    let cache = folder.path().join("display.cursor-hover.json");
    for (version, expected) in [
        (DETECTOR_VERSION, Some(vec![125.0])),
        (DETECTOR_VERSION - 1, None),
    ] {
        std::fs::write(
            &cache,
            serde_json::json!({ "version": version, "displayBytes": 3, "changesMs": [125.0] })
                .to_string(),
        )
        .unwrap();
        assert_eq!(cached_changes(&display), expected);
    }
}

#[test]
fn analysis_that_has_to_wait_does_not_open_the_video() {
    let missing = Path::new("no-such-recording.mp4");
    let cursor = pointer_in_the_middle(1);
    let asked = Cell::new(0);
    let waits_then_stops = || {
        asked.set(asked.get() + 1);
        if asked.get() < 3 {
            Schedule::Wait
        } else {
            Schedule::Stop
        }
    };

    assert_eq!(analyze(missing, &cursor, 0.0, &waits_then_stops), Ok(None));
    assert!(analyze(missing, &cursor, 0.0, &|| Schedule::Run).is_err());
}

#[test]
fn analysis_stopped_midway_saves_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let display = folder.path().join("display.mp4");
    highlight_recording(&display);
    let asked = Cell::new(0);
    let stops_midway = || {
        asked.set(asked.get() + 1);
        if asked.get() < 40 {
            Schedule::Run
        } else {
            Schedule::Stop
        }
    };

    let result = analyze(
        &display,
        &pointer_in_the_middle(RECORDING_SECS),
        0.0,
        &stops_midway,
    );

    assert_eq!(result, Ok(None));
    assert!(asked.get() >= 40);
    assert!(!folder.path().join("display.cursor-hover.json").exists());
}
