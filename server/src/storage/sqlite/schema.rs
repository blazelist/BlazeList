//! Schema initialization, PRAGMA configuration, and version management.

use std::env;

use blazelist_protocol::ZERO_HASH;
use rusqlite::{Connection, params};

use super::SqliteStorage;
use crate::storage::StorageError;

impl SqliteStorage {
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
            PRAGMA foreign_keys = {fk};\
            PRAGMA synchronous = {sync};\
            PRAGMA cache_size = {cs};\
            PRAGMA mmap_size = {mm};\
            PRAGMA temp_store = {ts};\
            PRAGMA busy_timeout = {bt};\
            ",
            jm = env_or("BLAZELIST_SQLITE_JOURNAL_MODE", "WAL"),
            fk = env_or("BLAZELIST_SQLITE_FOREIGN_KEYS", "ON"),
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
    /// - **Different major version, `allow_migration` is true:** returns
    ///   [`StorageError::MigrationNotImplemented`] (migration logic is not
    ///   yet written).
    pub(super) fn check_schema_version(
        conn: &Connection,
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
                if stored_ver.major != current.major {
                    // Breaking change -- major version differs.
                    if allow_migration {
                        return Err(StorageError::MigrationNotImplemented {
                            stored: stored_ver,
                            current: current.clone(),
                        });
                    } else {
                        return Err(StorageError::IncompatibleVersion {
                            stored: stored_ver,
                            current: current.clone(),
                        });
                    }
                }
                // Same major version -- update to current (non-breaking).
                conn.execute(
                    "UPDATE schema_version SET major = ?1, minor = ?2, patch = ?3 WHERE id = 1",
                    params![
                        current.major as i64,
                        current.minor as i64,
                        current.patch as i64
                    ],
                )?;
            }
        }

        Ok(())
    }
}
