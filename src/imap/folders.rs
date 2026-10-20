use async_imap::types::{Capability, NameAttribute};
use futures::TryStreamExt;
use serde::Serialize;

use super::transport::connect;
use crate::config::Account;
use crate::error::VivariumError;

#[derive(Debug, Clone, Serialize)]
pub struct FolderDiscovery {
    pub account: String,
    pub provider: String,
    pub resolved: ResolvedFolders,
    pub capabilities: CapabilityReport,
    pub folders: Vec<RemoteFolder>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResolvedFolders {
    pub inbox: String,
    pub archive: String,
    pub trash: String,
    pub sent: String,
    pub drafts: String,
    pub label_roots: Vec<String>,
}

/// IMAP capabilities discovered during connection.
///
/// Each boolean represents whether the server advertised support for a
/// particular extension.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CapabilityReport {
    pub uidplus: bool,
    pub move_supported: bool,
    pub special_use: bool,
    pub append: bool,
    pub idle: bool,
    pub raw: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RemoteFolder {
    pub name: String,
    pub delimiter: Option<String>,
    pub attributes: Vec<String>,
}

/// Discover folders and capabilities from the account's IMAP server.
///
/// # Errors
/// Returns an error if the IMAP connection, capability check, or folder
/// listing fails.
pub async fn discover_folders(
    account: &Account,
    reject_invalid_certs: bool,
) -> Result<FolderDiscovery, VivariumError> {
    let mut session = connect(account, reject_invalid_certs).await?;
    let caps = session
        .capabilities()
        .await
        .map_err(|e| VivariumError::Imap(format!("CAPABILITY failed: {e}")))?;
    let capabilities = capability_report(&caps);
    let names = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|e| VivariumError::Imap(format!("LIST failed: {e}")))?;
    let folders = names
        .map_ok(|name| RemoteFolder {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(str::to_string),
            attributes: name.attributes().iter().map(attribute_name).collect(),
        })
        .try_collect()
        .await
        .map_err(|e| VivariumError::Imap(format!("LIST stream failed: {e}")))?;
    session.logout().await.ok();

    Ok(FolderDiscovery {
        account: account.name.clone(),
        provider: account.provider.to_string(),
        resolved: resolved_folders(account),
        capabilities,
        folders,
    })
}

#[must_use]
pub fn resolved_folders(account: &Account) -> ResolvedFolders {
    ResolvedFolders {
        inbox: account.inbox_folder(),
        archive: account.archive_folder(),
        trash: account.trash_folder(),
        sent: account.sent_folder(),
        drafts: account.drafts_folder(),
        label_roots: account.label_roots(),
    }
}

/// Virtual or aggregate IMAP views that must never be used as a sync source
/// or as a mutation source. Expunging All Mail deletes the real message.
#[must_use]
pub fn is_virtual_mailbox(name: &str) -> bool {
    matches!(
        normalize_mailbox_name(name).as_str(),
        "all mail"
            | "[gmail]/all mail"
            | "[gmail]/important"
            | "[gmail]/starred"
            | "spam"
            | "junk"
            | "[gmail]/spam"
    )
}

/// Folders that `sync --all` must not copy into the local archive.
#[must_use]
pub fn is_unsafe_sync_mailbox(name: &str) -> bool {
    is_virtual_mailbox(name)
        || matches!(
            normalize_mailbox_name(name).as_str(),
            "trash" | "deleted" | "deleted messages" | "[gmail]/trash"
        )
}

fn normalize_mailbox_name(name: &str) -> String {
    name.trim().trim_matches('"').to_ascii_lowercase()
}

fn capability_report(caps: &async_imap::types::Capabilities) -> CapabilityReport {
    let mut raw = caps.iter().map(capability_name).collect::<Vec<_>>();
    raw.sort();
    CapabilityReport {
        uidplus: caps.has_str("UIDPLUS"),
        move_supported: caps.has_str("MOVE"),
        special_use: caps.has_str("SPECIAL-USE"),
        append: caps.has_str("APPEND") || caps.has_str("IMAP4rev1"),
        idle: caps.has_str("IDLE"),
        raw,
    }
}

fn capability_name(capability: &Capability) -> String {
    match capability {
        Capability::Imap4rev1 => "IMAP4rev1".into(),
        Capability::Auth(value) => format!("AUTH={value}"),
        Capability::Atom(value) => value.clone(),
    }
}

fn attribute_name(attribute: &NameAttribute<'_>) -> String {
    format!("{attribute:?}")
}

#[cfg(test)]
#[path = "folders_test.rs"]
mod tests;
