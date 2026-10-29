use std::{
    fs::{File, FileTimes, Metadata},
    io::ErrorKind,
    path::Path,
};

#[derive(Debug)]
pub(crate) enum MoveError {
    InUse,
    Other(std::io::Error),
}

impl From<std::io::Error> for MoveError {
    fn from(error: std::io::Error) -> Self {
        const ACCESS_DENIED: i32 = 5;
        const SHARING_VIOLATION: i32 = 32;
        const LOCK_VIOLATION: i32 = 33;
        let in_use = cfg!(windows)
            && matches!(
                error.raw_os_error(),
                Some(ACCESS_DENIED | SHARING_VIOLATION | LOCK_VIOLATION)
            );
        if in_use {
            Self::InUse
        } else {
            Self::Other(error)
        }
    }
}

pub(crate) fn move_path(from: &Path, to: &Path) -> Result<(), MoveError> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::CrossesDevices => copy_verify_delete(from, to),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn copy_verify_delete(from: &Path, to: &Path) -> Result<(), MoveError> {
    if to.exists() {
        return Err(MoveError::Other(ErrorKind::AlreadyExists.into()));
    }
    let copied = copy_tree(from, to).and_then(|()| verify_same_tree(from, to));
    if let Err(error) = copied {
        remove_path(to).ok();
        return Err(error.into());
    }
    remove_path(from).map_err(MoveError::from)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let metadata = std::fs::metadata(from)?;
    if metadata.is_dir() {
        std::fs::create_dir(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(from, to)?;
    }
    keep_dates(to, &metadata)
}

fn verify_same_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let original = std::fs::metadata(from)?;
    let copy = std::fs::metadata(to)?;
    if original.is_dir() != copy.is_dir() || (!original.is_dir() && original.len() != copy.len()) {
        return Err(std::io::Error::other(format!(
            "The copy of {} does not match the original",
            from.display()
        )));
    }
    if original.is_dir() {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            verify_same_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

pub(crate) fn keep_dates(path: &Path, from: &Metadata) -> std::io::Result<()> {
    let mut times = FileTimes::new();
    if let Ok(modified) = from.modified() {
        times = times.set_modified(modified);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTimesExt;
        if let Ok(created) = from.created() {
            times = times.set_created(created);
        }
    }
    open_for_dates(path)?.set_times(times)
}

#[cfg(windows)]
fn open_for_dates(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    File::options()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_for_dates(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_between_drives_copies_verifies_keeps_dates_then_deletes_the_original() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("D/Old.cap");
        std::fs::create_dir_all(project.join("content")).unwrap();
        std::fs::write(project.join("content/output.mp4"), b"video").unwrap();
        std::fs::write(project.join("recording-meta.json"), b"{}").unwrap();
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        File::options()
            .write(true)
            .open(project.join("content/output.mp4"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let original_dates = std::fs::metadata(&project).unwrap();
        let destination = root.path().join("E/New.ever");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();

        copy_verify_delete(&project, &destination).unwrap();

        assert!(!project.exists());
        assert_eq!(
            std::fs::read(destination.join("content/output.mp4")).unwrap(),
            b"video"
        );
        assert_eq!(
            std::fs::metadata(destination.join("content/output.mp4"))
                .unwrap()
                .modified()
                .unwrap(),
            old
        );
        assert_eq!(
            std::fs::metadata(&destination).unwrap().created().ok(),
            original_dates.created().ok()
        );
    }

    #[test]
    fn a_failed_copy_between_drives_leaves_the_original_and_no_partial_copy() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("D/Old.cap");
        std::fs::create_dir_all(project.join("content")).unwrap();
        std::fs::write(project.join("recording-meta.json"), b"{}").unwrap();
        let unreadable = project.join("content/display.mp4");
        std::fs::create_dir_all(unreadable.parent().unwrap()).unwrap();
        std::fs::write(&unreadable, b"video").unwrap();
        let destination = root.path().join("E/New.ever");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let taken = root.path().join("E/Taken.ever");
        std::fs::create_dir_all(&taken).unwrap();
        std::fs::write(taken.join("recording-meta.json"), b"other").unwrap();

        assert!(copy_verify_delete(&project, &taken).is_err());
        assert_eq!(
            std::fs::read(taken.join("recording-meta.json")).unwrap(),
            b"other"
        );

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _lock = File::options()
                .read(true)
                .share_mode(0)
                .open(&unreadable)
                .unwrap();
            assert!(matches!(
                copy_verify_delete(&project, &destination),
                Err(MoveError::InUse)
            ));
            assert!(!destination.exists());
        }

        assert!(project.join("recording-meta.json").is_file());
        assert!(unreadable.is_file());
    }
}
