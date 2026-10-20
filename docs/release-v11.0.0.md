# vivi-mail 11.0.0

Safety-review follow-up to 10.0.0. The CLI surface is the same; several
defaults are not.

**TLS certificate validation is now required for every non-loopback host, and
`sync --all` no longer copies All Mail, Trash, Spam, or other virtual
folders.** Both are intentional. They change what a previously working config
or flag will do.

This is the work from [PR #1](https://github.com/ianzepp/vivi-mail/pull/1).

## Breaking changes

### TLS: `--insecure` and `reject_invalid_certs = false` are loopback-only

Certificate validation is on for every account. `reject_invalid_certs = false`
in `config.toml` and the `--insecure` flag are honored only when **both** IMAP
and SMTP hosts resolve to loopback (`localhost`, `127.0.0.1`, `::1`). That is
the local Proton Bridge case, where the certificate is often untrusted on
purpose.

A remote mail host always validates. Setting the flag or the config key does
not turn validation off for it.

If you were using `--insecure` or `reject_invalid_certs = false` against a
remote server, that configuration will now fail to connect until the host
presents a valid certificate. Point those options at a local Bridge, or fix
the remote certificate.

### `sync --all` copies Inbox, Sent, and the real Archive folder only

`--all` no longer syncs All Mail, Trash, Spam, Gmail Important/Starred, or
other aggregate / virtual views. It copies Inbox, Sent, and the provider's
real Archive folder.

User-facing archive operations target that real Archive folder. A mutation
whose remote identity points at a virtual mailbox is refused, so an expunge
cannot hit the real message through All Mail.

If you were relying on `--all` to populate a local copy of All Mail or Trash,
those folders will no longer appear in a sync. Sync the real folders instead.

## Other behavior that is now the default

**Sync is read-only on the remote.** Sync, `watch-inbox`, and metadata listing
open folders with `EXAMINE` rather than `SELECT`, and full-body downloads use
`BODY.PEEK[]` / `BODY.PEEK[HEADER]`. A fetch cannot set `\Seen`. Flag, move,
expunge, and append still `SELECT` on explicit mutation paths, which is
correct.

**Queued send is at-most-once after intent is durable.** The RFC 5322
Message-ID (or a content hash) is recorded with status `sending` and fsynced
**before** the SMTP call. A later run never resends a `sending` or `executed`
item with that key. A failed SMTP call does not block a new enqueue. If the
process dies after recording intent and before SMTP, drop the stuck item and
re-enqueue only after confirming the message never arrived.

**Temp files, blobs, and SQLite sidecars are owner-only (mode 600).** Compose
and reply editor drafts go under the account's `.vivarium/tmp/` directory, not
the shared system temp dir. Blob writes are temp + fsync + rename; a truncated
or mismatched blob is rewritten on the next ingest. Opening a database chmods
the db and its `-wal` / `-shm` / `-journal` sidecars. Failed `password_cmd` /
`token_cmd` errors omit command output.

**Sync dedup keys include UIDVALIDITY.** Size keys are
`{role}-{uidvalidity}-{uid}`. A UIDVALIDITY change marks old `remote_bindings`
stale so a reused UID cannot skip new mail.

## Upgrading

Install 11.0.0 from the GitHub release assets, or build from this tag.

No on-disk schema migration is required. Existing archives open as before.

Two config checks before you rely on it:

1. If `~/.vivarium/config.toml` has `reject_invalid_certs = false`, keep it
   only for a local Bridge. It does nothing for remote hosts.
2. If a scheduled `sync --all` was meant to copy All Mail or Trash, it will
   not. Point it at Inbox / Sent / Archive, or at the specific folders you
   want.

```sh
VIVI_VERSION=v11.0.0
curl -fsSL "https://github.com/ianzepp/vivi-mail/releases/download/${VIVI_VERSION}/vivi-mail-$(uname -m | sed 's/arm64/aarch64/')-apple-darwin.tar.gz" |
  tar xz -C ~/.local/bin
```
