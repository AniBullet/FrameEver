use std::cmp::Ordering;

use semver::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Existing {
    Unknown,
    Older(String),
    Same,
    Newer(String),
}

pub fn compare(this: &str, installed: Option<&str>) -> Existing {
    let Some(installed) = installed.map(str::trim).filter(|value| !value.is_empty()) else {
        return Existing::Unknown;
    };
    let (Ok(this), Ok(parsed)) = (Version::parse(this.trim()), Version::parse(installed)) else {
        return Existing::Unknown;
    };
    match parsed.cmp(&this) {
        Ordering::Less => Existing::Older(installed.to_string()),
        Ordering::Equal => Existing::Same,
        Ordering::Greater => Existing::Newer(installed.to_string()),
    }
}
