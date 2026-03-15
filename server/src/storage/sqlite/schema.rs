//! Schema initialization, PRAGMA configuration, and version management.

use std::env;

use blazelist_protocol::ZERO_HASH;
use rusqlite::{params, Connection, Transaction};
use uuid::Uuid;

use super::SqliteStorage;
use crate::storage::StorageError;

impl SqliteStorage {
    pub(super) fn ensure_schema_version_table(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                id      INTEGER PRIMARY KEY CHECK (id = 1),
                major   INTEGER NOT NULL,
                minor   INTEGER NOT NULL,
                patch   INTEGER NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Apply per-connection PRAGMA settings.
    ///
    /// Called on both the writer and reader connections so they share the
    /// same performance tuning.
    pub(super) fn apply_pragmas(conn: &Connection) -> Result<(), StorageError> {
        // Read an env-var or fall back to the provided default.
        // Values are restricted to alphanumeric characters, hyphens and
        // underscores so they are safe to interpolate into PRAGMA statements.
        fn env_or(var: &str, default: &str) -> String {
            let val = env::var(var).unwrap_or_else(|_| default.to_owned());
            assert!(
                val.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "invalid value for {var}: only ASCII alphanumeric, '-' and '_' are allowed"
            );
            val
        }

        conn.execute_batch(&format!(
            "\
            PRAGMA journal_mode = {jm};\
            PRAGMA foreign_keys = ON;\
            PRAGMA synchronous = {sync};\
            PRAGMA cache_size = {cs};\
            PRAGMA mmap_size = {mm};\
            PRAGMA temp_store = {ts};\
            PRAGMA busy_timeout = {bt};\
            ",
            jm = env_or("BLAZELIST_SQLITE_JOURNAL_MODE", "WAL"),
            sync = env_or("BLAZELIST_SQLITE_SYNCHRONOUS", "NORMAL"),
            // ~8 GiB page cache (negative value = KiB); greatly benefits read-heavy workloads.
            cs = env_or("BLAZELIST_SQLITE_CACHE_SIZE", "-8388608"),
            // Memory-mapped I/O up to 8 GiB; OS page cache serves reads without extra copies.
            mm = env_or("BLAZELIST_SQLITE_MMAP_SIZE", "8589934592"),
            ts = env_or("BLAZELIST_SQLITE_TEMP_STORE", "MEMORY"),
            // Wait up to 5 s on a locked database instead of returning SQLITE_BUSY immediately.
            bt = env_or("BLAZELIST_SQLITE_BUSY_TIMEOUT", "5000"),
        ))?;

        Ok(())
    }

    /// Create all tables, indexes, and seed rows if they don't already exist.
    pub(super) fn init_schema(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cards (
                id                  BLOB PRIMARY KEY,
                content             TEXT    NOT NULL,
                priority            INTEGER NOT NULL,
                tags                BLOB    NOT NULL,
                blazed              INTEGER NOT NULL,
                created_at          INTEGER NOT NULL,
                modified_at         INTEGER NOT NULL,
                due_date            INTEGER,
                count               INTEGER NOT NULL,
                ancestor_hash       BLOB    NOT NULL,
                hash                BLOB    NOT NULL,
                root_sequence_at    INTEGER NOT NULL DEFAULT 0,
                bucket              INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS card_versions (
                card_id         BLOB    NOT NULL,
                count           INTEGER NOT NULL,
                content         TEXT    NOT NULL,
                priority        INTEGER NOT NULL,
                tags            BLOB    NOT NULL,
                blazed          INTEGER NOT NULL,
                created_at      INTEGER NOT NULL,
                modified_at     INTEGER NOT NULL,
                due_date        INTEGER,
                ancestor_hash   BLOB    NOT NULL,
                hash            BLOB    NOT NULL,
                root_sequence_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (card_id, count)
            );

            CREATE TABLE IF NOT EXISTS tags (
                id                  BLOB PRIMARY KEY,
                title               TEXT    NOT NULL,
                color               BLOB,
                created_at          INTEGER NOT NULL,
                modified_at         INTEGER NOT NULL,
                count               INTEGER NOT NULL,
                ancestor_hash       BLOB    NOT NULL,
                hash                BLOB    NOT NULL,
                root_sequence_at    INTEGER NOT NULL DEFAULT 0,
                bucket              INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS tag_versions (
                tag_id          BLOB    NOT NULL,
                count           INTEGER NOT NULL,
                title           TEXT    NOT NULL,
                color           BLOB,
                created_at      INTEGER NOT NULL,
                modified_at     INTEGER NOT NULL,
                ancestor_hash   BLOB    NOT NULL,
                hash            BLOB    NOT NULL,
                root_sequence_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (tag_id, count)
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_cards_unique_priority
                ON cards(priority);

            CREATE TABLE IF NOT EXISTS deleted_entities (
                id                  BLOB PRIMARY KEY,
                hash                BLOB NOT NULL,
                root_sequence_at    INTEGER NOT NULL DEFAULT 0,
                bucket              INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS root_state (
                id          INTEGER PRIMARY KEY CHECK (id = 1),
                hash        BLOB    NOT NULL,
                sequence    INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS root_history (
                sequence    INTEGER PRIMARY KEY,
                hash        BLOB NOT NULL,
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS root_buckets (
                bucket      INTEGER PRIMARY KEY,
                hash        BLOB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_cards_bucket ON cards(bucket, id);
            CREATE INDEX IF NOT EXISTS idx_tags_bucket ON tags(bucket, id);
            CREATE INDEX IF NOT EXISTS idx_deleted_entities_bucket ON deleted_entities(bucket, id);
            CREATE INDEX IF NOT EXISTS idx_card_versions_seq ON card_versions(root_sequence_at);
            CREATE INDEX IF NOT EXISTS idx_tag_versions_seq ON tag_versions(root_sequence_at);
            CREATE INDEX IF NOT EXISTS idx_cards_root_seq ON cards(root_sequence_at);
            CREATE INDEX IF NOT EXISTS idx_tags_root_seq ON tags(root_sequence_at);
            CREATE INDEX IF NOT EXISTS idx_deleted_entities_root_seq ON deleted_entities(root_sequence_at);

            CREATE TABLE IF NOT EXISTS schema_version (
                id      INTEGER PRIMARY KEY CHECK (id = 1),
                major   INTEGER NOT NULL,
                minor   INTEGER NOT NULL,
                patch   INTEGER NOT NULL
            );
            ",
        )?;
        // Ensure the singleton root row exists.
        conn.execute(
            "INSERT OR IGNORE INTO root_state (id, hash, sequence) VALUES (1, ?1, 0)",
            params![ZERO_HASH.as_bytes().as_slice()],
        )?;
        // Ensure all 256 bucket rows exist.
        {
            let mut stmt =
                conn.prepare("INSERT OR IGNORE INTO root_buckets (bucket, hash) VALUES (?1, ?2)")?;
            for b in 0u16..256 {
                stmt.execute(params![b as i64, ZERO_HASH.as_bytes().as_slice()])?;
            }
        }
        Ok(())
    }

    /// Check the stored schema version against the current protocol version.
    ///
    /// - **New database (no version row):** inserts the current version.
    /// - **Same major version:** updates the stored version to current.
    /// - **Different major version, `allow_migration` is false:** returns
    ///   [`StorageError::IncompatibleVersion`].
    /// - **Different major version, `allow_migration` is true:** migrates
    ///   major-to-major in one atomic transaction.
    pub(super) fn check_schema_version(
        conn: &mut Connection,
        allow_migration: bool,
    ) -> Result<(), StorageError> {
        use blazelist_protocol::PROTOCOL_VERSION;

        let current = &PROTOCOL_VERSION;

        let stored: Option<(i64, i64, i64)> = conn
            .query_row(
                "SELECT major, minor, patch FROM schema_version WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        match stored {
            None => {
                // Fresh database -- record the current version.
                conn.execute(
                    "INSERT INTO schema_version (id, major, minor, patch) VALUES (1, ?1, ?2, ?3)",
                    params![
                        current.major as i64,
                        current.minor as i64,
                        current.patch as i64
                    ],
                )?;
            }
            Some((major, minor, patch)) => {
                let stored_ver =
                    blazelist_protocol::Version::new(major as u64, minor as u64, patch as u64);

                if stored_ver.major == current.major {
                    // Same major version -- update to current (non-breaking).
                    conn.execute(
                        "UPDATE schema_version SET major = ?1, minor = ?2, patch = ?3 WHERE id = 1",
                        params![
                            current.major as i64,
                            current.minor as i64,
                            current.patch as i64
                        ],
                    )?;
                    return Ok(());
                }

                if !allow_migration || stored_ver.major > current.major {
                    // Refuse startup unless migration is explicitly enabled.
                    // Also refuse downgrades; only upgrades are supported.
                    return Err(StorageError::IncompatibleVersion {
                        stored: stored_ver,
                        current: current.clone(),
                    });
                }

                Self::migrate_schema_major_to_major(conn, &stored_ver, current)?;
            }
        }

        Ok(())
    }

    fn migrate_schema_major_to_major(
        conn: &mut Connection,
        stored: &blazelist_protocol::Version,
        current: &blazelist_protocol::Version,
    ) -> Result<(), StorageError> {
        let tx = conn.transaction()?;
        for major in stored.major..current.major {
            Self::migrate_one_major_step(&tx, major, major + 1)?;
        }
        tx.execute(
            "UPDATE schema_version SET major = ?1, minor = ?2, patch = ?3 WHERE id = 1",
            params![
                current.major as i64,
                current.minor as i64,
                current.patch as i64
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn migrate_one_major_step(
        tx: &Transaction<'_>,
        from_major: u64,
        to_major: u64,
    ) -> Result<(), StorageError> {
        match (from_major, to_major) {
            (0, 1) => Self::migrate_v0_to_v1(tx),
            // v1→v2: Card.priority widened from NonNegativeI64 to i64.
            // SQLite stores both as INTEGER; existing values are valid. No-op.
            (1, 2) => Ok(()),
            _ => Err(StorageError::MigrationNotImplemented {
                stored: blazelist_protocol::Version::new(from_major, 0, 0),
                current: blazelist_protocol::Version::new(to_major, 0, 0),
            }),
        }
    }

    fn migrate_v0_to_v1(tx: &Transaction<'_>) -> Result<(), StorageError> {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS root_state (
                id          INTEGER PRIMARY KEY CHECK (id = 1),
                hash        BLOB    NOT NULL,
                sequence    INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS root_history (
                sequence    INTEGER PRIMARY KEY,
                hash        BLOB NOT NULL,
                created_at  INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS root_buckets (
                bucket      INTEGER PRIMARY KEY,
                hash        BLOB NOT NULL
            );",
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO root_state (id, hash, sequence) VALUES (1, ?1, 0)",
            params![ZERO_HASH.as_bytes().as_slice()],
        )?;
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO root_buckets (bucket, hash) VALUES (?1, ?2)")?;
            for bucket_id in 0u8..=255 {
                stmt.execute(params![bucket_id as i64, ZERO_HASH.as_bytes().as_slice()])?;
            }
        }

        Self::ensure_column(
            tx,
            "cards",
            "root_sequence_at",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::ensure_column(tx, "cards", "bucket", "INTEGER NOT NULL DEFAULT 0")?;
        Self::ensure_column(
            tx,
            "card_versions",
            "root_sequence_at",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::ensure_column(tx, "tags", "root_sequence_at", "INTEGER NOT NULL DEFAULT 0")?;
        Self::ensure_column(tx, "tags", "bucket", "INTEGER NOT NULL DEFAULT 0")?;
        Self::ensure_column(
            tx,
            "tag_versions",
            "root_sequence_at",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::ensure_column(
            tx,
            "deleted_entities",
            "root_sequence_at",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::ensure_column(
            tx,
            "deleted_entities",
            "bucket",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        Self::backfill_bucket_column(tx, "cards", "id")?;
        Self::backfill_bucket_column(tx, "tags", "id")?;
        Self::backfill_bucket_column(tx, "deleted_entities", "id")?;

        for bucket_id in 0u8..=255 {
            let bucket_hash = Self::recompute_bucket_hash(tx, bucket_id)?;
            tx.execute(
                "UPDATE root_buckets SET hash = ?1 WHERE bucket = ?2",
                params![bucket_hash.as_bytes().as_slice(), bucket_id as i64],
            )?;
        }
        Self::recompute_root_from_buckets(tx)?;
        Ok(())
    }

    fn ensure_column(
        tx: &Transaction<'_>,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), StorageError> {
        let pragma_sql = format!("PRAGMA table_info({table})");
        let mut stmt = tx.prepare(&pragma_sql)?;
        let mut rows = stmt.query([])?;
        let mut exists = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                exists = true;
                break;
            }
        }
        drop(rows);
        drop(stmt);

        if !exists {
            tx.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {definition};"
            ))?;
        }
        Ok(())
    }

    fn backfill_bucket_column(
        tx: &Transaction<'_>,
        table: &str,
        id_column: &str,
    ) -> Result<(), StorageError> {
        let select_sql = format!("SELECT {id_column} FROM {table}");
        let mut stmt = tx.prepare(&select_sql)?;
        let ids: Vec<Vec<u8>> = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let update_sql = format!("UPDATE {table} SET bucket = ?1 WHERE {id_column} = ?2");
        let mut update_stmt = tx.prepare(&update_sql)?;
        for id_bytes in ids {
            let id = Uuid::from_slice(&id_bytes).map_err(|e| {
                StorageError::Internal(format!(
                    "invalid UUID while migrating {table}.{id_column}: {e}"
                ))
            })?;
            update_stmt.execute(params![Self::bucket_of(id) as i64, id_bytes])?;
        }
        Ok(())
    }

    /// Force a WAL checkpoint, writing all committed WAL pages back to the
    /// main database file and truncating the WAL.
    pub fn checkpoint(&self) {
        let conn = self.writer.lock().unwrap();
        match conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
            Ok(()) => tracing::debug!("WAL checkpoint completed"),
            Err(e) => tracing::debug!(%e, "WAL checkpoint failed"),
        }
    }
}
