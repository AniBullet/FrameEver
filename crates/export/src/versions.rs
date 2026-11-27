use std::path::{Path, PathBuf};

pub const OUTPUT: &str = "content/output.mp4";
pub const COMPRESSED: &str = "content/output_compressed.mp4";
pub const ORIGINAL: &str = "content/output_original.mp4";
pub const COMPRESSING: &str = "content/output_compressing.mp4";
pub const ORIGINAL_DELETED: &str = "content/original_deleted";

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

pub fn compressed_only(bundle: &Path) -> bool {
    bundle.join(ORIGINAL_DELETED).is_file()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressError {
    AlreadyCompressed,
    Failed(String),
}

pub fn compress(
    bundle: &Path,
    progress: impl FnMut(u64, u64) -> bool,
) -> Result<PathBuf, CompressError> {
    let _turn = ONE_COMPRESSION_AT_A_TIME
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if !matches!(video_versions(bundle), Some(VideoVersions::Single { .. }))
        || compressed_only(bundle)
    {
        return Err(CompressError::AlreadyCompressed);
    }
    let staging = bundle.join(COMPRESSING);
    let target = bundle.join(COMPRESSED);
    let result = crate::compress::compress_to_hevc(&bundle.join(OUTPUT), &staging, progress)
        .and_then(|()| std::fs::rename(&staging, &target).map_err(|error| error.to_string()));
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result.map(|()| target).map_err(CompressError::Failed)
}

pub fn keep_only_compressed(bundle: &Path) -> std::io::Result<()> {
    if matches!(
        video_versions(bundle),
        Some(VideoVersions::CompressedCopy { .. })
    ) {
        let output = bundle.join(OUTPUT);
        let parked = bundle.join(ORIGINAL);
        std::fs::rename(&output, &parked)?;
        if let Err(error) = std::fs::rename(bundle.join(COMPRESSED), &output) {
            let _ = std::fs::rename(&parked, &output);
            return Err(error);
        }
    }
    std::fs::remove_file(bundle.join(ORIGINAL))?;
    std::fs::write(bundle.join(ORIGINAL_DELETED), b"")
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

    #[test]
    fn a_recording_with_a_compressed_copy_or_only_the_compressed_video_is_not_compressed_again() {
        let copy = bundle(b"original-video");
        std::fs::write(copy.path().join(COMPRESSED), b"small").unwrap();
        assert_eq!(
            compress(copy.path(), |_, _| true),
            Err(CompressError::AlreadyCompressed)
        );

        let only = bundle(b"small");
        std::fs::write(only.path().join(ORIGINAL_DELETED), b"").unwrap();
        assert_eq!(
            compress(only.path(), |_, _| true),
            Err(CompressError::AlreadyCompressed)
        );
    }

    #[test]
    fn keeping_only_the_compressed_copy_moves_it_in_place_and_marks_the_original_gone() {
        let dir = bundle(b"original-video");
        std::fs::write(dir.path().join(COMPRESSED), b"small").unwrap();
        keep_only_compressed(dir.path()).unwrap();
        assert_eq!(std::fs::read(dir.path().join(OUTPUT)).unwrap(), b"small");
        assert!(!dir.path().join(ORIGINAL).exists());
        assert!(!dir.path().join(COMPRESSED).exists());
        assert!(compressed_only(dir.path()));
    }
}
