use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use frameever_verify::{
    audio_code::AudioSource,
    fixture::{self, AudioTrackSpec, FixtureSpec, SegmentSpec, Timing, VideoTrackSpec},
};

use super::*;
use crate::library::view::{Compression, ExportFile};

const LONG_WAIT: Duration = Duration::from_secs(20);
const BOUNDS: (u32, u32) = (160, 90);

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(segments: Vec<SegmentSpec>) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "frameever-player-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let spec = FixtureSpec {
            name: "player".to_string(),
            segments,
        };
        fixture::write_fixture(&spec, &dir).unwrap();
        Self { dir }
    }

    fn entry(&self) -> Entry {
        Entry {
            path: self.dir.clone(),
            kind: MediaKind::Studio,
            name: "player".to_string(),
            size: 0,
            duration: None,
            created_millis: 0.,
            exports: Vec::new(),
            compression: Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: true,
            audio_only: false,
            id: None,
            rebuilt: None,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("leaving {}: {error}", self.dir.display());
        }
    }
}

fn silent_segment(duration: f64) -> SegmentSpec {
    SegmentSpec {
        display: VideoTrackSpec::new(320, 180, 30, Timing::Constant { duration }),
        camera: None,
        mic: None,
        system_audio: None,
    }
}

fn audio_track(source: AudioSource, duration: f64, start_time: f64) -> AudioTrackSpec {
    AudioTrackSpec {
        source,
        sample_rate: 48_000,
        channels: 1,
        duration,
        start_time,
    }
}

fn start_at(position: f64) -> Start {
    Start {
        position,
        muted: true,
        volume: 1.,
        looping: false,
        bounds: BOUNDS,
    }
}

fn open(segments: Vec<Segment>, position: f64) -> (Player, flume::Receiver<Event>) {
    let project = segments[0].video.clone();
    Player::open(&project, move || Ok(segments), start_at(position), None).unwrap()
}

fn open_looping(segments: Vec<Segment>, looping: bool) -> (Player, flume::Receiver<Event>) {
    let project = segments[0].video.clone();
    let start = Start {
        looping,
        ..start_at(0.)
    };
    Player::open(&project, move || Ok(segments), start, None).unwrap()
}

fn assert_plays_on_from_the_start(segments: Vec<Segment>, late: f64) {
    let (player, events) = open_looping(segments, true);
    player.play();
    let deadline = Instant::now() + LONG_WAIT;
    let mut reached_late = false;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left).expect("it never started over") {
            Event::Ended => panic!("a looping preview ended"),
            Event::Frame => {
                let position = player.take_frame().unwrap().position;
                reached_late |= position > late;
                if reached_late && position < 0.1 {
                    break;
                }
            }
            _ => {}
        }
    }
    let shown = player.stats().shown;
    let settle = Instant::now() + Duration::from_millis(300);
    while let Ok(event) = events.recv_deadline(settle) {
        assert_ne!(event, Event::Ended);
        if event == Event::Frame {
            player.take_frame();
        }
    }
    assert!(player.stats().shown > shown + 3, "{:?}", player.stats());
}

fn wait_for_event(
    events: &flume::Receiver<Event>,
    mut matches: impl FnMut(&Event) -> bool,
) -> Event {
    let deadline = Instant::now() + LONG_WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = events.recv_timeout(left).expect("the event never came");
        if matches(&event) {
            return event;
        }
    }
}

#[test]
fn a_recording_plays_to_the_end_at_its_own_pace() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let (player, events) = open(segments, 0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    assert_eq!(player.take_frame().unwrap().position, 0.);
    let started = Instant::now();
    player.play();
    wait_for_event(&events, |event| *event == Event::Ended);
    let elapsed = started.elapsed().as_secs_f64();
    assert!(
        (0.9..1.5).contains(&elapsed),
        "a one second clip took {elapsed:.3} s"
    );
    let stats = player.stats();
    assert_eq!(stats.shown + stats.skipped, 30, "{stats:?}");
}

#[test]
fn frames_are_scaled_down_to_the_bounds() {
    let fixture = Fixture::new(vec![silent_segment(0.5)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let image = player.take_frame().unwrap().image;
    let size = image.size(0);
    assert_eq!((i32::from(size.width), i32::from(size.height)), (160, 90));
}

#[test]
fn a_paused_seek_shows_the_frame_at_that_time() {
    let fixture = Fixture::new(vec![silent_segment(2.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    player.seek(1.5);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let shown = player.take_frame().unwrap().position;
    assert!((shown - 1.5).abs() < 0.04, "showed {shown}");
}

#[test]
fn a_seek_past_the_end_shows_the_last_frame() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    player.seek(5.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let shown = player.take_frame().unwrap().position;
    assert!(shown > 0.9, "showed {shown}");
}

#[test]
fn a_looping_preview_plays_on_from_the_start_without_ending() {
    let fixture = Fixture::new(vec![silent_segment(0.5)]);
    assert_plays_on_from_the_start(segments_of(&fixture.entry()).unwrap(), 0.3);
}

#[test]
fn a_looping_preview_wraps_from_the_last_segment_to_the_first() {
    let fixture = Fixture::new(vec![silent_segment(0.3), silent_segment(0.3)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    assert_eq!(segments.len(), 2);
    assert_plays_on_from_the_start(segments, 0.4);
}

#[test]
fn a_preview_with_looping_off_ends_and_stays_at_the_end() {
    let fixture = Fixture::new(vec![silent_segment(0.4)]);
    let (player, events) = open_looping(segments_of(&fixture.entry()).unwrap(), false);
    player.play();
    let mut positions = Vec::new();
    let settle = Instant::now() + LONG_WAIT;
    let mut ended = false;
    while let Ok(event) = events.recv_deadline(settle) {
        match event {
            Event::Frame => positions.push(player.take_frame().unwrap().position),
            Event::Ended => {
                ended = true;
                break;
            }
            _ => {}
        }
    }
    assert!(ended);
    let shown = player.stats().shown;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(player.stats().shown, shown);
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn turning_looping_off_while_playing_takes_effect_at_the_next_end() {
    let fixture = Fixture::new(vec![silent_segment(0.6)]);
    let (player, events) = open_looping(segments_of(&fixture.entry()).unwrap(), true);
    player.play();
    wait_for_event(&events, |event| {
        matches!(event, Event::Frame) && player.take_frame().is_some_and(|f| f.position > 0.1)
    });
    player.set_looping(false);
    wait_for_event(&events, |event| *event == Event::Ended);
}

#[test]
fn a_forward_step_at_the_last_frame_does_not_wrap_when_looping() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open_looping(segments_of(&fixture.entry()).unwrap(), true);
    first_frame(&player, &events);
    let last = seek_to(&player, 5.).position;
    assert!(last > 0.9, "showed {last}");
    let shown = player.stats().shown;
    player.step(Direction::Forward);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(player.stats().shown, shown);
    assert!(!events.try_iter().any(|event| event == Event::Ended));
}

#[test]
fn a_seek_to_the_end_while_paused_does_not_wrap_when_looping() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open_looping(segments_of(&fixture.entry()).unwrap(), true);
    first_frame(&player, &events);
    let last = seek_to(&player, 1.).position;
    assert!(last > 0.9, "showed {last}");
    let shown = player.stats().shown;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(player.stats().shown, shown);
}

#[test]
fn playing_after_the_end_starts_over() {
    let fixture = Fixture::new(vec![silent_segment(0.3)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    player.play();
    wait_for_event(&events, |event| *event == Event::Ended);
    player.take_frame();
    player.play();
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let shown = player.take_frame().unwrap().position;
    assert!(shown < 0.2, "showed {shown}");
    wait_for_event(&events, |event| *event == Event::Ended);
}

#[test]
fn playing_after_a_seek_to_the_end_starts_over() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    player.seek(1.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    player.play();
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let shown = player.take_frame().unwrap().position;
    assert!(shown < 0.2, "showed {shown}");
}

#[test]
fn segments_play_one_after_another_on_one_timeline() {
    let fixture = Fixture::new(vec![silent_segment(0.5), silent_segment(0.5)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    assert_eq!(segments.len(), 2);
    let (player, events) = open(segments, 0.);
    let Event::Opened { duration } =
        wait_for_event(&events, |event| matches!(event, Event::Opened { .. }))
    else {
        unreachable!();
    };
    assert!((duration - 1.).abs() < 0.1, "{duration}");
    player.seek(0.75);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let shown = player.take_frame().unwrap().position;
    assert!((shown - 0.75).abs() < 0.04, "showed {shown}");
    player.seek(0.);
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    player.take_frame();
    let started = Instant::now();
    player.play();
    wait_for_event(&events, |event| *event == Event::Ended);
    assert!(started.elapsed().as_secs_f64() > 0.7);
}

const FRAME: f64 = 1. / 30.;

fn variable_segment(slots: &[u32], gop: u32) -> SegmentSpec {
    SegmentSpec {
        display: VideoTrackSpec {
            gop,
            ..VideoTrackSpec::new(320, 180, 30, Timing::Slots(slots.to_vec()))
        },
        camera: None,
        mic: None,
        system_audio: None,
    }
}

fn decodable_positions(segments: &[Segment]) -> Vec<f64> {
    let mut start = 0.;
    let mut positions = Vec::new();
    for segment in segments {
        let mut reader = video::VideoReader::open(&segment.video).unwrap();
        while let Some(frame) = reader.next() {
            positions.push(start + frame.local);
        }
        start += probe_duration(&segment.video).unwrap();
    }
    positions
}

fn wait_for_shown(player: &Player, shown: u64) {
    let deadline = Instant::now() + LONG_WAIT;
    while player.stats().shown < shown {
        assert!(Instant::now() < deadline, "only {:?}", player.stats());
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn first_frame(player: &Player, events: &flume::Receiver<Event>) -> ShownFrame {
    wait_for_event(events, |event| matches!(event, Event::Frame));
    player.take_frame().unwrap()
}

fn seek_to(player: &Player, position: f64) -> ShownFrame {
    let shown = player.stats().shown;
    player.seek(position);
    wait_for_shown(player, shown + 1);
    player.take_frame().unwrap()
}

fn step_to(player: &Player, direction: Direction) -> ShownFrame {
    let shown = player.stats().shown;
    player.step(direction);
    wait_for_shown(player, shown + 1);
    player.take_frame().unwrap()
}

#[test]
fn stepping_back_then_forward_returns_to_the_same_frame() {
    let fixture = Fixture::new(vec![silent_segment(3.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    for target in [1., 61. * FRAME] {
        let shown = seek_to(&player, target).position;
        let back = step_to(&player, Direction::Back).position;
        assert!(
            (shown - back - FRAME).abs() < 1e-3,
            "from {shown} back to {back}"
        );
        let forward = step_to(&player, Direction::Forward).position;
        assert_eq!(forward, shown);
    }
}

#[test]
fn steps_at_either_end_stay_put() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    assert_eq!(first_frame(&player, &events).position, 0.);
    let shown = player.stats().shown;
    player.step(Direction::Back);
    player.step(Direction::Forward);
    wait_for_shown(&player, shown + 1);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(player.stats().shown, shown + 1);
    let second = player.take_frame().unwrap().position;
    assert!((second - FRAME).abs() < 1e-3, "showed {second}");

    let last = seek_to(&player, 5.).position;
    let shown = player.stats().shown;
    player.step(Direction::Forward);
    player.step(Direction::Back);
    wait_for_shown(&player, shown + 1);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(player.stats().shown, shown + 1);
    let before_last = player.take_frame().unwrap().position;
    assert!(
        (last - before_last - FRAME).abs() < 1e-3,
        "{last} then {before_last}"
    );
}

#[test]
fn stepping_visits_every_frame_of_a_variable_rate_recording_once_each_way() {
    let first_slots = vec![0, 1, 2, 5, 6, 9, 10, 11, 17, 18, 19, 20, 24];
    let second_slots = vec![0, 3, 4, 5, 8, 13, 14];
    let fixture = Fixture::new(vec![
        variable_segment(&first_slots, 4),
        variable_segment(&second_slots, 3),
    ]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let expected = decodable_positions(&segments);
    let uneven = first_slots[..first_slots.len() - 1]
        .iter()
        .zip(&expected)
        .all(|(slot, position)| (position - f64::from(*slot) * FRAME).abs() < 1e-6);
    assert!(
        uneven && expected.len() >= first_slots.len() + second_slots.len() - 2,
        "{expected:?}"
    );
    let (player, events) = open(segments, 0.);
    let mut forward = vec![first_frame(&player, &events).position];
    for _ in 1..expected.len() {
        forward.push(step_to(&player, Direction::Forward).position);
    }
    assert_eq!(forward, expected);
    let mut back = vec![*forward.last().unwrap()];
    for _ in 1..expected.len() {
        back.push(step_to(&player, Direction::Back).position);
    }
    back.reverse();
    assert_eq!(back, expected);
}

#[test]
fn a_step_that_cannot_move_still_says_it_finished() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    let shown = player.stats().shown;
    player.step(Direction::Back);
    wait_for_event(&events, |event| *event == Event::Stepped);
    assert_eq!(player.stats().shown, shown);

    seek_to(&player, 5.);
    let shown = player.stats().shown;
    player.step(Direction::Forward);
    wait_for_event(&events, |event| *event == Event::Stepped);
    assert_eq!(player.stats().shown, shown);
}

#[test]
fn every_step_says_it_finished() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    for _ in 0..3 {
        player.step(Direction::Forward);
    }
    for _ in 0..3 {
        wait_for_event(&events, |event| *event == Event::Stepped);
    }
}

#[test]
fn queued_steps_each_take_effect() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    let shown = player.stats().shown;
    for _ in 0..3 {
        player.step(Direction::Forward);
    }
    wait_for_shown(&player, shown + 3);
    let third = player.take_frame().unwrap().position;
    assert!((third - 3. * FRAME).abs() < 1e-3, "showed {third}");
    player.step(Direction::Back);
    player.step(Direction::Back);
    wait_for_shown(&player, shown + 5);
    let first = player.take_frame().unwrap().position;
    assert!((first - FRAME).abs() < 1e-3, "showed {first}");
}

#[test]
fn a_step_pauses_playback() {
    let fixture = Fixture::new(vec![silent_segment(2.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    player.play();
    std::thread::sleep(Duration::from_millis(300));
    player.step(Direction::Forward);
    std::thread::sleep(Duration::from_millis(300));
    let shown = player.stats().shown;
    let stepped = player.take_frame().unwrap().position;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(player.stats().shown, shown);
    assert!(player.take_frame().is_none());
    assert!(stepped < 0.9, "showed {stepped}");
}

#[test]
fn a_bounds_change_after_a_step_keeps_the_frame() {
    let fixture = Fixture::new(vec![silent_segment(2.)]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    first_frame(&player, &events);
    seek_to(&player, 1.);
    let back = step_to(&player, Direction::Back).position;
    let shown = player.stats().shown;
    player.set_bounds((80, 45));
    wait_for_shown(&player, shown + 1);
    let frame = player.take_frame().unwrap();
    assert_eq!(frame.position, back);
    let size = frame.image.size(0);
    assert_eq!((i32::from(size.width), i32::from(size.height)), (80, 45));
}

#[test]
fn a_shown_frame_carries_the_frame_interval_of_its_segment() {
    let fixture = Fixture::new(vec![
        silent_segment(0.5),
        SegmentSpec {
            display: VideoTrackSpec::new(320, 180, 60, Timing::Constant { duration: 0.5 }),
            camera: None,
            mic: None,
            system_audio: None,
        },
    ]);
    let (player, events) = open(segments_of(&fixture.entry()).unwrap(), 0.);
    let first = first_frame(&player, &events);
    assert!((first.frame_interval - FRAME).abs() < 1e-6, "{first:?}");
    let later = seek_to(&player, 0.75);
    assert!((later.frame_interval - 1. / 60.).abs() < 1e-6, "{later:?}");
}

#[cfg(windows)]
#[test]
fn closing_the_player_lets_go_of_the_file() {
    let fixture = Fixture::new(vec![silent_segment(2.)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let video = segments[0].video.clone();
    let (player, events) = open(segments, 0.);
    player.play();
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let moved = video.with_extension("moved");
    assert!(
        std::fs::rename(&video, &moved).is_err(),
        "an open video can't be moved on Windows"
    );
    release(&video);
    drop(player);
    std::fs::rename(&video, &moved).unwrap();
}

#[test]
fn a_missing_file_says_so() {
    let fixture = Fixture::new(vec![silent_segment(0.5)]);
    let mut segments = segments_of(&fixture.entry()).unwrap();
    segments[0].video = fixture.dir.join("gone.mp4");
    let (_player, events) = open(segments, 0.);
    assert_eq!(
        wait_for_event(&events, |event| matches!(event, Event::Failed(_))),
        Event::Failed(PlayError::Missing)
    );
}

#[test]
fn a_damaged_file_says_so() {
    let fixture = Fixture::new(vec![silent_segment(0.5)]);
    let damaged = fixture.dir.join("damaged.mp4");
    std::fs::write(&damaged, vec![7u8; 4096]).unwrap();
    let (_player, events) = open(vec![Segment::file(damaged)], 0.);
    assert_eq!(
        wait_for_event(&events, |event| matches!(event, Event::Failed(_))),
        Event::Failed(PlayError::Unreadable)
    );
}

#[test]
fn the_latest_export_plays_before_the_raw_recording() {
    let fixture = Fixture::new(vec![silent_segment(0.5)]);
    let mut entry = fixture.entry();
    entry.exports = vec![ExportFile {
        path: PathBuf::from("made.mp4"),
        name: "made.mp4".to_string(),
        exported_millis: 1.,
        size: Some(1),
        video: true,
    }];
    assert_eq!(
        segments_of(&entry).unwrap(),
        vec![Segment::file(PathBuf::from("made.mp4"))]
    );
}

#[test]
fn instant_plays_its_current_version() {
    let entry = Entry {
        kind: MediaKind::Instant,
        ..Fixture::new(vec![silent_segment(0.5)]).entry()
    };
    assert_eq!(
        segments_of(&entry).unwrap(),
        vec![Segment::file(instant_output_path(&entry.path))]
    );
}

#[test]
fn a_raw_recording_plays_with_its_microphone_and_system_audio_in_step() {
    let fixture = Fixture::new(vec![SegmentSpec {
        display: VideoTrackSpec {
            start_time: 0.25,
            ..VideoTrackSpec::new(320, 180, 30, Timing::Constant { duration: 0.5 })
        },
        camera: None,
        mic: Some(audio_track(AudioSource::Mic, 0.5, 0.5)),
        system_audio: Some(audio_track(AudioSource::System, 0.5, 0.25)),
    }]);
    let segments = segments_of(&fixture.entry()).unwrap();
    assert_eq!(segments.len(), 1);
    let delays: Vec<f64> = segments[0].audio.iter().map(|track| track.delay).collect();
    assert_eq!(delays, vec![0.25, 0.]);
    assert!(segments[0].audio[0].path.ends_with("audio-input.ogg"));
    assert!(segments[0].audio[1].path.ends_with("system_audio.ogg"));
}

#[test]
fn audio_tracks_reach_the_output_mixed_and_play_to_the_end() {
    let fixture = Fixture::new(vec![SegmentSpec {
        display: VideoTrackSpec::new(320, 180, 30, Timing::Constant { duration: 1. }),
        camera: None,
        mic: Some(audio_track(AudioSource::Mic, 1., 0.)),
        system_audio: None,
    }]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let timeline = Timeline::new(vec![1.]);
    let mut mix = audio::SegmentedMix::new(segments, timeline, 48_000, 0.);
    let mut samples = Vec::new();
    while let Some(block) = mix.next_block() {
        samples.extend(block);
    }
    assert_eq!(samples.len(), 96_000);
    assert!(samples.iter().any(|sample| sample.abs() > 0.01));
}

#[test]
fn a_seek_moves_the_audio_to_the_same_place() {
    let fixture = Fixture::new(vec![SegmentSpec {
        display: VideoTrackSpec::new(320, 180, 30, Timing::Constant { duration: 1. }),
        camera: None,
        mic: Some(audio_track(AudioSource::Mic, 1., 0.)),
        system_audio: None,
    }]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let timeline = Timeline::new(vec![1.]);
    let mut mix = audio::SegmentedMix::new(segments, timeline, 48_000, 0.);
    mix.seek(0.75);
    let mut frames = 0;
    while let Some(block) = mix.next_block() {
        frames += block.len() / audio::STEREO;
    }
    assert_eq!(frames, 12_000);
}

struct Measured {
    onset: Instant,
}

fn tone_onsets(
    samples: &[f32],
    at: Instant,
    rate: f64,
    last: &mut Option<Instant>,
) -> Vec<Measured> {
    samples
        .chunks_exact(audio::STEREO)
        .enumerate()
        .filter(|(_, pair)| pair[0].abs() > 0.05)
        .filter_map(|(frame, _)| {
            let onset = at + Duration::from_secs_f64(frame as f64 / rate);
            let fresh =
                last.is_none_or(|last| onset.saturating_duration_since(last).as_secs_f64() > 0.5);
            fresh.then(|| {
                *last = Some(onset);
                Measured { onset }
            })
        })
        .collect()
}

fn brightness(image: &RenderImage) -> f64 {
    let bytes = image.as_bytes(0).unwrap();
    bytes.iter().map(|byte| f64::from(*byte)).sum::<f64>() / bytes.len() as f64
}

#[test]
fn a_strip_has_every_frame_and_never_grows_past_the_clip() {
    let fixture = Fixture::new(vec![silent_segment(2.), silent_segment(2.)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let hold = Hold::on(&fixture.dir);
    let strip::Built::Frames(frames) = strip::build(&segments, &hold) else {
        panic!("no strip");
    };
    assert_eq!(frames.len(), strip::STRIP_FRAMES);
    assert_eq!(frames[0].size(0).width.0, 320);
}

#[test]
fn a_strip_stops_when_the_project_is_released() {
    let fixture = Fixture::new(vec![silent_segment(1.)]);
    let segments = segments_of(&fixture.entry()).unwrap();
    let hold = Hold::on(&fixture.dir);
    hold.stopper().stop();
    assert!(matches!(
        strip::build(&segments, &hold),
        strip::Built::Stopped
    ));
}

#[test]
fn a_strip_of_a_missing_file_fails() {
    let segments = vec![Segment::file(PathBuf::from("missing.mp4"))];
    let hold = Hold::on(Path::new("missing"));
    assert!(matches!(
        strip::build(&segments, &hold),
        strip::Built::Failed
    ));
}

#[test]
fn release_waits_for_a_hold_to_end() {
    let project = PathBuf::from("held-project");
    let hold = Hold::on(&project);
    let worker = std::thread::spawn(move || {
        while !hold.stopped() {
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    release(&project);
    worker.join().unwrap();
    assert!(
        !lock(&OPEN_PLAYERS)
            .iter()
            .any(|player| player.project == project_key(&project))
    );
}

#[test]
fn measure_strip_cost() {
    let Some(file) = std::env::var_os("FRAMEEVER_PLAYER_FILE").map(PathBuf::from) else {
        return;
    };
    let segments = vec![Segment::file(file.clone())];
    for run in 0..3 {
        let hold = Hold::on(&file);
        let started = Instant::now();
        let strip::Built::Frames(frames) = strip::build(&segments, &hold) else {
            panic!("no strip");
        };
        let elapsed = started.elapsed();
        let bytes: usize = frames
            .iter()
            .map(|frame| frame.as_bytes(0).map_or(0, <[u8]>::len))
            .sum();
        println!(
            "strip {run}: {:.0} ms for {} frames, {:.1} MB",
            elapsed.as_secs_f64() * 1000.,
            frames.len(),
            bytes as f64 / 1_048_576.
        );
    }
}

#[test]
fn measure_frame_timing_and_audio_sync() {
    let Some(file) = std::env::var_os("FRAMEEVER_PLAYER_FILE").map(PathBuf::from) else {
        return;
    };
    let bounds = std::env::var("FRAMEEVER_PLAYER_BOUNDS")
        .ok()
        .and_then(|value| {
            let (width, height) = value.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or((630, 354));
    let rate = cpal_rate();
    let (onset_tx, onset_rx) = flume::unbounded();
    let mut last_onset = None;
    let tap: AudioTap = Box::new(move |samples, at| {
        for onset in tone_onsets(samples, at, rate, &mut last_onset) {
            onset_tx.send(onset.onset).ok();
        }
    });
    let segments = vec![Segment::file(file)];
    let project = segments[0].video.clone();
    let (player, events) = Player::open(
        &project,
        move || Ok(segments),
        Start {
            position: 0.,
            muted: true,
            volume: 1.,
            looping: false,
            bounds,
        },
        Some(tap),
    )
    .unwrap();
    wait_for_event(&events, |event| matches!(event, Event::Frame));
    let first = player.take_frame().unwrap();
    println!("first frame {:?}", first.image.size(0));
    player.play();
    let started = Instant::now();
    let mut shown: Vec<(Instant, f64)> = Vec::new();
    let mut flashes: Vec<Instant> = Vec::new();
    let mut flash_on = false;
    loop {
        match events.recv_timeout(LONG_WAIT).unwrap() {
            Event::Frame => {
                let at = Instant::now();
                let frame = player.take_frame().unwrap();
                let bright = brightness(&frame.image) > 128.;
                if bright && !flash_on {
                    flashes.push(at);
                }
                flash_on = bright;
                shown.push((at, frame.position));
            }
            Event::Ended => break,
            Event::Failed(error) => panic!("{error:?}"),
            _ => {}
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    let mut gaps: Vec<f64> = shown
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).as_secs_f64() * 1000.)
        .collect();
    for (index, gap) in gaps.iter().enumerate().filter(|(_, gap)| **gap > 40.) {
        println!(
            "stall of {gap:.1} ms after frame {index} at {:.3}",
            shown[index].1
        );
    }
    gaps.sort_by(f64::total_cmp);
    let late: Vec<f64> = shown
        .iter()
        .map(|(at, position)| {
            ((*at - started).as_secs_f64() - (position - shown[0].1)) * 1000.
                - ((shown[0].0 - started).as_secs_f64() * 1000.)
        })
        .collect();
    let percentile = |values: &[f64], at: f64| values[((values.len() - 1) as f64 * at) as usize];
    let stats = player.stats();
    println!(
        "played {elapsed:.2} s, shown {}, skipped {}, {:.1} fps",
        stats.shown,
        stats.skipped,
        shown.len() as f64 / elapsed
    );
    println!(
        "frame gap ms: median {:.2}, p95 {:.2}, p99 {:.2}, max {:.2}",
        percentile(&gaps, 0.5),
        percentile(&gaps, 0.95),
        percentile(&gaps, 0.99),
        percentile(&gaps, 1.)
    );
    let mut sorted = late.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "frame lateness ms: min {:.2}, median {:.2}, max {:.2}",
        percentile(&sorted, 0.),
        percentile(&sorted, 0.5),
        percentile(&sorted, 1.)
    );
    let onsets: Vec<Instant> = onset_rx.drain().collect();
    let mut sync: Vec<f64> = flashes
        .iter()
        .filter_map(|flash| {
            onsets
                .iter()
                .map(|onset| {
                    if flash >= onset {
                        (*flash - *onset).as_secs_f64() * 1000.
                    } else {
                        -(*onset - *flash).as_secs_f64() * 1000.
                    }
                })
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        })
        .collect();
    println!("flashes {}, beeps {}", flashes.len(), onsets.len());
    println!("picture after sound, ms, per beep: {sync:.1?}");
    sync.sort_by(f64::total_cmp);
    if !sync.is_empty() {
        println!(
            "sync ms: min {:.1}, median {:.1}, max {:.1}",
            percentile(&sync, 0.),
            percentile(&sync, 0.5),
            percentile(&sync, 1.)
        );
    }
}

fn cpal_rate() -> f64 {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .default_output_device()
        .and_then(|device| device.default_output_config().ok())
        .map_or(48_000., |config| f64::from(config.sample_rate().0))
}
