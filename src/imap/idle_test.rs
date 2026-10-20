use super::*;

#[test]
fn wait_modes_are_explicitly_inbound_only() {
    assert_ne!(InboxWaitMode::ImapIdle, InboxWaitMode::Poll);
}

#[test]
fn inbound_watch_opens_inbox_read_only() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/imap/idle.rs")).unwrap();
    assert!(
        source.contains("examine(\"INBOX\")"),
        "watch-inbox must EXAMINE rather than SELECT so IDLE cannot write mailbox state"
    );
    assert!(!source.contains("select(\"INBOX\")"));
}
