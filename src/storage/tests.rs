use super::*;

#[test]
fn schema_upgrade_creates_tables_on_old_databases() {
    // Simulate a database created at schema 6, before the metadata table
    // existed: a metadata row only, no other tables.
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("mail.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE storage_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO storage_metadata (key, value) VALUES ('schema_version', '6');",
    )
    .unwrap();
    schema::ensure_schema(&conn).unwrap();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    for table in ["blobs", "messages", "remote_bindings", "message_metadata"] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                rusqlite::params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "table {table} missing after upgrade");
    }
    let version: String = conn
        .query_row(
            "SELECT value FROM storage_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, "8");
}

#[test]
fn existing_database_with_legacy_columns_and_mailspace_tables_still_works() {
    // Databases written before the mailspace tables moved to their own crate
    // carry absorbed_at/absorbed_by on `messages`, a mailspace_events table,
    // and the work-graph tables. None of them are created here any more, so
    // this pins that an existing database keeps working untouched.
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("storage.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE storage_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO storage_metadata (key, value) VALUES ('schema_version', '8');
         CREATE TABLE blobs (
           content_id TEXT PRIMARY KEY,
           blob_relpath TEXT NOT NULL UNIQUE,
           byte_size INTEGER NOT NULL,
           rfc_message_id TEXT,
           parsed_at TEXT NOT NULL
         );
         CREATE TABLE messages (
           message_id TEXT PRIMARY KEY,
           account TEXT NOT NULL,
           content_id TEXT NOT NULL REFERENCES blobs(content_id) ON DELETE RESTRICT,
           local_role TEXT NOT NULL,
           read_state INTEGER NOT NULL DEFAULT 0,
           starred INTEGER NOT NULL DEFAULT 0,
           draft_state TEXT,
           discovered_at TEXT NOT NULL,
           updated_at TEXT NOT NULL,
           deleted_at TEXT,
           absorbed_at TEXT,
           absorbed_by TEXT
         );
         CREATE TABLE remote_bindings (
           message_id TEXT PRIMARY KEY REFERENCES messages(message_id) ON DELETE CASCADE,
           account TEXT NOT NULL,
           provider TEXT NOT NULL,
           remote_mailbox TEXT NOT NULL,
           remote_uid INTEGER NOT NULL,
           remote_uidvalidity INTEGER NOT NULL,
           last_verified_at TEXT NOT NULL,
           stale INTEGER NOT NULL DEFAULT 0,
           UNIQUE (account, remote_mailbox, remote_uidvalidity, remote_uid)
         );
         CREATE TABLE message_metadata (
           content_id TEXT PRIMARY KEY REFERENCES blobs(content_id) ON DELETE CASCADE,
           date TEXT NOT NULL,
           from_addr TEXT NOT NULL,
           to_addr TEXT NOT NULL,
           cc_addr TEXT NOT NULL,
           bcc_addr TEXT NOT NULL,
           subject TEXT NOT NULL,
           normalized_message_id TEXT
         );
         CREATE TABLE mailspace_events (
           event_id INTEGER PRIMARY KEY AUTOINCREMENT,
           occurred_at TEXT NOT NULL
         );
         CREATE TABLE work_graphs (handle TEXT PRIMARY KEY);",
    )
    .unwrap();
    drop(conn);

    let mut storage = Storage::open(tmp.path()).unwrap();
    let raw = message_bytes("legacy@example.com", "body");
    let stored = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:11".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 11,
                    remote_uidvalidity: 42,
                }),
            },
            &raw,
        )
        .unwrap();

    assert!(storage.message_by_id(&stored.message_id).unwrap().is_some());
    assert_eq!(storage.count_messages_for_account("acct").unwrap(), 1);
}

#[test]
fn import_dedupes_blobs_but_keeps_distinct_message_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("dup@example.com", "same body");
    let first = write_catalog_file(tmp.path(), "inbox-1.eml", &raw);
    let second = write_catalog_file(tmp.path(), "archive-2.eml", &raw);

    let entries = vec![
        catalog_entry("acct", "one", &first, "INBOX", Some(remote("INBOX", 7))),
        catalog_entry(
            "acct",
            "two",
            &second,
            "Archive",
            Some(remote("Archive", 8)),
        ),
    ];

    let result = import_catalog_entries(tmp.path(), &entries).unwrap();
    let storage = Storage::open(tmp.path()).unwrap();

    assert_eq!(result.imported_messages, 2);
    assert_eq!(result.imported_blobs, 1);
    assert_eq!(storage.blob_count().unwrap(), 1);
    assert_eq!(storage.message_count().unwrap(), 2);
    assert_eq!(storage.remote_binding_count().unwrap(), 2);
}

#[test]
fn import_persists_blob_and_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = b"Message-ID: <meta@example.com>\r\nFrom: Agent <agent@example.com>\r\nTo: User <user@example.com>\r\nSubject: hello\r\n\r\nbody";
    let path = write_catalog_file(tmp.path(), "inbox-1.eml", raw);
    let entries = vec![catalog_entry(
        "acct",
        "one",
        &path,
        "INBOX",
        Some(remote("INBOX", 7)),
    )];

    import_catalog_entries(tmp.path(), &entries).unwrap();
    let storage = Storage::open(tmp.path()).unwrap();
    let data = storage.read_blob(&resulting_content_id(raw)).unwrap();

    assert_eq!(data, raw);
    assert_eq!(storage.blob_count().unwrap(), 1);
    assert_eq!(storage.message_count().unwrap(), 1);
}

#[test]
fn fallback_message_ids_are_stable_for_unbound_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("local@example.com", "body");
    let path = write_catalog_file(tmp.path(), "draft-1.eml", &raw);
    let entry = catalog_entry("acct", "draft-handle", &path, "Drafts", None);

    let mut storage = Storage::open(tmp.path()).unwrap();
    let first = storage
        .ingest_message(&request_from_catalog_entry(&entry), &raw)
        .unwrap();
    let second = storage
        .ingest_message(&request_from_catalog_entry(&entry), &raw)
        .unwrap();

    assert_eq!(first.message_id, second.message_id);
    assert_eq!(storage.message_count().unwrap(), 1);
}

#[test]
fn direct_ingest_api_supports_clean_break_sync_target() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("direct@example.com", "body");
    let request = MessageIngestRequest {
        account: "acct".into(),
        local_role: "inbox".into(),
        read_state: false,
        starred: false,
        message_id_hint: None,
        seed_hint: "uid:99".into(),
        remote: Some(RemoteBindingInput {
            account: "acct".into(),
            provider: "protonmail".into(),
            remote_mailbox: "INBOX".into(),
            remote_uid: 99,
            remote_uidvalidity: 42,
        }),
    };

    let mut storage = Storage::open(tmp.path()).unwrap();
    let stored = storage.ingest_message(&request, &raw).unwrap();

    assert!(stored.message_id.starts_with("msg_"));
    assert_eq!(storage.blob_count().unwrap(), 1);
    assert_eq!(storage.message_count().unwrap(), 1);
    assert_eq!(storage.remote_binding_count().unwrap(), 1);
}

#[test]
fn short_handles_resolve_uniquely_for_storage_native_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(tmp.path()).unwrap();

    let first = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:1".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 1,
                    remote_uidvalidity: 42,
                }),
            },
            &message_bytes("one@example.com", "first"),
        )
        .unwrap();
    let second = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:2".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 2,
                    remote_uidvalidity: 42,
                }),
            },
            &message_bytes("two@example.com", "second"),
        )
        .unwrap();

    let first_handle = storage.display_handle(&first.message_id).unwrap();
    let second_handle = storage.display_handle(&second.message_id).unwrap();

    assert_ne!(first_handle, second_handle);
    assert!(first_handle.len() >= 7);
    assert_eq!(
        storage.resolve_message_token(&first_handle).unwrap(),
        first.message_id
    );
    assert_eq!(
        storage.resolve_message_token(&second_handle).unwrap(),
        second.message_id
    );
}

#[test]
fn short_handle_map_is_a_fixed_width_prefix_of_the_basis() {
    let ids = vec![
        "msg_abcd1234aaaabbbbcccc0001".to_string(),
        "msg_abcd1234aaaabbbbcccc0002".to_string(),
        "msg_beef5678aaaabbbbcccc0003".to_string(),
        "not-a-native-id".to_string(),
    ];

    let map = short_handle_map(&ids);

    assert_eq!(map["msg_abcd1234aaaabbbbcccc0001"], "abcd1234");
    assert_eq!(map["msg_beef5678aaaabbbbcccc0003"], "beef5678");
    // A shared prefix is a shared handle: the map does no cross-record work.
    assert_eq!(
        map["msg_abcd1234aaaabbbbcccc0001"],
        map["msg_abcd1234aaaabbbbcccc0002"]
    );
    // An id that is not storage-native is its own handle.
    assert_eq!(map["not-a-native-id"], "not-a-native-id");
}

#[test]
fn a_shared_short_handle_resolves_as_ambiguous_rather_than_by_guess() {
    let tmp = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(tmp.path()).unwrap();
    let first = ingest_hinted(&mut storage, "msg_abcd1234aaaabbbbcccc0001", 1);
    let second = ingest_hinted(&mut storage, "msg_abcd1234aaaabbbbcccc0002", 2);
    assert_ne!(first, second);

    assert_eq!(storage.display_handle(&first).unwrap(), "abcd1234");
    assert_eq!(storage.display_handle(&second).unwrap(), "abcd1234");
    // The shared token is reported, never resolved by guessing.
    assert!(storage.resolve_message_token("abcd1234").is_err());
    // The full id still addresses each message.
    assert_eq!(storage.resolve_message_token(&first).unwrap(), first);
    assert_eq!(storage.resolve_message_token(&second).unwrap(), second);
}

fn ingest_hinted(storage: &mut Storage, message_id: &str, remote_uid: u32) -> String {
    storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: Some(message_id.to_string()),
                seed_hint: format!("remote_uid:{remote_uid}"),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid,
                    remote_uidvalidity: 42,
                }),
            },
            &message_bytes("hinted@example.com", "body"),
        )
        .unwrap()
        .message_id
}

#[test]
fn content_id_prefix_can_resolve_message() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("content@example.com", "body");
    let mut storage = Storage::open(tmp.path()).unwrap();
    let stored = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:3".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 3,
                    remote_uidvalidity: 42,
                }),
            },
            &raw,
        )
        .unwrap();

    let prefix = &stored.content_id[..12];
    assert_eq!(
        storage.resolve_message_token(prefix).unwrap(),
        stored.message_id
    );
}

#[test]
fn local_size_fallback_uses_remote_uid_shape_for_storage_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("size@example.com", "body");
    let mut storage = Storage::open(tmp.path()).unwrap();
    storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:7".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 7,
                    remote_uidvalidity: 42,
                }),
            },
            &raw,
        )
        .unwrap();

    let sizes = storage.local_sizes_by_role("inbox").unwrap();
    assert_eq!(sizes.get("inbox-7"), Some(&(raw.len() as u64)));
}

#[test]
fn latest_from_prefers_exact_address_and_ignores_memos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(tmp.path()).unwrap();
    ingest_dated(
        &mut storage,
        "mind",
        "sent",
        "mind@faberlang.local",
        "Wed, 01 Jan 2020 00:00:00 +0000",
        "old-mail",
    );
    ingest_dated(
        &mut storage,
        "mind",
        "memos",
        "mind@faberlang.local",
        "Fri, 03 Jan 2020 00:00:00 +0000",
        "newer-memo",
    );
    let newer = ingest_dated(
        &mut storage,
        "mind",
        "sent",
        "mind@faberlang.local",
        "Thu, 02 Jan 2020 00:00:00 +0000",
        "newer-mail",
    );
    let found = storage
        .latest_message_from_addresses(&["mind@faberlang.local".into()])
        .unwrap()
        .expect("signal");
    assert_eq!(found.message_id, newer);
    assert_eq!(found.local_role, "sent");
    assert!(!found.handle.is_empty());
}

#[test]
fn latest_from_matches_display_name_form() {
    let tmp = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(tmp.path()).unwrap();
    let id = ingest_dated(
        &mut storage,
        "acct",
        "sent",
        "Agent <agent@example.com>",
        "Thu, 02 Jan 2020 00:00:00 +0000",
        "named",
    );
    let found = storage
        .latest_message_from_addresses(&["agent@example.com".into()])
        .unwrap()
        .expect("display-name from");
    assert_eq!(found.message_id, id);
}

#[test]
fn schema_v6_adds_from_addr_date_index_on_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Storage::open(tmp.path()).unwrap();
    storage
        .conn
        .execute("DROP INDEX message_metadata_from_addr_date_idx", [])
        .unwrap();
    storage
        .conn
        .execute(
            "INSERT OR REPLACE INTO storage_metadata (key, value) VALUES ('schema_version', '5')",
            [],
        )
        .unwrap();
    drop(storage);
    let storage = Storage::open(tmp.path()).unwrap();
    let present: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'message_metadata_from_addr_date_idx'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(present, 1);
    let version: String = storage
        .conn
        .query_row(
            "SELECT value FROM storage_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, "8");
}

fn ingest_dated(
    storage: &mut Storage,
    account: &str,
    role: &str,
    from: &str,
    date: &str,
    seed: &str,
) -> String {
    let raw = format!(
        "Message-ID: <{seed}@example.com>\r\nFrom: {from}\r\nTo: x@example.com\r\n\
         Date: {date}\r\nSubject: s\r\n\r\nbody"
    )
    .into_bytes();
    storage
        .ingest_message(
            &MessageIngestRequest {
                account: account.into(),
                local_role: role.into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: seed.into(),
                remote: None,
            },
            &raw,
        )
        .unwrap()
        .message_id
}

fn message_bytes(message_id: &str, body: &str) -> Vec<u8> {
    format!(
            "Message-ID: <{message_id}>\r\nFrom: Agent <agent@example.com>\r\nTo: User <user@example.com>\r\nSubject: hi\r\n\r\n{body}"
        )
        .into_bytes()
}

fn resulting_content_id(data: &[u8]) -> String {
    sha256_hex(data)
}

fn write_catalog_file(root: &Path, name: &str, data: &[u8]) -> String {
    let path = root.join(name);
    fs::write(&path, data).unwrap();
    path.to_string_lossy().to_string()
}

fn catalog_entry(
    account: &str,
    handle: &str,
    blob_path: &str,
    folder: &str,
    remote: Option<RemoteIdentity>,
) -> CatalogEntry {
    CatalogEntry {
        handle: handle.into(),
        account: account.into(),
        content_id: sha256_hex(&fs::read(blob_path).unwrap()),
        blob_path: blob_path.into(),
        local_role: local_role(folder),
        read_state: false,
        starred: false,
        date: "2026-05-03T12:00:00Z".into(),
        from: "agent@example.com".into(),
        to: "user@example.com".into(),
        cc: String::new(),
        bcc: String::new(),
        subject: "hi".into(),
        rfc_message_id: "meta@example.com".into(),
        remote,
    }
}

fn remote(mailbox: &str, uid: u32) -> RemoteIdentity {
    RemoteIdentity {
        account: "acct".into(),
        provider: "protonmail".into(),
        remote_mailbox: mailbox.into(),
        local_folder: mailbox.to_ascii_lowercase(),
        uid,
        uidvalidity: 42,
        rfc_message_id: "meta@example.com".into(),
        size: 128,
        content_fingerprint: "unused".into(),
    }
}

#[test]
fn mark_message_deleted_scopes_to_the_given_account() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("move@example.com", "body");
    let mut storage = Storage::open(tmp.path()).unwrap();
    let stored = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:5".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 5,
                    remote_uidvalidity: 42,
                }),
            },
            &raw,
        )
        .unwrap();

    // Another account's handle must not reach this message.
    let changed = storage
        .mark_message_deleted("other", &stored.message_id)
        .unwrap();
    assert!(!changed);

    // Original message must be unchanged.
    let sizes = storage.local_sizes_by_role("inbox").unwrap();
    assert!(sizes.contains_key("inbox-5"));
}

#[test]
fn mark_message_deleted_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = message_bytes("del@example.com", "body");
    let mut storage = Storage::open(tmp.path()).unwrap();
    let stored = storage
        .ingest_message(
            &MessageIngestRequest {
                account: "acct".into(),
                local_role: "inbox".into(),
                read_state: false,
                starred: false,
                message_id_hint: None,
                seed_hint: "remote_uid:6".into(),
                remote: Some(RemoteBindingInput {
                    account: "acct".into(),
                    provider: "protonmail".into(),
                    remote_mailbox: "INBOX".into(),
                    remote_uid: 6,
                    remote_uidvalidity: 42,
                }),
            },
            &raw,
        )
        .unwrap();

    // First delete succeeds.
    assert!(
        storage
            .mark_message_deleted("acct", &stored.message_id)
            .unwrap()
    );

    // Second delete returns false — already soft-deleted.
    assert!(
        !storage
            .mark_message_deleted("acct", &stored.message_id)
            .unwrap()
    );
}

// ---------------------------------------------------------------------------
// Test helpers extracted from production modules
// ---------------------------------------------------------------------------

fn local_role(folder: &str) -> String {
    match folder {
        "INBOX" | "Inbox" | "inbox" => "inbox".into(),
        "Archive" | "archive" => "archive".into(),
        "Trash" | "trash" => "trash".into(),
        "Sent" | "sent" => "sent".into(),
        "Drafts" | "drafts" => "drafts".into(),
        other => other.to_ascii_lowercase(),
    }
}

impl Storage {
    fn blob_count(&self) -> Result<usize, VivariumError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM blobs", [], |row| row.get(0))
            .map_err(|e| VivariumError::Other(format!("failed to count blobs: {e}")))
    }

    fn message_count(&self) -> Result<usize, VivariumError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .map_err(|e| VivariumError::Other(format!("failed to count messages: {e}")))
    }

    fn remote_binding_count(&self) -> Result<usize, VivariumError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM remote_bindings", [], |row| row.get(0))
            .map_err(|e| VivariumError::Other(format!("failed to count remote bindings: {e}")))
    }
}
