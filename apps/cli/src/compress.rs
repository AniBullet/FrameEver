use std::path::{Path, PathBuf};

use clap::Args;
use frameever_export::versions::{self, CompressError};
use serde::Serialize;

use crate::{OutputFormat, resolve_format, write_json};

#[derive(Args)]
pub struct Compress {
    #[arg(required = true)]
    projects: Vec<PathBuf>,
    #[arg(long)]
    keep_only_compressed: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Event {
    Progress {
        project: PathBuf,
        percent: u64,
    },
    Compressed {
        project: PathBuf,
        saved_bytes: u64,
        saved_percent: u64,
        kept_original: bool,
    },
    Skipped {
        project: PathBuf,
        reason: &'static str,
    },
}

impl Compress {
    pub fn run(self, json: bool) -> Result<(), String> {
        let format = resolve_format(json, self.format);
        let mut failed = Vec::new();
        for project in &self.projects {
            match compress_one(project, self.keep_only_compressed, &mut |event| {
                emit(format, &event)
            }) {
                Ok(event) => emit(format, &event)?,
                Err(error) => {
                    eprintln!("{}: {error}", project.display());
                    failed.push(project.display().to_string());
                }
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!("Could not compress {}", failed.join(", ")))
        }
    }
}

fn compress_one(
    project: &Path,
    keep_only_compressed: bool,
    progress: &mut dyn FnMut(Event) -> Result<(), String>,
) -> Result<Event, String> {
    if versions::video_versions(project).is_none() {
        return Err("It is not an Instant recording with a video".to_string());
    }
    let mut shown = None;
    match versions::compress(project, |done, total| {
        let percent = done * 100 / total.max(1);
        if shown != Some(percent / 10) {
            shown = Some(percent / 10);
            return progress(Event::Progress {
                project: project.to_path_buf(),
                percent,
            })
            .is_ok();
        }
        true
    }) {
        Err(CompressError::AlreadyCompressed) => {
            return Ok(Event::Skipped {
                project: project.to_path_buf(),
                reason: "alreadyCompressed",
            });
        }
        Err(CompressError::Failed(error)) => return Err(error),
        Ok(_) => {}
    }
    let compressed = versions::video_versions(project);
    let saved_bytes = compressed.map_or(0, versions::VideoVersions::saved);
    let saved_percent = compressed.and_then(|v| v.saving_percent()).unwrap_or(0);
    if keep_only_compressed {
        versions::keep_only_compressed(project)
            .map_err(|error| format!("Could not remove the original: {error}"))?;
    }
    Ok(Event::Compressed {
        project: project.to_path_buf(),
        saved_bytes,
        saved_percent,
        kept_original: !keep_only_compressed,
    })
}

fn emit(format: OutputFormat, event: &Event) -> Result<(), String> {
    match format {
        OutputFormat::Json => write_json(event),
        OutputFormat::Text => {
            match event {
                Event::Progress { project, percent } => {
                    eprintln!("{}: {percent}%", project.display());
                }
                Event::Compressed {
                    project,
                    saved_percent,
                    kept_original,
                    ..
                } => println!(
                    "{}: compressed, {saved_percent}% smaller{}",
                    project.display(),
                    if *kept_original {
                        ", original kept"
                    } else {
                        ", original removed"
                    }
                ),
                Event::Skipped { project, .. } => {
                    println!("{}: already has a compressed version", project.display());
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_a_recorded_video_is_refused() {
        let empty = tempfile::tempdir().unwrap();
        assert!(compress_one(empty.path(), false, &mut |_| Ok(())).is_err());
    }

    #[test]
    fn a_recording_that_already_has_a_compressed_version_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("content")).unwrap();
        std::fs::write(dir.path().join(versions::OUTPUT), b"original").unwrap();
        std::fs::write(dir.path().join(versions::COMPRESSED), b"small").unwrap();
        assert_eq!(
            compress_one(dir.path(), false, &mut |_| Ok(())),
            Ok(Event::Skipped {
                project: dir.path().to_path_buf(),
                reason: "alreadyCompressed",
            })
        );
    }
}
