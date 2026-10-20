use self::download::download_missing;
use self::plan::{inbox_plan, plans_for};
use super::query::fetch_remote_messages;
use super::transport::RemoteMessage;
use crate::catalog::CatalogEntry;
use crate::config::{Account, Provider, StorageMode};
use crate::error::VivariumError;
use crate::storage::{MessageIngestRequest, RemoteBindingInput, Storage};
use crate::store::MailStore;
use crate::sync::{SyncResult, SyncWindow};

mod download;
mod plan;

pub use plan::SyncSelection;

#[derive(Debug)]
struct FolderPlan {
    remote_folder: String,
    local_folder: &'static str,
    dedupe_scope: DedupeScope,
}

struct SyncFolderRequest<'a> {
    account: &'a Account,
    store: &'a MailStore,
    plan: FolderPlan,
    reject_invalid_certs: bool,
    limit: Option<usize>,
    window: SyncWindow,
}

#[derive(Clone, Copy, Debug)]
enum DedupeScope {
    LocalFolder,
    AllFolders,
}

/// Sync messages from the account's IMAP server into the local store.
///
/// # Errors
/// Returns an error if the IMAP connection, folder listing, or message
/// download fails, or if the account uses an unsupported provider or
/// storage mode.
pub async fn sync_inbox_messages(
    account: &Account,
    store: &MailStore,
    reject_invalid_certs: bool,
    limit: Option<usize>,
    window: SyncWindow,
) -> Result<SyncResult, VivariumError> {
    sync_plans(
        account,
        store,
        reject_invalid_certs,
        limit,
        window,
        vec![inbox_plan(account)],
    )
    .await
}

/// Sync the folders chosen by `selection` into the local store.
///
/// # Errors
/// Returns an error if the IMAP connection, folder listing, or message
/// download fails, the selection cannot be resolved, or the account uses an
/// unsupported provider or storage mode.
pub async fn sync_messages(
    account: &Account,
    store: &MailStore,
    reject_invalid_certs: bool,
    limit: Option<usize>,
    window: SyncWindow,
    selection: &SyncSelection,
) -> Result<SyncResult, VivariumError> {
    sync_plans(
        account,
        store,
        reject_invalid_certs,
        limit,
        window,
        plans_for(account, selection)?,
    )
    .await
}

async fn sync_plans(
    account: &Account,
    store: &MailStore,
    reject_invalid_certs: bool,
    limit: Option<usize>,
    window: SyncWindow,
    plans: Vec<FolderPlan>,
) -> Result<SyncResult, VivariumError> {
    if matches!(account.provider, Provider::ProtonApi) {
        return Err(VivariumError::Config(format!(
            "account '{}' uses provider = \"proton-api\"; IMAP sync is not available for direct Proton API accounts yet",
            account.name
        )));
    }

    if matches!(account.resolved_storage_mode(), StorageMode::Proxy) {
        return Err(VivariumError::Config(format!(
            "account '{}' uses storage_mode = \"proxy\"; sync requires headers, bodies, or semantic storage",
            account.name
        )));
    }

    let mut result = SyncResult::default();
    for plan in plans {
        // `--limit` and `--since`/`--before` apply independently per folder.
        // Inbox cannot consume the cap and starve Sent.
        let r = sync_folder(SyncFolderRequest {
            account,
            store,
            plan,
            reject_invalid_certs,
            limit,
            window,
        })
        .await?;
        result.new += r.new;
        result.cataloged_entries.extend(r.cataloged_entries);
    }

    Ok(result)
}

/// Compare remote messages against local files and return missing ones.
fn find_missing(
    remote_messages: &[RemoteMessage],
    store: &MailStore,
    local_folder: &str,
    dedupe_scope: DedupeScope,
) -> Result<Vec<RemoteMessage>, VivariumError> {
    let local_sizes = store.local_sizes(local_folder)?;
    let rfc_index = match dedupe_scope {
        DedupeScope::LocalFolder => store.build_rfc_index(local_folder)?,
        DedupeScope::AllFolders => {
            let mut index = store.build_rfc_index(local_folder)?;
            for folder in ["inbox", "sent", "drafts"] {
                index.extend(store.build_rfc_index(folder)?);
            }
            index
        }
    };
    let mut missing: Vec<RemoteMessage> = Vec::new();

    for remote in remote_messages {
        if let Some(ref rfc_message_id) = remote.rfc_message_id
            && store.rfc_index_contains(&rfc_index, rfc_message_id)
        {
            continue;
        }

        let Some(uidvalidity) = remote.uidvalidity else {
            missing.push(remote.clone());
            continue;
        };
        let msg_id = crate::storage::remote_size_key(local_folder, uidvalidity, remote.uid);
        if let Some(local_size) = local_sizes.get(&msg_id)
            && *local_size == remote.size
        {
            continue;
        }
        missing.push(remote.clone());
    }

    Ok(missing)
}

/// Sync messages from the account's IMAP server into the local store.
async fn sync_folder(request: SyncFolderRequest<'_>) -> Result<SyncResult, VivariumError> {
    let remote_folder = request.plan.remote_folder.as_str();
    let local_folder = request.plan.local_folder;
    let remote_messages = fetch_remote_messages(
        request.account,
        remote_folder,
        request.reject_invalid_certs,
        request.window,
    )
    .await?;
    if remote_messages.is_empty() {
        return Ok(SyncResult::default());
    }
    let mut storage = Storage::open(request.store.root())?;
    refresh_remote_flags(
        &mut storage,
        &request.account.name,
        remote_folder,
        &remote_messages,
    )?;
    drop(storage);

    let mut missing = find_missing(
        &remote_messages,
        request.store,
        local_folder,
        request.plan.dedupe_scope,
    )?;
    if missing.is_empty() {
        tracing::info!(
            folder = remote_folder,
            total = remote_messages.len(),
            "all messages up to date"
        );
        return Ok(SyncResult::default());
    }
    let total_missing = missing.len();
    truncate_missing(&mut missing, request.limit);
    if missing.is_empty() {
        return Ok(SyncResult::default());
    }
    tracing::info!(
        folder = remote_folder,
        total = remote_messages.len(),
        missing = total_missing,
        downloading = missing.len(),
        "downloading new messages"
    );

    download_missing(
        request.account,
        request.store,
        remote_folder,
        local_folder,
        missing,
        request.reject_invalid_certs,
    )
    .await
}

fn truncate_missing(missing: &mut Vec<RemoteMessage>, limit: Option<usize>) {
    if let Some(limit) = limit {
        missing.truncate(limit);
    }
}

fn refresh_remote_flags(
    storage: &mut Storage,
    account: &str,
    remote_folder: &str,
    remote_messages: &[RemoteMessage],
) -> Result<(), VivariumError> {
    if let Some(uidvalidity) = remote_messages.iter().find_map(|remote| remote.uidvalidity) {
        storage.mark_stale_uidvalidity(account, remote_folder, uidvalidity)?;
    }
    for remote in remote_messages {
        let Some(uidvalidity) = remote.uidvalidity else {
            continue;
        };
        storage.update_remote_flags(
            account,
            remote_folder,
            uidvalidity,
            remote.uid,
            remote.read_state,
            remote.starred,
        )?;
    }
    Ok(())
}

/// Store a single parsed message in hash-addressed storage.
fn store_message(
    storage: &mut Storage,
    account: &Account,
    remote_folder: &str,
    local_folder: &str,
    body: &[u8],
    remote: &RemoteMessage,
) -> Result<CatalogEntry, VivariumError> {
    let uid = remote.uid;
    let stored = storage.ingest_message(
        &MessageIngestRequest {
            account: account.name.clone(),
            local_role: local_folder.to_string(),
            read_state: remote.read_state,
            starred: remote.starred,
            message_id_hint: None,
            seed_hint: format!("remote_uid:{uid}"),
            remote: remote.uidvalidity.map(|uidvalidity| RemoteBindingInput {
                account: account.name.clone(),
                provider: account.provider.to_string(),
                remote_mailbox: remote_folder.to_string(),
                remote_uid: uid,
                remote_uidvalidity: uidvalidity,
            }),
        },
        body,
    )?;
    storage
        .catalog_entry(&account.name, &stored.message_id)?
        .ok_or_else(|| {
            VivariumError::Other(format!(
                "stored message missing from catalog view: {}",
                stored.message_id
            ))
        })
}

fn uid_set_string(uids: &[u32]) -> String {
    uids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests;
