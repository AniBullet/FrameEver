use std::path::{Path, PathBuf};

const OUTPUT: &str = "content/output.mp4";
const COMPRESSED: &str = "content/output_compressed.mp4";
const ORIGINAL: &str = "content/output_original.mp4";
const COMPRESSING: &str = "content/output_compressing.mp4";
const ORIGINAL_DELETED: &str = "content/original_deleted";

static ONE_COMPRESSION_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoVersions {
    Single { size: u64 },
    CompressedCopy { original: u64, compressed: u64 },
    UsingCompressed { original: u64, compressed: u64 },
}

impl VideoVersions {
    fn sizes(self) -> Option<(u64, u64)> {
        match self {
            Self::Single { .. } => None,
            Self::CompressedCopy {
                original,
                compressed,
            }
            | Self::UsingCompressed {
                original,
                compressed,
            } => Some((original, compressed)),
        }
    }

    pub fn saving_percent(self) -> Option<u64> {
        let (original, compressed) = self.sizes()?;
        (original > 0).then(|| original.saturating_sub(compressed) * 100 / original)
    }

    pub fn saved(self) -> u64 {
        self.sizes().map_or(0, |(original, compressed)| {
            original.saturating_sub(compressed)
        })
    }
}

fn size_of(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
}

pub fn video_versions(bundle: &Path) -> Option<VideoVersions> {
    let output = size_of(&bundle.join(OUTPUT))?;
    if let Some(original) = size_of(&bundle.join(ORIGINAL)) {
        return Some(VideoVersions::UsingCompressed {
            original,
            compressed: output,
        });
    }
    if let Some(compressed) = size_of(&bundle.join(COMPRESSED)) {
        return Some(VideoVersions::CompressedCopy {
            original: output,
            compressed,
        });
    }
    Some(VideoVersions::Single { size: output })
}

pub fn compress(bundle: &Path, progress: impl FnMut(u64, u64) -> bool) -> Result<PathBuf, String> {
    let _turn = ONE_COMPRESSION_AT_A_TIME
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if !matches!(video_versions(bundle), Some(VideoVersions::Single { .. })) {
        return Err(crate::i18n::t!("This recording already has a compressed version").to_string());
    }
    let staging = bundle.join(COMPRESSING);
    let target = bundle.join(COMPRESSED);
    let result =
        frameever_export::compress::compress_to_hevc(&bundle.join(OUTPUT), &staging, progress)
            .and_then(|()| std::fs::rename(&staging, &target).map_err(|error| error.to_string()));
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result.map(|()| target)
}

fn swap(bundle: &Path, current_to: &str, next_from: &str) -> std::io::Result<()> {
    super::player::release(bundle);
    let output = bundle.join(OUTPUT);
    let parked = bundle.join(current_to);
    std::fs::rename(&output, &parked)?;
    if let Err(error) = std::fs::rename(bundle.join(next_from), &output) {
        let _ = std::fs::rename(&parked, &output);
        return Err(error);
    }
    Ok(())
}

pub fn use_compressed(bundle: &Path) -> std::io::Result<()> {
    swap(bundle, ORIGINAL, COMPRESSED)
}

pub fn restore_original(bundle: &Path) -> std::io::Result<()> {
    swap(bundle, COMPRESSED, ORIGINAL)
}

pub fn delete_compressed(bundle: &Path) -> std::io::Result<()> {
    super::player::release(bundle);
    if matches!(
        video_versions(bundle),
        Some(VideoVersions::UsingCompressed { .. })
    ) {
        restore_original(bundle)?;
    }
    std::fs::remove_file(bundle.join(COMPRESSED))
}

pub fn delete_original(bundle: &Path) -> std::io::Result<()> {
    super::player::release(bundle);
    if matches!(
        video_versions(bundle),
        Some(VideoVersions::CompressedCopy { .. })
    ) {
        use_compressed(bundle)?;
    }
    std::fs::remove_file(bundle.join(ORIGINAL))?;
    std::fs::write(bundle.join(ORIGINAL_DELETED), b"")
}

pub fn compressed_only(bundle: &Path) -> bool {
    bundle.join(ORIGINAL_DELETED).is_file()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    Compressed,
    Original,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedVersion {
    bundle: PathBuf,
    version: Version,
    was_using_compressed: bool,
}

impl DeletedVersion {
    pub fn file(&self) -> PathBuf {
        version_file(self.version)
            .split('/')
            .fold(self.bundle.clone(), |path, part| path.join(part))
    }
}

fn version_file(version: Version) -> &'static str {
    match version {
        Version::Compressed => COMPRESSED,
        Version::Original => ORIGINAL,
    }
}

fn using_compressed(bundle: &Path) -> bool {
    matches!(
        video_versions(bundle),
        Some(VideoVersions::UsingCompressed { .. })
    )
}

fn settle_on(bundle: &Path, compressed: bool) -> std::io::Result<()> {
    match (compressed, using_compressed(bundle)) {
        (true, false) => use_compressed(bundle),
        (false, true) => restore_original(bundle),
        _ => Ok(()),
    }
}

pub fn delete_version_with(
    bundle: &Path,
    version: Version,
    delete: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<DeletedVersion, String> {
    super::player::release(bundle);
    let was_using_compressed = using_compressed(bundle);
    settle_on(bundle, version == Version::Original).map_err(|error| error.to_string())?;
    if let Err(error) = delete(&bundle.join(version_file(version))) {
        if let Err(error) = settle_on(bundle, was_using_compressed) {
            tracing::warn!(path = %bundle.display(), "putting the video version back failed: {error}");
        }
        return Err(error);
    }
    if version == Version::Original {
        std::fs::write(bundle.join(ORIGINAL_DELETED), b"").map_err(|error| error.to_string())?;
    }
    Ok(DeletedVersion {
        bundle: bundle.to_path_buf(),
        version,
        was_using_compressed,
    })
}

pub fn settle_restored(deleted: &DeletedVersion) -> std::io::Result<()> {
    if deleted.version == Version::Original {
        match std::fs::remove_file(deleted.bundle.join(ORIGINAL_DELETED)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    settle_on(&deleted.bundle, deleted.was_using_compressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(original: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("content")).unwrap();
        std::fs::write(dir.path().join(OUTPUT), original).unwrap();
        dir
    }

    fn output(dir: &tempfile::TempDir) -> Vec<u8> {
        std::fs::read(dir.path().join(OUTPUT)).unwrap()
    }

    #[test]
    fn versions_switch_back_and_forth_and_either_can_be_deleted() {
        let dir = bundle(b"original-video");
        assert_eq!(
            video_versions(dir.path()),
            Some(VideoVersions::Single { size: 14 })
        );

        std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
        let copy = video_versions(dir.path()).unwrap();
        assert_eq!(
            copy,
            VideoVersions::CompressedCopy {
                original: 14,
                compressed: 5
            }
        );
        assert_eq!(copy.saving_percent(), Some(64));
        assert_eq!(copy.saved(), 9);

        use_compressed(dir.path()).unwrap();
        assert_eq!(output(&dir), b"small");
        assert_eq!(
            video_versions(dir.path()),
            Some(VideoVersions::UsingCompressed {
                original: 14,
                compressed: 5
            })
        );

        restore_original(dir.path()).unwrap();
        assert_eq!(output(&dir), b"original-video");
        assert!(dir.path().join(COMPRESSED).is_file());

        delete_original(dir.path()).unwrap();
        assert_eq!(output(&dir), b"small");
        assert_eq!(
            video_versions(dir.path()),
            Some(VideoVersions::Single { size: 5 })
        );
    }

    #[test]
    fn deleting_the_original_leaves_a_project_known_as_compressed_only() {
        let dir = bundle(b"original-video");
        std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
        assert!(!compressed_only(dir.path()));
        delete_original(dir.path()).unwrap();
        assert!(compressed_only(dir.path()));
    }

    #[test]
    fn deleting_the_compressed_version_while_in_use_brings_the_original_back() {
        let dir = bundle(b"original-video");
        std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
        use_compressed(dir.path()).unwrap();
        delete_compressed(dir.path()).unwrap();
        assert_eq!(output(&dir), b"original-video");
        assert_eq!(
            video_versions(dir.path()),
            Some(VideoVersions::Single { size: 14 })
        );
    }

    fn move_aside(aside: &Path) -> impl FnOnce(&Path) -> Result<(), String> {
        move |file| std::fs::rename(file, aside).map_err(|error| error.to_string())
    }

    fn undo(deleted: &DeletedVersion, aside: &Path) {
        std::fs::rename(aside, deleted.file()).unwrap();
        settle_restored(deleted).unwrap();
    }

    #[test]
    fn deleting_either_version_and_undoing_returns_to_the_same_state_in_every_starting_state() {
        for version in [Version::Compressed, Version::Original] {
            for using in [false, true] {
                let dir = bundle(b"original-video");
                let holding = tempfile::tempdir().unwrap();
                let aside = holding.path().join("video");
                std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
                if using {
                    use_compressed(dir.path()).unwrap();
                }
                let before = video_versions(dir.path());
                let shown = output(&dir);

                let deleted = delete_version_with(dir.path(), version, move_aside(&aside)).unwrap();
                let kept: &[u8] = match version {
                    Version::Compressed => b"original-video",
                    Version::Original => b"small",
                };
                assert_eq!(output(&dir), kept);
                assert_eq!(
                    video_versions(dir.path()),
                    Some(VideoVersions::Single {
                        size: kept.len() as u64
                    })
                );
                assert_eq!(compressed_only(dir.path()), version == Version::Original);

                undo(&deleted, &aside);
                assert_eq!(video_versions(dir.path()), before);
                assert_eq!(output(&dir), shown);
                assert!(!compressed_only(dir.path()));
            }
        }
    }

    #[test]
    fn a_failed_version_delete_changes_nothing() {
        for version in [Version::Compressed, Version::Original] {
            for using in [false, true] {
                let dir = bundle(b"original-video");
                std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
                if using {
                    use_compressed(dir.path()).unwrap();
                }
                let before = video_versions(dir.path());
                let result =
                    delete_version_with(dir.path(), version, |_| Err("in use".to_string()));
                assert_eq!(result, Err("in use".to_string()));
                assert_eq!(video_versions(dir.path()), before);
                assert!(!compressed_only(dir.path()));
            }
        }
    }

    #[test]
    fn a_failed_swap_leaves_the_current_video_in_place() {
        let dir = bundle(b"original-video");
        assert!(use_compressed(dir.path()).is_err());
        assert_eq!(output(&dir), b"original-video");
        assert!(!dir.path().join(ORIGINAL).exists());
    }

    #[test]
    fn a_bundle_without_a_video_has_no_versions_and_a_second_compress_is_refused() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(video_versions(empty.path()), None);

        let dir = bundle(b"original-video");
        std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
        assert!(compress(dir.path(), |_, _| true).is_err());
    }
}
