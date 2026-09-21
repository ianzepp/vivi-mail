# vivi-mail

Local-first email archive, retrieval, and write layer for private agents: IMAP
and SMTP, the direct Proton API, sync, send, local indexing, semantic search,
drafts, and the trusted agent mailbox. The CLI binary is `vivi-mail`.

`vivi-mail` is one of three sibling repositories, split from a single project on
2026-09-21:

| Repo | Owns |
| --- | --- |
| [`vivi`](https://github.com/ianzepp/vivi) | Project mailspaces, roles, goals, work graphs, the `vivi` binary |
| [`vivi-mail`](https://github.com/ianzepp/vivi-mail) (this repo) | Email transport, sync, the local archive, search, drafts, and the `~/.vivarium/` home |
| [`vivi-pty`](https://github.com/ianzepp/vivi-pty) | The project-scoped PTY runtime adapter |

They share no code and no data. This repo owns the mail root; project
coordination state under `<project>/.vivi/` belongs to `vivi`.

## Why

Local agents need access to email, and the tools built for humans carry decades
of assumptions about interactive clients. `vivi-mail` keeps the important part
simple: raw RFC 5322 bytes stay on disk as immutable `.eml` blobs, while mutable
mailbox state and derived indexes live in SQLite beside them.

It is designed for isolated agent containers. A container can be initialized
with a Proton username plus a password or `password_cmd`, run
`vivi-mail proton login`, and then sync or send directly through Proton's API
with no Bridge setup, no generated Bridge passwords, and no shared Bridge state.
That path follows Proton's internal API shape rather than a stable public
contract, so Bridge remains the conservative compatibility option.

[VISION.md](VISION.md) holds the original vision statement for this half of the
project, kept as intent rather than as a description of the shipped tool.

## Install

Release binaries are published as assets on this repository's GitHub releases,
which is the only distribution channel — there is no package-manager formula.
Each archive contains the `vivi-mail` binary:

- `vivi-mail-aarch64-apple-darwin.tar.gz`
- `vivi-mail-x86_64-apple-darwin.tar.gz`
- `vivi-mail-x86_64-unknown-linux-gnu.tar.gz`

```sh
VIVI_VERSION=v10.0.0
curl -fsSL "https://github.com/ianzepp/vivi-mail/releases/download/${VIVI_VERSION}/vivi-mail-$(uname -m | sed 's/arm64/aarch64/')-apple-darwin.tar.gz" |
  tar xz -C ~/.local/bin
```

Linux `aarch64` has no binary archive; build from source instead.

From source, requires Rust 1.93+:

```sh
git clone https://github.com/ianzepp/vivi-mail.git
cd vivi-mail
cargo install --path .
```

The `outbox` feature is not in the released binary. To get `auth` and `token`,
build with `cargo install --path . --features outbox`.

## Quick Start

```sh
vivi-mail init
```

This creates `~/.vivarium/` with:

- `config.toml` — general settings such as mail root and TLS policy
- `accounts.toml` — account credentials, created with mode `600`
- `agent/prompt.md` — default prompt for `vivi-mail agent poll`

Edit `accounts.toml` to add an account, then verify and sync:

```sh
vivi-mail doctor --account proton          # check config, IMAP, and SMTP
vivi-mail sync --account proton --limit 100
vivi-mail list -n 25
```

Semantic embedding settings are never guessed. To use `storage_mode =
"semantic"`, `sync --embed`, or semantic search, name your own embedding service
in `config.toml` or pass the options on the explicit index command.

## Commands

`vivi-mail --help` is the live top-level list: `init`, `sync`, `sync-events`,
`folders`, `doctor`, `proton`, `render`, `watch-inbox`, `list`, `show`, `thread`,
`reply`, `compose`, `export`, `search`, `index`, `agent`, `exec`, `enqueue`,
`queue`, `labels`, `label`. With `--features outbox` it also has `auth` and
`token`.

Every command accepts `--account <name>`; account-scoped commands use the first
account in `accounts.toml` when it is omitted, and `sync` and `list` operate on
all accounts.

```
vivi-mail init                                 # create the config directory and files
vivi-mail sync                                 # sync all accounts
vivi-mail sync --account proton --since 3mo    # bound the window
vivi-mail sync --account proton --json         # machine-readable summary
vivi-mail sync --account proton --reset        # drop the local cache, full resync
vivi-mail sync-events --account agent-proton --watch --json
vivi-mail folders --account proton --json      # list remote IMAP folders
vivi-mail list sent                            # list a folder
vivi-mail list --filter DoorDash               # match handle, sender, or subject
vivi-mail show 4f8c2d1 --json                  # read one message with citation metadata
vivi-mail thread 4f8c2d1 --json                # local thread context
vivi-mail export 4f8c2d1 > message.eml         # raw RFC 5322 bytes
vivi-mail search "invoice" --folder inbox --count
vivi-mail index rebuild --account proton
vivi-mail doctor --account proton
vivi-mail render report.md --output report.pdf
vivi-mail watch-inbox --account proton --json  # inbound-only IMAP event source
vivi-mail agent poll --from person@example.com --json
```

Write commands are split by effect. `vivi-mail exec ...` performs the external
write now. `vivi-mail enqueue ...` records a durable pending item under the
selected account, and `vivi-mail queue run ...` is the explicit later execution
step.

`compose` and `reply` can build multipart drafts with both plain text and HTML:
`--html-body <html>` for explicit HTML, or `--html-body-auto` with `--body` to
generate a simple styled alternative. Drafts stay local until you run
`vivi-mail exec send path/to/draft.eml`.

## Configuration

`~/.vivarium/config.toml` holds general settings:

```toml
[defaults]
mail_root = "~/.vivarium"
reject_invalid_certs = true
check_interval_secs = 300
# embedding_provider = "ollama"
# embedding_model = "your-embedding-model"
# embedding_endpoint = "http://your-embedding-host/api/embed"
```

`~/.vivarium/accounts.toml` holds the accounts. Proton Bridge:

```toml
[[accounts]]
name = "proton"
email = "you@proton.me"
username = "you@proton.me"
auth = "password"
password = "your-bridge-app-password"
provider = "protonmail"
storage_mode = "headers"   # proxy | headers | bodies | semantic
```

Direct Proton API, for containers with no Bridge:

```toml
[[accounts]]
name = "agent-proton"
email = "agent@proton.me"
username = "agent@proton.me"
auth = "password"
password_cmd = "printenv PROTON_PASSWORD"
provider = "proton-api"
storage_mode = "semantic"
```

`VIVI_HOME` overrides the config directory. Legacy locations are still read when
they exist: `~/.config/vivarium/` for config and `~/.local/share/vivarium/` for
the mail root.

## Storage Layout

The mail root defaults to `~/.vivarium/`. Each account lives under
`<mail_root>/<account>/`:

```
~/.vivarium/
├── config.toml
├── accounts.toml              # mode 600
├── agent/
│   ├── prompt.md
│   └── prompts/<account>.md
└── proton/
    ├── blobs/
    │   └── ab/cd/<content_id>.eml
    └── .vivarium/
        ├── storage.sqlite     # message rows, remote bindings, flags, metadata
        ├── embeddings/        # provider- and model-scoped semantic indexes
        ├── proton/            # encrypted-payload cache for direct API accounts
        └── proton-events.json
```

Rules:

- `blobs/` is the immutable content store and the raw-message source of truth.
  A blob is never rewritten; mailbox state lives in the database beside it.
- `.vivarium/storage.sqlite` holds mutable state. Role, flags, and remote
  bindings can change without touching a blob.
- `.vivarium/embeddings/` holds semantic indexes and is disposable.
- Message handles shown by the CLI are short prefixes of `vivi-mail`-local
  `message_id` values. They are stable within a local cache and are deliberately
  not folder-and-UID identifiers.

## Providers

Provider differences are handled at the account boundary:

| Provider | `provider =` | Read source | Send source |
| --- | --- | --- | --- |
| Direct Proton API | `"proton-api"` | Proton API | Proton API |
| Proton Bridge | `"protonmail"` | Bridge IMAP | Bridge SMTP |
| Gmail | `"gmail"` | Gmail IMAP labels | SMTP |
| Standard | `"standard"` | IMAP folders | SMTP |

Bridge-backed Gmail and ProtonMail use their provider's `All Mail` view only as
an internal sync source for the local `Archive/` corpus; user-facing archive
operations target the provider's real `Archive` folder. Standard IMAP accounts
sync `INBOX` and `Sent` directly. Direct Proton API accounts map Proton labels
and message state into the same local roles without IMAP.

## Account Mutation Policy

Each account can declare a mutation `policy` that controls which remote side
effects it is authorized to perform, independent of command names or queue
provenance.

| Policy | `policy =` | Permitted remote operations |
| --- | --- | --- |
| Full-write (default) | `full-write` | Archive, move, trash, delete, expunge, flag, send |
| Read-only | `read-only` | Sync, read, search, show |
| Archive | `archive` | Archive, non-trash moves, flags; denies trash, delete, expunge, send |

```toml
[[accounts]]
name = "vault"
email = "vault@proton.me"
policy = "read-only"
```

Policy is enforced at enqueue admission and authoritatively during queue
execution, so a stale or hand-written queued item cannot bypass it. Folder
aliases are normalized first: `trash`, `deleted`, and provider-specific trash
folder names all classify as a denied move-to-trash under `read-only` and
`archive`. Check the effective policy with `vivi-mail doctor --account <name>`.

## Security

- `accounts.toml` is created with `chmod 600` and checked on load.
- Group- or world-readable `accounts.toml` is rejected unless
  `--ignore-permissions` is set.
- `password_cmd` is supported as an alternative to a plaintext password:
  ```toml
  password_cmd = "security find-generic-password -s vivarium -a you@proton.me -w"
  ```
- XOAUTH2 is supported with `auth = "xoauth2"` and `token_cmd`, which must print
  a current OAuth access token:
  ```toml
  auth = "xoauth2"
  token_cmd = "security find-generic-password -s gmail-access-token -w"
  ```
- Certificate validation is on for `provider = "protonmail"` by default. Set
  `reject_invalid_certs = false` on an account, or pass `--insecure` for a
  single run, when a local Bridge uses an untrusted certificate.
- Direct Proton API sessions are stored under the account's private state
  directory and refresh without reusing the account password on every command.
- Direct Proton encrypted-payload caches are account-local private artifacts. Do
  not publish or package them in release artifacts.
- `[judgment]` config is **not** here. It moved to the consuming project's
  `.vivi/mailspace.toml` and belongs to `vivi`.

## Local Operations

For a scheduled refresh, run a bounded sync from launchd, cron, or a similar
user-level scheduler:

```sh
vivi-mail sync --account proton --since 3mo
```

For a maintenance pass that refreshes derived state without downloading a batch:

```sh
vivi-mail sync --account proton --limit 0
```

The normal repair path is a clean reset:

```sh
vivi-mail sync --account <name> --reset
```

That clears the local cache for the account and re-downloads and re-indexes from
the account's remote source of truth: the Proton API for `provider =
"proton-api"`, IMAP otherwise. When deterministic search or thread state drifts
without needing a full reset, use `vivi-mail index rebuild --account <name>`.

## Architecture

- **Raw `.eml` blobs are the source of truth.** They are preserved unchanged
  under `blobs/`, addressed by content hash.
- **Mutable mailbox state lives beside them in `storage.sqlite`.** Local role,
  flags, and remote bindings never rename a blob.
- **Remote access is provider-scoped.** Direct Proton API accounts bypass Bridge
  entirely; Bridge, Gmail, and standard accounts use IMAP and SMTP.
- **Derived data is disposable and rebuildable.** Indexes and embeddings can be
  rebuilt from blobs plus storage metadata.
- **Search results point back to stable local content.** JSON output carries the
  short handle, the internal `message_id`, and `content_id` citation data.
- **Full corpus contents never leave the machine by default.** Any cloud access
  is explicit, narrow, and user-approved.
- **Writes have one gate.** `exec` performs them now; `enqueue` and `queue`
  defer them, and the account policy bounds both.

## License

MIT
