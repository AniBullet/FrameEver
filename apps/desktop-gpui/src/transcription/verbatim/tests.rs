use super::*;

const RATE: u32 = 16_000;

fn word(text: &str, start: f32, end: f32) -> CaptionWord {
    CaptionWord {
        text: text.to_string(),
        start,
        end,
    }
}

fn texts(words: &[CaptionWord]) -> Vec<&str> {
    words.iter().map(|word| word.text.as_str()).collect()
}

fn bursts(seconds: f64, spans: &[(f64, f64)], noise: f32) -> Vec<f32> {
    let rate = f64::from(RATE);
    let mut seed = 0x2545_f491_u32;
    (0..(seconds * rate) as usize)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let hiss = (seed as f32 / u32::MAX as f32 - 0.5) * noise;
            let t = i as f64 / rate;
            let voiced = spans.iter().any(|(a, b)| t >= *a && t < *b);
            hiss + if voiced {
                (t * 180.0 * std::f64::consts::TAU).sin() as f32 * 0.4
            } else {
                0.0
            }
        })
        .collect()
}

fn assert_spans(words: &[CaptionWord], expected: &[(f64, f64)]) {
    for (word, (start, end)) in words.iter().zip(expected) {
        assert!((f64::from(word.start) - start).abs() <= 0.006, "{word:?}");
        assert!((f64::from(word.end) - end).abs() <= 0.006, "{word:?}");
    }
}

#[test]
fn subword_tokens_join_into_words_and_keep_repeats() {
    let words = words_from_subword_tokens([
        word(" I", 0.0, 0.16),
        word(" I", 0.16, 0.4),
        word(" th", 0.4, 0.48),
        word("ink", 0.48, 0.64),
        word(",", 0.64, 0.72),
        word(" we", 0.72, 0.8),
        word("'re", 0.8, 0.96),
        word(" twenty", 0.96, 1.2),
        word("-two", 1.2, 1.4),
        word(" ", 1.4, 1.48),
        word("um", 1.48, 1.6),
    ]);
    assert_eq!(
        texts(&words),
        ["I", "I", "think,", "we're", "twenty-two", "um"]
    );
    assert_eq!((words[2].start, words[2].end), (0.4, 0.64));
    assert!((words[1].end - 0.4).abs() < 1e-6);
    assert!(words_from_subword_tokens(Vec::new()).is_empty());
}

#[test]
fn a_subword_word_stops_shortly_after_its_last_token_instead_of_over_the_pause() {
    let words = words_from_subword_tokens([word(" so", 0.5, 2.0), word(" yes", 2.0, 2.3)]);
    assert!((words[0].end - 0.74).abs() < 1e-6, "{:?}", words[0]);
}

#[test]
fn snapping_moves_boundaries_onto_the_pauses_around_an_english_filler() {
    let samples = bursts(4.0, &[(0.5, 1.0), (1.3, 1.8), (2.1, 2.6)], 0.002);
    let loudness = Loudness::new(&samples, RATE);
    let mut words = vec![
        word("so", 0.42, 1.1),
        word("um", 1.1, 1.95),
        word("yes", 1.95, 2.75),
    ];
    snap_to_pauses(&mut words, &loudness);
    assert_spans(&words, &[(0.5, 1.0), (1.3, 1.8), (2.1, 2.6)]);
}

#[test]
fn snapping_moves_boundaries_onto_the_pauses_around_a_chinese_filler() {
    let samples = bursts(3.0, &[(0.3, 0.7), (1.0, 1.4), (1.7, 2.3)], 0.002);
    let loudness = Loudness::new(&samples, RATE);
    let mut words = vec![
        word("我们", 0.25, 0.9),
        word("嗯，", 0.9, 1.6),
        word("开始", 1.6, 2.5),
    ];
    snap_to_pauses(&mut words, &loudness);
    assert_spans(&words, &[(0.3, 0.7), (1.0, 1.4), (1.7, 2.3)]);
}

#[test]
fn snapping_leaves_continuous_speech_and_short_closures_alone() {
    let samples = bursts(3.0, &[(0.5, 1.2), (1.215, 2.0)], 0.002);
    let loudness = Loudness::new(&samples, RATE);
    let mut words = vec![word("one", 0.5, 1.0), word("two", 1.0, 2.0)];
    snap_to_pauses(&mut words, &loudness);
    assert_eq!((words[0].end, words[1].start), (1.0, 1.0));
    assert!((words[0].start - 0.5).abs() <= 0.006);
    assert!((words[1].end - 2.0).abs() <= 0.006);
}

#[test]
fn snapping_handles_silence_empty_input_and_one_word() {
    let silence = vec![0.0; RATE as usize];
    let loudness = Loudness::new(&silence, RATE);
    let mut words = vec![word("hi", 0.2, 0.6)];
    snap_to_pauses(&mut words, &loudness);
    assert!(words[0].start < words[0].end);
    snap_to_pauses(&mut [], &loudness);
    let empty = Loudness::new(&[], RATE);
    let mut words = vec![word("hi", 0.2, 0.6)];
    snap_to_pauses(&mut words, &empty);
    assert_eq!((words[0].start, words[0].end), (0.2, 0.6));
}

#[test]
fn loudness_threshold_follows_a_changing_noise_floor() {
    let mut samples = bursts(20.0, &[(2.0, 3.0), (15.0, 16.0)], 0.001);
    for (i, sample) in samples.iter_mut().enumerate().skip(10 * RATE as usize) {
        *sample += (i as f32 * 0.37).sin() * 0.02;
    }
    let loudness = Loudness::new(&samples, RATE);
    assert_eq!(loudness.silent_runs(3.2, 4.0, 0.03).len(), 1);
    assert_eq!(loudness.silent_runs(16.2, 17.0, 0.03).len(), 1);
    assert!(loudness.silent_runs(2.2, 2.8, 0.03).is_empty());
    assert!(loudness.silent_runs(15.2, 15.8, 0.03).is_empty());
}

#[test]
fn hyphenated_stutters_become_separate_words() {
    let words = split_stutters(vec![
        word("I-I", 1.0, 1.4),
        word("the-the-the,", 2.0, 2.6),
        word("re-record", 3.0, 3.5),
        word("mm-hmm.", 4.0, 4.4),
        word("so", 5.0, 5.2),
        word("we-we-start.", 6.0, 6.6),
        word("我们", 7.0, 7.4),
    ]);
    assert_eq!(
        texts(&words),
        [
            "I",
            "I",
            "the",
            "the",
            "the,",
            "re-record",
            "mm-hmm.",
            "so",
            "we",
            "we",
            "start.",
            "我们"
        ]
    );
    assert!((words[1].start - 1.2).abs() < 1e-6 && (words[1].end - 1.4).abs() < 1e-6);
    assert!((words[4].start - 2.4).abs() < 1e-5);
    assert!(split_stutters(Vec::new()).is_empty());
}

#[test]
fn finished_timings_keep_groups_ordered_and_snapped() {
    let samples = bursts(4.0, &[(0.5, 1.0), (1.3, 1.8), (2.1, 2.6)], 0.002);
    let groups = finish_word_timings(
        vec![
            vec![word("I-I", 0.42, 1.1)],
            vec![word("um", 1.0, 1.95), word("yes", 1.9, 2.75)],
        ],
        &samples,
        RATE,
    );
    assert_eq!(groups.len(), 2);
    assert_eq!(texts(&groups[0]), ["I", "I"]);
    assert_eq!(texts(&groups[1]), ["um", "yes"]);
    let words: Vec<CaptionWord> = groups.into_iter().flatten().collect();
    for pair in words.windows(2) {
        assert!(pair[0].end <= pair[1].start, "{pair:?}");
    }
    assert!((f64::from(words[0].start) - 0.5).abs() <= 0.006);
    assert!((f64::from(words[2].start) - 1.3).abs() <= 0.006);
    assert!((f64::from(words[3].end) - 2.6).abs() <= 0.006);
}
