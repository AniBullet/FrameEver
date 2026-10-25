use std::{path::PathBuf, time::SystemTime};

use frameever_project::{RecordingMeta, RecordingMetaInner};
use frameever_utils::storage::{ProjectKind, Storage, projects_in};
use serde::Serialize;

use crate::{OutputFormat, write_json};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingRow {
    path: PathBuf,
    name: String,
    recording_type: &'static str,
    output_path: PathBuf,
    output_exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sharing_link: Option<String>,
}

fn recording_type(meta: &RecordingMeta) -> &'static str {
    match meta.inner {
        RecordingMetaInner::Studio(_) => "studio",
        RecordingMetaInner::Instant(_) => "instant",
    }
}

fn collect_rows(projects: Vec<PathBuf>) -> Vec<RecordingRow> {
    let mut entries: Vec<(PathBuf, SystemTime, RecordingMeta)> = projects
        .into_iter()
        .filter_map(|path| {
            let meta = RecordingMeta::load_for_project(&path).ok()?;
            let created = path
                .metadata()
                .and_then(|m| m.created())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some((path, created, meta))
        })
        .collect();

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    entries
        .into_iter()
        .map(|(path, _, meta)| {
            let output_path = meta.output_path();
            RecordingRow {
                output_exists: output_path.exists(),
                recording_type: recording_type(&meta),
                name: meta.pretty_name,
                sharing_link: meta.sharing.map(|s| s.link),
                output_path,
                path,
            }
        })
        .collect()
}

pub fn list(dir: Option<PathBuf>, format: OutputFormat) -> Result<(), String> {
    let (dirs, projects) = match dir {
        Some(dir) => {
            let projects = projects_in(&dir);
            (vec![dir], projects)
        }
        None => {
            let storage = Storage::from_environment();
            (
                storage.known_dirs(ProjectKind::Recording),
                storage.projects(ProjectKind::Recording),
            )
        }
    };

    let rows = collect_rows(projects);

    match format {
        OutputFormat::Json => write_json(&rows),
        OutputFormat::Text => {
            if rows.is_empty() {
                let searched = dirs
                    .iter()
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>();
                if searched.is_empty() {
                    println!("No recordings found");
                } else {
                    println!("No recordings found in {}", searched.join(", "));
                }
                return Ok(());
            }
            for row in &rows {
                println!(
                    "{}  [{}]  {}",
                    row.name,
                    row.recording_type,
                    row.path.display()
                );
                if let Some(link) = &row.sharing_link {
                    println!("  link: {link}");
                }
            }
            Ok(())
        }
    }
}
