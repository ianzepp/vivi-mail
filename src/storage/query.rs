use std::collections::HashMap;

use super::{
    CatalogEntry, OptionalExtension, Storage, StoredMessageView, VivariumError, fs, message_query,
    params, raw_stored_message_from_row,
};

impl Storage {
    /// Read the raw message bytes, resolving handles/prefixes first.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the token cannot be resolved, the
    /// message is not found, or the file read fails.
    pub fn read_message(&self, message_id: &str) -> Result<Vec<u8>, VivariumError> {
        let resolved = self.resolve_message_token(message_id)?;
        let Some(view) = self.message_by_id(&resolved)? else {
            return Err(VivariumError::Message(format!(
                "message not found: {message_id}"
            )));
        };
        fs::read(self.mail_root.join(view.blob_relpath)).map_err(Into::into)
    }

    /// Look up a stored message view by exact message ID.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query or handle
    /// resolution fails.
    pub fn message_by_id(
        &self,
        message_id: &str,
    ) -> Result<Option<StoredMessageView>, VivariumError> {
        let mut message = self
            .conn
            .query_row(
                &message_query("WHERE m.message_id = ?1"),
                params![message_id],
                raw_stored_message_from_row,
            )
            .optional()
            .map_err(|e| VivariumError::Other(format!("failed to read stored message: {e}")))?;
        if let Some(message) = &mut message {
            message.handle = self.display_handle(&message.message_id)?;
        }
        Ok(message)
    }

    /// List all messages in a given local role across all accounts.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query or handle
    /// decoration fails.
    pub fn list_messages_by_role(
        &self,
        local_role: &str,
    ) -> Result<Vec<StoredMessageView>, VivariumError> {
        self.list_messages_by_query(
            "WHERE m.local_role = ?1 AND m.deleted_at IS NULL",
            params![local_role],
        )
    }

    fn list_messages_by_query(
        &self,
        where_clause: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<StoredMessageView>, VivariumError> {
        let sql = format!(
            "{} ORDER BY md.date DESC, m.message_id",
            message_query(where_clause)
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| VivariumError::Other(format!("failed to prepare storage listing: {e}")))?;
        let rows = stmt
            .query_map(params, raw_stored_message_from_row)
            .map_err(|e| VivariumError::Other(format!("failed to list stored messages: {e}")))?;
        let messages: Result<Vec<_>, _> = rows
            .map(|row| {
                row.map_err(|e| {
                    VivariumError::Other(format!("failed to read stored message row: {e}"))
                })
            })
            .collect();
        self.decorate_handles(messages?)
    }

    /// List all non-deleted messages across all accounts.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query or handle
    /// decoration fails.
    pub fn list_messages(&self) -> Result<Vec<StoredMessageView>, VivariumError> {
        self.list_messages_by_query("WHERE m.deleted_at IS NULL", [])
    }

    /// List catalog entries for an account.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query fails.
    pub fn list_catalog_entries(&self, account: &str) -> Result<Vec<CatalogEntry>, VivariumError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "{} ORDER BY md.date DESC, m.message_id",
                message_query("WHERE m.account = ?1 AND m.deleted_at IS NULL")
            ))
            .map_err(|e| {
                VivariumError::Other(format!("failed to prepare catalog view listing: {e}"))
            })?;
        let rows = stmt
            .query_map(params![account], raw_stored_message_from_row)
            .map_err(|e| VivariumError::Other(format!("failed to query catalog view: {e}")))?;
        let messages: Result<Vec<_>, _> = rows
            .map(|row| {
                row.map_err(|e| {
                    VivariumError::Other(format!("failed to read catalog view row: {e}"))
                })
            })
            .collect();
        Ok(messages?
            .into_iter()
            .map(|message| self.catalog_entry_from_view(message))
            .collect())
    }

    /// Look up a single catalog entry by handle or message ID for an account.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query fails.
    pub fn catalog_entry(
        &self,
        account: &str,
        handle_or_id: &str,
    ) -> Result<Option<CatalogEntry>, VivariumError> {
        let Some(view) = self
            .conn
            .query_row(
                &format!(
                    "{} WHERE m.account = ?1 AND m.deleted_at IS NULL AND m.message_id = ?2",
                    message_query("")
                ),
                params![account, handle_or_id],
                raw_stored_message_from_row,
            )
            .optional()
            .map_err(|e| VivariumError::Other(format!("failed to read catalog entry: {e}")))?
        else {
            return Ok(None);
        };
        Ok(Some(self.catalog_entry_from_view(view)))
    }

    /// Count non-deleted messages for an account.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query fails.
    pub fn count_messages_for_account(&self, account: &str) -> Result<usize, VivariumError> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE account = ?1 AND deleted_at IS NULL",
                params![account],
                |row| row.get(0),
            )
            .map_err(|e| VivariumError::Other(format!("failed to count stored messages: {e}")))
    }

    /// Map of message identifier → byte size for a given local role.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query fails.
    pub fn local_sizes_by_role(
        &self,
        local_role: &str,
    ) -> Result<HashMap<String, u64>, VivariumError> {
        let messages = self.list_messages_by_role(local_role)?;
        Ok(messages
            .into_iter()
            .map(|message| {
                let key = message
                    .remote
                    .as_ref()
                    .map(|remote| format!("{local_role}-{}", remote.remote_uid))
                    .unwrap_or(message.message_id);
                (key, message.byte_size)
            })
            .collect())
    }

    /// Build an RFC-message-id → (`remote_uid`, `byte_size`) index for a role.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database query fails.
    pub fn rfc_index_by_role(
        &self,
        local_role: &str,
    ) -> Result<HashMap<String, (u32, u64)>, VivariumError> {
        let messages = self.list_messages_by_role(local_role)?;
        let mut map = HashMap::new();
        for message in messages {
            let Some(rfc_message_id) = message.normalized_message_id.clone() else {
                continue;
            };
            let uid = message
                .remote
                .as_ref()
                .map(|remote| remote.remote_uid)
                .or_else(|| {
                    message
                        .message_id
                        .rsplit_once('-')
                        .and_then(|(_, uid)| uid.parse().ok())
                })
                .unwrap_or(0);
            map.insert(rfc_message_id, (uid, message.byte_size));
        }
        Ok(map)
    }
}
