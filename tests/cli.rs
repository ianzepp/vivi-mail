//! CLI parsing coverage for the mail command surface, ported from the parent
//! repo's `tests/cli.rs`. Each test parses with `MailCli` and matches a
//! `MailCommand` variant.

use std::path::PathBuf;

use clap::Parser;
use vivi_mail::cli::{
    AgentArgs, AgentCommand, ComposeCommand, DoctorArgs, EnqueueArgs, EnqueueCommand, ExecArgs,
    ExecCommand, IndexArgs, IndexCommand, LabelArgs, LabelsArgs, ListArgs, MailCli, MailCommand,
    ProtonArgs, ProtonCommand, QueueArgs, QueueCommand, ReplyCommand, SearchArgs, SyncArgs,
    SyncEventsArgs, WatchInboxArgs,
};

#[test]
fn parses_render_explain_and_engine() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "render",
        "--explain",
        "--format",
        "pdf",
        "--engine",
        "pandoc-tectonic",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Render(command) => {
            assert!(command.explain);
            assert_eq!(command.engine.as_deref(), Some("pandoc-tectonic"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_compose_document_attachments() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "compose",
        "--to",
        "a@example.com",
        "--subject",
        "report",
        "--attach",
        "notes.txt",
        "--attach-document",
        "report.md",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Compose(command) => {
            assert_eq!(command.attachments, [std::path::PathBuf::from("notes.txt")]);
            assert_eq!(
                command.attach_document,
                Some(std::path::PathBuf::from("report.md"))
            );
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_sync_index_and_embed() {
    let cli = MailCli::try_parse_from(["vivi-mail", "sync", "--index", "--embed"]).unwrap();

    match cli.command {
        MailCommand::Sync(SyncArgs { index, embed, .. }) => {
            assert!(index);
            assert!(embed);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_sync_embed_without_index() {
    let cli = MailCli::try_parse_from(["vivi-mail", "sync", "--embed"]).unwrap();

    match cli.command {
        MailCommand::Sync(SyncArgs { index, embed, .. }) => {
            assert!(!index);
            assert!(embed);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_sync_json() {
    let cli = MailCli::try_parse_from(["vivi-mail", "sync", "--json"]).unwrap();

    match cli.command {
        MailCommand::Sync(SyncArgs { json, .. }) => assert!(json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_watch_inbox_event_contract() {
    let cli = MailCli::try_parse_from(["vivi-mail", "watch-inbox", "--account", "agent", "--json"])
        .unwrap();

    match cli.command {
        MailCommand::WatchInbox(WatchInboxArgs { account, json }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn old_outbox_coupled_watch_surface_is_removed() {
    assert!(MailCli::try_parse_from(["vivi-mail", "watch"]).is_err());
}

#[test]
fn parses_sync_events_watch() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "sync-events",
        "--account",
        "agent",
        "--bootstrap",
        "--watch",
        "--interval",
        "5m",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::SyncEvents(SyncEventsArgs {
            account,
            bootstrap,
            watch,
            interval,
            json,
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert!(bootstrap);
            assert!(watch);
            assert_eq!(interval, "5m");
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_list_filter() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "list",
        "inbox",
        "--filter",
        "DoorDash",
        "--unread",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::List(ListArgs {
            folder,
            filter,
            unread,
            read,
            starred,
            unstarred,
            json,
            ..
        }) => {
            assert_eq!(folder, "inbox");
            assert_eq!(filter.as_deref(), Some("DoorDash"));
            assert!(unread);
            assert!(!read);
            assert!(!starred);
            assert!(!unstarred);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_list_starred_filter() {
    let cli = MailCli::try_parse_from(["vivi-mail", "list", "--starred"]).unwrap();

    match cli.command {
        MailCommand::List(ListArgs {
            folder,
            starred,
            unstarred,
            ..
        }) => {
            assert_eq!(folder, "inbox");
            assert!(starred);
            assert!(!unstarred);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_list_flagged_alias() {
    let cli = MailCli::try_parse_from(["vivi-mail", "list", "--flagged"]).unwrap();

    match cli.command {
        MailCommand::List(ListArgs { starred, .. }) => assert!(starred),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_multiple_list_star_modes() {
    let err =
        MailCli::try_parse_from(["vivi-mail", "list", "--starred", "--unstarred"]).unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parses_doctor_json() {
    let cli =
        MailCli::try_parse_from(["vivi-mail", "doctor", "--account", "proton", "--json"]).unwrap();

    match cli.command {
        MailCommand::Doctor(DoctorArgs { account, json }) => {
            assert_eq!(account.as_deref(), Some("proton"));
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_proton_auth_info_json() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "proton",
        "auth-info",
        "--account",
        "agent",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Proton(ProtonArgs {
            command: ProtonCommand::AuthInfo { account, json },
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_proton_login_check_totp() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "proton",
        "login-check",
        "--account",
        "agent",
        "--totp-code",
        "123456",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Proton(ProtonArgs {
            command:
                ProtonCommand::LoginCheck {
                    account,
                    totp_code,
                    json,
                },
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert_eq!(totp_code.as_deref(), Some("123456"));
            assert!(!json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_proton_login_json() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "proton",
        "login",
        "--account",
        "agent",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Proton(ProtonArgs {
            command:
                ProtonCommand::Login {
                    account,
                    totp_code,
                    json,
                },
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert_eq!(totp_code, None);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_proton_identity_json() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "proton",
        "identity",
        "--account",
        "agent",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Proton(ProtonArgs {
            command: ProtonCommand::Identity { account, json },
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_proton_session_check_json() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "proton",
        "session-check",
        "--account",
        "agent",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Proton(ProtonArgs {
            command: ProtonCommand::SessionCheck { account, json },
        }) => {
            assert_eq!(account.as_deref(), Some("agent"));
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_default_compose_command() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "compose",
        "--to",
        "a@example.com",
        "--cc",
        "b@example.com",
        "--bcc",
        "c@example.com",
        "--subject",
        "hello",
        "--body",
        "body",
        "--append-remote",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Compose(ComposeCommand {
            to,
            cc,
            bcc,
            from,
            subject,
            body,
            html_body,
            html_body_auto,
            append_remote,
            ..
        }) => {
            assert_eq!(to, vec!["a@example.com"]);
            assert_eq!(cc, vec!["b@example.com"]);
            assert_eq!(bcc, vec!["c@example.com"]);
            assert_eq!(from, None);
            assert_eq!(subject, "hello");
            assert_eq!(body.as_deref(), Some("body"));
            assert_eq!(html_body, None);
            assert!(!html_body_auto);
            assert!(append_remote);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_compose_html_body_auto() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "compose",
        "--to",
        "a@example.com",
        "--subject",
        "hello",
        "--body",
        "body",
        "--html-body-auto",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Compose(ComposeCommand {
            html_body,
            html_body_auto,
            ..
        }) => {
            assert_eq!(html_body, None);
            assert!(html_body_auto);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_compose_from() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "compose",
        "--from",
        "Alias <alias@example.com>",
        "--to",
        "a@example.com",
        "--subject",
        "hello",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Compose(ComposeCommand { from, .. }) => {
            assert_eq!(from.as_deref(), Some("Alias <alias@example.com>"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_compose_html_body_auto_without_body() {
    let err = MailCli::try_parse_from([
        "vivi-mail",
        "compose",
        "--to",
        "a@example.com",
        "--subject",
        "hello",
        "--html-body-auto",
    ])
    .unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn parses_default_reply_command() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "reply",
        "handle-1",
        "--body",
        "thanks",
        "--append-remote",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Reply(ReplyCommand {
            handle,
            from,
            body,
            html_body,
            html_body_auto,
            append_remote,
        }) => {
            assert_eq!(handle, "handle-1");
            assert_eq!(from, None);
            assert_eq!(body.as_deref(), Some("thanks"));
            assert_eq!(html_body, None);
            assert!(!html_body_auto);
            assert!(append_remote);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_reply_html_body_and_auto_together() {
    let err = MailCli::try_parse_from([
        "vivi-mail",
        "reply",
        "handle-1",
        "--body",
        "thanks",
        "--html-body",
        "<p>thanks</p>",
        "--html-body-auto",
    ])
    .unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn rejects_multiple_exec_flag_modes() {
    let err =
        MailCli::try_parse_from(["vivi-mail", "exec", "flag", "abc123", "--read", "--unread"])
            .unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parses_enqueue_archive() {
    let cli = MailCli::try_parse_from(["vivi-mail", "enqueue", "archive", "handle-1"]).unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Archive { handles },
        }) => {
            assert_eq!(handles, vec!["handle-1"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_enqueue_archive_batch() {
    let cli = MailCli::try_parse_from(["vivi-mail", "enqueue", "archive", "one", "two"]).unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Archive { handles },
        }) => {
            assert_eq!(handles, vec!["one", "two"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_unknown_agent_subcommand() {
    let err = MailCli::try_parse_from(["vivi-mail", "agent", "bogus"]).unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn parses_agent_poll_defaults() {
    let cli = MailCli::try_parse_from(["vivi-mail", "agent", "poll", "--from", "ian@example.com"])
        .unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Poll {
                    from_addr,
                    folder,
                    dry_run,
                    json,
                    codex_command,
                    codex_args,
                },
        }) => {
            assert_eq!(from_addr, "ian@example.com");
            assert_eq!(folder, "inbox");
            assert!(!dry_run);
            assert!(!json);
            assert_eq!(codex_command, "codex");
            assert_eq!(codex_args, vec!["exec", "-"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_agent_archive_plan() {
    let cli = MailCli::try_parse_from(["vivi-mail", "agent", "archive", "one", "two"]).unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Archive {
                    handles,
                    execute,
                    json,
                },
        }) => {
            assert_eq!(handles, vec!["one", "two"]);
            assert!(!execute);
            assert!(!json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_agent_archive_execute() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "agent",
        "archive",
        "one",
        "--execute",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Archive {
                    handles,
                    execute,
                    json,
                },
        }) => {
            assert_eq!(handles, vec!["one"]);
            assert!(execute);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_agent_delete_execute() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "agent",
        "delete",
        "one",
        "two",
        "--expunge",
        "--confirm",
        "--execute",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Delete {
                    handles,
                    expunge,
                    confirm,
                    execute,
                    ..
                },
        }) => {
            assert_eq!(handles, vec!["one", "two"]);
            assert!(expunge);
            assert!(confirm);
            assert!(execute);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_agent_move_and_flag() {
    let cli =
        MailCli::try_parse_from(["vivi-mail", "agent", "move", "h", "trash", "--execute"]).unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Move {
                    handle,
                    folder,
                    execute,
                    ..
                },
        }) => {
            assert_eq!(handle, "h");
            assert_eq!(folder, "trash");
            assert!(execute);
        }
        other => panic!("unexpected command: {other:?}"),
    }

    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "agent",
        "flag",
        "h",
        "--star",
        "--execute",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Agent(AgentArgs {
            command:
                AgentCommand::Flag {
                    handle,
                    read,
                    unread,
                    star,
                    unstar,
                    execute,
                    json,
                },
        }) => {
            assert_eq!(handle, "h");
            assert!(star);
            assert!(!read);
            assert!(!unread);
            assert!(!unstar);
            assert!(execute);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_enqueue_delete_batch_expunge() {
    let cli =
        MailCli::try_parse_from(["vivi-mail", "enqueue", "delete", "one", "two", "--expunge"])
            .unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Delete {
                handles, expunge, ..
            },
        }) => {
            assert_eq!(handles, vec!["one", "two"]);
            assert!(expunge);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_enqueue_send() {
    let cli = MailCli::try_parse_from(["vivi-mail", "enqueue", "send", "draft.eml"]).unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Send { path, from },
        }) => {
            assert_eq!(path, PathBuf::from("draft.eml"));
            assert_eq!(from, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_enqueue_send_from() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "enqueue",
        "send",
        "draft.eml",
        "--from",
        "alias@example.com",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Send { path, from },
        }) => {
            assert_eq!(path, PathBuf::from("draft.eml"));
            assert_eq!(from.as_deref(), Some("alias@example.com"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_enqueue_reply_body() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "enqueue",
        "reply",
        "handle-1",
        "--body",
        "thanks",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Enqueue(EnqueueArgs {
            command: EnqueueCommand::Reply { handle, body },
        }) => {
            assert_eq!(handle, "handle-1");
            assert_eq!(body, "thanks");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_exec_archive_json() {
    let cli = MailCli::try_parse_from(["vivi-mail", "exec", "archive", "one", "--json"]).unwrap();

    match cli.command {
        MailCommand::Exec(ExecArgs {
            command: ExecCommand::Archive { handles, json },
        }) => {
            assert_eq!(handles, vec!["one"]);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_exec_delete_expunge_confirm() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "exec",
        "delete",
        "abc123",
        "def456",
        "--expunge",
        "--confirm",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Exec(ExecArgs {
            command:
                ExecCommand::Delete {
                    handles,
                    expunge,
                    confirm,
                    ..
                },
        }) => {
            assert_eq!(handles, vec!["abc123", "def456"]);
            assert!(expunge);
            assert!(confirm);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_exec_send() {
    let cli = MailCli::try_parse_from(["vivi-mail", "exec", "send", "message.eml"]).unwrap();

    match cli.command {
        MailCommand::Exec(ExecArgs {
            command: ExecCommand::Send { path, from },
        }) => {
            assert_eq!(path, PathBuf::from("message.eml"));
            assert_eq!(from, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_exec_send_from() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "exec",
        "send",
        "message.eml",
        "--from",
        "alias@example.com",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Exec(ExecArgs {
            command: ExecCommand::Send { path, from },
        }) => {
            assert_eq!(path, PathBuf::from("message.eml"));
            assert_eq!(from.as_deref(), Some("alias@example.com"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_removed_top_level_write_surface() {
    let err = MailCli::try_parse_from(["vivi-mail", "archive", "abc123"]).unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn parses_queue_run_all() {
    let cli = MailCli::try_parse_from(["vivi-mail", "queue", "run", "--all"]).unwrap();

    match cli.command {
        MailCommand::Queue(QueueArgs {
            command: QueueCommand::Run { ids, all },
        }) => {
            assert!(ids.is_empty());
            assert!(all);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_labels_json() {
    let cli = MailCli::try_parse_from(["vivi-mail", "labels", "--json"]).unwrap();

    match cli.command {
        MailCommand::Labels(LabelsArgs { json }) => assert!(json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_index_rebuild() {
    let cli = MailCli::try_parse_from(["vivi-mail", "index", "rebuild"]).unwrap();

    match cli.command {
        MailCommand::Index(IndexArgs {
            command: IndexCommand::Rebuild,
        }) => {}
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_index_embeddings_pending_limit() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "index",
        "embeddings",
        "--pending",
        "--limit",
        "2",
        "--provider",
        "ollama",
        "--model",
        "mail-embedding-model",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Index(IndexArgs {
            command:
                IndexCommand::Embeddings {
                    pending,
                    rebuild,
                    limit,
                    provider,
                    model,
                    endpoint,
                    ..
                },
        }) => {
            assert!(pending);
            assert!(!rebuild);
            assert_eq!(limit, Some(2));
            assert_eq!(provider.as_deref(), Some("ollama"));
            assert_eq!(model.as_deref(), Some("mail-embedding-model"));
            assert_eq!(endpoint, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_semantic_and_hybrid_search_flags() {
    let semantic = MailCli::try_parse_from(["vivi-mail", "search", "hello", "--semantic"]).unwrap();
    let hybrid = MailCli::try_parse_from(["vivi-mail", "search", "hello", "--hybrid"]).unwrap();

    match semantic.command {
        MailCommand::Search(SearchArgs {
            semantic, hybrid, ..
        }) => {
            assert!(semantic);
            assert!(!hybrid);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    match hybrid.command {
        MailCommand::Search(SearchArgs {
            semantic, hybrid, ..
        }) => {
            assert!(!semantic);
            assert!(hybrid);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_search_count_and_folder() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "search",
        "DoorDash",
        "--folder",
        "inbox",
        "--count",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Search(SearchArgs {
            query,
            folder,
            count,
            ..
        }) => {
            assert_eq!(query, "DoorDash");
            assert_eq!(folder.as_deref(), Some("inbox"));
            assert!(count);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_search_sender_filters() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "search",
        "invoice",
        "--from",
        "person@example.com",
        "--from-domain",
        "example.com",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Search(SearchArgs {
            from_addr,
            from_domain,
            ..
        }) => {
            assert_eq!(from_addr.as_deref(), Some("person@example.com"));
            assert_eq!(from_domain.as_deref(), Some("example.com"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_label_add_dry_run_json() {
    let cli = MailCli::try_parse_from([
        "vivi-mail",
        "label",
        "handle-1",
        "--add",
        "Work",
        "--dry-run",
        "--json",
    ])
    .unwrap();

    match cli.command {
        MailCommand::Label(LabelArgs {
            handle,
            add,
            remove,
            dry_run,
            json,
        }) => {
            assert_eq!(handle, "handle-1");
            assert_eq!(add.as_deref(), Some("Work"));
            assert!(remove.is_none());
            assert!(dry_run);
            assert!(json);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}
