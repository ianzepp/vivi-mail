# vivi-mail 10.0.0

The first release of `vivi-mail` as its own repository and its own binary.

Everything here ran inside the `vivarium` package until 9.4.0, when the crates
were separated, and inside the `vivarium` repository until 10.0.0. This release
is the repository extraction plus the cleanup that only became possible once
the halves stopped sharing a tree.

**The behavior of the mail commands is unchanged.** What changed is where they
live and what they are called.

## Why

`vivi-mail` owns everything that talks to a mail provider or to the local
archive: IMAP, SMTP, the direct Proton API, OAuth, sync, send, the review
queue, indexing, embeddings, search, threading, and the trusted agent mailbox.
That work shares no data and no runtime state with the project mailspace, which
now lives in [`ianzepp/vivi`](https://github.com/ianzepp/vivi).

As separate repositories, the two can move independently: the mail half can
take a transport or provider dependency without weighing down the mailspace
binary, and the mailspace can change its own storage without touching this one.

## The binary

`vivi-mail` is the CLI. Its commands are the ones that left the `vivi` binary:

`init`, `sync`, `sync-events`, `folders`, `doctor`, `proton`, `render`,
`watch-inbox`, `list`, `show`, `thread`, `reply`, `compose`, `export`, `search`,
`index`, `agent`, `exec`, `enqueue`, `queue`, `labels`, `label`.

`auth` and `token` are behind the `outbox` cargo feature and are **not** in the
released binary:

```sh
cargo install --path . --features outbox
```

Everything else in the released binary is built with default features.

Two things about the surface changed in the move, both deliberate:

- The global `--project` flag did not come along. Mail commands are
  account-scoped, never project-scoped, and no mail command read it.
- `init` now prints `vivi-mail is ready` and points at `vivi-mail sync` rather
  than at commands this binary does not have.

## What was removed

The split left this crate carrying code that belonged to the other half.

**The mailspace storage layer.** Storage served both halves before the split,
so this crate held a second, unused copy of it: six modules
(`backlog_graph`, `events`, `goals`, `graph`, `item_metadata`, `links`), their
DDL, `Storage::open_mailspace`, the `MailspaceMoveWithReply` and absorb surface
including the `absorbed_at`/`absorbed_by` columns, and three unreferenced
`deliver_raw_batch*` functions that carried the last write to `mailspace_events`.

A database this crate creates now holds `storage_metadata`, `blobs`,
`messages`, `remote_bindings`, and `message_metadata` — nothing else. Existing
databases keep their extra tables; they are ignored, and a test pins that one
still opens, ingests, reads, and counts.

**Dead code**, about 1,030 lines: the entire `outbox` module (the watcher it
exposed had no callers once the outbox surface left the CLI), the
`remote_reference` / `remote_reference_status` API, the unused `*_for_accounts`
handle helpers in storage, `read_blob`, `latest_message_from_addresses`,
`index_path`, and the `extract_attachments` stub that always returned an empty
list. The `outbox` cargo feature stays — it still gates `auth` and `token`.

Two behavior consequences, both intended: `mark_message_deleted` no longer
refuses to delete a message marked absorbed, since absorb was a mailspace
concept and the columns are gone; and the ingest path no longer writes
mailspace event rows into the storage database, which nothing read.

## New in this repository

- **Its own release workflow.** Three targets, one archive each
  (`vivi-mail-<target>.tar.gz`), published to the GitHub release with generated
  notes.
- **`install.sh`**, which selects the archive for the local platform and falls
  back to a source build.
- **A README** covering install, accounts, configuration, the on-disk layout,
  providers, the account mutation policy, and operations.
- **`VISION.md`**, which describes this half of the product. It arrived from
  `vivi` with a status note marking it as intent rather than description, since
  it predates the rewrite from a plain Maildir tree to content-addressed blobs
  with a SQLite index.
- **`docs/release-smoke-checks.md`**, the pre-release checklist for changes that
  touch transport, provider routing, sync, indexing, or outbound writes. It
  moved here because that is what it checks.

## Upgrading

Replace `vivi <mail-command>` with `vivi-mail <mail-command>`. The account
configuration at `~/.vivarium/`, the archive layout, and the SQLite schema are
unchanged, and no migration is needed. If you configured `[judgment]` in
`~/.vivarium/config.toml`, that table moved to each project's
`.vivi/mailspace.toml` and belongs to `vivi` now.
