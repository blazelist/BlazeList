//! Root hash and bucket hash computation.

use blake3::Hash;
use blazelist_protocol::{Utc, ZERO_HASH};
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::SqliteStorage;

impl SqliteStorage {
    /// Recompute the hash for a single bucket by querying all entities in that
    /// bucket (cards, tags, deleted entities) sorted by UUID. Returns ZERO_HASH
    /// if the bucket is empty.
    pub(super) fn recompute_bucket_hash(
        conn: &Connection,
        bucket: u8,
    ) -> Result<Hash, rusqlite::Error> {
        let bucket_i64 = bucket as i64;

        let mut card_stmt = conn.prepare("SELECT hash FROM cards WHERE bucket = ?1 ORDER BY id")?;
        let card_hashes: Vec<Vec<u8>> = card_stmt
            .query_map(params![bucket_i64], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;

        let mut tag_stmt = conn.prepare("SELECT hash FROM tags WHERE bucket = ?1 ORDER BY id")?;
        let tag_hashes: Vec<Vec<u8>> = tag_stmt
            .query_map(params![bucket_i64], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;

        let mut deleted_stmt =
            conn.prepare("SELECT hash FROM deleted_entities WHERE bucket = ?1 ORDER BY id")?;
        let deleted_hashes: Vec<Vec<u8>> = deleted_stmt
            .query_map(params![bucket_i64], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;

        if card_hashes.is_empty() && tag_hashes.is_empty() && deleted_hashes.is_empty() {
            return Ok(ZERO_HASH);
        }

        let mut hasher = blake3::Hasher::new();
        for h in &card_hashes {
            hasher.update(h);
        }
        for h in &tag_hashes {
            hasher.update(h);
        }
        for h in &deleted_hashes {
            hasher.update(h);
        }
        Ok(hasher.finalize())
    }

    /// Compute the root hash from all 256 cached bucket hashes. Updates
    /// root_state and root_history. Returns the new root sequence.
    pub(super) fn recompute_root_from_buckets(conn: &Connection) -> Result<i64, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT hash FROM root_buckets ORDER BY bucket")?;
        let bucket_hashes: Vec<Vec<u8>> = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;

        let all_zero = bucket_hashes
            .iter()
            .all(|h| h.as_slice() == ZERO_HASH.as_bytes());

        let root_hash = if all_zero {
            ZERO_HASH
        } else {
            let mut hasher = blake3::Hasher::new();
            for h in &bucket_hashes {
                hasher.update(h);
            }
            hasher.finalize()
        };

        conn.execute(
            "UPDATE root_state SET hash = ?1, sequence = sequence + 1 WHERE id = 1",
            params![root_hash.as_bytes().as_slice()],
        )?;
        let new_sequence: i64 =
            conn.query_row("SELECT sequence FROM root_state WHERE id = 1", [], |row| {
                row.get(0)
            })?;

        // Insert into root_history for corruption detection.
        conn.execute(
            "INSERT INTO root_history (sequence, hash, created_at) VALUES (?1, ?2, ?3)",
            params![
                new_sequence,
                root_hash.as_bytes().as_slice(),
                Utc::now().timestamp_millis()
            ],
        )?;

        Ok(new_sequence)
    }

    /// Recompute the root hash after mutations to the given entity IDs.
    /// Only the affected buckets are recomputed, then the root is derived
    /// from the 256 cached bucket hashes. Returns the new root sequence.
    pub(super) fn recompute_root_for_ids(
        conn: &Connection,
        ids: &[Uuid],
    ) -> Result<i64, rusqlite::Error> {
        // Collect unique affected bucket numbers.
        let mut buckets: Vec<u8> = ids.iter().map(|id| Self::bucket_of(*id)).collect();
        buckets.sort_unstable();
        buckets.dedup();

        // Recompute and update each affected bucket.
        for &bucket in &buckets {
            let bucket_hash = Self::recompute_bucket_hash(conn, bucket)?;
            conn.execute(
                "UPDATE root_buckets SET hash = ?1 WHERE bucket = ?2",
                params![bucket_hash.as_bytes().as_slice(), bucket as i64],
            )?;
        }

        Self::recompute_root_from_buckets(conn)
    }
}
