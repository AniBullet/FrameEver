use std::path::Path;

pub fn is_same_file(source: &Path, destination: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let source = source.metadata()?;
        let destination = destination.metadata()?;
        Ok(source.dev() == destination.dev() && source.ino() == destination.ino())
    }
    #[cfg(not(unix))]
    {
        same_file::is_same_file(source, destination)
    }
}
