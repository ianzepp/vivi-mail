use super::*;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn atomic_write_replaces_truncated_destination() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("blob.eml");
    fs::write(&path, b"partial").unwrap();

    atomic_write_private(&path, b"complete-message").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"complete-message");
}

#[cfg(unix)]
#[test]
fn atomic_write_creates_owner_only_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nested").join("blob.eml");

    atomic_write_private(&path, b"secret").unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn secure_sqlite_path_tightens_wal_and_shm() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("storage.sqlite");
    let wal = tmp.path().join("storage.sqlite-wal");
    let shm = tmp.path().join("storage.sqlite-shm");
    fs::write(&db, b"db").unwrap();
    fs::write(&wal, b"wal").unwrap();
    fs::write(&shm, b"shm").unwrap();
    fs::set_permissions(&wal, fs::Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(&shm, fs::Permissions::from_mode(0o644)).unwrap();

    secure_sqlite_path(&db).unwrap();

    assert_eq!(
        fs::metadata(&db).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&wal).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&shm).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn atomic_write_leaves_no_tmp_sibling() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("blob.eml");
    atomic_write_private(&path, b"ok").unwrap();
    let leftovers: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp siblings: {leftovers:?}");
}

#[test]
fn write_helpers_compile_against_file_api() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("scratch");
    let mut file = secure_create_file(&path).unwrap();
    file.write_all(b"x").unwrap();
}
