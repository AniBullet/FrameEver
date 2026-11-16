use std::borrow::Cow;

use super::*;
use crate::CaptionTrackSegment;

fn timed(texts: &[&str]) -> Vec<CaptionWord> {
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| CaptionWord {
            text: (*text).to_string(),
            start: index as f32 * 0.3,
            end: index as f32 * 0.3 + 0.25,
        })
        .collect()
}

fn found(texts: &[&str]) -> Vec<(FillerKind, Vec<String>)> {
    let words = timed(texts);
    detect_filler_words(&words)
        .into_iter()
        .map(|filler| {
            (
                filler.kind,
                words[filler.words]
                    .iter()
                    .map(|word| word.text.clone())
                    .collect(),
            )
        })
        .collect()
}

fn kinds_and_text(list: &[(FillerKind, &[&str])]) -> Vec<(FillerKind, Vec<String>)> {
    list.iter()
        .map(|(kind, texts)| (*kind, texts.iter().map(|text| text.to_string()).collect()))
        .collect()
}

use FillerKind::{FillerPhrase, Hesitation};

#[test]
fn english_hesitations_are_found_with_their_punctuation() {
    assert_eq!(
        found(&[
            "So,", "um,", "I", "think,", "uh,", "we", "should", "Hmm.", "start"
        ]),
        kinds_and_text(&[
            (Hesitation, &["um,"]),
            (Hesitation, &["uh,"]),
            (Hesitation, &["Hmm."])
        ])
    );
}

#[test]
fn drawn_out_english_hesitations_count_and_backchannels_do_not() {
    assert_eq!(
        found(&["Ummmm", "mm-hmm", "uh-huh", "errrm", "mmmm", "summer"]),
        kinds_and_text(&[
            (Hesitation, &["Ummmm"]),
            (Hesitation, &["errrm"]),
            (Hesitation, &["mmmm"])
        ])
    );
}

#[test]
fn english_filler_phrases_need_to_be_set_off() {
    assert_eq!(
        found(&[
            "It", "was,", "like,", "huge,", "you", "know,", "and", "I", "like", "pizza"
        ]),
        kinds_and_text(&[
            (FillerPhrase, &["like,"]),
            (FillerPhrase, &["you", "know,"])
        ])
    );
    assert!(found(&["you", "know", "how", "it", "goes"]).is_empty());
    assert_eq!(
        found(&["Basically,", "we", "ship", "it"]),
        kinds_and_text(&[(FillerPhrase, &["Basically,"])])
    );
}

#[test]
fn chinese_hesitations_are_found_alone_or_repeated() {
    assert_eq!(
        found(&["嗯，", "我", "觉得", "呃", "我们", "先", "嗯嗯", "开始"]),
        kinds_and_text(&[
            (Hesitation, &["嗯，"]),
            (Hesitation, &["呃"]),
            (Hesitation, &["嗯嗯"])
        ])
    );
}

#[test]
fn chinese_particles_are_hesitations_only_when_they_stand_alone() {
    assert_eq!(
        found(&["啊，", "这个", "功能", "好", "啊。", "我们", "走", "吧"]),
        kinds_and_text(&[(Hesitation, &["啊，"])])
    );
    assert!(found(&["啊！", "太好", "了"]).is_empty());
}

#[test]
fn chinese_filler_phrases_are_found_across_tokens_when_set_off() {
    assert_eq!(
        found(&[
            "那",
            "个，",
            "我们",
            "就是，",
            "嗯",
            "先",
            "做",
            "那个",
            "人",
            "就是",
            "他"
        ]),
        kinds_and_text(&[
            (FillerPhrase, &["那", "个，"]),
            (FillerPhrase, &["就是，"]),
            (Hesitation, &["嗯"])
        ])
    );
}

#[test]
fn a_pause_after_a_chinese_phrase_sets_it_off() {
    let mut words = timed(&["那个", "我们", "开始"]);
    words[1].start += 1.2;
    words[1].end += 1.2;
    words[2].start += 1.2;
    words[2].end += 1.2;
    assert_eq!(
        detect_filler_words(&words),
        vec![FillerWord {
            kind: FillerPhrase,
            words: 0..1
        }]
    );
}

#[test]
fn nothing_is_found_in_empty_or_fluent_speech() {
    assert!(found(&[]).is_empty());
    assert!(found(&["We", "ship", "the", "dashboard", "today."]).is_empty());
    assert!(found(&["我们", "今天", "发布", "这个", "功能。"]).is_empty());
}

#[test]
fn transcript_language_is_english_chinese_or_neither() {
    assert_eq!(
        filler_language(&timed(&[
            "so",
            "I",
            "think",
            "we",
            "should",
            "start",
            "with",
            "the",
            "dashboard"
        ])),
        Some("en")
    );
    assert_eq!(
        filler_language(&timed(&[
            "嗯，",
            "我",
            "觉得",
            "FrameEver",
            "很",
            "好",
            "用。"
        ])),
        Some("zh")
    );
    assert_eq!(
        filler_language(&timed(&[
            "Er", "kommt", "um", "acht", "und", "bleibt", "bis", "zehn"
        ])),
        None
    );
    assert_eq!(filler_language(&timed(&["the", "café"])), None);
    assert_eq!(filler_language(&timed(&["これは", "テスト", "です"])), None);
    assert_eq!(filler_language(&[]), None);
}

fn track(text: &str, words: &[(&str, f32, f32)]) -> CaptionTrackSegment {
    CaptionTrackSegment {
        id: "caption".into(),
        start: f64::from(words.first().map_or(0.0, |w| w.1)),
        end: f64::from(words.last().map_or(1.0, |w| w.2)),
        text: text.into(),
        words: words
            .iter()
            .map(|(text, start, end)| CaptionWord {
                text: (*text).into(),
                start: *start,
                end: *end,
            })
            .collect(),
        fade_duration_override: Some(0.3),
        linger_duration_override: None,
        position_override: None,
        color_override: None,
        background_color_override: None,
        font_size_override: Some(40),
    }
}

#[test]
fn display_hides_english_hesitations_and_trims_edges() {
    let segment = track(
        "Um, so I, uh, think um",
        &[
            ("Um,", 1.0, 1.4),
            ("so", 1.6, 1.8),
            ("I,", 1.8, 1.9),
            ("uh,", 2.0, 2.3),
            ("think", 2.4, 2.7),
            ("um", 2.8, 3.2),
        ],
    );

    let shown = caption_display_segment(&segment, true).unwrap();
    assert_eq!(shown.text, "so I, think");
    assert_eq!(
        shown
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>(),
        ["so", "I,", "think"]
    );
    assert!((shown.start - 1.6).abs() < 1e-6);
    assert!((shown.end - 2.7).abs() < 1e-6);
    assert_eq!(shown.font_size_override, Some(40));
    assert_eq!(shown.fade_duration_override, Some(0.3));

    let verbatim = caption_display_segment(&segment, false).unwrap();
    assert!(matches!(verbatim, Cow::Borrowed(_)));
    assert_eq!(verbatim.text, segment.text);
}

#[test]
fn display_hides_chinese_hesitations_but_keeps_filler_phrases() {
    let segment = track(
        "嗯，那个，我们开始",
        &[
            ("嗯，", 0.0, 0.4),
            ("那", 0.5, 0.6),
            ("个，", 0.6, 0.8),
            ("我们", 0.9, 1.2),
            ("开始", 1.2, 1.5),
        ],
    );
    let shown = caption_display_segment(&segment, true).unwrap();
    assert_eq!(shown.text, "那个，我们开始");
    assert!((shown.start - 0.5).abs() < 1e-6);
}

#[test]
fn display_keeps_plain_captions_borrowed_and_drops_filler_only_ones() {
    let plain = track(
        "so I think",
        &[("so", 1.0, 1.2), ("I", 1.2, 1.3), ("think", 1.3, 1.6)],
    );
    assert!(matches!(
        caption_display_segment(&plain, true),
        Some(Cow::Borrowed(_))
    ));
    let fillers = track("Um, uh.", &[("Um,", 1.0, 1.4), ("uh.", 1.5, 1.9)]);
    assert!(caption_display_segment(&fillers, true).is_none());
    assert!(caption_display_segment(&fillers, false).is_some());
}

#[test]
fn display_filters_edited_text_and_wordless_captions() {
    let edited = track(
        "Um we ship today",
        &[("um", 1.0, 1.3), ("we", 1.4, 1.6), ("ship", 1.6, 2.0)],
    );
    let shown = caption_display_segment(&edited, true).unwrap();
    assert_eq!(shown.text, "we ship today");
    assert_eq!(shown.words.len(), 2);

    let mut wordless = track("hmm okay then", &[]);
    wordless.start = 4.0;
    wordless.end = 6.0;
    let shown = caption_display_segment(&wordless, true).unwrap();
    assert_eq!(shown.text, "okay then");
    assert_eq!((shown.start, shown.end), (4.0, 6.0));
}

use FillerKind::{FalseStart, RepeatedWord};

#[test]
fn repeated_words_mark_every_copy_but_the_last() {
    assert_eq!(
        found(&["I", "I", "think", "we", "we", "we", "should", "go"]),
        kinds_and_text(&[
            (RepeatedWord, &["I"]),
            (RepeatedWord, &["we"]),
            (RepeatedWord, &["we"]),
        ])
    );
}

#[test]
fn repeated_phrases_are_found_through_hesitations() {
    assert_eq!(
        found(&["I", "want", "to,", "um,", "I", "want", "to", "show", "you"]),
        kinds_and_text(&[
            (RepeatedWord, &["I", "want", "to,"]),
            (Hesitation, &["um,"])
        ])
    );
}

#[test]
fn intentional_doubles_and_repeats_across_a_sentence_are_kept() {
    for sentence in [
        &["it", "is", "very", "very", "fast"][..],
        &["she", "had", "had", "enough"],
        &["call", "nine", "one", "one"],
        &["Go.", "Go", "now."],
    ] {
        assert!(
            found(sentence)
                .iter()
                .all(|(kind, _)| *kind != RepeatedWord),
            "{sentence:?}"
        );
    }
}

#[test]
fn cut_off_fragments_and_restarted_clauses_are_false_starts() {
    assert_eq!(
        found(&["we", "need", "the", "wh-", "the", "settings"]),
        kinds_and_text(&[(FalseStart, &["the", "wh-"])])
    );
    assert_eq!(
        found(&["we", "should,", "um,", "we", "need", "to", "restart"]),
        kinds_and_text(&[(FalseStart, &["we", "should,"]), (Hesitation, &["um,"])])
    );
    assert_eq!(
        found(&["I", "think,", "um,", "I", "can", "fix", "it"]),
        kinds_and_text(&[(Hesitation, &["um,"])])
    );
}

#[test]
fn chinese_repeated_words_are_found() {
    assert_eq!(
        found(&["我", "我", "觉得", "可以"]),
        kinds_and_text(&[(RepeatedWord, &["我"])])
    );
}
