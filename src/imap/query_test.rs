use super::*;

#[test]
fn metadata_fetch_items_include_message_id_header() {
    assert!(METADATA_FETCH_ITEMS.contains("UID"));
    assert!(METADATA_FETCH_ITEMS.contains("FLAGS"));
    assert!(METADATA_FETCH_ITEMS.contains("RFC822.SIZE"));
    assert!(METADATA_FETCH_ITEMS.contains("BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]"));
}

#[test]
fn metadata_fetch_never_uses_non_peek_body() {
    assert!(!METADATA_FETCH_ITEMS.contains("BODY[HEADER"));
    assert!(!METADATA_FETCH_ITEMS.contains("BODY[]"));
    assert!(METADATA_FETCH_ITEMS.contains("BODY.PEEK["));
}

#[test]
fn remote_metadata_opens_mailbox_read_only() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/imap/query.rs")).unwrap();
    assert!(
        source.contains("examine(remote_folder)"),
        "sync metadata listing must EXAMINE rather than SELECT"
    );
    assert!(!source.contains("select(remote_folder)"));
}
