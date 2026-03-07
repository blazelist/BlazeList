//! Write operations: push and delete inner logic, sequence stamping.

use blazelist_protocol::{Card, DeletedEntity, Entity, NonNegativeI64, Tag, ZERO_HASH};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::storage::error::{PushError, PushOpError, StorageError};

use super::SqliteStorage;
use super::helpers::{CARD_COLS, TAG_COLS};

impl SqliteStorage {
    /// Core card push logic on an existing connection/transaction.
    /// Does NOT call recompute_root, stamp root_sequence_at, or commit.
    pub(super) fn push_card_versions_inner(
        conn: &Connection,
        versions: &[Card],
    ) -> Result<(), PushOpError> {
        if versions.is_empty() {
            return Err(PushError::EmptyChain.into());
        }

        let card_id = versions[0].id();
        for v in versions {
            if !v.verify() {
                return Err(PushError::HashVerificationFailed.into());
            }
        }

        if Self::is_deleted(conn, card_id)? {
            return Err(PushError::AlreadyDeleted.into());
        }

        let current_hash: Option<Vec<u8>> = conn
            .query_row(
                "SELECT hash FROM cards WHERE id = ?1",
                params![card_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .ok();

        let expected_ancestor = match &current_hash {
            Some(bytes) => Self::hash_from_bytes(bytes),
            None => ZERO_HASH,
        };

        if versions[0].ancestor_hash() != expected_ancestor {
            if current_hash.is_some() {
                let latest = conn.query_row(
                    &format!("SELECT {CARD_COLS} FROM cards WHERE id = ?1"),
                    params![card_id.as_bytes().as_slice()],
                    Self::card_from_row,
                )?;
                return Err(PushError::CardAncestorMismatch(Box::new(latest)).into());
            }
            return Err(PushError::HashVerificationFailed.into());
        }

        for w in versions.windows(2) {
            if w[1].ancestor_hash() != w[0].hash() {
                return Err(PushError::HashVerificationFailed.into());
            }
        }

        for v in versions {
            let tags_bytes = Self::serialize_tags(v.tags());
            conn.execute(
                "INSERT INTO card_versions (card_id, count, content, priority, \
                 tags, blazed, created_at, modified_at, due_date, ancestor_hash, hash) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    v.id().as_bytes().as_slice(),
                    i64::from(v.count()),
                    v.content(),
                    i64::from(v.priority()),
                    tags_bytes,
                    v.blazed(),
                    v.created_at().timestamp_millis(),
                    v.modified_at().timestamp_millis(),
                    v.due_date().map(|d| d.timestamp_millis()),
                    v.ancestor_hash().as_bytes().as_slice(),
                    v.hash().as_bytes().as_slice(),
                ],
            )?;
        }

        let latest = versions.last().unwrap();
        let dup: Option<(Vec<u8>, i64)> = conn
            .query_row(
                "SELECT id, priority FROM cards WHERE priority = ?1 AND id != ?2 LIMIT 1",
                params![
                    i64::from(latest.priority()),
                    latest.id().as_bytes().as_slice()
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((id_bytes, priority_raw)) = dup {
            let conflicting_id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
            let priority = NonNegativeI64::try_from(priority_raw).unwrap();
            return Err(PushError::DuplicatePriority {
                conflicting_id,
                priority,
            }
            .into());
        }

        let tags_bytes = Self::serialize_tags(latest.tags());
        conn.execute(
            "INSERT OR REPLACE INTO cards (id, content, priority, tags, blazed, \
             created_at, modified_at, due_date, count, ancestor_hash, hash, bucket) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                latest.id().as_bytes().as_slice(),
                latest.content(),
                i64::from(latest.priority()),
                tags_bytes,
                latest.blazed(),
                latest.created_at().timestamp_millis(),
                latest.modified_at().timestamp_millis(),
                latest.due_date().map(|d| d.timestamp_millis()),
                i64::from(latest.count()),
                latest.ancestor_hash().as_bytes().as_slice(),
                latest.hash().as_bytes().as_slice(),
                Self::bucket_of(latest.id()) as i64,
            ],
        )?;

        Ok(())
    }

    /// Core tag push logic on an existing connection/transaction.
    /// Does NOT call recompute_root, stamp root_sequence_at, or commit.
    pub(super) fn push_tag_versions_inner(
        conn: &Connection,
        versions: &[Tag],
    ) -> Result<(), PushOpError> {
        if versions.is_empty() {
            return Err(PushError::EmptyChain.into());
        }

        let tag_id = versions[0].id();
        for v in versions {
            if !v.verify() {
                return Err(PushError::HashVerificationFailed.into());
            }
        }

        if Self::is_deleted(conn, tag_id)? {
            return Err(PushError::AlreadyDeleted.into());
        }

        let current_hash: Option<Vec<u8>> = conn
            .query_row(
                "SELECT hash FROM tags WHERE id = ?1",
                params![tag_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .ok();

        let expected_ancestor = match &current_hash {
            Some(bytes) => Self::hash_from_bytes(bytes),
            None => ZERO_HASH,
        };

        if versions[0].ancestor_hash() != expected_ancestor {
            if current_hash.is_some() {
                let latest = conn.query_row(
                    &format!("SELECT {TAG_COLS} FROM tags WHERE id = ?1"),
                    params![tag_id.as_bytes().as_slice()],
                    Self::tag_from_row,
                )?;
                return Err(PushError::TagAncestorMismatch(Box::new(latest)).into());
            }
            return Err(PushError::HashVerificationFailed.into());
        }

        for w in versions.windows(2) {
            if w[1].ancestor_hash() != w[0].hash() {
                return Err(PushError::HashVerificationFailed.into());
            }
        }

        for v in versions {
            conn.execute(
                "INSERT INTO tag_versions (tag_id, count, title, color, created_at, modified_at, \
                 ancestor_hash, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    v.id().as_bytes().as_slice(),
                    i64::from(v.count()),
                    v.title(),
                    v.color().map(|c| vec![c.r, c.g, c.b]),
                    v.created_at().timestamp_millis(),
                    v.modified_at().timestamp_millis(),
                    v.ancestor_hash().as_bytes().as_slice(),
                    v.hash().as_bytes().as_slice(),
                ],
            )?;
        }

        let latest = versions.last().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tags (id, title, color, created_at, modified_at, count, \
             ancestor_hash, hash, bucket) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                latest.id().as_bytes().as_slice(),
                latest.title(),
                latest.color().map(|c| vec![c.r, c.g, c.b]),
                latest.created_at().timestamp_millis(),
                latest.modified_at().timestamp_millis(),
                i64::from(latest.count()),
                latest.ancestor_hash().as_bytes().as_slice(),
                latest.hash().as_bytes().as_slice(),
                Self::bucket_of(latest.id()) as i64,
            ],
        )?;

        Ok(())
    }

    /// Core card deletion on an existing connection/transaction.
    /// Does NOT call recompute_root, stamp root_sequence_at, or commit.
    pub(super) fn delete_card_inner(
        conn: &Connection,
        id: Uuid,
    ) -> Result<DeletedEntity, StorageError> {
        if Self::is_deleted(conn, id).map_err(StorageError::from)? {
            return Err(StorageError::AlreadyDeleted);
        }
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE id = ?1",
                params![id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(StorageError::from)?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }

        let deleted = DeletedEntity::new(id);
        conn.execute(
            "DELETE FROM card_versions WHERE card_id = ?1",
            params![id.as_bytes().as_slice()],
        )
        .map_err(StorageError::from)?;
        conn.execute(
            "DELETE FROM cards WHERE id = ?1",
            params![id.as_bytes().as_slice()],
        )
        .map_err(StorageError::from)?;
        conn.execute(
            "INSERT OR REPLACE INTO deleted_entities (id, hash, root_sequence_at, bucket) \
             VALUES (?1, ?2, 0, ?3)",
            params![
                id.as_bytes().as_slice(),
                deleted.hash().as_bytes().as_slice(),
                Self::bucket_of(id) as i64,
            ],
        )
        .map_err(StorageError::from)?;
        Ok(deleted)
    }

    /// Core tag deletion on an existing connection/transaction.
    /// Does NOT call recompute_root, stamp root_sequence_at, or commit.
    pub(super) fn delete_tag_inner(
        conn: &Connection,
        id: Uuid,
    ) -> Result<DeletedEntity, StorageError> {
        if Self::is_deleted(conn, id).map_err(StorageError::from)? {
            return Err(StorageError::AlreadyDeleted);
        }
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tags WHERE id = ?1",
                params![id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(StorageError::from)?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }

        let deleted = DeletedEntity::new(id);
        conn.execute(
            "DELETE FROM tag_versions WHERE tag_id = ?1",
            params![id.as_bytes().as_slice()],
        )
        .map_err(StorageError::from)?;
        conn.execute(
            "DELETE FROM tags WHERE id = ?1",
            params![id.as_bytes().as_slice()],
        )
        .map_err(StorageError::from)?;
        conn.execute(
            "INSERT OR REPLACE INTO deleted_entities (id, hash, root_sequence_at, bucket) \
             VALUES (?1, ?2, 0, ?3)",
            params![
                id.as_bytes().as_slice(),
                deleted.hash().as_bytes().as_slice(),
                Self::bucket_of(id) as i64,
            ],
        )
        .map_err(StorageError::from)?;
        Ok(deleted)
    }

    /// Stamp `root_sequence_at` on a card and its unstamped version rows.
    pub(super) fn stamp_card_sequence(
        conn: &Connection,
        id: Uuid,
        sequence: i64,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE cards SET root_sequence_at = ?1 WHERE id = ?2",
            params![sequence, id.as_bytes().as_slice()],
        )?;
        conn.execute(
            "UPDATE card_versions SET root_sequence_at = ?1 \
             WHERE card_id = ?2 AND root_sequence_at = 0",
            params![sequence, id.as_bytes().as_slice()],
        )?;
        Ok(())
    }

    /// Stamp `root_sequence_at` on a tag and its unstamped version rows.
    pub(super) fn stamp_tag_sequence(
        conn: &Connection,
        id: Uuid,
        sequence: i64,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE tags SET root_sequence_at = ?1 WHERE id = ?2",
            params![sequence, id.as_bytes().as_slice()],
        )?;
        conn.execute(
            "UPDATE tag_versions SET root_sequence_at = ?1 \
             WHERE tag_id = ?2 AND root_sequence_at = 0",
            params![sequence, id.as_bytes().as_slice()],
        )?;
        Ok(())
    }

    /// Stamp `root_sequence_at` on a deleted entity.
    pub(super) fn stamp_delete_sequence(
        conn: &Connection,
        id: Uuid,
        sequence: i64,
    ) -> Result<(), rusqlite::Error> {
        conn.execute(
            "UPDATE deleted_entities SET root_sequence_at = ?1 WHERE id = ?2",
            params![sequence, id.as_bytes().as_slice()],
        )?;
        Ok(())
    }
}
