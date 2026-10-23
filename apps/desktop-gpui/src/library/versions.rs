use std::path::{Path, PathBuf};

const OUTPUT: &str = "content/output.mp4";
const COMPRESSED: &str = "content/output_compressed.mp4";
const ORIGINAL: &str = "content/output_original.mp4";
const COMPRESSING: &str = "content/output_compressing.mp4";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoVersions {
    Single { size: u64 },
    CompressedCopy { original: u64, compressed: u64 },
    UsingCompressed { original: u64, compressed: u64 },
}

impl VideoVersions {
    pub fn saving_percent(self) -> Option<u64> {
        let (original, compressed) = match self {
            Self::Single { .. } => return None,
            Self::CompressedCopy {
                original,
                compressed,
            }
            | Self::UsingCompressed {
                original,
                compressed,
            } => (original, compressed),
        };
        (original > 0).then(|| original.saturating_sub(compressed) * 100 / original)
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
    if !matches!(video_versions(bundle), Some(VideoVersions::Single { .. })) {
        return Err(crate::i18n::t!("This recording already has a compressed version").to_string());
    }
    let staging = bundle.join(COMPRESSING);
    let target = bundle.join(COMPRESSED);
    let result = cap_export::compress::compress_to_hevc(&bundle.join(OUTPUT), &staging, progress)
        .and_then(|()| std::fs::rename(&staging, &target).map_err(|error| error.to_string()));
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result.map(|()| target)
}

fn swap(bundle: &Path, current_to: &str, next_from: &str) -> std::io::Result<()> {
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
    if matches!(
        video_versions(bundle),
        Some(VideoVersions::UsingCompressed { .. })
    ) {
        restore_original(bundle)?;
    }
    std::fs::remove_file(bundle.join(COMPRESSED))
}

pub fn delete_original(bundle: &Path) -> std::io::Result<()> {
    if matches!(
        video_versions(bundle),
        Some(VideoVersions::CompressedCopy { .. })
    ) {
        use_compressed(bundle)?;
    }
    std::fs::remove_file(bundle.join(ORIGINAL))
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
