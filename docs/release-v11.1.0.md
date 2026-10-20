# vivi-mail 11.1.0

Folder selection for IMAP `sync`. The 11.0.0 safety defaults stay: TLS
validation on non-loopback hosts, `EXAMINE` / `BODY.PEEK[]`, and virtual
mailboxes are never a sync source.

This is the work from [PR #4](https://github.com/ianzepp/vivi-mail/pull/4).

A plain `vivi-mail sync --account X` is still Inbox and Sent. What is new is
the ability to name a different set, persist that set per account, and the
way `--limit` is applied.

## Folder selection

IMAP `sync` can cover a chosen folder set instead of always pulling Inbox and
Sent. That is the Proton Bridge case where a plain sync downloaded hundreds of
Sent messages you did not want yet.

### CLI

`--folder` is repeatable. `--folders` is a visible alias. Both accept a
comma-separated list, so `--folder inbox,sent` works the same as
`--folders inbox,sent`. `--folder` / `--folders` and `--all` are mutually
exclusive.

```sh
vivi-mail sync --account proton                      # Inbox + Sent (unchanged default)
vivi-mail sync --account proton --folder inbox       # Inbox only
vivi-mail sync --account proton --folder inbox --folder sent
vivi-mail sync --account proton --folders inbox,sent --limit 50
vivi-mail sync --account proton --all                # Inbox + Sent + real Archive
```

Logical names: `inbox`, `sent`, `archive`, `drafts`, `trash` (plus `draft` /
`deleted` aliases). Real IMAP names for those roles also work (`INBOX`,
`[Gmail]/Sent Mail`, …). Arbitrary IMAP folders without a local role (`Lists`,
label trees) are rejected; only those five roles map into the local store.

### Config

Optional per-account default, used only when no `--folder` / `--all` is given.
Existing `accounts.toml` files keep parsing. Unset `sync_folders` keeps Inbox
and Sent.

```toml
[[accounts]]
name = "proton"
provider = "protonmail"
sync_folders = ["inbox"]
```

### Precedence

1. `--folder` / `--folders` (explicit run list)
2. `--all` (Inbox + Sent + real Archive; never All Mail / Trash / Spam)
3. account `sync_folders`
4. default Inbox + Sent

`--folder` wins over `--all`, which wins over `sync_folders`, which wins over
Inbox + Sent.

## Behavior change: `--limit` is per folder

`--limit N` is now a **per-folder** cap, not a remaining budget shared across
folders. Inbox cannot consume the whole cap and leave Sent empty.

`sync --folder inbox --folder sent --limit 10` can fetch 10 from Inbox and 10
from Sent.

`--since` / `--before` already ran per folder (IMAP `SEARCH SINCE` / `BEFORE`);
help text now says so.

`--limit 0` still skips downloads for the whole account (a maintenance pass).

Different per-folder caps in one run (`inbox 100`, `sent 10`) are two runs:
`--folder inbox --limit 100` then `--folder sent --limit 10`.

## What is refused

**Virtual views are always refused**, even if named: All Mail, Gmail
Important / Starred, Spam. `--all` and the default never include them either.

**Trash and Drafts are included only when explicitly named** on `--folder` /
`--folders` or in `sync_folders`. `--all` still skips them.

`--reset` remains account-wide and cannot be combined with `--folder` /
`--folders` (or `--limit` / `--since` / `--before`). `--reset` plus a
`sync_folders` default still wipes the whole cache, then only re-downloads the
configured folders.

Direct Proton API accounts are label-based: they ignore `--all` (as in 11.0.0)
and **reject** `--folder` / `sync_folders` so a selector cannot silently no-op.
`watch-inbox` is still inbox-only.

## Upgrading

Install 11.1.0 from the GitHub release assets, or build from this tag.

No on-disk schema migration is required. Existing archives open as before.
Existing `accounts.toml` files do not need a `sync_folders` key.

If a scheduled `sync --limit N` across Inbox and Sent was treating `N` as a
shared budget, it will now fetch up to `N` from **each** selected folder.
Lower the limit, or name one folder, if that is more than you wanted.

```sh
VIVI_VERSION=v11.1.0
curl -fsSL "https://github.com/ianzepp/vivi-mail/releases/download/${VIVI_VERSION}/vivi-mail-$(uname -m | sed 's/arm64/aarch64/')-apple-darwin.tar.gz" |
  tar xz -C ~/.local/bin
```
