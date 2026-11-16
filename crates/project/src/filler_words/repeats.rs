use std::ops::Range;

use super::{FillerKind, normalize_word};
use crate::CaptionWord;

const SENTENCE_PAUSE: f64 = 1.0;
const MAX_REPEATED_PHRASE: usize = 4;
const MAX_FALSE_START: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trailing {
    None,
    Comma,
    Terminal,
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
            Some('.' | '!' | '?' | '。' | '！' | '？') => Self::Terminal,
            Some(',' | ';' | ':' | '，' | '、' | '；' | '：') => Self::Comma,
            Some('-' | '—' | '–') => Self::Break,
            _ => Self::None,
        }
    }

    fn merge(self, other: Self) -> Self {
        if other == Self::None { self } else { other }
    }

    fn ends_sentence(self) -> bool {
        self == Self::Terminal
    }

    fn sets_off(self) -> bool {
        self != Self::None
    }
}

struct Word {
    index: usize,
    norm: String,
    trailing: Trailing,
    cut_off: bool,
    start: f64,
    end: f64,
    hesitation: bool,
}

fn words_of(words: &[CaptionWord], hesitations: &[bool]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for (index, word) in words.iter().enumerate() {
        let norm = normalize_word(&word.text);
        let trailing = Trailing::of(&word.text);
        if norm.is_empty() {
            if let Some(previous) = out.last_mut() {
                previous.trailing = previous.trailing.merge(trailing);
            }
            continue;
        }
        let trimmed = word.text.trim().trim_end_matches(['"', '\'', '”', '’']);
        let cut_off = (trimmed.ends_with('-') || trimmed.ends_with('—') || trimmed.ends_with('–'))
            && !trimmed.ends_with("--");
        out.push(Word {
            index,
            norm,
            trailing,
            cut_off,
            start: f64::from(word.start),
            end: f64::from(word.end),
            hesitation: hesitations.get(index).copied().unwrap_or(false),
        });
    }
    out
}

fn is_number_word(norm: &str) -> bool {
    norm.chars().all(|c| c.is_ascii_digit() || c == '\'')
        || matches!(
            norm,
            "zero"
                | "oh"
                | "one"
                | "two"
                | "three"
                | "four"
                | "five"
                | "six"
                | "seven"
                | "eight"
                | "nine"
                | "ten"
                | "hundred"
                | "thousand"
                | "million"
                | "billion"
        )
}

fn is_intentional_double(norm: &str) -> bool {
    is_number_word(norm)
        || matches!(
            norm,
            "very"
                | "really"
                | "no"
                | "yes"
                | "yeah"
                | "bye"
                | "ha"
                | "haha"
                | "ho"
                | "la"
                | "knock"
                | "many"
                | "much"
                | "more"
                | "far"
                | "long"
                | "had"
                | "that"
                | "is"
                | "do"
                | "well"
                | "go"
                | "chop"
                | "tick"
                | "night"
                | "boo"
                | "hush"
                | "bang"
                | "beep"
                | "blah"
                | "bla"
                | "yada"
                | "wow"
                | "hey"
                | "hi"
                | "ok"
                | "okay"
                | "please"
                | "quick"
                | "come"
                | "now"
                | "pretty"
                | "super"
                | "谢谢"
                | "好"
                | "对"
                | "是"
                | "哈"
                | "哈哈"
                | "慢慢"
                | "常常"
                | "刚刚"
        )
}

fn is_clause_opener(norm: &str) -> bool {
    matches!(
        norm,
        "i" | "i'm"
            | "i've"
            | "i'll"
            | "i'd"
            | "we"
            | "we're"
            | "we've"
            | "we'll"
            | "you"
            | "you're"
            | "you'll"
            | "he"
            | "she"
            | "it"
            | "it's"
            | "they"
            | "they're"
            | "this"
            | "that"
            | "these"
            | "those"
            | "there"
            | "there's"
            | "the"
            | "a"
            | "an"
            | "my"
            | "our"
            | "your"
            | "if"
            | "when"
            | "what"
            | "how"
            | "why"
            | "so"
            | "and"
            | "but"
            | "to"
            | "in"
            | "on"
            | "at"
            | "for"
            | "with"
            | "let's"
            | "let"
            | "can"
            | "is"
            | "was"
            | "do"
            | "does"
            | "did"
            | "我"
            | "我们"
            | "你"
            | "你们"
            | "他"
            | "她"
            | "它"
            | "他们"
            | "这"
            | "那"
            | "这个"
            | "如果"
    )
}

fn takes_clause_complement(norm: &str, clause_len: usize) -> bool {
    if matches!(norm, "is" | "was") {
        return clause_len >= 3;
    }
    matches!(
        norm,
        "think"
            | "thought"
            | "guess"
            | "know"
            | "knew"
            | "mean"
            | "meant"
            | "believe"
            | "feel"
            | "felt"
            | "said"
            | "say"
            | "says"
            | "hope"
            | "bet"
            | "suppose"
            | "remember"
            | "realized"
            | "realised"
            | "found"
            | "noticed"
            | "see"
            | "saw"
            | "wish"
            | "heard"
            | "decided"
            | "figured"
            | "wonder"
            | "promise"
            | "assume"
            | "understand"
            | "imagine"
            | "觉得"
            | "认为"
            | "知道"
            | "说"
    )
}

fn is_opening_marker(norm: &str) -> bool {
    matches!(
        norm,
        "so" | "okay" | "ok" | "alright" | "right" | "well" | "basically" | "actually" | "like"
    )
}

fn so_is_degree_word(next: &Word) -> bool {
    matches!(
        next.norm.as_str(),
        "much"
            | "many"
            | "far"
            | "long"
            | "few"
            | "little"
            | "what"
            | "to"
            | "called"
            | "good"
            | "bad"
            | "cool"
            | "fast"
            | "slow"
            | "easy"
            | "hard"
            | "big"
            | "small"
            | "close"
            | "sorry"
            | "glad"
            | "happy"
            | "do"
            | "did"
            | "does"
    )
}

fn longest_silence(words: &[Word], from: usize, to: usize) -> f64 {
    (from + 1..=to.min(words.len().saturating_sub(1)))
        .map(|at| words[at].start - words[at - 1].end)
        .fold(0.0, f64::max)
}

fn gap_before(words: &[Word], at: usize) -> f64 {
    match at.checked_sub(1).and_then(|previous| words.get(previous)) {
        Some(previous) => words[at].start - previous.end,
        None => f64::INFINITY,
    }
}

fn starts_sentence(words: &[Word], at: usize) -> bool {
    let mut cursor = at;
    loop {
        let Some(previous) = cursor.checked_sub(1) else {
            return true;
        };
        if words[previous].trailing.ends_sentence() || gap_before(words, cursor) >= SENTENCE_PAUSE {
            return true;
        }
        if !words[previous].hesitation && !is_opening_marker(&words[previous].norm) {
            return false;
        }
        cursor = previous;
    }
}

fn set_off_before(words: &[Word], at: usize) -> bool {
    if starts_sentence(words, at) {
        return true;
    }
    let previous = &words[at - 1];
    previous.trailing.sets_off() || previous.hesitation || previous.cut_off
}

fn repetitions(words: &[Word], out: &mut Vec<(FillerKind, usize, usize)>) {
    let spoken: Vec<usize> = (0..words.len())
        .filter(|&at| !words[at].hesitation && !words[at].cut_off)
        .collect();
    let mut cursor = 0;
    while cursor < spoken.len() {
        let mut matched = None;
        for size in (1..=MAX_REPEATED_PHRASE).rev() {
            if cursor + size * 2 > spoken.len() {
                continue;
            }
            let first = &spoken[cursor..cursor + size];
            let second = &spoken[cursor + size..cursor + size * 2];
            if !first
                .iter()
                .zip(second)
                .all(|(a, b)| words[*a].norm == words[*b].norm)
            {
                continue;
            }
            if first
                .iter()
                .chain(&second[..size - 1])
                .any(|&at| words[at].trailing.ends_sentence())
            {
                continue;
            }
            let last_of_first = first[size - 1];
            if size == 1 && is_intentional_double(&words[last_of_first].norm)
                || size > 1
                    && first
                        .iter()
                        .all(|&at| is_intentional_double(&words[at].norm))
            {
                continue;
            }
            if longest_silence(words, first[0], second[size - 1]) >= SENTENCE_PAUSE {
                continue;
            }
            let coordinated = |at: &usize| matches!(words[*at].norm.as_str(), "and" | "or" | "nor");
            let chained = cursor
                .checked_sub(1)
                .is_some_and(|previous| words[spoken[previous]].norm == words[last_of_first].norm);
            if size > 1 && (coordinated(&last_of_first) || first.iter().any(coordinated) && chained)
            {
                continue;
            }
            if size == 1
                && words[last_of_first].norm == "so"
                && spoken
                    .get(cursor + 2)
                    .is_some_and(|&next| so_is_degree_word(&words[next]))
            {
                continue;
            }
            matched = Some(size);
            break;
        }
        match matched {
            Some(size) => {
                let first = spoken[cursor];
                let last = spoken[cursor + size - 1];
                let fragment = words[last + 1..spoken[cursor + size]]
                    .iter()
                    .any(|word| word.cut_off);
                let kind = if fragment {
                    FillerKind::FalseStart
                } else {
                    FillerKind::RepeatedWord
                };
                out.push((kind, first, last + 1));
                cursor += size;
            }
            None => cursor += 1,
        }
    }
}

fn false_starts(words: &[Word], out: &mut Vec<(FillerKind, usize, usize)>) {
    for (at, word) in words.iter().enumerate() {
        if word.cut_off && !word.hesitation {
            out.push((FillerKind::FalseStart, at, at + 1));
        }
    }
    for start in 0..words.len() {
        let opener = &words[start];
        if opener.hesitation || opener.cut_off || !is_clause_opener(&opener.norm) {
            continue;
        }
        let at_boundary = set_off_before(words, start)
            || start.checked_sub(1).is_some_and(|previous| {
                matches!(
                    words[previous].norm.as_str(),
                    "and" | "but" | "so" | "then" | "because" | "or" | "if" | "when"
                )
            });
        if !at_boundary {
            continue;
        }
        for size in 2..=MAX_FALSE_START {
            let last = start + size - 1;
            let Some(abandoned) = words.get(last) else {
                break;
            };
            if words[start..=last]
                .iter()
                .any(|word| word.hesitation || word.cut_off)
                || words[start..last]
                    .iter()
                    .any(|word| word.trailing.sets_off())
                || abandoned.trailing.ends_sentence()
            {
                break;
            }
            let mut restart = last + 1;
            let mut interrupted = abandoned.trailing == Trailing::Break;
            while words
                .get(restart)
                .is_some_and(|next| next.hesitation || next.cut_off)
            {
                interrupted = true;
                restart += 1;
            }
            let Some(restarted) = words.get(restart) else {
                break;
            };
            if !interrupted
                || restarted.norm != opener.norm
                || takes_clause_complement(&abandoned.norm, size)
                || longest_silence(words, last, restart) >= SENTENCE_PAUSE
            {
                continue;
            }
            let restated = words[restart..]
                .iter()
                .take(size)
                .map(|word| word.norm.as_str())
                .eq(words[start..=last].iter().map(|word| word.norm.as_str()));
            if !restated {
                out.push((FillerKind::FalseStart, start, last + 1));
            }
            break;
        }
    }
}

pub(super) fn repeats_and_false_starts(
    words: &[CaptionWord],
    hesitations: &[bool],
) -> Vec<(FillerKind, Range<usize>)> {
    let words = words_of(words, hesitations);
    let mut found = Vec::new();
    repetitions(&words, &mut found);
    false_starts(&words, &mut found);
    found
        .into_iter()
        .map(|(kind, start, end)| (kind, words[start].index..words[end - 1].index + 1))
        .collect()
}
