use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ffmpeg::{
    ChannelLayout,
    format::{Sample as FfmpegSample, sample::Type as SampleType},
    software::resampling,
};

use super::{AudioTap, AudioTrack, Segment, clock::Clock, container_start, timeline::Timeline};

pub const STEREO: usize = 2;
const BUFFER_SECONDS: usize = 2;
const BLOCK_FRAMES: usize = 1024;
const GAP_TOLERANCE_SECONDS: f64 = 0.02;
const IDLE_WAIT: Duration = Duration::from_millis(10);
const MAX_OUTPUT_LATENCY: f64 = 0.5;

#[derive(Debug)]
pub struct AudioBuffer {
    samples: VecDeque<f32>,
    capacity: usize,
    rate: u32,
    start: f64,
    consumed: u64,
    finished: bool,
    generation: u64,
}

impl AudioBuffer {
    pub fn new(rate: u32, start: f64) -> Self {
        let capacity = rate as usize * STEREO * BUFFER_SECONDS;
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
            rate,
            start,
            consumed: 0,
            finished: false,
            generation: 0,
        }
    }

    pub fn reset(&mut self, start: f64) -> u64 {
        self.samples.clear();
        self.start = start;
        self.consumed = 0;
        self.finished = false;
        self.generation += 1;
        self.generation
    }

    pub fn room(&self) -> usize {
        self.capacity.saturating_sub(self.samples.len()) / STEREO
    }

    pub fn push(&mut self, generation: u64, samples: &[f32]) {
        if generation == self.generation {
            self.samples.extend(samples);
        }
    }

    pub fn finish(&mut self, generation: u64) {
        if generation == self.generation {
            self.finished = true;
        }
    }

    pub fn read(&mut self, out: &mut [f32]) {
        let available = self.samples.len().min(out.len());
        for (slot, sample) in out.iter_mut().zip(self.samples.drain(..available)) {
            *slot = sample;
        }
        out[available..].fill(0.);
        let advanced = if self.finished { out.len() } else { available };
        self.consumed += (advanced / STEREO) as u64;
    }

    pub fn position(&self, latency: f64) -> f64 {
        let played = self.consumed as f64 / f64::from(self.rate);
        self.start + (played - latency).max(0.)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alignment {
    pub silence: usize,
    pub skip: usize,
}

pub fn align(expected: i64, pts: i64, frames: usize, tolerance: i64) -> Alignment {
    let gap = pts - expected;
    if gap > tolerance {
        Alignment {
            silence: gap as usize,
            skip: 0,
        }
    } else if gap < -tolerance {
        Alignment {
            silence: 0,
            skip: (expected - pts).min(frames as i64) as usize,
        }
    } else {
        Alignment {
            silence: 0,
            skip: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct TrackBuffer {
    pub samples: VecDeque<f32>,
    pub done: bool,
}

impl TrackBuffer {
    fn frames(&self) -> usize {
        self.samples.len() / STEREO
    }
}

pub fn mixable(tracks: &[TrackBuffer]) -> usize {
    let waiting = tracks
        .iter()
        .filter(|track| !track.done)
        .map(TrackBuffer::frames)
        .min();
    waiting.unwrap_or_else(|| tracks.iter().map(TrackBuffer::frames).max().unwrap_or(0))
}

pub fn mix(tracks: &mut [TrackBuffer], frames: usize) -> Vec<f32> {
    let mut mixed = vec![0.; frames * STEREO];
    for track in tracks {
        let take = track.samples.len().min(mixed.len());
        for (slot, sample) in mixed.iter_mut().zip(track.samples.drain(..take)) {
            *slot += sample;
        }
    }
    for sample in &mut mixed {
        *sample = sample.clamp(-1., 1.);
    }
    mixed
}

struct TrackReader {
    input: ffmpeg::format::context::Input,
    stream: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: resampling::Context,
    time_base: f64,
    start: f64,
    delay: f64,
    rate: u32,
    expected: i64,
    draining: bool,
}

pub fn has_audio(path: &Path) -> bool {
    ffmpeg::format::input(path)
        .is_ok_and(|input| input.streams().best(ffmpeg::media::Type::Audio).is_some())
}

impl TrackReader {
    fn open(track: &AudioTrack, rate: u32) -> Result<Option<Self>, ffmpeg::Error> {
        let input = ffmpeg::format::input(&track.path)?;
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Ok(None);
        };
        let index = stream.index();
        let time_base = f64::from(stream.time_base());
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .audio()?;
        let layout = match decoder.channel_layout() {
            layout if layout.is_empty() => ChannelLayout::default(i32::from(decoder.channels())),
            layout => layout,
        };
        let resampler = resampling::Context::get(
            decoder.format(),
            layout,
            decoder.rate(),
            FfmpegSample::F32(SampleType::Packed),
            ChannelLayout::STEREO,
            rate,
        )?;
        let start = container_start(&input);
        Ok(Some(Self {
            input,
            stream: index,
            decoder,
            resampler,
            time_base,
            start,
            delay: track.delay,
            rate,
            expected: 0,
            draining: false,
        }))
    }

    fn seek(&mut self, local: f64) {
        self.expected = (local * f64::from(self.rate)).round() as i64;
        self.draining = false;
        let target = ((local - self.delay).max(0.) + self.start) * 1_000_000.;
        let target = target as i64;
        if let Err(error) = self.input.seek(target, ..target) {
            tracing::debug!("seeking the audio failed: {error}");
        }
        self.decoder.flush();
    }

    fn fill(&mut self, into: &mut TrackBuffer) {
        let mut decoded = ffmpeg::frame::Audio::empty();
        loop {
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    self.take(&decoded, into);
                    return;
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::ffi::EAGAIN => {}
                Err(_) => {
                    into.done = true;
                    return;
                }
            }
            if self.draining {
                into.done = true;
                return;
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => {
                    if let Err(error) = self.decoder.send_packet(&packet) {
                        tracing::debug!("skipping an audio packet: {error}");
                    }
                }
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.draining = true;
                    if self.decoder.send_eof().is_err() {
                        into.done = true;
                        return;
                    }
                }
                Err(error) => {
                    tracing::debug!("reading the audio failed: {error}");
                    into.done = true;
                    return;
                }
            }
        }
    }

    fn take(&mut self, decoded: &ffmpeg::frame::Audio, into: &mut TrackBuffer) {
        let capacity = decoded.samples() as u64 * u64::from(self.rate)
            / u64::from(decoded.rate().max(1))
            + 256;
        let mut resampled = ffmpeg::frame::Audio::new(
            FfmpegSample::F32(SampleType::Packed),
            capacity as usize,
            ChannelLayout::STEREO,
        );
        if let Err(error) = self.resampler.run(decoded, &mut resampled) {
            tracing::debug!("resampling the audio failed: {error}");
            return;
        }
        let frames = resampled.samples();
        let seconds = decoded
            .pts()
            .map_or(self.expected as f64 / f64::from(self.rate), |pts| {
                pts as f64 * self.time_base - self.start + self.delay
            });
        let pts = (seconds * f64::from(self.rate)).round() as i64;
        let tolerance = (GAP_TOLERANCE_SECONDS * f64::from(self.rate)) as i64;
        let alignment = align(self.expected, pts, frames, tolerance);
        into.samples
            .extend(std::iter::repeat_n(0., alignment.silence * STEREO));
        let bytes = &resampled.data(0)[..frames * STEREO * size_of::<f32>()];
        into.samples.extend(
            bytes
                .chunks_exact(size_of::<f32>())
                .skip(alignment.skip * STEREO)
                .map(|sample| f32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]])),
        );
        self.expected += (alignment.silence + frames - alignment.skip) as i64;
    }
}

struct SegmentMix {
    readers: Vec<TrackReader>,
    tracks: Vec<TrackBuffer>,
    emitted: u64,
    frames: u64,
}

impl SegmentMix {
    fn open(audio: &[AudioTrack], local: f64, duration: f64, rate: u32) -> Self {
        let mut readers: Vec<TrackReader> = audio
            .iter()
            .filter_map(|track| match TrackReader::open(track, rate) {
                Ok(reader) => reader,
                Err(error) => {
                    tracing::warn!(path = %track.path.display(), "opening the audio failed: {error}");
                    None
                }
            })
            .collect();
        for reader in &mut readers {
            reader.seek(local);
        }
        let tracks = readers.iter().map(|_| TrackBuffer::default()).collect();
        Self {
            readers,
            tracks,
            emitted: (local * f64::from(rate)).round() as u64,
            frames: (duration * f64::from(rate)).round() as u64,
        }
    }

    fn next_block(&mut self) -> Option<Vec<f32>> {
        let left = self.frames.saturating_sub(self.emitted) as usize;
        if left == 0 {
            return None;
        }
        let block = left.min(BLOCK_FRAMES);
        while mixable(&self.tracks) < block {
            let Some(index) = self
                .tracks
                .iter()
                .position(|track| !track.done && track.frames() < block)
            else {
                break;
            };
            self.readers[index].fill(&mut self.tracks[index]);
        }
        let ready = mixable(&self.tracks).min(block);
        let frames = if ready == 0 { block } else { ready };
        self.emitted += frames as u64;
        Some(mix(&mut self.tracks, frames))
    }
}

pub struct SegmentedMix {
    segments: Vec<Segment>,
    timeline: Timeline,
    rate: u32,
    current: usize,
    mix: SegmentMix,
}

impl SegmentedMix {
    pub fn new(segments: Vec<Segment>, timeline: Timeline, rate: u32, position: f64) -> Self {
        let (current, local) = timeline.locate(position);
        let mix = SegmentMix::open(
            &segments[current].audio,
            local,
            timeline.duration_of(current),
            rate,
        );
        Self {
            segments,
            timeline,
            rate,
            current,
            mix,
        }
    }

    pub fn seek(&mut self, position: f64) {
        *self = Self::new(
            std::mem::take(&mut self.segments),
            self.timeline.clone(),
            self.rate,
            position,
        );
    }

    pub fn next_block(&mut self) -> Option<Vec<f32>> {
        loop {
            if let Some(block) = self.mix.next_block() {
                return Some(block);
            }
            if self.current + 1 >= self.timeline.len() {
                return None;
            }
            self.current += 1;
            self.mix = SegmentMix::open(
                &self.segments[self.current].audio,
                0.,
                self.timeline.duration_of(self.current),
                self.rate,
            );
        }
    }
}

pub enum AudioCommand {
    Seek { generation: u64, position: f64 },
    Stop,
}

#[derive(Clone)]
pub struct Output {
    buffer: Arc<Mutex<AudioBuffer>>,
    muted: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
}

impl Output {
    fn new(rate: u32, position: f64, muted: bool) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(AudioBuffer::new(rate, position))),
            muted: Arc::new(AtomicBool::new(muted)),
            playing: Arc::new(AtomicBool::new(false)),
        }
    }

    fn buffer(&self) -> MutexGuard<'_, AudioBuffer> {
        super::lock(&self.buffer)
    }

    pub fn reset(&self, position: f64) -> u64 {
        self.buffer().reset(position)
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
    }
}

pub struct Feed {
    pub segments: Vec<Segment>,
    pub timeline: Timeline,
    pub position: f64,
    pub muted: bool,
    pub clock: Arc<Clock>,
    pub tap: Option<AudioTap>,
}

struct Callback {
    output: Output,
    clock: Arc<Clock>,
    tap: Option<AudioTap>,
    channels: usize,
    stereo: Vec<f32>,
}

impl Callback {
    fn fill<T: SizedSample + FromSample<f32>>(&mut self, out: &mut [T], latency: f64) {
        if !self.output.playing.load(Ordering::Acquire) {
            out.fill(T::EQUILIBRIUM);
            return;
        }
        let frames = out.len() / self.channels;
        self.stereo.resize(frames * STEREO, 0.);
        let now = Instant::now();
        {
            let mut buffer = self.output.buffer();
            let reached = buffer.position(latency);
            buffer.read(&mut self.stereo);
            self.clock.place(reached, now);
        }
        if let Some(tap) = &mut self.tap {
            tap(&self.stereo, now + Duration::from_secs_f64(latency));
        }
        let muted = self.output.muted.load(Ordering::Acquire);
        for (frame, pair) in out
            .chunks_exact_mut(self.channels)
            .zip(self.stereo.chunks_exact(STEREO))
        {
            write_frame(frame, pair, muted);
        }
    }
}

fn write_frame<T: SizedSample + FromSample<f32>>(frame: &mut [T], pair: &[f32], muted: bool) {
    let gain = if muted { 0. } else { 1. };
    match frame {
        [only] => *only = T::from_sample((pair[0] + pair[1]) * 0.5 * gain),
        [left, right, rest @ ..] => {
            *left = T::from_sample(pair[0] * gain);
            *right = T::from_sample(pair[1] * gain);
            rest.fill(T::EQUILIBRIUM);
        }
        [] => {}
    }
}

fn output_latency(info: &cpal::OutputCallbackInfo) -> f64 {
    let stamp = info.timestamp();
    stamp
        .playback
        .duration_since(&stamp.callback)
        .map_or(0., |latency| latency.as_secs_f64())
        .min(MAX_OUTPUT_LATENCY)
}

fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut callback: Callback,
    clock: Arc<Clock>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |out: &mut [T], info| callback.fill(out, output_latency(info)),
        move |error| {
            tracing::warn!("the preview's audio stream failed: {error}");
            clock.follow_wall(Instant::now());
        },
        None,
    )
}

fn open_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    callback: Callback,
    clock: Arc<Clock>,
) -> Option<cpal::Stream> {
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(device, &config, callback, clock),
        SampleFormat::F64 => build_stream::<f64>(device, &config, callback, clock),
        SampleFormat::I16 => build_stream::<i16>(device, &config, callback, clock),
        SampleFormat::I32 => build_stream::<i32>(device, &config, callback, clock),
        SampleFormat::U16 => build_stream::<u16>(device, &config, callback, clock),
        SampleFormat::U8 => build_stream::<u8>(device, &config, callback, clock),
        format => {
            tracing::warn!("no preview audio for the sample format {format:?}");
            return None;
        }
    };
    stream
        .and_then(|stream| {
            stream
                .play()
                .map_err(|error| cpal::BuildStreamError::BackendSpecific {
                    err: cpal::BackendSpecificError {
                        description: error.to_string(),
                    },
                })?;
            Ok(stream)
        })
        .map_err(|error| tracing::warn!("opening the preview's audio failed: {error}"))
        .ok()
}

fn next_command(
    commands: &flume::Receiver<AudioCommand>,
    waiting: bool,
) -> Result<Option<AudioCommand>, flume::RecvTimeoutError> {
    let first = if waiting {
        match commands.recv_timeout(IDLE_WAIT) {
            Ok(command) => command,
            Err(flume::RecvTimeoutError::Timeout) => return Ok(None),
            Err(error) => return Err(error),
        }
    } else {
        match commands.try_recv() {
            Ok(command) => command,
            Err(_) => return Ok(None),
        }
    };
    Ok(std::iter::once(first)
        .chain(commands.try_iter())
        .reduce(|latest, command| match latest {
            AudioCommand::Stop => latest,
            AudioCommand::Seek { .. } => command,
        }))
}

pub fn run(
    feed: Feed,
    commands: flume::Receiver<AudioCommand>,
    ready: flume::Sender<Option<Output>>,
) {
    let Feed {
        segments,
        timeline,
        position,
        muted,
        clock,
        tap,
    } = feed;
    let device = cpal::default_host().default_output_device();
    let supported = device
        .as_ref()
        .and_then(|device| device.default_output_config().ok());
    let (Some(device), Some(supported)) = (device, supported) else {
        ready.send(None).ok();
        return;
    };
    let rate = supported.sample_rate().0;
    let output = Output::new(rate, position, muted);
    let callback = Callback {
        output: output.clone(),
        clock: clock.clone(),
        tap,
        channels: usize::from(supported.channels()),
        stereo: Vec::new(),
    };
    let Some(_stream) = open_stream(&device, &supported, callback, clock) else {
        ready.send(None).ok();
        return;
    };
    if ready.send(Some(output.clone())).is_err() {
        return;
    }
    let mut generation = 0;
    let mut mix = SegmentedMix::new(segments, timeline, rate, position);
    let mut finished = false;
    loop {
        let waiting = finished || output.buffer().room() < BLOCK_FRAMES;
        match next_command(&commands, waiting) {
            Ok(Some(AudioCommand::Seek {
                generation: next,
                position,
            })) => {
                generation = next;
                mix.seek(position);
                finished = false;
                continue;
            }
            Ok(None) => {}
            Ok(Some(AudioCommand::Stop)) | Err(_) => return,
        }
        if waiting {
            continue;
        }
        match mix.next_block() {
            Some(block) => output.buffer().push(generation, &block),
            None => {
                output.buffer().finish(generation);
                finished = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_advances_only_by_what_was_played() {
        let mut buffer = AudioBuffer::new(100, 4.);
        buffer.push(0, &[0.5; 40]);
        let mut out = [1.; 20];
        buffer.read(&mut out);
        assert_eq!(out, [0.5; 20]);
        assert!((buffer.position(0.) - 4.1).abs() < 1e-9);
    }

    #[test]
    fn an_underrun_plays_silence_and_holds_the_position() {
        let mut buffer = AudioBuffer::new(100, 0.);
        buffer.push(0, &[0.5; 4]);
        let mut out = [1.; 8];
        buffer.read(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.5, 0.5, 0., 0., 0., 0.]);
        assert!((buffer.position(0.) - 0.02).abs() < 1e-9);
    }

    #[test]
    fn after_the_last_sample_silence_keeps_the_clock_moving() {
        let mut buffer = AudioBuffer::new(100, 0.);
        buffer.finish(0);
        let mut out = [1.; 200];
        buffer.read(&mut out);
        assert!((buffer.position(0.) - 1.).abs() < 1e-9);
    }

    #[test]
    fn output_latency_keeps_the_position_behind_the_read_head_but_not_before_the_start() {
        let mut buffer = AudioBuffer::new(100, 2.);
        assert_eq!(buffer.position(0.05), 2.);
        buffer.push(0, &[0.; 40]);
        let mut out = [0.; 40];
        buffer.read(&mut out);
        assert!((buffer.position(0.05) - 2.15).abs() < 1e-9);
    }

    #[test]
    fn a_seek_drops_what_was_buffered_and_blocks_from_before_it() {
        let mut buffer = AudioBuffer::new(100, 0.);
        buffer.push(0, &[0.5; 8]);
        let generation = buffer.reset(7.);
        buffer.push(0, &[0.5; 8]);
        assert_eq!(buffer.room(), 200);
        buffer.push(generation, &[0.25; 2]);
        let mut out = [0.; 2];
        buffer.read(&mut out);
        assert_eq!(out, [0.25; 2]);
        assert!((buffer.position(0.) - 7.01).abs() < 1e-9);
    }

    #[test]
    fn a_late_block_is_preceded_by_silence() {
        assert_eq!(
            align(100, 500, 10, 8),
            Alignment {
                silence: 400,
                skip: 0
            }
        );
    }

    #[test]
    fn a_block_from_before_the_target_is_skipped_up_to_its_length() {
        assert_eq!(
            align(100, 95, 1000, 2),
            Alignment {
                silence: 0,
                skip: 5
            }
        );
        assert_eq!(
            align(5000, 95, 1000, 2),
            Alignment {
                silence: 0,
                skip: 1000
            }
        );
    }

    #[test]
    fn small_timestamp_jitter_changes_nothing() {
        let none = Alignment {
            silence: 0,
            skip: 0,
        };
        assert_eq!(align(100, 104, 10, 8), none);
        assert_eq!(align(100, 96, 10, 8), none);
    }

    fn track(samples: &[f32], done: bool) -> TrackBuffer {
        TrackBuffer {
            samples: samples.iter().copied().collect(),
            done,
        }
    }

    #[test]
    fn a_mix_waits_for_the_slowest_track_still_decoding() {
        let tracks = [track(&[0.; 8], false), track(&[0.; 2], false)];
        assert_eq!(mixable(&tracks), 1);
    }

    #[test]
    fn a_finished_track_no_longer_holds_the_mix_back() {
        let tracks = [track(&[0.; 8], false), track(&[], true)];
        assert_eq!(mixable(&tracks), 4);
        let tracks = [track(&[0.; 8], true), track(&[0.; 2], true)];
        assert_eq!(mixable(&tracks), 4);
    }

    #[test]
    fn tracks_add_up_and_clip_at_full_scale() {
        let mut tracks = [
            track(&[0.5, 0.25, 0.75, 0.], false),
            track(&[0.25, 0.25, 0.75, 0.], true),
        ];
        assert_eq!(mix(&mut tracks, 2), vec![0.75, 0.5, 1., 0.]);
        assert!(tracks[0].samples.is_empty());
    }

    #[test]
    fn a_short_track_mixes_as_silence() {
        let mut tracks = [track(&[0.5; 4], false), track(&[0.25, 0.25], true)];
        assert_eq!(mix(&mut tracks, 2), vec![0.75, 0.75, 0.5, 0.5]);
    }

    fn seek_to(position: f64) -> AudioCommand {
        AudioCommand::Seek {
            generation: 1,
            position,
        }
    }

    #[test]
    fn only_the_latest_of_many_seeks_is_taken() {
        let (commands, receiver) = flume::unbounded();
        commands.send(seek_to(1.)).unwrap();
        commands.send(seek_to(2.)).unwrap();
        let Ok(Some(AudioCommand::Seek { position, .. })) = next_command(&receiver, false) else {
            panic!("expected a seek");
        };
        assert_eq!(position, 2.);
    }

    #[test]
    fn a_stop_wins_over_queued_seeks() {
        let (commands, receiver) = flume::unbounded();
        commands.send(seek_to(1.)).unwrap();
        commands.send(AudioCommand::Stop).unwrap();
        commands.send(seek_to(2.)).unwrap();
        assert!(matches!(
            next_command(&receiver, false),
            Ok(Some(AudioCommand::Stop))
        ));
    }

    #[test]
    fn stereo_goes_to_the_first_two_channels_and_mono_takes_the_average() {
        let mut surround = [1f32; 4];
        write_frame(&mut surround, &[0.5, 0.25], false);
        assert_eq!(surround, [0.5, 0.25, 0., 0.]);
        let mut mono = [1f32];
        write_frame(&mut mono, &[0.5, 0.25], false);
        assert_eq!(mono, [0.375]);
    }

    #[test]
    fn muting_silences_the_output() {
        let mut stereo = [1f32; 2];
        write_frame(&mut stereo, &[0.5, 0.25], true);
        assert_eq!(stereo, [0., 0.]);
    }
}
