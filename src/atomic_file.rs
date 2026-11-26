use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let file_name = path
        .file_name()
        .context("atomic file path must have a file name")?
        .to_string_lossy();
    let (temporary_path, mut temporary_file) = create_temporary_file(parent, &file_name)?;
    let result = (|| {
        temporary_file
            .write_all(content)
            .with_context(|| format!("writing {}", temporary_path.display()))?;
        temporary_file
            .sync_all()
            .with_context(|| format!("syncing {}", temporary_path.display()))?;
        drop(temporary_file);
        fs::rename(&temporary_path, path)
            .with_context(|| format!("replacing {}", path.display()))?;
        fs::File::open(parent)
            .with_context(|| format!("opening {}", parent.display()))?
            .sync_all()
            .with_context(|| format!("syncing directory {}", parent.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn create_temporary_file(parent: &Path, file_name: &str) -> Result<(PathBuf, fs::File)> {
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("creating {}", temporary_path.display()));
            }
        }
    }
    anyhow::bail!(
        "could not allocate a temporary file in {}",
        parent.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_file_atomically_without_leaving_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock.toml");
        fs::write(&path, "old").unwrap();

        write(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replace_preserves_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("destination");
        fs::create_dir(&path).unwrap();

        assert!(write(&path, b"new").is_err());

        assert!(path.is_dir());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
