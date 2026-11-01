use std::path::{Path, PathBuf};

use frameever_utils::brand::PROJECT_EXTENSION;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};

pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,
    changes: flume::Receiver<()>,
}

impl LibraryWatcher {
    pub fn start(dirs: &[PathBuf]) -> Option<Self> {
        let (sender, changes) = flume::unbounded();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            if event.is_ok_and(|event| is_project_change(&event)) {
                let _ = sender.send(());
            }
        })
        .map_err(|error| tracing::warn!(%error, "Could not watch the storage folder"))
        .ok()?;
        for dir in dirs {
            if let Err(error) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                tracing::warn!(%error, dir = %dir.display(), "Could not watch a library folder");
            }
        }
        Some(Self {
            _watcher: watcher,
            changes,
        })
    }

    pub async fn changed(&self) -> bool {
        self.changes.recv_async().await.is_ok()
    }

    pub fn settle(&self) {
        self.changes.drain().for_each(drop);
    }
}

fn is_project_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == PROJECT_EXTENSION)
}

fn is_project_change(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    ) && event.paths.iter().any(|path| is_project_path(path))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);
    const QUIET: Duration = Duration::from_millis(400);

    fn drain(watcher: &LibraryWatcher) {
        while watcher.changes.recv_timeout(QUIET).is_ok() {}
    }

    #[test]
    fn folders_added_renamed_or_deleted_in_explorer_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let watcher = LibraryWatcher::start(&[dir.path().to_path_buf()]).unwrap();

        let project = dir.path().join("Demo.ever");
        std::fs::create_dir(&project).unwrap();
        watcher.changes.recv_timeout(WAIT).unwrap();
        drain(&watcher);

        let renamed = dir.path().join("Renamed.ever");
        std::fs::rename(&project, &renamed).unwrap();
        watcher.changes.recv_timeout(WAIT).unwrap();
        drain(&watcher);

        std::fs::remove_dir(&renamed).unwrap();
        watcher.changes.recv_timeout(WAIT).unwrap();
    }

    #[test]
    fn other_files_and_writes_inside_a_project_stay_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Demo.ever");
        std::fs::create_dir(&project).unwrap();
        let watcher = LibraryWatcher::start(&[dir.path().to_path_buf()]).unwrap();

        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        std::fs::write(project.join("segment.m4s"), b"x").unwrap();
        assert!(watcher.changes.recv_timeout(QUIET).is_err());
    }
}
