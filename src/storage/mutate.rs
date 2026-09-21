use super::{Storage, Utc, VivariumError, params};

impl Storage {
    /// Update the read and starred state of a message identified by its remote
    /// binding.
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database update fails.
    pub fn update_remote_flags(
        &mut self,
        account: &str,
        remote_mailbox: &str,
        remote_uidvalidity: u32,
        remote_uid: u32,
        read_state: bool,
        starred: bool,
    ) -> Result<bool, VivariumError> {
        let now = Utc::now().to_rfc3339();
        let changed = self
            .conn
            .execute(
                "UPDATE messages
                 SET read_state = ?5, starred = ?6, updated_at = ?7
                 WHERE deleted_at IS NULL
                   AND message_id = (
                       SELECT message_id FROM remote_bindings
                       WHERE account = ?1
                         AND remote_mailbox = ?2
                         AND remote_uidvalidity = ?3
                         AND remote_uid = ?4
                   )",
                params![
                    account,
                    remote_mailbox,
                    remote_uidvalidity,
                    remote_uid,
                    i32::from(read_state),
                    i32::from(starred),
                    now
                ],
            )
            .map_err(|e| VivariumError::Other(format!("failed to update message flags: {e}")))?;
        if changed > 0 {
            self.invalidate_handle_cache();
        }
        Ok(changed > 0)
    }

    /// Mark a message as deleted (soft delete via `deleted_at` timestamp).
    ///
    /// # Errors
    /// Returns a [`VivariumError`] if the database update fails.
    pub fn mark_message_deleted(
        &mut self,
        account: &str,
        message_id: &str,
    ) -> Result<bool, VivariumError> {
        let now = Utc::now().to_rfc3339();
        let changed = self
            .conn
            .execute(
                "UPDATE messages
                 SET deleted_at = ?3, updated_at = ?3
                 WHERE account = ?1 AND message_id = ?2 AND deleted_at IS NULL",
                params![account, message_id, now],
            )
            .map_err(|e| VivariumError::Other(format!("failed to mark message deleted: {e}")))?;
        if changed > 0 {
            self.invalidate_handle_cache();
        }
        Ok(changed > 0)
    }
}
