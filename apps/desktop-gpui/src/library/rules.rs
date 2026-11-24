use std::collections::HashMap;

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{
    MediaKind,
    view::{Clock, Entry, StatusFilter},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Matching {
    #[default]
    All,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IsOp {
    Is,
    IsNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KindValue {
    Studio,
    Instant,
    Screenshot,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateOp {
    InTheLast,
    OlderThan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextOp {
    Contains,
    DoesNotContain,
    Is,
    StartsWith,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Kind,
    Date,
    Export,
    Title,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    Kind { op: IsOp, value: KindValue },
    Date { op: DateOp, days: u32 },
    Export { op: IsOp, value: StatusFilter },
    Title { op: TextOp, value: String },
    Unknown(Value),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "field", rename_all = "camelCase")]
enum KnownRule {
    Kind { op: IsOp, value: KindValue },
    Date { op: DateOp, days: u32 },
    Export { op: IsOp, value: StatusFilter },
    Title { op: TextOp, value: String },
}

impl Serialize for Rule {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let known = match self.clone() {
            Rule::Kind { op, value } => KnownRule::Kind { op, value },
            Rule::Date { op, days } => KnownRule::Date { op, days },
            Rule::Export { op, value } => KnownRule::Export { op, value },
            Rule::Title { op, value } => KnownRule::Title { op, value },
            Rule::Unknown(value) => return value.serialize(serializer),
        };
        known.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(match KnownRule::deserialize(&value) {
            Ok(KnownRule::Kind { op, value }) => Rule::Kind { op, value },
            Ok(KnownRule::Date { op, days }) => Rule::Date { op, days },
            Ok(KnownRule::Export { op, value }) => Rule::Export { op, value },
            Ok(KnownRule::Title { op, value }) => Rule::Title { op, value },
            Err(_) => Rule::Unknown(value),
        })
    }
}

pub const DAY_CHOICES: [u32; 4] = [1, 7, 30, 365];

impl Rule {
    pub fn starting(field: Field) -> Self {
        match field {
            Field::Kind => Rule::Kind {
                op: IsOp::Is,
                value: KindValue::Studio,
            },
            Field::Date => Rule::Date {
                op: DateOp::InTheLast,
                days: 7,
            },
            Field::Export => Rule::Export {
                op: IsOp::Is,
                value: StatusFilter::NotExported,
            },
            Field::Title => Rule::Title {
                op: TextOp::Contains,
                value: String::new(),
            },
        }
    }

    pub fn field(&self) -> Option<Field> {
        match self {
            Rule::Kind { .. } => Some(Field::Kind),
            Rule::Date { .. } => Some(Field::Date),
            Rule::Export { .. } => Some(Field::Export),
            Rule::Title { .. } => Some(Field::Title),
            Rule::Unknown(_) => None,
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            Rule::Title { value, .. } => !value.trim().is_empty(),
            _ => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub matching: Matching,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

pub struct Compiled {
    matching: Matching,
    rules: Vec<CompiledRule>,
}

enum CompiledRule {
    Kind {
        op: IsOp,
        value: KindValue,
    },
    Since {
        from: Option<NaiveDate>,
        inside: bool,
    },
    Export {
        op: IsOp,
        value: StatusFilter,
    },
    Title {
        op: TextOp,
        needle: String,
    },
    Never,
}

pub struct Candidate<'a> {
    entry: &'a Entry,
    title: String,
}

impl<'a> Candidate<'a> {
    pub fn new(entry: &'a Entry) -> Self {
        Self {
            entry,
            title: entry.name.trim().to_lowercase(),
        }
    }
}

impl SmartFolder {
    pub fn compile(&self, clock: &Clock) -> Compiled {
        Compiled {
            matching: self.matching,
            rules: self
                .rules
                .iter()
                .filter(|rule| rule.is_complete())
                .map(|rule| compile_rule(rule, clock.today))
                .collect(),
        }
    }
}

fn compile_rule(rule: &Rule, today: NaiveDate) -> CompiledRule {
    match rule {
        Rule::Kind { op, value } => CompiledRule::Kind {
            op: *op,
            value: *value,
        },
        Rule::Date { op, days } => CompiledRule::Since {
            from: today.checked_sub_days(Days::new(u64::from(days.saturating_sub(1)))),
            inside: *op == DateOp::InTheLast,
        },
        Rule::Export { op, value } => CompiledRule::Export {
            op: *op,
            value: *value,
        },
        Rule::Title { op, value } => CompiledRule::Title {
            op: *op,
            needle: value.trim().to_lowercase(),
        },
        Rule::Unknown(_) => CompiledRule::Never,
    }
}

fn kind_of(entry: &Entry) -> KindValue {
    match entry.kind {
        _ if entry.audio_only => KindValue::Audio,
        MediaKind::Studio => KindValue::Studio,
        MediaKind::Instant => KindValue::Instant,
        MediaKind::Screenshot => KindValue::Screenshot,
    }
}

fn rule_matches(rule: &CompiledRule, candidate: &Candidate<'_>, clock: &Clock) -> bool {
    let entry = candidate.entry;
    match rule {
        CompiledRule::Kind { op, value } => (kind_of(entry) == *value) == (*op == IsOp::Is),
        CompiledRule::Since { from, inside } => {
            let within = from.is_none_or(|from| clock.date_of(entry.created_millis) >= from);
            within == *inside
        }
        CompiledRule::Export { op, value } => value.matches(entry) == (*op == IsOp::Is),
        CompiledRule::Title { op, needle } => {
            let title = candidate.title.as_str();
            match op {
                TextOp::Contains => title.contains(needle.as_str()),
                TextOp::DoesNotContain => !title.contains(needle.as_str()),
                TextOp::Is => title == needle,
                TextOp::StartsWith => title.starts_with(needle.as_str()),
            }
        }
        CompiledRule::Never => false,
    }
}

impl Compiled {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn matches(&self, candidate: &Candidate<'_>, clock: &Clock) -> bool {
        if self.rules.is_empty() {
            return false;
        }
        let test = |rule: &CompiledRule| rule_matches(rule, candidate, clock);
        match self.matching {
            Matching::All => self.rules.iter().all(test),
            Matching::Any => self.rules.iter().any(test),
        }
    }
}

pub fn evaluate_all(
    folders: &[SmartFolder],
    entries: &[Entry],
    clock: &Clock,
) -> HashMap<String, Vec<usize>> {
    let compiled: Vec<(&str, Compiled)> = folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder.compile(clock)))
        .collect();
    let mut members: HashMap<String, Vec<usize>> = folders
        .iter()
        .map(|folder| (folder.id.clone(), Vec::new()))
        .collect();
    for (index, entry) in entries.iter().enumerate() {
        let candidate = Candidate::new(entry);
        for (id, rules) in &compiled {
            if rules.matches(&candidate, clock)
                && let Some(held) = members.get_mut(*id)
            {
                held.push(index);
            }
        }
    }
    members
}

#[cfg(test)]
mod tests;
