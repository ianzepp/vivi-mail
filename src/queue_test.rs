use super::*;

#[test]
fn enqueue_round_trips_pending_item() {
    let tmp = tempfile::tempdir().unwrap();
    let item = QueueItem::new(
        "acct".into(),
        QueuedCommand::Archive {
            handles: vec!["one".into()],
        },
    );

    enqueue(tmp.path(), &item).unwrap();
    let loaded = load(tmp.path(), &item.id).unwrap();

    assert_eq!(loaded.status, QueueStatus::Pending);
    assert_eq!(loaded.command, item.command);
}

#[test]
fn send_dedupe_key_prefers_message_id() {
    let data =
        b"From: a@example.com\r\nTo: b@example.com\r\nMessage-ID: <once@example.com>\r\n\r\nbody";
    assert_eq!(send_dedupe_key(data), "msgid:once@example.com");
}

#[test]
fn send_dedupe_key_falls_back_to_content_hash() {
    let data = b"From: a@example.com\r\nTo: b@example.com\r\n\r\nbody";
    let key = send_dedupe_key(data);
    assert!(key.starts_with("sha256:"));
    assert_eq!(key, send_dedupe_key(data));
}

#[test]
fn send_already_attempted_treats_sending_and_executed_as_terminal() {
    let mut sending = QueueItem::new(
        "acct".into(),
        QueuedCommand::Send {
            path: "a.eml".into(),
            from: None,
        },
    );
    sending.send_message_id = Some("msgid:once@example.com".into());
    sending.mark(QueueStatus::Sending, None);
    let mut failed = sending.clone();
    failed.id = "other".into();
    failed.mark(QueueStatus::Failed, Some("smtp".into()));

    assert!(send_already_attempted(
        std::slice::from_ref(&sending),
        "msgid:once@example.com",
        "new"
    ));
    assert!(!send_already_attempted(
        std::slice::from_ref(&failed),
        "msgid:once@example.com",
        "new"
    ));
}

#[test]
fn list_hides_non_pending_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let pending = QueueItem::new(
        "acct".into(),
        QueuedCommand::Archive {
            handles: vec!["one".into()],
        },
    );
    let mut executed = QueueItem::new(
        "acct".into(),
        QueuedCommand::Archive {
            handles: vec!["two".into()],
        },
    );
    executed.mark(QueueStatus::Executed, None);

    enqueue(tmp.path(), &pending).unwrap();
    enqueue(tmp.path(), &executed).unwrap();

    assert_eq!(list(tmp.path(), false).unwrap().len(), 1);
    assert_eq!(list(tmp.path(), true).unwrap().len(), 2);
}
