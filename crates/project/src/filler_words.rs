use std::{borrow::Cow, ops::Range};

use crate::{CaptionTrackSegment, CaptionWord, caption_text_from_words, is_cjk};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillerKind {
    Hesitation,
    FillerPhrase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillerWord {
    pub kind: FillerKind,
    pub words: Range<usize>,
}

const ENGLISH_HESITATIONS: &[&str] = &[
    "um", "umm", "uh", "uhh", "uhm", "er", "erm", "ah", "ahh", "eh", "hm", "hmm", "mm", "mmm",
];

const DRAWN_OUT_ENGLISH_HESITATIONS: &[&str] =
    &["um", "uh", "uhm", "er", "erm", "ehm", "ah", "eh", "hm"];

const CHINESE_HESITATIONS: &[char] = &['嗯', '呃', '额', '唔', '呣'];

const CHINESE_PARTICLES: &[char] = &['啊', '哦', '噢', '哎', '唉', '诶', '欸'];

const ENGLISH_FILLER_PHRASES: &[&[&str]] = &[
    &["you", "know"],
    &["i", "mean"],
    &["kind", "of"],
    &["sort", "of"],
    &["like"],
    &["basically"],
    &["actually"],
    &["literally"],
    &["honestly"],
];

const CHINESE_FILLER_PHRASES: &[&str] = &["怎么说呢", "就是说", "那个", "这个", "就是"];

const SETTING_OFF_PAUSE_SECONDS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trailing {
    None,
    Comma,
    Terminal,
    Exclamation,
    Question,
    Break,
}

impl Trailing {
    fn of(text: &str) -> Self {
        let trimmed = text
            .trim()
            .trim_end_matches(['"', '\'', '”', '’', ')', ']', '）', '」']);
        if trimmed.ends_with("...") || trimmed.ends_with('…') {
            return Self::Break;
        }
        match trimmed.chars().last() {
            Some('?' | '？') => Self::Question,
            Some('!' | '！') => Self::Exclamation,
            Some('.' | '。') => Self::Terminal,
            Some(',' | ';' | ':' | '，' | '、' | '；' | '：') => Self::Comma,
            Some('-' | '—' | '–') => Self::Break,
            _ => Self::None,
        }
    }
}

pub fn normalize_word(text: &str) -> String {
    text.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase()
}

fn is_english_hesitation(norm: &str) -> bool {
    if ENGLISH_HESITATIONS.contains(&norm) {
        return true;
    }
    if norm.is_empty() || !norm.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    if norm.len() >= 2 && norm.chars().all(|c| c == 'm') {
        return true;
    }
    let mut squeezed = String::with_capacity(norm.len());
    for c in norm.chars() {
        if !squeezed.ends_with(c) {
            squeezed.push(c);
        }
    }
    DRAWN_OUT_ENGLISH_HESITATIONS.contains(&squeezed.as_str())
}

fn is_chinese_hesitation(norm: &str) -> bool {
    !norm.is_empty() && norm.chars().all(|c| CHINESE_HESITATIONS.contains(&c))
}

fn is_chinese_particle(norm: &str) -> bool {
    let mut chars = norm.chars();
    chars
        .next()
        .is_some_and(|first| CHINESE_PARTICLES.contains(&first) && chars.all(|c| c == first))
}

struct Word {
    norm: String,
    trailing: Trailing,
    start: f32,
    end: f32,
}

fn sets_off_after(words: &[Word], hesitations: &[bool], last: usize) -> bool {
    if matches!(words[last].trailing, Trailing::Comma | Trailing::Break) {
        return true;
    }
    words.get(last + 1).is_some_and(|next| {
        hesitations[last + 1] || next.start - words[last].end >= SETTING_OFF_PAUSE_SECONDS
    })
}

fn sets_off_before(words: &[Word], hesitations: &[bool], first: usize) -> bool {
    first
        .checked_sub(1)
        .is_none_or(|previous| words[previous].trailing != Trailing::None || hesitations[previous])
}

fn english_phrase_at(words: &[Word], hesitations: &[bool], at: usize) -> Option<usize> {
    ENGLISH_FILLER_PHRASES.iter().find_map(|phrase| {
        let last = at + phrase.len() - 1;
        let matches = last < words.len()
            && phrase
                .iter()
                .zip(&words[at..=last])
                .all(|(expected, word)| word.norm == *expected)
            && words[at..last]
                .iter()
                .all(|word| word.trailing == Trailing::None)
            && !hesitations[at..=last].iter().any(|hesitation| *hesitation);
        (matches
            && sets_off_before(words, hesitations, at)
            && sets_off_after(words, hesitations, last))
        .then_some(last)
    })
}

fn chinese_phrase_at(words: &[Word], hesitations: &[bool], at: usize) -> Option<usize> {
    CHINESE_FILLER_PHRASES.iter().find_map(|phrase| {
        let mut joined = String::new();
        for last in at..words.len() {
            if hesitations[last] || (last > at && words[last - 1].trailing != Trailing::None) {
                return None;
            }
            joined.push_str(&words[last].norm);
            if joined == *phrase {
                return sets_off_after(words, hesitations, last).then_some(last);
            }
            if !phrase.starts_with(joined.as_str()) {
                return None;
            }
        }
        None
    })
}

const ENGLISH_MARKERS: [&str; 24] = [
    "the", "and", "a", "to", "of", "i", "you", "it", "is", "that", "in", "we", "this", "so", "for",
    "on", "but", "with", "was", "just", "be", "are", "have", "what",
];

fn is_kana_or_hangul(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}')
}

pub fn filler_language(words: &[CaptionWord]) -> Option<&'static str> {
    let mut total = 0usize;
    let mut han = 0usize;
    let mut markers = 0usize;
    let mut plain_latin = true;
    for word in words {
        let norm = normalize_word(&word.text);
        if norm.is_empty() {
            continue;
        }
        if norm.chars().any(is_kana_or_hangul) {
            return None;
        }
        total += 1;
        if norm.chars().any(is_cjk) {
            han += 1;
        } else if !norm
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '\'' || c == '-')
        {
            plain_latin = false;
        }
        if ENGLISH_MARKERS.contains(&norm.as_str()) {
            markers += 1;
        }
    }
    if total == 0 {
        None
    } else if han * 2 >= total {
        Some("zh")
    } else if han == 0 && plain_latin && markers * 8 >= total {
        Some("en")
    } else {
        None
    }
}

pub fn hesitations(words: &[CaptionWord]) -> Vec<bool> {
    let mut flags = vec![false; words.len()];
    for filler in detect_filler_words(words) {
        if filler.kind == FillerKind::Hesitation {
            flags[filler.words].iter_mut().for_each(|flag| *flag = true);
        }
    }
    flags
}

pub fn caption_display_segment(
    segment: &CaptionTrackSegment,
    hide_hesitations: bool,
) -> Option<Cow<'_, CaptionTrackSegment>> {
    if !hide_hesitations {
        return Some(Cow::Borrowed(segment));
    }
    let word_flags = hesitations(&segment.words);
    let text_tokens: Vec<CaptionWord> = segment
        .text
        .split_whitespace()
        .map(|token| CaptionWord {
            text: token.to_string(),
            ..CaptionWord::default()
        })
        .collect();
    let text_flags = hesitations(&text_tokens);
    if !word_flags.contains(&true) && !text_flags.contains(&true) {
        return Some(Cow::Borrowed(segment));
    }
    let words: Vec<CaptionWord> = segment
        .words
        .iter()
        .zip(&word_flags)
        .filter(|(_, hesitation)| !**hesitation)
        .map(|(word, _)| word.clone())
        .collect();
    let text_follows_words = segment
        .text
        .split_whitespace()
        .eq(caption_text_from_words(&segment.words).split_whitespace());
    let text = if text_follows_words {
        caption_text_from_words(&words)
    } else {
        text_tokens
            .iter()
            .zip(&text_flags)
            .filter(|(_, hesitation)| !**hesitation)
            .map(|(token, _)| token.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    if text.is_empty() {
        return None;
    }
    let mut start = segment.start;
    let mut end = segment.end;
    if let (Some(first), Some(last)) = (words.first(), words.last()) {
        if word_flags.first() == Some(&true) {
            start = start.max(f64::from(first.start));
        }
        if word_flags.last() == Some(&true) {
            end = end.min(f64::from(last.end)).max(start);
        }
    }
    Some(Cow::Owned(CaptionTrackSegment {
        start,
        end,
        text,
        words,
        ..segment.clone()
    }))
}

pub fn detect_filler_words(words: &[CaptionWord]) -> Vec<FillerWord> {
    let words: Vec<Word> = words
        .iter()
        .map(|word| Word {
            norm: normalize_word(&word.text),
            trailing: Trailing::of(&word.text),
            start: word.start,
            end: word.end,
        })
        .collect();

    let mut hesitations: Vec<bool> = words
        .iter()
        .map(|word| is_english_hesitation(&word.norm) || is_chinese_hesitation(&word.norm))
        .collect();
    for at in 0..words.len() {
        if !hesitations[at]
            && is_chinese_particle(&words[at].norm)
            && !matches!(
                words[at].trailing,
                Trailing::Exclamation | Trailing::Question
            )
            && sets_off_before(&words, &hesitations, at)
        {
            hesitations[at] = true;
        }
    }

    let mut found = Vec::new();
    let mut at = 0;
    while at < words.len() {
        if hesitations[at] {
            found.push(FillerWord {
                kind: FillerKind::Hesitation,
                words: at..at + 1,
            });
            at += 1;
            continue;
        }
        match english_phrase_at(&words, &hesitations, at)
            .or_else(|| chinese_phrase_at(&words, &hesitations, at))
        {
            Some(last) => {
                found.push(FillerWord {
                    kind: FillerKind::FillerPhrase,
                    words: at..last + 1,
                });
                at = last + 1;
            }
            None => at += 1,
        }
    }
    found
}

#[cfg(test)]
mod tests;
