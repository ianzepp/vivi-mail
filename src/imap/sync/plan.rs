//! Decide which IMAP folders a sync run should cover.

use super::{DedupeScope, FolderPlan};
use crate::config::Account;
use crate::error::VivariumError;
use crate::imap::{is_unsafe_sync_mailbox, is_virtual_mailbox, mailbox_names_eq};

/// Which IMAP folders a sync run should cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncSelection {
    /// Inbox and Sent, matching 11.0.0 `vivi-mail sync` with no flags.
    Default,
    /// Inbox, Sent, and the provider's real Archive folder (`--all`).
    All,
    /// Explicit names from `--folder`/`--folders` or account `sync_folders`.
    Named(Vec<String>),
}

impl SyncSelection {
    /// Resolve CLI flags and optional account defaults.
    ///
    /// Precedence: `--folder`/`--folders` over `--all` over `sync_folders`
    /// over Inbox+Sent. `--all` and named folders are mutually exclusive.
    ///
    /// # Errors
    /// Returns an error if `--all` is combined with named folders, a name is
    /// empty, or `sync_folders` is present but empty.
    pub fn from_inputs(
        all: bool,
        named: &[String],
        account_defaults: Option<&[String]>,
    ) -> Result<Self, VivariumError> {
        let named = normalize_selector_names(named)?;
        if all && !named.is_empty() {
            return Err(VivariumError::Config(
                "--folder/--folders cannot be combined with --all".into(),
            ));
        }
        if !named.is_empty() {
            return Ok(Self::Named(named));
        }
        if all {
            return Ok(Self::All);
        }
        named_from_account_defaults(account_defaults)
    }

    /// Whether this selection is an explicit folder list (CLI or config).
    #[must_use]
    pub fn is_named(&self) -> bool {
        matches!(self, Self::Named(_))
    }
}

pub(super) fn plans_for(
    account: &Account,
    selection: &SyncSelection,
) -> Result<Vec<FolderPlan>, VivariumError> {
    match selection {
        SyncSelection::Default => Ok(default_plans(account)),
        SyncSelection::All => Ok(all_plans(account)),
        SyncSelection::Named(names) => named_plans(account, names),
    }
}

pub(super) fn inbox_plan(account: &Account) -> FolderPlan {
    FolderPlan {
        remote_folder: account.inbox_folder(),
        local_folder: "inbox",
        dedupe_scope: DedupeScope::LocalFolder,
    }
}

fn named_from_account_defaults(
    account_defaults: Option<&[String]>,
) -> Result<SyncSelection, VivariumError> {
    let Some(defaults) = account_defaults else {
        return Ok(SyncSelection::Default);
    };
    let defaults = normalize_selector_names(defaults)?;
    if defaults.is_empty() {
        return Err(VivariumError::Config(
            "sync_folders is empty; omit the key to use the default (inbox and sent)".into(),
        ));
    }
    Ok(SyncSelection::Named(defaults))
}

fn normalize_selector_names(names: &[String]) -> Result<Vec<String>, VivariumError> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(VivariumError::Config(
                "folder selector contains an empty name".into(),
            ));
        }
        if !out
            .iter()
            .any(|existing| mailbox_names_eq(existing, trimmed))
        {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

fn default_plans(account: &Account) -> Vec<FolderPlan> {
    vec![inbox_plan(account), sent_plan(account)]
}

fn all_plans(account: &Account) -> Vec<FolderPlan> {
    let mut folders = default_plans(account);
    let archive = archive_plan(account);
    if !is_unsafe_sync_mailbox(&archive.remote_folder) {
        folders.push(archive);
    }
    folders
}

fn named_plans(account: &Account, names: &[String]) -> Result<Vec<FolderPlan>, VivariumError> {
    let mut plans = Vec::new();
    for name in names {
        let plan = plan_for_name(account, name)?;
        refuse_virtual(&plan, name)?;
        if !already_selected(&plans, &plan) {
            plans.push(plan);
        }
    }
    Ok(plans)
}

fn already_selected(plans: &[FolderPlan], plan: &FolderPlan) -> bool {
    plans.iter().any(|existing| {
        existing.local_folder == plan.local_folder
            || mailbox_names_eq(&existing.remote_folder, &plan.remote_folder)
    })
}

fn plan_for_name(account: &Account, name: &str) -> Result<FolderPlan, VivariumError> {
    if is_virtual_mailbox(name) {
        return Err(virtual_sync_folder(name));
    }
    if let Some(plan) = logical_plan(account, name) {
        return Ok(plan);
    }
    resolved_plan(account, name).ok_or_else(|| unknown_sync_folder(name))
}

fn logical_plan(account: &Account, name: &str) -> Option<FolderPlan> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "inbox" => inbox_plan(account),
        "sent" => sent_plan(account),
        "archive" => archive_plan(account),
        "draft" | "drafts" => drafts_plan(account),
        "trash" | "deleted" => trash_plan(account),
        _ => return None,
    })
}

fn resolved_plan(account: &Account, name: &str) -> Option<FolderPlan> {
    [
        inbox_plan(account),
        sent_plan(account),
        archive_plan(account),
        drafts_plan(account),
        trash_plan(account),
    ]
    .into_iter()
    .find(|plan| mailbox_names_eq(name, &plan.remote_folder))
}

fn sent_plan(account: &Account) -> FolderPlan {
    FolderPlan {
        remote_folder: account.sent_folder(),
        local_folder: "sent",
        dedupe_scope: DedupeScope::LocalFolder,
    }
}

fn archive_plan(account: &Account) -> FolderPlan {
    FolderPlan {
        remote_folder: account.archive_folder(),
        local_folder: "archive",
        dedupe_scope: DedupeScope::AllFolders,
    }
}

fn drafts_plan(account: &Account) -> FolderPlan {
    FolderPlan {
        remote_folder: account.drafts_folder(),
        local_folder: "drafts",
        dedupe_scope: DedupeScope::LocalFolder,
    }
}

fn trash_plan(account: &Account) -> FolderPlan {
    FolderPlan {
        remote_folder: account.trash_folder(),
        local_folder: "trash",
        dedupe_scope: DedupeScope::LocalFolder,
    }
}

fn refuse_virtual(plan: &FolderPlan, name: &str) -> Result<(), VivariumError> {
    if is_virtual_mailbox(&plan.remote_folder) {
        return Err(virtual_sync_folder(name));
    }
    Ok(())
}

fn virtual_sync_folder(name: &str) -> VivariumError {
    VivariumError::Config(format!(
        "refusing to sync virtual mailbox '{name}'; All Mail, Spam, Important, and Starred are never a sync source. Use inbox, sent, archive, drafts, or trash"
    ))
}

fn unknown_sync_folder(name: &str) -> VivariumError {
    VivariumError::Config(format!(
        "unsupported sync folder '{name}'; expected inbox, sent, archive, drafts, trash, or the account's real IMAP names for those roles"
    ))
}
