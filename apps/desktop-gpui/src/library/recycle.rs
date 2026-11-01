use std::path::{Path, PathBuf};

use super::versions::{self, DeletedVersion, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deleted {
    Project(PathBuf),
    Version(DeletedVersion),
}

impl Deleted {
    fn path(&self) -> PathBuf {
        match self {
            Self::Project(path) => path.clone(),
            Self::Version(version) => version.file(),
        }
    }
}

#[cfg(windows)]
fn on_own_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(work)
        .join()
        .unwrap_or_else(|_| Err("the Recycle Bin call panicked".to_string()))
}

#[cfg(windows)]
pub fn recycle(path: &Path) -> Result<(), String> {
    let path = path.to_path_buf();
    on_own_thread(move || trash::delete(&path).map_err(|error| error.to_string()))
}

#[cfg(windows)]
fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

#[cfg(windows)]
fn restore(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    on_own_thread(move || {
        let binned = trash::os_limited::list().map_err(|error| error.to_string())?;
        let mut items = Vec::with_capacity(paths.len());
        let mut missing = Vec::new();
        for path in paths {
            let item = binned
                .iter()
                .filter(|item| same_path(&item.original_path(), &path))
                .max_by_key(|item| item.time_deleted);
            match item {
                Some(item) => items.push(item.clone()),
                None => missing.push(path),
            }
        }
        trash::os_limited::restore_all(items).map_err(|error| error.to_string())?;
        Ok(missing)
    })
}

#[cfg(not(windows))]
pub fn recycle(path: &Path) -> Result<(), String> {
    Err(format!(
        "the Recycle Bin is Windows-only: {}",
        path.display()
    ))
}

#[cfg(not(windows))]
fn restore(_paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    Err("the Recycle Bin is Windows-only".to_string())
}

pub fn recycle_version(bundle: &Path, version: Version) -> Result<Deleted, String> {
    versions::delete_version_with(bundle, version, recycle).map(Deleted::Version)
}

pub fn undo(deleted: &[Deleted]) -> Result<(), String> {
    let missing = restore(deleted.iter().map(Deleted::path).collect())?;
    for deleted in deleted {
        if let Deleted::Version(version) = deleted
            && !missing.contains(&version.file())
        {
            versions::settle_restored(version).map_err(|error| error.to_string())?;
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("not in the Recycle Bin: {missing:?}"))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const EXPORT: &[u8] = b"exported-video";

    struct Storage {
        root: tempfile::TempDir,
        recordings: PathBuf,
        project: PathBuf,
        export: PathBuf,
    }

    impl Storage {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let recordings = root.path().canonicalize().unwrap().join("Recordings");
            let project = recordings.join("Demo.ever");
            std::fs::create_dir_all(project.join("content")).unwrap();
            std::fs::write(project.join("content/output.mp4"), b"original-video").unwrap();
            let export = recordings.parent().unwrap().join("Demo.mp4");
            std::fs::write(&export, EXPORT).unwrap();
            let project = dunce::simplified(&project).to_path_buf();
            Self {
                root,
                recordings,
                project,
                export,
            }
        }

        fn dirs(&self) -> Vec<PathBuf> {
            vec![self.recordings.clone()]
        }

        fn purge(&self, paths: &[&Path]) {
            let binned: Vec<_> = trash::os_limited::list()
                .unwrap()
                .into_iter()
                .filter(|item| {
                    paths
                        .iter()
                        .any(|path| same_path(&item.original_path(), path))
                })
                .collect();
            trash::os_limited::purge_all(binned).unwrap();
            assert!(self.root.path().exists());
        }
    }

    #[test]
    fn a_deleted_project_goes_to_the_recycle_bin_and_undo_brings_it_back_with_exports_untouched() {
        let storage = Storage::new();

        let deleted = crate::library::recycle_project_in(&storage.dirs(), &storage.project)
            .unwrap()
            .unwrap();
        assert!(!storage.project.exists());
        assert_eq!(std::fs::read(&storage.export).unwrap(), EXPORT);

        undo(&[deleted]).unwrap();
        assert_eq!(
            std::fs::read(storage.project.join("content/output.mp4")).unwrap(),
            b"original-video"
        );
        assert_eq!(std::fs::read(&storage.export).unwrap(), EXPORT);
    }

    #[test]
    fn undo_takes_the_newest_copy_when_the_same_path_was_deleted_twice() {
        let storage = Storage::new();
        let first = crate::library::recycle_project_in(&storage.dirs(), &storage.project)
            .unwrap()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::create_dir_all(storage.project.join("content")).unwrap();
        std::fs::write(storage.project.join("content/output.mp4"), b"second").unwrap();
        let second = crate::library::recycle_project_in(&storage.dirs(), &storage.project)
            .unwrap()
            .unwrap();
        assert_eq!(first, second);

        undo(&[second]).unwrap();
        assert_eq!(
            std::fs::read(storage.project.join("content/output.mp4")).unwrap(),
            b"second"
        );
        storage.purge(&[&storage.project]);
    }

    #[test]
    fn only_the_compressed_copy_goes_and_undo_puts_it_back() {
        let storage = Storage::new();
        let content = storage.project.join("content");
        let compressed = content.join("output_compressed.mp4");
        std::fs::write(&compressed, b"small").unwrap();

        let deleted = recycle_version(&storage.project, Version::Compressed).unwrap();
        assert!(!compressed.exists());
        assert_eq!(
            std::fs::read(content.join("output.mp4")).unwrap(),
            b"original-video"
        );

        undo(&[deleted]).unwrap();
        assert_eq!(std::fs::read(&compressed).unwrap(), b"small");
    }

    #[test]
    fn a_project_with_a_file_held_open_stays_and_says_so() {
        use std::os::windows::fs::OpenOptionsExt;

        let storage = Storage::new();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(storage.project.join("content/output.mp4"))
            .unwrap();

        let result = crate::library::recycle_project_in(&storage.dirs(), &storage.project);
        assert!(result.is_err(), "{result:?}");
        drop(held);
        assert!(storage.project.join("content/output.mp4").is_file());
    }

    #[test]
    fn a_path_outside_the_storage_folders_is_refused_and_left_alone() {
        let storage = Storage::new();
        let result = crate::library::recycle_project_in(&storage.dirs(), &storage.export);
        assert!(result.is_err());
        assert!(storage.export.is_file());
    }

    #[test]
    fn undo_brings_back_what_is_left_when_one_item_has_gone_from_the_recycle_bin() {
        let storage = Storage::new();
        let other = storage.recordings.join("Other.ever");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("keep"), b"kept").unwrap();
        let other = dunce::simplified(&other).to_path_buf();

        let first = crate::library::recycle_project_in(&storage.dirs(), &storage.project)
            .unwrap()
            .unwrap();
        let second = crate::library::recycle_project_in(&storage.dirs(), &other)
            .unwrap()
            .unwrap();
        storage.purge(&[&storage.project]);

        assert!(undo(&[first, second]).is_err());
        assert_eq!(std::fs::read(other.join("keep")).unwrap(), b"kept");
    }

    #[test]
    fn undo_says_so_when_the_item_has_left_the_recycle_bin() {
        let storage = Storage::new();
        let deleted = crate::library::recycle_project_in(&storage.dirs(), &storage.project)
            .unwrap()
            .unwrap();
        storage.purge(&[&storage.project]);

        assert!(undo(&[deleted]).is_err());
    }
}
