use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use frameever_project::{RecordingMeta, RecordingMetaInner, StudioRecordingMeta};
use gpui::RenderImage;

use super::{MediaKind, instant_output_path, view::Entry};

mod audio;
mod clock;
pub mod strip;
mod timeline;
mod timer;
pub(crate) mod video;

use audio::{AudioCommand, Feed, Output};
use clock::Clock;
use timeline::Timeline;
use timer::FineTimer;
use video::{DecodedFrame, Pace, VideoReader, pace, wait_for};

const TICK_EVERY: Duration = Duration::from_millis(250);
const IDLE_WAIT: Duration = Duration::from_secs(3600);
const DRAIN_WAIT: Duration = Duration::from_millis(10);
const AUDIO_READY_WAIT: Duration = Duration::from_secs(3);
const END_TOLERANCE: f64 = 0.01;
const RELEASE_WAIT: Duration = Duration::from_secs(2);

pub type AudioTap = Box<dyn FnMut(&[f32], Instant) + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayError {
    Missing,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
    pub path: PathBuf,
    pub delay: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub video: PathBuf,
    pub audio: Vec<AudioTrack>,
}

impl Segment {
    fn file(path: PathBuf) -> Self {
        Self {
            audio: vec![AudioTrack {
                path: path.clone(),
                delay: 0.,
            }],
            video: path,
        }
    }
}

fn studio_segments(meta: &RecordingMeta, studio: &StudioRecordingMeta) -> Vec<Segment> {
    let track =
        |display: &frameever_project::VideoMeta, audio: &frameever_project::AudioMeta| AudioTrack {
            path: meta.path(&audio.path),
            delay: audio.start_time.unwrap_or(0.) - display.start_time.unwrap_or(0.),
        };
    match studio {
        StudioRecordingMeta::SingleSegment { segment } => vec![Segment {
            video: meta.path(&segment.display.path),
            audio: segment
                .audio
                .iter()
                .map(|audio| track(&segment.display, audio))
                .collect(),
        }],
        StudioRecordingMeta::MultipleSegments { inner } => inner
            .segments
            .iter()
            .map(|segment| Segment {
                video: meta.path(&segment.display.path),
                audio: [&segment.mic, &segment.system_audio]
                    .into_iter()
                    .flatten()
                    .map(|audio| track(&segment.display, audio))
                    .collect(),
            })
            .collect(),
    }
}

pub fn segments_of(entry: &Entry) -> Result<Vec<Segment>, PlayError> {
    match entry.kind {
        MediaKind::Screenshot => Err(PlayError::Unreadable),
        MediaKind::Instant => Ok(vec![Segment::file(instant_output_path(&entry.path))]),
        MediaKind::Studio => {
            if let Some(export) = entry.latest_export() {
                return Ok(vec![Segment::file(export.path.clone())]);
            }
            let meta = RecordingMeta::load_for_project(&entry.path).map_err(|error| {
                tracing::warn!(path = %entry.path.display(), "reading the project failed: {error}");
                PlayError::Unreadable
            })?;
            match &meta.inner {
                RecordingMetaInner::Studio(studio) => Ok(studio_segments(&meta, studio)),
                RecordingMetaInner::Instant(_) => {
                    Ok(vec![Segment::file(instant_output_path(&entry.path))])
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShownFrame {
    pub image: Arc<RenderImage>,
    pub position: f64,
    pub source: (u32, u32),
    pub frame_interval: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Opened { duration: f64 },
    Frame,
    Tick { position: f64 },
    Ended,
    Failed(PlayError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub shown: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Start {
    pub position: f64,
    pub muted: bool,
    pub volume: f32,
    pub looping: bool,
    pub bounds: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Back,
    Forward,
}

enum Command {
    Play,
    Pause,
    Seek(f64),
    Step(Direction),
    Muted(bool),
    Volume(f32),
    Looping(bool),
    Bounds((u32, u32)),
    Stop,
}

struct OpenPlayer {
    id: u64,
    project: PathBuf,
    commands: flume::Sender<Command>,
}

static OPEN_PLAYERS: Mutex<Vec<OpenPlayer>> = Mutex::new(Vec::new());
static PLAYER_CLOSED: Condvar = Condvar::new();
static NEXT_PLAYER: AtomicU64 = AtomicU64::new(0);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn project_key(project: &Path) -> PathBuf {
    dunce::canonicalize(project).unwrap_or_else(|_| project.to_path_buf())
}

struct OpenGuard {
    id: u64,
}

impl OpenGuard {
    fn new(project: &Path, commands: flume::Sender<Command>) -> Self {
        let id = NEXT_PLAYER.fetch_add(1, Ordering::Relaxed);
        lock(&OPEN_PLAYERS).push(OpenPlayer {
            id,
            project: project_key(project),
            commands,
        });
        Self { id }
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        lock(&OPEN_PLAYERS).retain(|player| player.id != self.id);
        PLAYER_CLOSED.notify_all();
    }
}

pub struct Hold {
    _guard: OpenGuard,
    commands: flume::Sender<Command>,
    received: flume::Receiver<Command>,
}

#[derive(Clone)]
pub struct HoldStop(flume::Sender<Command>);

impl HoldStop {
    pub fn stop(&self) {
        self.0.send(Command::Stop).ok();
    }
}

impl Hold {
    pub fn on(project: &Path) -> Self {
        let (commands, received) = flume::unbounded();
        Self {
            _guard: OpenGuard::new(project, commands.clone()),
            commands,
            received,
        }
    }

    pub fn stopper(&self) -> HoldStop {
        HoldStop(self.commands.clone())
    }

    pub fn stopped(&self) -> bool {
        self.received
            .try_iter()
            .any(|command| matches!(command, Command::Stop))
    }
}

pub fn release(project: &Path) {
    let project = project_key(project);
    let holding = |players: &[OpenPlayer]| players.iter().any(|player| player.project == project);
    let players = lock(&OPEN_PLAYERS);
    for player in players.iter().filter(|player| player.project == project) {
        player.commands.send(Command::Stop).ok();
    }
    let (players, _) = PLAYER_CLOSED
        .wait_timeout_while(players, RELEASE_WAIT, |players| holding(players))
        .unwrap_or_else(PoisonError::into_inner);
    if holding(&players) {
        tracing::warn!(path = %project.display(), "the library preview is still closing");
    }
}

fn container_start(input: &ffmpeg::format::context::Input) -> f64 {
    match unsafe { (*input.as_ptr()).start_time } {
        ffmpeg::ffi::AV_NOPTS_VALUE => 0.,
        start => start as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE),
    }
}

#[derive(Default)]
struct FrameSlot {
    frame: Mutex<Option<ShownFrame>>,
}

impl FrameSlot {
    fn put(&self, frame: ShownFrame) -> bool {
        lock(&self.frame).replace(frame).is_none()
    }

    fn take(&self) -> Option<ShownFrame> {
        lock(&self.frame).take()
    }
}

pub struct Player {
    commands: flume::Sender<Command>,
    slot: Arc<FrameSlot>,
    stats: Arc<Mutex<Stats>>,
}

impl Player {
    pub fn open(
        project: &Path,
        load: impl FnOnce() -> Result<Vec<Segment>, PlayError> + Send + 'static,
        start: Start,
        tap: Option<AudioTap>,
    ) -> Option<(Self, flume::Receiver<Event>)> {
        let (commands, command_rx) = flume::unbounded();
        let (event_tx, events) = flume::unbounded();
        let slot = Arc::new(FrameSlot::default());
        let stats = Arc::new(Mutex::new(Stats::default()));
        let guard = OpenGuard::new(project, commands.clone());
        let worker_slot = slot.clone();
        let worker_stats = stats.clone();
        std::thread::Builder::new()
            .name("library-player".into())
            .spawn(move || {
                let _guard = guard;
                match load().and_then(|segments| {
                    Worker::open(
                        segments,
                        start,
                        tap,
                        command_rx,
                        event_tx.clone(),
                        worker_slot,
                        worker_stats,
                    )
                }) {
                    Ok(worker) => worker.run(),
                    Err(error) => {
                        event_tx.send(Event::Failed(error)).ok();
                    }
                }
            })
            .map_err(|error| tracing::error!("starting the preview failed: {error}"))
            .ok()?;
        Some((
            Self {
                commands,
                slot,
                stats,
            },
            events,
        ))
    }

    fn send(&self, command: Command) {
        self.commands.send(command).ok();
    }

    pub fn play(&self) {
        self.send(Command::Play);
    }

    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    pub fn seek(&self, position: f64) {
        self.send(Command::Seek(position));
    }

    pub fn step(&self, direction: Direction) {
        self.send(Command::Step(direction));
    }

    pub fn set_muted(&self, muted: bool) {
        self.send(Command::Muted(muted));
    }

    pub fn set_volume(&self, volume: f32) {
        self.send(Command::Volume(volume));
    }

    pub fn set_looping(&self, looping: bool) {
        self.send(Command::Looping(looping));
    }

    pub fn set_bounds(&self, bounds: (u32, u32)) {
        self.send(Command::Bounds(bounds));
    }

    pub fn take_frame(&self) -> Option<ShownFrame> {
        self.slot.take()
    }

    pub fn stats(&self) -> Stats {
        *lock(&self.stats)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.send(Command::Stop);
    }
}

struct AudioLink {
    output: Output,
    commands: flume::Sender<AudioCommand>,
    thread: JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct FrameKey {
    segment: usize,
    local: f64,
}

struct TimedFrame {
    decoded: DecodedFrame,
    key: FrameKey,
    position: f64,
    interval: f64,
}

enum Search {
    Nearest {
        target: f64,
        last: Option<TimedFrame>,
    },
    After {
        from: Shown,
    },
    Before {
        from: Shown,
        reach: f64,
        last: Option<TimedFrame>,
    },
}

#[derive(Debug, Clone, Copy)]
struct Shown {
    key: FrameKey,
    position: f64,
    next_follows: bool,
}

struct Worker {
    segments: Vec<Segment>,
    timeline: Timeline,
    current: usize,
    reader: VideoReader,
    pending: Option<TimedFrame>,
    search: Option<Search>,
    steps: VecDeque<Direction>,
    bounds: (u32, u32),
    playing: bool,
    looping: bool,
    timer: Option<FineTimer>,
    drained: bool,
    ended: bool,
    shown: Option<Shown>,
    shown_at: Instant,
    ticked_at: Instant,
    clock: Arc<Clock>,
    audio: Option<AudioLink>,
    commands: flume::Receiver<Command>,
    events: flume::Sender<Event>,
    slot: Arc<FrameSlot>,
    stats: Arc<Mutex<Stats>>,
}

fn open_audio(
    segments: &[Segment],
    timeline: &Timeline,
    start: Start,
    clock: &Arc<Clock>,
    tap: Option<AudioTap>,
) -> Option<AudioLink> {
    let audible = segments
        .iter()
        .flat_map(|segment| &segment.audio)
        .any(|track| audio::has_audio(&track.path));
    if !audible {
        return None;
    }
    let (commands, command_rx) = flume::unbounded();
    let (ready_tx, ready) = flume::bounded(1);
    let feed = Feed {
        segments: segments.to_vec(),
        timeline: timeline.clone(),
        position: start.position,
        muted: start.muted,
        volume: start.volume,
        clock: clock.clone(),
        tap,
    };
    let thread = std::thread::Builder::new()
        .name("library-player-audio".into())
        .spawn(move || audio::run(feed, command_rx, ready_tx))
        .map_err(|error| tracing::warn!("starting the preview's audio failed: {error}"))
        .ok()?;
    let output = ready.recv_timeout(AUDIO_READY_WAIT).ok().flatten()?;
    clock.follow_audio(Instant::now());
    Some(AudioLink {
        output,
        commands,
        thread,
    })
}

impl Worker {
    fn open(
        segments: Vec<Segment>,
        start: Start,
        tap: Option<AudioTap>,
        commands: flume::Receiver<Command>,
        events: flume::Sender<Event>,
        slot: Arc<FrameSlot>,
        stats: Arc<Mutex<Stats>>,
    ) -> Result<Self, PlayError> {
        if segments.is_empty() {
            return Err(PlayError::Unreadable);
        }
        let durations = segments
            .iter()
            .map(|segment| probe_duration(&segment.video))
            .collect::<Result<Vec<_>, _>>()?;
        let timeline = Timeline::new(durations);
        let (current, local) = timeline.locate(start.position);
        let mut reader = VideoReader::open(&segments[current].video)?;
        if start.position > 0. {
            reader.seek(local);
        }
        let now = Instant::now();
        let clock = Arc::new(Clock::new(start.position, now));
        let audio = open_audio(&segments, &timeline, start, &clock, tap);
        events
            .send(Event::Opened {
                duration: timeline.total(),
            })
            .ok();
        Ok(Self {
            segments,
            timeline,
            current,
            reader,
            pending: None,
            search: Some(Search::Nearest {
                target: start.position,
                last: None,
            }),
            steps: VecDeque::new(),
            bounds: start.bounds,
            playing: false,
            looping: start.looping,
            timer: None,
            drained: false,
            ended: false,
            shown: None,
            shown_at: now,
            ticked_at: now,
            clock,
            audio,
            commands,
            events,
            slot,
            stats,
        })
    }

    fn run(mut self) {
        loop {
            let wait = self.step();
            let command = if wait.is_zero() {
                self.commands.try_recv().map_err(|error| match error {
                    flume::TryRecvError::Empty => flume::RecvTimeoutError::Timeout,
                    flume::TryRecvError::Disconnected => flume::RecvTimeoutError::Disconnected,
                })
            } else {
                self.commands.recv_timeout(wait)
            };
            match command {
                Ok(command) => {
                    if !self.apply(command) {
                        break;
                    }
                }
                Err(flume::RecvTimeoutError::Timeout) => {}
                Err(flume::RecvTimeoutError::Disconnected) => break,
            }
        }
        if let Some(audio) = self.audio.take() {
            audio.commands.send(AudioCommand::Stop).ok();
            if audio.thread.join().is_err() {
                tracing::warn!("the preview's audio thread panicked");
            }
        }
    }

    fn apply(&mut self, first: Command) -> bool {
        let mut seek = None;
        let mut next = Some(first);
        while let Some(command) = next {
            match command {
                Command::Stop => return false,
                Command::Seek(position) => {
                    self.steps.clear();
                    seek = Some(position);
                }
                Command::Step(direction) => {
                    self.set_playing(false);
                    self.steps.push_back(direction);
                }
                Command::Play => {
                    self.steps.clear();
                    if let Some(position) = seek.take() {
                        self.seek(position);
                    }
                    self.set_playing(true);
                }
                Command::Pause => self.set_playing(false),
                Command::Muted(muted) => {
                    if let Some(audio) = &self.audio {
                        audio.output.set_muted(muted);
                    }
                }
                Command::Volume(volume) => {
                    if let Some(audio) = &self.audio {
                        audio.output.set_volume(volume);
                    }
                }
                Command::Looping(looping) => self.looping = looping,
                Command::Bounds(bounds) => {
                    let changed = bounds != self.bounds;
                    self.bounds = bounds;
                    let settled = seek.is_none() && self.search.is_none() && self.steps.is_empty();
                    if changed && !self.playing && settled {
                        seek = Some(self.clock.position());
                    }
                }
            }
            next = self.commands.try_recv().ok();
        }
        if let Some(position) = seek {
            self.seek(position);
        }
        true
    }

    fn at_end(&self) -> bool {
        self.ended || self.clock.position() + END_TOLERANCE >= self.timeline.total()
    }

    fn set_playing(&mut self, playing: bool) {
        if playing && self.at_end() {
            self.seek(0.);
        }
        self.playing = playing;
        self.timer = playing.then(FineTimer::new);
        self.clock.set_running(playing, Instant::now());
        if let Some(audio) = &self.audio {
            audio.output.set_playing(playing);
        }
    }

    fn locate(&self, position: f64) -> FrameKey {
        let (segment, local) = self.timeline.locate(position);
        FrameKey { segment, local }
    }

    fn move_reader(&mut self, FrameKey { segment, local }: FrameKey) -> bool {
        if segment != self.current {
            match VideoReader::open(&self.segments[segment].video) {
                Ok(reader) => {
                    self.reader = reader;
                    self.current = segment;
                }
                Err(error) => {
                    self.events.send(Event::Failed(error)).ok();
                    return false;
                }
            }
        }
        self.reader.seek(local);
        self.pending = None;
        self.drained = false;
        if let Some(shown) = &mut self.shown {
            shown.next_follows = false;
        }
        true
    }

    fn seek(&mut self, position: f64) {
        let position = position.clamp(0., self.timeline.total());
        if !self.move_reader(self.locate(position)) {
            return;
        }
        self.search = Some(Search::Nearest {
            target: position,
            last: None,
        });
        self.place(position);
    }

    fn place(&mut self, position: f64) {
        self.ended = false;
        self.clock.place(position, Instant::now());
        if let Some(audio) = &self.audio {
            let generation = audio.output.reset(position);
            audio
                .commands
                .send(AudioCommand::Seek {
                    generation,
                    position,
                })
                .ok();
        }
    }

    fn decode(&mut self) -> Option<TimedFrame> {
        loop {
            if let Some(decoded) = self.reader.next() {
                return Some(TimedFrame {
                    key: FrameKey {
                        segment: self.current,
                        local: decoded.local,
                    },
                    position: self.timeline.start_of(self.current) + decoded.local,
                    interval: self.reader.frame_gap(),
                    decoded,
                });
            }
            let next = self.current + 1;
            if next >= self.timeline.len() {
                return None;
            }
            match VideoReader::open(&self.segments[next].video) {
                Ok(reader) => {
                    self.reader = reader;
                    self.current = next;
                }
                Err(error) => {
                    tracing::warn!("opening the next segment failed: {error:?}");
                    self.events.send(Event::Failed(error)).ok();
                    return None;
                }
            }
        }
    }

    fn show(&mut self, frame: &TimedFrame) {
        match self.reader.image(&frame.decoded, self.bounds) {
            Ok(image) => {
                if self.slot.put(ShownFrame {
                    image,
                    position: frame.position,
                    source: frame.decoded.size(),
                    frame_interval: frame.interval,
                }) {
                    self.events.send(Event::Frame).ok();
                }
                self.shown = Some(Shown {
                    key: frame.key,
                    position: frame.position,
                    next_follows: true,
                });
                self.shown_at = Instant::now();
                lock(&self.stats).shown += 1;
            }
            Err(error) => {
                tracing::warn!("drawing a video frame failed: {error}");
                if self.shown.is_none() {
                    self.events.send(Event::Failed(PlayError::Unreadable)).ok();
                }
            }
        }
    }

    fn begin_step(&mut self, direction: Direction) {
        let Some(shown) = self.shown else {
            self.steps.clear();
            return;
        };
        match direction {
            Direction::Forward => {
                if shown.next_follows || self.move_reader(shown.key) {
                    self.search = Some(Search::After { from: shown });
                }
            }
            Direction::Back => {
                let reach = (shown.position - self.reader.frame_gap()).max(0.);
                if self.move_reader(self.locate(reach)) {
                    self.search = Some(Search::Before {
                        from: shown,
                        reach,
                        last: None,
                    });
                }
            }
        }
    }

    fn finish_step(&mut self) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown.next_follows = true;
        let position = shown.position;
        self.place(position);
    }

    fn advance_search(&mut self, search: Search) -> Duration {
        let Some(frame) = self.pending.take().or_else(|| self.decode()) else {
            self.drained = true;
            self.end_search(search);
            return Duration::ZERO;
        };
        match search {
            Search::Nearest { target, .. } if frame.position < target => {
                self.search = Some(Search::Nearest {
                    target,
                    last: Some(frame),
                });
            }
            Search::Nearest { target, last } => match last {
                Some(last) if target - last.position < frame.position - target => {
                    self.show(&last);
                    self.pending = Some(frame);
                }
                _ => self.show(&frame),
            },
            Search::After { from } if frame.key <= from.key => {
                self.search = Some(Search::After { from });
            }
            Search::After { .. } => {
                self.show(&frame);
                self.finish_step();
            }
            Search::Before { from, reach, .. } if frame.key < from.key => {
                self.search = Some(Search::Before {
                    from,
                    reach,
                    last: Some(frame),
                });
            }
            Search::Before {
                last: Some(last), ..
            } => {
                self.show(&last);
                self.pending = Some(frame);
                self.finish_step();
            }
            Search::Before { from, reach, .. } if reach > 0. => {
                let reach = (from.position - 2. * (from.position - reach)).max(0.);
                if self.move_reader(self.locate(reach)) {
                    self.search = Some(Search::Before {
                        from,
                        reach,
                        last: None,
                    });
                }
            }
            Search::Before { from, .. } => {
                if frame.key > from.key {
                    self.pending = Some(frame);
                }
                self.finish_step();
            }
        }
        Duration::ZERO
    }

    fn end_search(&mut self, search: Search) {
        match search {
            Search::Nearest { last, .. } => {
                if let Some(last) = last {
                    self.show(&last);
                }
                if self.shown.is_none() {
                    self.events.send(Event::Failed(PlayError::Unreadable)).ok();
                }
            }
            Search::After { .. } | Search::Before { last: None, .. } => self.finish_step(),
            Search::Before {
                last: Some(last), ..
            } => {
                self.show(&last);
                self.finish_step();
            }
        }
    }

    fn tick(&mut self, now: Instant, position: f64) {
        if now.saturating_duration_since(self.ticked_at) >= TICK_EVERY {
            self.ticked_at = now;
            self.events.send(Event::Tick { position }).ok();
        }
    }

    fn step(&mut self) -> Duration {
        if self.search.is_none()
            && let Some(direction) = self.steps.pop_front()
        {
            self.begin_step(direction);
        }
        if let Some(search) = self.search.take() {
            return self.advance_search(search);
        }
        if !self.steps.is_empty() {
            return Duration::ZERO;
        }
        if !self.playing || self.ended {
            return IDLE_WAIT;
        }
        let now = Instant::now();
        let position = self.clock.position_at(now);
        self.tick(now, position);
        if self.pending.is_none() && !self.drained {
            self.pending = self.decode();
            self.drained = self.pending.is_none();
        }
        let Some(decoded) = self.pending.take() else {
            if position + END_TOLERANCE >= self.timeline.total() {
                if self.looping {
                    self.seek(0.);
                    return Duration::ZERO;
                }
                self.set_playing(false);
                self.ended = true;
                self.events.send(Event::Ended).ok();
                return IDLE_WAIT;
            }
            return DRAIN_WAIT;
        };
        let since_shown = now.saturating_duration_since(self.shown_at);
        match pace(decoded.position, position, since_shown) {
            Pace::Wait(lead) => {
                self.pending = Some(decoded);
                wait_for(lead, now, self.ticked_at + TICK_EVERY)
            }
            Pace::Show => {
                self.show(&decoded);
                Duration::ZERO
            }
            Pace::Skip => {
                if let Some(shown) = &mut self.shown {
                    shown.next_follows = false;
                }
                lock(&self.stats).skipped += 1;
                Duration::ZERO
            }
        }
    }
}

fn probe_duration(path: &Path) -> Result<f64, PlayError> {
    if !path.is_file() {
        return Err(PlayError::Missing);
    }
    frameever_enc_ffmpeg::remux::get_media_duration(path)
        .map(|duration| duration.as_secs_f64())
        .ok_or(PlayError::Unreadable)
}

#[cfg(test)]
#[path = "player/tests.rs"]
mod tests;
