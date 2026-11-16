use frameever_project::CaptionWord;

const TOKEN_REACH_SECONDS: f32 = 0.24;

pub(super) fn words_from_subword_tokens(
    tokens: impl IntoIterator<Item = CaptionWord>,
) -> Vec<CaptionWord> {
    let mut words: Vec<CaptionWord> = Vec::new();
    let mut open = false;
    for token in tokens {
        let text = token.text.trim_start();
        if text.trim().is_empty() {
            open = false;
            continue;
        }
        let joins = text.starts_with('\'') || text.starts_with('-');
        let starts_word = token.text.starts_with(' ') && !joins;
        match words.last_mut() {
            Some(word) if open && !starts_word => {
                word.text.push_str(text);
                if !text.chars().all(|c| c.is_ascii_punctuation()) {
                    word.end = token.end.min(token.start + TOKEN_REACH_SECONDS);
                }
            }
            _ => words.push(CaptionWord {
                text: text.to_string(),
                start: token.start,
                end: token.end.min(token.start + TOKEN_REACH_SECONDS),
            }),
        }
        open = true;
    }
    words
}

const HOP_SECONDS: f64 = 0.005;
const STATS_BLOCK_SECONDS: f64 = 1.0;
const STATS_SPAN_BLOCKS: usize = 5;
const SILENCE_ABOVE_FLOOR_DB: f32 = 8.0;
const SILENCE_BELOW_PEAK_DB: f32 = 45.0;
const MAX_RUN_EXTENSION_SECONDS: f64 = 2.0;

pub(super) struct Loudness {
    rate: f64,
    levels: Vec<f32>,
    silence_below: Vec<f32>,
}

impl Loudness {
    pub fn new(samples: &[f32], rate: u32) -> Self {
        let hop = ((f64::from(rate) * HOP_SECONDS) as usize).max(1);
        let frames = samples.len().div_ceil(hop);
        let levels: Vec<f32> = (0..frames)
            .map(|frame| {
                let centre = frame * hop;
                let window =
                    &samples[centre.saturating_sub(hop)..(centre + hop).min(samples.len())];
                let power = window.iter().map(|s| s * s).sum::<f32>() / window.len().max(1) as f32;
                10.0 * (power + 1e-12).log10()
            })
            .collect();
        let per_block = (STATS_BLOCK_SECONDS / HOP_SECONDS) as usize;
        let blocks = levels.len().div_ceil(per_block).max(1);
        let silence_below = (0..blocks)
            .map(|block| {
                let first = block.saturating_sub(STATS_SPAN_BLOCKS) * per_block;
                let last = ((block + STATS_SPAN_BLOCKS + 1) * per_block).min(levels.len());
                let mut window: Vec<f32> = levels.get(first..last).unwrap_or_default().to_vec();
                if window.is_empty() {
                    return f32::NEG_INFINITY;
                }
                window.sort_by(f32::total_cmp);
                let at = |q: f32| window[((window.len() - 1) as f32 * q) as usize];
                (at(0.1) + SILENCE_ABOVE_FLOOR_DB).max(at(0.95) - SILENCE_BELOW_PEAK_DB)
            })
            .collect();
        Self {
            rate: 1.0 / HOP_SECONDS,
            levels,
            silence_below,
        }
    }

    fn frame(&self, seconds: f64) -> usize {
        ((seconds * self.rate).round().max(0.0) as usize).min(self.levels.len())
    }

    fn seconds(&self, frame: usize) -> f64 {
        frame as f64 / self.rate
    }

    fn silent(&self, frame: usize) -> bool {
        let block = (frame as f64 / (STATS_BLOCK_SECONDS * self.rate)) as usize;
        let threshold = self
            .silence_below
            .get(block.min(self.silence_below.len().saturating_sub(1)))
            .copied()
            .unwrap_or(f32::NEG_INFINITY);
        self.levels
            .get(frame)
            .is_some_and(|level| *level < threshold)
    }

    pub fn silent_runs(&self, from: f64, to: f64, min_seconds: f64) -> Vec<(f64, f64)> {
        let (first, last) = (self.frame(from), self.frame(to));
        let min_frames = (min_seconds * self.rate).round().max(1.0) as usize;
        let reach = (MAX_RUN_EXTENSION_SECONDS * self.rate) as usize;
        let mut runs = Vec::new();
        let mut frame = first;
        while frame < last {
            if !self.silent(frame) {
                frame += 1;
                continue;
            }
            let mut start = frame;
            if frame == first {
                let floor = first.saturating_sub(reach);
                while start > floor && self.silent(start - 1) {
                    start -= 1;
                }
            }
            let ceiling = (last + reach).min(self.levels.len());
            while frame < ceiling && self.silent(frame) {
                frame += 1;
            }
            if frame - start >= min_frames {
                runs.push((self.seconds(start), self.seconds(frame)));
            }
        }
        runs
    }
}

pub(super) fn split_stutters(words: Vec<CaptionWord>) -> Vec<CaptionWord> {
    let mut out = Vec::with_capacity(words.len());
    for word in words {
        let body = word.text.trim_end_matches(|c: char| !c.is_alphanumeric());
        let tail = &word.text[body.len()..];
        let parts: Vec<&str> = body.split('-').collect();
        let alphabetic = parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(char::is_alphabetic));
        let repeats = parts
            .iter()
            .take_while(|part| part.to_lowercase() == parts[0].to_lowercase())
            .count();
        if !alphabetic || repeats < 2 {
            out.push(word);
            continue;
        }
        let mut pieces: Vec<String> = parts[..repeats]
            .iter()
            .map(|part| (*part).to_string())
            .collect();
        if repeats < parts.len() {
            pieces.push(parts[repeats..].join("-"));
        }
        let step = (word.end - word.start).max(0.0) / pieces.len() as f32;
        let last = pieces.len() - 1;
        for (index, piece) in pieces.into_iter().enumerate() {
            out.push(CaptionWord {
                text: if index == last {
                    format!("{piece}{tail}")
                } else {
                    piece
                },
                start: word.start + step * index as f32,
                end: word.start + step * (index + 1) as f32,
            });
        }
    }
    out
}

const SNAP_SEARCH_SECONDS: f64 = 0.1;
const SNAP_END_SEARCH_SECONDS: f64 = 0.45;
const SNAP_OUTER_SEARCH_SECONDS: f64 = 0.4;
const MIN_PAUSE_SECONDS: f64 = 0.03;
const MIN_WORD_SECONDS: f64 = 0.04;
const EDGE_SLACK_SECONDS: f64 = 0.02;

pub(super) fn snap_to_pauses(words: &mut [CaptionWord], loudness: &Loudness) {
    for index in 0..=words.len() {
        let (left, right) = words.split_at_mut(index);
        match (left.last_mut(), right.first_mut()) {
            (Some(before), Some(after)) => {
                let (end, start) = (f64::from(before.end), f64::from(after.start));
                let lower = f64::from(before.start) + MIN_WORD_SECONDS;
                let upper = f64::from(after.end) - MIN_WORD_SECONDS;
                let after_end = loudness
                    .silent_runs(
                        (end - EDGE_SLACK_SECONDS).max(lower),
                        (end + SNAP_END_SEARCH_SECONDS).min(start.max(end)),
                        MIN_PAUSE_SECONDS,
                    )
                    .into_iter()
                    .next();
                if let Some((silence_start, _)) = after_end {
                    before.end = silence_start.clamp(lower, upper.max(lower)) as f32;
                }
                let before_start = loudness
                    .silent_runs(
                        (start - SNAP_SEARCH_SECONDS).max(f64::from(before.end)),
                        (start + EDGE_SLACK_SECONDS).min(upper),
                        MIN_PAUSE_SECONDS,
                    )
                    .into_iter()
                    .last();
                if let Some((_, silence_end)) = before_start {
                    after.start = silence_end.clamp(f64::from(before.end), upper.max(lower)) as f32;
                }
                if after.start < before.end {
                    after.start = before.end;
                }
            }
            (None, Some(first)) => {
                let start = f64::from(first.start);
                let from = start - SNAP_OUTER_SEARCH_SECONDS;
                let to = (start + SNAP_SEARCH_SECONDS).min(f64::from(first.end) - MIN_WORD_SECONDS);
                if let Some((_, silence_end)) =
                    loudness.silent_runs(from, to, MIN_PAUSE_SECONDS).last()
                {
                    first.start = silence_end.min(f64::from(first.end) - MIN_WORD_SECONDS) as f32;
                }
            }
            (Some(last), None) => {
                let end = f64::from(last.end);
                let from =
                    (end - SNAP_SEARCH_SECONDS).max(f64::from(last.start) + MIN_WORD_SECONDS);
                let to = end + SNAP_OUTER_SEARCH_SECONDS;
                if let Some((silence_start, _)) =
                    loudness.silent_runs(from, to, MIN_PAUSE_SECONDS).first()
                {
                    last.end = silence_start.max(f64::from(last.start) + MIN_WORD_SECONDS) as f32;
                }
            }
            (None, None) => {}
        }
    }
}

fn keep_order(words: &mut [CaptionWord]) {
    for index in 1..words.len() {
        let previous_end = words[index - 1].end;
        let word = &mut words[index];
        if word.start < previous_end {
            word.start = previous_end;
        }
        if word.end < word.start {
            word.end = word.start;
        }
    }
}

pub(super) fn finish_word_timings(
    groups: Vec<Vec<CaptionWord>>,
    samples: &[f32],
    rate: u32,
) -> Vec<Vec<CaptionWord>> {
    let groups: Vec<Vec<CaptionWord>> = groups.into_iter().map(split_stutters).collect();
    let lengths: Vec<usize> = groups.iter().map(Vec::len).collect();
    let mut words: Vec<CaptionWord> = groups.into_iter().flatten().collect();
    snap_to_pauses(&mut words, &Loudness::new(samples, rate));
    keep_order(&mut words);
    let mut words = words.into_iter();
    lengths
        .into_iter()
        .map(|length| words.by_ref().take(length).collect())
        .collect()
}

#[cfg(test)]
mod tests;
