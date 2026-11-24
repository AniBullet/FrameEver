use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use frameever_project::{RecordingMeta, RecordingMetaInner, StudioRecordingMeta};
use serde::{Deserialize, Serialize};

use super::{MediaKind, view::Entry};

pub const BARS: usize = 48;
const CACHE_FILE: &str = "library-visuals.json";
const CACHE_VERSION: u32 = 1;
const STEP_MS: u64 = 100;
const EXCERPT_CHARS: usize = 120;
const CONFIG_READ_MAX: u64 = 8 * 1024 * 1024;
const PROJECT_CONFIG: &str = "project-config.json";
const RECORDING_META: &str = "recording-meta.json";

pub type Wave = Arc<[u8; BARS]>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Visual {
    pub wave: Option<Wave>,
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub project: PathBuf,
    audio_only: bool,
}

impl Job {
    pub fn of(entry: &Entry) -> Option<Self> {
        (entry.kind == MediaKind::Studio && entry.screenshot.is_none()).then(|| Self {
            project: entry.path.clone(),
            audio_only: entry.audio_only,
        })
    }

    fn stamp(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |value: u128| {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        };
        for name in [RECORDING_META, PROJECT_CONFIG] {
            match std::fs::metadata(self.project.join(name)) {
                Ok(meta) => {
                    mix(u128::from(meta.len()));
                    mix(meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map_or(0, |since| since.as_nanos()));
                }
                Err(_) => mix(u128::MAX),
            }
        }
        mix(u128::from(self.audio_only));
        format!("{hash:016x}")
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Cache {
    version: u32,
    #[serde(default)]
    items: BTreeMap<String, Remembered>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remembered {
    stamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wave: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    excerpt: Option<String>,
}

impl Remembered {
    fn visual(&self) -> Visual {
        Visual {
            wave: self.wave.as_deref().and_then(wave_from_hex),
            excerpt: self.excerpt.clone(),
        }
    }
}

fn key_of(project: &Path) -> String {
    project.to_string_lossy().into_owned()
}

fn wave_to_hex(wave: &[u8; BARS]) -> String {
    wave.iter().map(|bar| format!("{bar:02x}")).collect()
}

fn wave_from_hex(hex: &str) -> Option<Wave> {
    if hex.len() != BARS * 2 {
        return None;
    }
    let mut wave = [0u8; BARS];
    for (index, slot) in wave.iter_mut().enumerate() {
        *slot = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(Arc::new(wave))
}

fn load_cache(path: &Path) -> Cache {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Cache>(&bytes).ok())
        .filter(|cache| cache.version == CACHE_VERSION)
        .unwrap_or_default()
}

fn save_cache(path: &Path, cache: &Cache) {
    let Some(dir) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(dir) {
        tracing::debug!(%error, "no folder for the library visuals cache");
        return;
    }
    let staged = dir.join(format!(".{CACHE_FILE}.{}.tmp", std::process::id()));
    let written = serde_json::to_vec(cache)
        .map_err(std::io::Error::other)
        .and_then(|json| {
            let mut file = std::fs::File::create(&staged)?;
            file.write_all(&json)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&staged, path));
    if let Err(error) = written {
        tracing::debug!(%error, "could not save the library visuals cache");
        if let Err(error) = std::fs::remove_file(&staged) {
            tracing::debug!(%error, "no staged visuals cache to remove");
        }
    }
}

pub fn cached(jobs: Vec<Job>, dir: &Path) -> (HashMap<PathBuf, Visual>, Vec<(Job, String)>) {
    let cache = load_cache(&dir.join(CACHE_FILE));
    let mut known = HashMap::with_capacity(jobs.len());
    let mut todo = Vec::new();
    for job in jobs {
        let stamp = job.stamp();
        match cache.items.get(&key_of(&job.project)) {
            Some(held) if held.stamp == stamp => {
                known.insert(job.project, held.visual());
            }
            _ => todo.push((job, stamp)),
        }
    }
    (known, todo)
}

pub fn work(job: &Job) -> Visual {
    if job.audio_only {
        Visual {
            wave: audio_wave(&job.project),
            excerpt: None,
        }
    } else {
        Visual {
            wave: None,
            excerpt: caption_excerpt(&job.project),
        }
    }
}

pub fn work_batch(dir: &Path, batch: Vec<(Job, String)>) -> Vec<(PathBuf, Visual)> {
    let done: Vec<(PathBuf, String, Visual)> = batch
        .into_iter()
        .map(|(job, stamp)| {
            let visual = work(&job);
            (job.project, stamp, visual)
        })
        .collect();
    remember(dir, &done, None);
    done.into_iter()
        .map(|(project, _, visual)| (project, visual))
        .collect()
}

pub fn forget_all_but(dir: &Path, keep: &HashSet<PathBuf>) {
    remember(dir, &[], Some(keep));
}

fn remember(dir: &Path, done: &[(PathBuf, String, Visual)], keep: Option<&HashSet<PathBuf>>) {
    static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = WRITER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = dir.join(CACHE_FILE);
    let mut cache = load_cache(&path);
    cache.version = CACHE_VERSION;
    for (project, stamp, visual) in done {
        cache.items.insert(
            key_of(project),
            Remembered {
                stamp: stamp.clone(),
                wave: visual.wave.as_deref().map(wave_to_hex),
                excerpt: visual.excerpt.clone(),
            },
        );
    }
    if let Some(keep) = keep {
        let keep: HashSet<String> = keep.iter().map(|project| key_of(project)).collect();
        cache.items.retain(|key, _| keep.contains(key));
    }
    save_cache(&path, &cache);
}

#[derive(Deserialize)]
struct CaptionsOnly {
    #[serde(default)]
    captions: Option<Captions>,
}

#[derive(Deserialize)]
struct Captions {
    #[serde(default)]
    segments: Vec<Said>,
}

#[derive(Deserialize)]
struct Said {
    #[serde(default)]
    start: f32,
    #[serde(default)]
    text: String,
}

fn caption_excerpt(project: &Path) -> Option<String> {
    let path = project.join(PROJECT_CONFIG);
    if std::fs::metadata(&path).map_or(true, |meta| meta.len() > CONFIG_READ_MAX) {
        return None;
    }
    let config: CaptionsOnly = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    let mut said = config.captions?.segments;
    said.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut text = String::new();
    for line in said {
        let line = line.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&line);
        if text.chars().count() > EXCERPT_CHARS {
            break;
        }
    }
    clip(text)
}

fn clip(text: String) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= EXCERPT_CHARS {
        return Some(text.to_owned());
    }
    let cut: String = text.chars().take(EXCERPT_CHARS).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    Some(format!(
        "{}…",
        cut.trim_end_matches([',', ';', ':', '.', '，', '。'])
    ))
}

fn audio_wave(project: &Path) -> Option<Wave> {
    let meta = RecordingMeta::load_for_project(project).ok()?;
    let RecordingMetaInner::Studio(studio) = &meta.inner else {
        return None;
    };
    let StudioRecordingMeta::MultipleSegments { inner } = &**studio else {
        return None;
    };
    let mut parts = Vec::new();
    let mut at = 0u64;
    for segment in &inner.segments {
        let mut longest = 0u64;
        for audio in segment.mic.iter().chain(segment.system_audio.iter()) {
            if let Some((steps, ms)) = envelope(&meta.path(&audio.path)) {
                longest = longest.max(ms);
                parts.push((steps, at));
            }
        }
        at += longest;
    }
    (at > 0).then(|| combine(&parts, at)).flatten()
}

#[derive(Default)]
struct Loudness {
    steps: Vec<f32>,
    sum: f64,
    count: usize,
    step_len: usize,
    samples: u64,
    rate: u32,
}

impl Loudness {
    fn take(&mut self, frame: &ffmpeg::frame::Audio) {
        if self.rate == 0 {
            self.rate = frame.rate().max(1);
            self.step_len = (u64::from(self.rate) * STEP_MS / 1000).max(1) as usize;
        }
        let samples = frame.samples();
        let stride = if frame.is_planar() {
            1
        } else {
            usize::from(frame.channels()).max(1)
        };
        let data = frame.data(0);
        let (width, read): (usize, fn(&[u8]) -> f32) = match frame.format() {
            ffmpeg::format::Sample::F32(_) => (4, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            ffmpeg::format::Sample::I16(_) => {
                (2, |b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.)
            }
            ffmpeg::format::Sample::I32(_) => (4, |b| {
                i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.
            }),
            _ => return,
        };
        for index in 0..samples {
            let at = index * stride * width;
            let Some(bytes) = data.get(at..at + width) else {
                break;
            };
            let sample = read(bytes);
            self.sum += f64::from(sample * sample);
            self.count += 1;
            if self.count == self.step_len {
                self.steps
                    .push((self.sum / self.count as f64).sqrt() as f32);
                self.sum = 0.;
                self.count = 0;
            }
        }
        self.samples += samples as u64;
    }
}

fn envelope(path: &Path) -> Option<(Vec<f32>, u64)> {
    let mut input = ffmpeg::format::input(path).ok()?;
    let stream = input.streams().best(ffmpeg::media::Type::Audio)?;
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .ok()?
        .decoder()
        .audio()
        .ok()?;
    let mut loudness = Loudness::default();
    let mut frame = ffmpeg::frame::Audio::empty();
    for (stream, packet) in input.packets() {
        if stream.index() != index || decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut frame).is_ok() {
            loudness.take(&frame);
        }
    }
    if decoder.send_eof().is_ok() {
        while decoder.receive_frame(&mut frame).is_ok() {
            loudness.take(&frame);
        }
    }
    if loudness.count > 0 {
        loudness
            .steps
            .push((loudness.sum / loudness.count as f64).sqrt() as f32);
    }
    if loudness.rate == 0 {
        return None;
    }
    Some((
        loudness.steps,
        loudness.samples * 1000 / u64::from(loudness.rate),
    ))
}

fn combine(parts: &[(Vec<f32>, u64)], total_ms: u64) -> Option<Wave> {
    let mut line = vec![0f32; (total_ms / STEP_MS).max(1) as usize];
    for (steps, offset_ms) in parts {
        let start = (*offset_ms / STEP_MS) as usize;
        for (index, value) in steps.iter().enumerate() {
            if let Some(slot) = line.get_mut(start + index) {
                *slot = slot.max(*value);
            }
        }
    }
    bars(&line)
}

fn bars(line: &[f32]) -> Option<Wave> {
    if line.is_empty() {
        return None;
    }
    let mut levels = [0f32; BARS];
    for (bar, slot) in levels.iter_mut().enumerate() {
        let from = (bar * line.len() / BARS).min(line.len() - 1);
        let to = ((bar + 1) * line.len() / BARS)
            .max(from + 1)
            .min(line.len());
        let peak = line[from..to]
            .iter()
            .fold(0f32, |most, value| most.max(*value));
        let db = if peak > 0. { 20. * peak.log10() } else { -90. };
        *slot = ((db + 54.) / 54.).clamp(0., 1.);
    }
    let top = levels.iter().fold(0f32, |most, value| most.max(*value));
    if top <= 0.02 {
        return None;
    }
    let mut wave = [0u8; BARS];
    for (bar, level) in wave.iter_mut().zip(levels) {
        *bar = ((level / top) * 255.).round() as u8;
    }
    Some(Arc::new(wave))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud_wave() -> Wave {
        let line: Vec<f32> = (0..480).map(|i| 0.01 + (i % 48) as f32 / 100.).collect();
        bars(&line).unwrap()
    }

    #[test]
    fn a_wave_survives_the_cache_text() {
        let wave = loud_wave();
        let hex = wave_to_hex(&wave);
        assert_eq!(wave_from_hex(&hex).as_deref(), Some(&*wave));
        assert!(wave_from_hex("zz").is_none());
    }

    #[test]
    fn bars_fill_to_the_loudest_and_silence_gives_nothing() {
        let mut line = vec![0.0001f32; 640];
        line[320] = 0.5;
        let wave = bars(&line).unwrap();
        assert_eq!(*wave.iter().max().unwrap(), 255);
        assert!(bars(&vec![0f32; 640]).is_none());
        assert!(bars(&[]).is_none());
    }

    #[test]
    fn parts_land_at_their_offsets() {
        let loud = vec![0.5f32; 10];
        let wave = combine(&[(loud, 9_000)], 10_000).unwrap();
        assert!(wave[..BARS * 8 / 10].iter().all(|bar| *bar < 40));
        assert!(wave[BARS - 2] > 200);
    }

    #[test]
    fn long_text_is_cut_on_a_word() {
        let text = "word ".repeat(100);
        let clipped = clip(text).unwrap();
        assert!(clipped.ends_with('\u{2026}'));
        assert!(clipped.chars().count() <= EXCERPT_CHARS + 1);
        assert_eq!(clip("  ".into()), None);
        assert_eq!(clip("Short line".into()).as_deref(), Some("Short line"));
    }

    fn project_with_captions(dir: &Path, lines: &[(f32, &str)]) -> PathBuf {
        let project = dir.join("Talk.ever");
        std::fs::create_dir_all(&project).unwrap();
        let segments: Vec<serde_json::Value> = lines
            .iter()
            .map(|(start, text)| serde_json::json!({ "id": "s", "start": start, "end": start + 1.0, "text": text }))
            .collect();
        std::fs::write(
            project.join(PROJECT_CONFIG),
            serde_json::to_vec(
                &serde_json::json!({ "captions": { "segments": segments, "settings": {} } }),
            )
            .unwrap(),
        )
        .unwrap();
        project
    }

    #[test]
    fn the_first_caption_words_come_in_spoken_order() {
        let dir = tempfile::tempdir().unwrap();
        let project = project_with_captions(
            dir.path(),
            &[(4.0, "the walk-through."), (0.5, "Welcome to")],
        );
        assert_eq!(
            caption_excerpt(&project).as_deref(),
            Some("Welcome to the walk-through.")
        );
        let silent = dir.path().join("Silent.ever");
        std::fs::create_dir_all(&silent).unwrap();
        std::fs::write(silent.join(PROJECT_CONFIG), b"{}").unwrap();
        assert_eq!(caption_excerpt(&silent), None);
    }

    #[test]
    fn edited_captions_replace_the_words_a_rescan_had_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let project = project_with_captions(dir.path(), &[(0.0, "We planned the launch")]);
        let job = Job {
            project: project.clone(),
            audio_only: false,
        };
        let keep = HashSet::from([project.clone()]);

        let (known, todo) = cached(vec![job.clone()], &cache);
        assert!(known.is_empty());
        let done: Vec<_> = todo
            .into_iter()
            .map(|(job, stamp)| (job.project.clone(), stamp, work(&job)))
            .collect();
        remember(&cache, &done, Some(&keep));
        let (known, todo) = cached(vec![job.clone()], &cache);
        assert!(todo.is_empty());
        assert_eq!(
            known[&project].excerpt.as_deref(),
            Some("We planned the launch")
        );

        std::thread::sleep(std::time::Duration::from_millis(20));
        project_with_captions(dir.path(), &[(0.0, "We moved the launch to Friday")]);
        let (known, todo) = cached(vec![job], &cache);
        assert!(known.is_empty());
        assert_eq!(
            work(&todo[0].0).excerpt.as_deref(),
            Some("We moved the launch to Friday")
        );
    }

    #[test]
    fn a_pass_stopped_partway_keeps_what_it_finished_for_the_next_one() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let jobs: Vec<Job> = ["One", "Two", "Three"]
            .into_iter()
            .map(|name| {
                let folder = dir.path().join(name);
                std::fs::create_dir_all(&folder).unwrap();
                Job {
                    project: project_with_captions(&folder, &[(0.0, name)]),
                    audio_only: false,
                }
            })
            .collect();

        let (_, mut todo) = cached(jobs.clone(), &cache);
        assert_eq!(todo.len(), 3);
        let first = todo.remove(0);
        let worked = work_batch(&cache, vec![first]);
        assert_eq!(worked.len(), 1);
        assert_eq!(worked[0].1.excerpt.as_deref(), Some("One"));

        let (known, todo) = cached(jobs.clone(), &cache);
        assert_eq!(known.len(), 1);
        assert_eq!(known[&jobs[0].project].excerpt.as_deref(), Some("One"));
        assert_eq!(
            todo.iter().map(|(job, _)| &job.project).collect::<Vec<_>>(),
            [&jobs[1].project, &jobs[2].project]
        );
    }

    #[test]
    fn only_studio_projects_get_a_visual_job() {
        let studio = Entry {
            path: PathBuf::from("a.ever"),
            kind: MediaKind::Studio,
            name: "a".into(),
            size: 0,
            duration: None,
            created_millis: 0.,
            exports: Vec::new(),
            compression: crate::library::view::Compression::None,
            compressible: false,
            clip_count: 1,
            thumbnail: None,
            thumbnail_version: None,
            screenshot: None,
            opens_editor: true,
            audio_only: true,
            id: None,
            rebuilt: None,
        };
        assert!(Job::of(&studio).is_some());
        assert!(
            Job::of(&Entry {
                kind: MediaKind::Instant,
                ..studio.clone()
            })
            .is_none()
        );
        assert!(
            Job::of(&Entry {
                kind: MediaKind::Screenshot,
                ..studio
            })
            .is_none()
        );
    }
}
