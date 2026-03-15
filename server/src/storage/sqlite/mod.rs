//! SQLite-backed storage implementation.

mod helpers;
mod root;
mod schema;
mod write;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use blazelist_protocol::{
    Card, CardFilter, ChangeSet, DateTime, DeletedEntity, Entity, NonNegativeI64, PushItem,
    RootState, SequenceHistoryEntry, SequenceOperation, SequenceOperationKind, Tag,
};
use rusqlite::{params, Connection, OpenFlags};
use uuid::Uuid;

use crate::storage::error::{BatchError, PushError, PushOpError, StorageError};
use crate::storage::traits::Storage;

use helpers::{CARD_COLS, CARD_VERSION_COLS, TAG_COLS, TAG_VERSION_COLS};

/// SQLite-backed storage.
///
/// Uses separate connections for reads and writes. With WAL journal mode
/// (the default), read queries proceed concurrently with writes without
/// blocking. Each connection is protected by its own [`Mutex`].
pub struct SqliteStorage {
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
}

impl SqliteStorage {
    /// Open (or create) the database at `path`.
    ///
    /// `allow_migration` controls whether an incompatible (different major
    /// version) database may be automatically migrated. When `false`, the
    /// server refuses to start if the on-disk schema was created by a
    /// different major protocol version.
    pub fn open(path: &Path, allow_migration: bool) -> Result<Self, StorageError> {
        let mut writer = Connection::open(path)?;
        Self::apply_pragmas(&writer)?;
        Self::ensure_schema_version_table(&writer)?;
        Self::check_schema_version(&mut writer, allow_migration)?;
        Self::init_schema(&writer)?;

        let reader = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::apply_pragmas(&reader)?;

        Ok(Self {
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
        })
    }

    /// Create an in-memory database (useful for tests).
    ///
    /// Uses a shared-cache URI so that both the read and write connections
    /// see the same in-memory database.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let uri = format!("file:blazelist_{}?mode=memory&cache=shared", Uuid::new_v4());
        let write_flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut writer = Connection::open_with_flags(&uri, write_flags)?;
        Self::apply_pragmas(&writer)?;
        Self::ensure_schema_version_table(&writer)?;
        Self::check_schema_version(&mut writer, false)?;
        Self::init_schema(&writer)?;

        let read_flags = OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let reader = Connection::open_with_flags(&uri, read_flags)?;
        Self::apply_pragmas(&reader)?;
        // Shared-cache mode uses table-level locking instead of WAL-based
        // concurrency. Allow the reader to proceed without acquiring shared
        // locks so it doesn't block on writer transactions.
        reader.execute_batch("PRAGMA read_uncommitted = ON")?;

        Ok(Self {
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
        })
    }
}

impl Storage for SqliteStorage {
    // -- Cards ---------------------------------------------------------------

    fn push_card_versions(&self, versions: &[Card]) -> Result<(), PushOpError> {
        let card_id = versions.first().ok_or(PushError::EmptyChain)?.id();
        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction()?;
        Self::push_card_versions_inner(&tx, versions)?;
        let seq = Self::recompute_root_for_ids(&tx, &[card_id])?;
        Self::stamp_card_sequence(&tx, card_id, seq)?;
        tx.commit()?;
        Ok(())
    }

    fn get_card(&self, id: Uuid) -> Result<Option<Card>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let result = conn.query_row(
            &format!("SELECT {CARD_COLS} FROM cards WHERE id = ?1"),
            params![id.as_bytes().as_slice()],
            Self::card_from_row,
        );
        match result {
            Ok(card) => Ok(Some(card)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_card_history(&self, id: Uuid, limit: Option<u32>) -> Result<Vec<Card>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let limit_clause = limit.map_or(String::new(), |n| format!(" LIMIT {n}"));
        let sql = format!(
            "SELECT {CARD_VERSION_COLS} FROM card_versions \
             WHERE card_id = ?1 ORDER BY count ASC{limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let cards = stmt
            .query_map(params![id.as_bytes().as_slice()], Self::card_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cards)
    }

    fn list_cards(
        &self,
        filter: CardFilter,
        limit: Option<u32>,
    ) -> Result<Vec<Card>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let limit_clause = limit.map_or(String::new(), |n| format!(" LIMIT {n}"));
        let where_clause = match filter {
            CardFilter::All => "",
            CardFilter::Blazed => "WHERE blazed = 1",
            CardFilter::Extinguished => "WHERE blazed = 0",
        };
        let sql = format!(
            "SELECT {CARD_COLS} FROM cards {where_clause} ORDER BY priority DESC{limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let cards = stmt
            .query_map([], Self::card_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cards)
    }

    fn delete_card(&self, id: Uuid) -> Result<DeletedEntity, StorageError> {
        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction()?;
        let deleted = Self::delete_card_inner(&tx, id)?;
        let seq = Self::recompute_root_for_ids(&tx, &[id])?;
        Self::stamp_delete_sequence(&tx, id, seq)?;
        tx.commit()?;
        Ok(deleted)
    }

    // -- Tags ----------------------------------------------------------------

    fn push_tag_versions(&self, versions: &[Tag]) -> Result<(), PushOpError> {
        let tag_id = versions.first().ok_or(PushError::EmptyChain)?.id();
        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction()?;
        Self::push_tag_versions_inner(&tx, versions)?;
        let seq = Self::recompute_root_for_ids(&tx, &[tag_id])?;
        Self::stamp_tag_sequence(&tx, tag_id, seq)?;
        tx.commit()?;
        Ok(())
    }

    fn get_tag(&self, id: Uuid) -> Result<Option<Tag>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let result = conn.query_row(
            &format!("SELECT {TAG_COLS} FROM tags WHERE id = ?1"),
            params![id.as_bytes().as_slice()],
            Self::tag_from_row,
        );
        match result {
            Ok(tag) => Ok(Some(tag)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_tag_history(&self, id: Uuid, limit: Option<u32>) -> Result<Vec<Tag>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let limit_clause = limit.map_or(String::new(), |n| format!(" LIMIT {n}"));
        let sql = format!(
            "SELECT {TAG_VERSION_COLS} FROM tag_versions \
             WHERE tag_id = ?1 ORDER BY count ASC{limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tags = stmt
            .query_map(params![id.as_bytes().as_slice()], Self::tag_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    fn list_tags(&self) -> Result<Vec<Tag>, StorageError> {
        let conn = self.reader.lock().unwrap();
        let mut stmt = conn.prepare(&format!("SELECT {TAG_COLS} FROM tags ORDER BY title"))?;
        let tags = stmt
            .query_map([], Self::tag_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    fn delete_tag(&self, id: Uuid) -> Result<DeletedEntity, StorageError> {
        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction()?;
        let deleted = Self::delete_tag_inner(&tx, id)?;
        let seq = Self::recompute_root_for_ids(&tx, &[id])?;
        Self::stamp_delete_sequence(&tx, id, seq)?;
        tx.commit()?;
        Ok(deleted)
    }

    // -- Root ----------------------------------------------------------------

    fn get_root(&self) -> Result<RootState, StorageError> {
        let conn = self.reader.lock().unwrap();
        let (hash_bytes, sequence_raw): (Vec<u8>, i64) = conn.query_row(
            "SELECT hash, sequence FROM root_state WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let sequence = NonNegativeI64::try_from(sequence_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })?;
        Ok(RootState {
            hash: Self::hash_from_bytes(&hash_bytes),
            sequence,
        })
    }

    // -- Sync ----------------------------------------------------------------

    fn get_changes_since(
        &self,
        sequence: NonNegativeI64,
        expected_hash: blake3::Hash,
    ) -> Result<ChangeSet, StorageError> {
        let mut conn = self.reader.lock().unwrap();
        let tx = conn.transaction()?;
        let since = i64::from(sequence);
        let stored_hash: Option<Vec<u8>> = tx
            .query_row(
                "SELECT hash FROM root_history WHERE sequence = ?1",
                params![since],
                |row| row.get(0),
            )
            .ok();

        if let Some(hash_bytes) = stored_hash {
            let server_hash = Self::hash_from_bytes(&hash_bytes);
            if server_hash != expected_hash {
                return Err(StorageError::RootHashMismatch {
                    sequence,
                    expected_hash: server_hash,
                });
            }
        }
        // If sequence not in history, proceed anyway (sequence predates history
        // table or is the initial state).

        let mut card_stmt = tx.prepare(&format!(
            "SELECT {CARD_COLS} FROM cards WHERE root_sequence_at > ?1"
        ))?;
        let cards = card_stmt
            .query_map(params![since], Self::card_from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut tag_stmt = tx.prepare(&format!(
            "SELECT {TAG_COLS} FROM tags WHERE root_sequence_at > ?1"
        ))?;
        let tags = tag_stmt
            .query_map(params![since], Self::tag_from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut del_stmt =
            tx.prepare("SELECT id, hash FROM deleted_entities WHERE root_sequence_at > ?1")?;
        let deleted = del_stmt
            .query_map(params![since], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let hash_bytes: Vec<u8> = row.get(1)?;
                let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
                let hash = Self::hash_from_bytes(&hash_bytes);
                Ok(DeletedEntity::from_parts(id, hash))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let (hash_bytes, sequence_raw): (Vec<u8>, i64) = tx.query_row(
            "SELECT hash, sequence FROM root_state WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let root_sequence = NonNegativeI64::try_from(sequence_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })?;
        let root = RootState {
            hash: Self::hash_from_bytes(&hash_bytes),
            sequence: root_sequence,
        };

        Ok(ChangeSet {
            cards,
            tags,
            deleted,
            root,
        })
    }

    // -- Batch ---------------------------------------------------------------

    fn push_batch(&self, items: &[PushItem]) -> Result<(), BatchError> {
        if items.is_empty() {
            return Ok(());
        }

        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction().map_err(|e| BatchError {
            index: 0,
            error: e.into(),
        })?;

        let mut affected_card_ids: Vec<Uuid> = Vec::new();
        let mut affected_tag_ids: Vec<Uuid> = Vec::new();
        let mut affected_deleted_ids: Vec<Uuid> = Vec::new();

        for (index, item) in items.iter().enumerate() {
            let result: Result<(), PushOpError> = match item {
                PushItem::Cards(versions) => {
                    if let Some(first) = versions.first() {
                        affected_card_ids.push(first.id());
                    }
                    Self::push_card_versions_inner(&tx, versions)
                }
                PushItem::Tags(versions) => {
                    if let Some(first) = versions.first() {
                        affected_tag_ids.push(first.id());
                    }
                    Self::push_tag_versions_inner(&tx, versions)
                }
                PushItem::DeleteCard { id } => {
                    affected_deleted_ids.push(*id);
                    Self::delete_card_inner(&tx, *id)
                        .map(|_| ())
                        .map_err(PushOpError::from)
                }
                PushItem::DeleteTag { id } => {
                    affected_deleted_ids.push(*id);
                    Self::delete_tag_inner(&tx, *id)
                        .map(|_| ())
                        .map_err(PushOpError::from)
                }
            };
            result.map_err(|error| BatchError { index, error })?;
        }

        let all_affected_ids: Vec<Uuid> = affected_card_ids
            .iter()
            .chain(affected_tag_ids.iter())
            .chain(affected_deleted_ids.iter())
            .copied()
            .collect();
        let seq = Self::recompute_root_for_ids(&tx, &all_affected_ids).map_err(|e| BatchError {
            index: 0,
            error: e.into(),
        })?;

        let batch_err = |e: rusqlite::Error| BatchError {
            index: 0,
            error: e.into(),
        };
        for id in &affected_card_ids {
            Self::stamp_card_sequence(&tx, *id, seq).map_err(batch_err)?;
        }
        for id in &affected_tag_ids {
            Self::stamp_tag_sequence(&tx, *id, seq).map_err(batch_err)?;
        }
        for id in &affected_deleted_ids {
            Self::stamp_delete_sequence(&tx, *id, seq).map_err(batch_err)?;
        }

        tx.commit().map_err(|e| BatchError {
            index: 0,
            error: e.into(),
        })?;
        Ok(())
    }

    // -- History -------------------------------------------------------------

    fn get_sequence_history(
        &self,
        after_sequence: Option<NonNegativeI64>,
        limit: Option<u32>,
    ) -> Result<Vec<SequenceHistoryEntry>, StorageError> {
        let mut conn = self.reader.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Fetch root history entries (descending by sequence), with optional pagination.
        let rh_sql = match (after_sequence, limit) {
            (Some(after), Some(n)) => format!(
                "SELECT sequence, hash, created_at FROM root_history \
                 WHERE sequence < {} ORDER BY sequence DESC LIMIT {n}",
                i64::from(after)
            ),
            (Some(after), None) => format!(
                "SELECT sequence, hash, created_at FROM root_history \
                 WHERE sequence < {} ORDER BY sequence DESC",
                i64::from(after)
            ),
            (None, Some(n)) => format!(
                "SELECT sequence, hash, created_at FROM root_history \
                 ORDER BY sequence DESC LIMIT {n}"
            ),
            (None, None) => "SELECT sequence, hash, created_at FROM root_history \
                             ORDER BY sequence DESC"
                .to_string(),
        };
        let mut rh_stmt = tx.prepare(&rh_sql)?;
        let root_entries: Vec<(i64, Vec<u8>, i64)> = rh_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<_, _>>()?;

        // Collect the sequence numbers we need operations for.
        let seq_set: std::collections::HashSet<i64> =
            root_entries.iter().map(|(seq, _, _)| *seq).collect();

        // 2. Fetch card version operations (only for the sequences we care about).
        let mut cv_stmt = tx.prepare(
            "SELECT card_id, count, root_sequence_at FROM card_versions WHERE root_sequence_at > 0",
        )?;
        let card_ops: Vec<(Vec<u8>, i64, i64)> = cv_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter(|r| r.as_ref().map_or(true, |(_, _, seq)| seq_set.contains(seq)))
            .collect::<Result<_, _>>()?;

        // 3. Fetch tag version operations.
        let mut tv_stmt = tx.prepare(
            "SELECT tag_id, count, root_sequence_at FROM tag_versions WHERE root_sequence_at > 0",
        )?;
        let tag_ops: Vec<(Vec<u8>, i64, i64)> = tv_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter(|r| r.as_ref().map_or(true, |(_, _, seq)| seq_set.contains(seq)))
            .collect::<Result<_, _>>()?;

        // 4. Fetch deleted entity operations.
        let mut de_stmt = tx.prepare(
            "SELECT id, root_sequence_at FROM deleted_entities WHERE root_sequence_at > 0",
        )?;
        let del_ops: Vec<(Vec<u8>, i64)> = de_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter(|r| r.as_ref().map_or(true, |(_, seq)| seq_set.contains(seq)))
            .collect::<Result<_, _>>()?;

        // 5. Group operations by root_sequence_at.
        let mut ops_by_seq: HashMap<i64, Vec<SequenceOperation>> = HashMap::new();

        for (id_bytes, count, seq) in &card_ops {
            let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
            let kind = if *count == 1 {
                SequenceOperationKind::CardCreated
            } else {
                SequenceOperationKind::CardUpdated
            };
            ops_by_seq.entry(*seq).or_default().push(SequenceOperation {
                entity_id: id,
                kind,
            });
        }

        for (id_bytes, count, seq) in &tag_ops {
            let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
            let kind = if *count == 1 {
                SequenceOperationKind::TagCreated
            } else {
                SequenceOperationKind::TagUpdated
            };
            ops_by_seq.entry(*seq).or_default().push(SequenceOperation {
                entity_id: id,
                kind,
            });
        }

        for (id_bytes, seq) in &del_ops {
            let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
            ops_by_seq.entry(*seq).or_default().push(SequenceOperation {
                entity_id: id,
                kind: SequenceOperationKind::EntityDeleted,
            });
        }

        // 6. Build entries from root history, attaching operations.
        let entries = root_entries
            .into_iter()
            .map(|(seq, hash_bytes, created_at_ms)| {
                let hash = Self::hash_from_bytes(&hash_bytes);
                let sequence = NonNegativeI64::try_from(seq).unwrap();
                let operations = ops_by_seq.remove(&seq).unwrap_or_default();
                let created_at = DateTime::from_timestamp_millis(created_at_ms).unwrap_or_default();
                SequenceHistoryEntry {
                    sequence,
                    hash,
                    operations,
                    created_at,
                }
            })
            .collect();

        Ok(entries)
    }
}
