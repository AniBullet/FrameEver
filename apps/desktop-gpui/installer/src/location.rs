use std::{
    fs::OpenOptions,
    path::{Component, Path, PathBuf},
};

const FORBIDDEN: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

const RESERVED: &[&str] = &["CON", "PRN", "AUX", "NUL"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Empty,
    Relative,
    Characters,
    ReservedName(String),
    WholeDrive,
    NotWritable,
}

fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_ascii_uppercase();
    let numbered = upper.len() == 4
        && (upper.starts_with("COM") || upper.starts_with("LPT"))
        && matches!(upper.as_bytes()[3], b'1'..=b'9');
    numbered
        || RESERVED.iter().any(|it| it.eq_ignore_ascii_case(stem))
        || name.ends_with(['.', ' '])
}

pub fn parse(text: &str) -> Result<PathBuf, Problem> {
    let text = text.trim().trim_matches('"').trim().replace('/', "\\");
    if text.is_empty() {
        return Err(Problem::Empty);
    }
    let path = Path::new(&text);
    if !path.is_absolute() {
        return Err(Problem::Relative);
    }
    let mut named = false;
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name
                    .chars()
                    .any(|it| it.is_control() || FORBIDDEN.contains(&it))
                {
                    return Err(Problem::Characters);
                }
                if reserved(&name) {
                    return Err(Problem::ReservedName(name.into_owned()));
                }
                named = true;
            }
            Component::ParentDir => return Err(Problem::Relative),
            _ => {}
        }
    }
    if !named {
        return Err(Problem::WholeDrive);
    }
    Ok(path.components().collect())
}

pub fn writable(path: &Path) -> bool {
    if path.is_file() {
        return false;
    }
    let Some(existing) = path.ancestors().find(|it| it.exists()) else {
        return false;
    };
    if !existing.is_dir() {
        return false;
    }
    let probe = existing.join(format!(".frameever-setup-{}.tmp", std::process::id()));
    let created = OpenOptions::new().write(true).create_new(true).open(&probe);
    match created {
        Ok(file) => {
            drop(file);
            std::fs::remove_file(&probe).is_ok()
        }
        Err(_) => false,
    }
}
