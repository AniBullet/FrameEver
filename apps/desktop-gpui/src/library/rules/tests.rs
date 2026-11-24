use std::path::PathBuf;

use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

use super::*;
use crate::library::view::{Compression, ExportFile};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
}

fn clock() -> Clock {
    Clock {
        today: today(),
        offset: FixedOffset::east_opt(0).unwrap(),
    }
}

fn at(day: u32, hour: u32, minute: u32) -> f64 {
    Utc.with_ymd_and_hms(2026, 11, day, hour, minute, 0)
        .unwrap()
        .timestamp_millis() as f64
}

fn entry(name: &str, kind: MediaKind, day: u32) -> Entry {
    Entry {
        path: PathBuf::from(format!("{name}-{day}.ever")),
        kind,
        name: name.to_string(),
        size: 0,
        duration: None,
        created_millis: at(day, 12, 0),
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

fn folder(matching: Matching, rules: Vec<Rule>) -> SmartFolder {
    SmartFolder {
        id: "f".into(),
        name: "F".into(),
        matching,
        rules,
        extra: Default::default(),
    }
}

fn title(op: TextOp, value: &str) -> Rule {
    Rule::Title {
        op,
        value: value.into(),
    }
}

fn holds(folder: &SmartFolder, entry: &Entry) -> bool {
    folder
        .compile(&clock())
        .matches(&Candidate::new(entry), &clock())
}

#[test]
fn title_rules_ignore_case() {
    let f = folder(Matching::All, vec![title(TextOp::Contains, "STANDUP")]);
    assert!(holds(&f, &entry("Weekly standup", MediaKind::Studio, 20)));
    assert!(!holds(&f, &entry("Weekly sync", MediaKind::Studio, 20)));
}

#[test]
fn every_title_operation_reads_like_the_design() {
    let demo = entry("Product demo", MediaKind::Studio, 20);
    assert!(holds(
        &folder(Matching::All, vec![title(TextOp::Is, " product DEMO ")]),
        &demo
    ));
    assert!(holds(
        &folder(Matching::All, vec![title(TextOp::StartsWith, "prod")]),
        &demo
    ));
    assert!(!holds(
        &folder(Matching::All, vec![title(TextOp::StartsWith, "demo")]),
        &demo
    ));
    assert!(holds(
        &folder(Matching::All, vec![title(TextOp::DoesNotContain, "draft")]),
        &demo
    ));
    assert!(!holds(
        &folder(Matching::All, vec![title(TextOp::DoesNotContain, "demo")]),
        &demo
    ));
}

#[test]
fn all_and_any() {
    let rules = vec![
        title(TextOp::Contains, "standup"),
        Rule::Kind {
            op: IsOp::Is,
            value: KindValue::Instant,
        },
    ];
    let studio_standup = entry("Standup", MediaKind::Studio, 20);
    assert!(!holds(
        &folder(Matching::All, rules.clone()),
        &studio_standup
    ));
    assert!(holds(&folder(Matching::Any, rules), &studio_standup));
}

#[test]
fn empty_and_unfinished_rules_match_nothing_instead_of_everything() {
    let item = entry("Anything", MediaKind::Studio, 20);
    assert!(!holds(&folder(Matching::All, vec![]), &item));
    let unfinished = folder(Matching::All, vec![title(TextOp::Contains, "   ")]);
    assert!(unfinished.compile(&clock()).is_empty());
    assert!(!holds(&unfinished, &item));
    let mixed = folder(
        Matching::All,
        vec![title(TextOp::Contains, ""), title(TextOp::Contains, "any")],
    );
    assert!(holds(&mixed, &item));
}

#[test]
fn audio_only_is_its_own_type_and_not_studio() {
    let podcast = Entry {
        audio_only: true,
        ..entry("Podcast", MediaKind::Studio, 20)
    };
    let is = |value| {
        folder(
            Matching::All,
            vec![Rule::Kind {
                op: IsOp::Is,
                value,
            }],
        )
    };
    assert!(holds(&is(KindValue::Audio), &podcast));
    assert!(!holds(&is(KindValue::Studio), &podcast));
    assert!(holds(
        &is(KindValue::Studio),
        &entry("Demo", MediaKind::Studio, 20)
    ));
    assert!(holds(
        &is(KindValue::Screenshot),
        &entry("Shot", MediaKind::Screenshot, 20)
    ));
    let not_audio = folder(
        Matching::All,
        vec![Rule::Kind {
            op: IsOp::IsNot,
            value: KindValue::Audio,
        }],
    );
    assert!(!holds(&not_audio, &podcast));
}

#[test]
fn export_rules_are_the_status_filters() {
    let exported = Entry {
        exports: vec![ExportFile {
            path: PathBuf::from("a.mp4"),
            name: "a.mp4".into(),
            exported_millis: 1.,
            size: Some(1),
            video: true,
        }],
        ..entry("Exported", MediaKind::Studio, 20)
    };
    let fresh = entry("Fresh", MediaKind::Studio, 20);
    let shot = entry("Shot", MediaKind::Screenshot, 20);
    let unexported = folder(
        Matching::All,
        vec![Rule::Export {
            op: IsOp::Is,
            value: StatusFilter::NotExported,
        }],
    );
    assert!(holds(&unexported, &fresh));
    assert!(!holds(&unexported, &exported));
    assert!(!holds(&unexported, &shot));
    let not_exported = folder(
        Matching::All,
        vec![Rule::Export {
            op: IsOp::IsNot,
            value: StatusFilter::Exported,
        }],
    );
    assert!(holds(&not_exported, &fresh));
    assert!(!holds(&not_exported, &exported));
}

#[test]
fn dates_use_the_local_day() {
    let today_rule = folder(
        Matching::All,
        vec![Rule::Date {
            op: DateOp::InTheLast,
            days: 1,
        }],
    );
    let late = Entry {
        created_millis: at(19, 23, 30),
        ..entry("late", MediaKind::Studio, 19)
    };
    assert!(!holds(&today_rule, &late));
    let plus_one = Clock {
        today: today(),
        offset: FixedOffset::east_opt(3600).unwrap(),
    };
    assert!(
        today_rule
            .compile(&plus_one)
            .matches(&Candidate::new(&late), &plus_one)
    );

    let last7 = folder(
        Matching::All,
        vec![Rule::Date {
            op: DateOp::InTheLast,
            days: 7,
        }],
    );
    assert!(holds(&last7, &entry("a", MediaKind::Studio, 14)));
    assert!(!holds(&last7, &entry("a", MediaKind::Studio, 13)));
    let older = folder(
        Matching::All,
        vec![Rule::Date {
            op: DateOp::OlderThan,
            days: 7,
        }],
    );
    assert!(!holds(&older, &entry("a", MediaKind::Studio, 14)));
    assert!(holds(&older, &entry("a", MediaKind::Studio, 13)));
}

#[test]
fn a_day_change_moves_an_item_out_of_in_the_last() {
    let today_rule = folder(
        Matching::All,
        vec![Rule::Date {
            op: DateOp::InTheLast,
            days: 1,
        }],
    );
    let item = entry("a", MediaKind::Studio, 20);
    assert!(holds(&today_rule, &item));
    let tomorrow = Clock {
        today: NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
        ..clock()
    };
    assert!(
        !today_rule
            .compile(&tomorrow)
            .matches(&Candidate::new(&item), &tomorrow)
    );
}

#[test]
fn unknown_rules_never_match_and_round_trip() {
    let json = r#"{"id":"x","name":"X","matching":"any","colour":"teal","rules":[
        {"field":"sentiment","is":"happy"},
        {"field":"title","op":"contains","value":"sync"}]}"#;
    let parsed: SmartFolder = serde_json::from_str(json).unwrap();
    assert!(matches!(parsed.rules[0], Rule::Unknown(_)));
    assert!(holds(&parsed, &entry("Sync", MediaKind::Studio, 20)));
    assert!(!holds(&parsed, &entry("Other", MediaKind::Studio, 20)));
    let again: serde_json::Value = serde_json::to_value(&parsed).unwrap();
    assert_eq!(again["rules"][0]["is"], "happy");
    assert_eq!(again["rules"][1]["value"], "sync");
    assert_eq!(again["colour"], "teal");
}

#[test]
fn a_known_rule_with_a_value_this_build_lacks_is_kept_as_unknown() {
    let json = r#"{"field":"kind","op":"is","value":"meeting"}"#;
    let rule: Rule = serde_json::from_str(json).unwrap();
    assert!(matches!(rule, Rule::Unknown(_)));
    assert_eq!(serde_json::to_string(&rule).unwrap(), json);
}

#[test]
fn every_starting_rule_survives_a_round_trip() {
    for field in [Field::Kind, Field::Date, Field::Export, Field::Title] {
        let rule = Rule::starting(field);
        let again: Rule = serde_json::from_value(serde_json::to_value(&rule).unwrap()).unwrap();
        assert_eq!(again, rule);
        assert_eq!(again.field(), Some(field));
    }
}

#[test]
fn ten_thousand_items_and_twenty_folders_evaluate_in_a_few_milliseconds() {
    let entries: Vec<Entry> = (0..10_000)
        .map(|i| {
            entry(
                &format!("Recording {i} about roadmap {}", i % 37),
                if i % 3 == 0 {
                    MediaKind::Instant
                } else {
                    MediaKind::Studio
                },
                1 + (i % 20) as u32,
            )
        })
        .collect();
    let folders: Vec<SmartFolder> = (0..20)
        .map(|i| SmartFolder {
            id: format!("f{i}"),
            name: format!("F{i}"),
            matching: if i % 2 == 0 {
                Matching::All
            } else {
                Matching::Any
            },
            rules: vec![
                title(TextOp::Contains, &format!("roadmap {i}")),
                Rule::Kind {
                    op: IsOp::Is,
                    value: KindValue::Instant,
                },
                Rule::Date {
                    op: DateOp::InTheLast,
                    days: 7,
                },
            ],
            extra: Default::default(),
        })
        .collect();
    let warm = evaluate_all(&folders, &entries, &clock());
    assert_eq!(warm.len(), 20);
    let best = (0..5)
        .map(|_| {
            let start = std::time::Instant::now();
            let out = evaluate_all(&folders, &entries, &clock());
            assert_eq!(out.len(), 20);
            start.elapsed()
        })
        .min()
        .unwrap();
    assert!(
        best < std::time::Duration::from_millis(if cfg!(debug_assertions) { 400 } else { 40 }),
        "{best:?}"
    );
}
