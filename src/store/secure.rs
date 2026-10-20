use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use crate::error::VivariumError;

pub(crate) fn secure_create_dir_all(path: &Path) -> Result<(), VivariumError> {
    fs::create_dir_all(path)?;
    secure_dir(path)
}

pub(crate) fn secure_create_file(path: &Path) -> Result<fs::File, VivariumError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path)?;
    secure_file(path)?;
    Ok(file)
}

fn secure_dir(path: &Path) -> Result<(), VivariumError> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub(crate) fn secure_file(path: &Path) -> Result<(), VivariumError> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Write `data` to `path` via a same-directory temp file, fsync, and rename.
///
/// A crash mid-write cannot leave a truncated destination: the final path
/// is replaced only after the temp file is fully written and fsynced.
pub(crate) fn atomic_write_private(path: &Path, data: &[u8]) -> Result<(), VivariumError> {
    let Some(parent) = path.parent() else {
        return Err(VivariumError::Other(format!(
            "path has no parent: {}",
            path.display()
        )));
    };
    secure_create_dir_all(parent)?;
    let tmp_path = temp_sibling(path);
    let mut file = secure_create_file(&tmp_path)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp_path, path)?;
    secure_file(path)?;
    fsync_dir(parent)
}

/// Restrict the database file and any WAL/SHM sidecars to owner-only access.
pub(crate) fn secure_sqlite_path(db_path: &Path) -> Result<(), VivariumError> {
    secure_file(db_path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = sidecar_path(db_path, suffix);
        if sidecar.exists() {
            secure_file(&sidecar)?;
        }
    }
    Ok(())
}

fn sidecar_path(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = db_path
        .file_name()
        .map_or_else(|| "storage.sqlite".into(), std::ffi::OsStr::to_os_string);
    name.push(suffix);
    db_path.with_file_name(name)
}

fn temp_sibling(path: &Path) -> std::path::PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blob");
    let unique = format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    match path.parent() {
        Some(parent) => parent.join(unique),
        None => std::path::PathBuf::from(unique),
    }
}

fn fsync_dir(path: &Path) -> Result<(), VivariumError> {
    let dir = File::open(path)?;
    dir.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[path = "secure_test.rs"]
mod tests;
