//! Row deserialization, serialization, and utility helpers.

use blake3::Hash;
use blazelist_protocol::{Card, DateTime, NonNegativeI64, Tag, Utc};
use rgb::RGB8;
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::SqliteStorage;

// Column lists for SELECT queries. Defined once to prevent drift between
// queries and the corresponding `*_from_row` deserializers.
pub(super) const CARD_COLS: &str = "id, content, priority, tags, blazed, created_at, modified_at, due_date, count, ancestor_hash, hash";
pub(super) const CARD_VERSION_COLS: &str = "card_id, content, priority, tags, blazed, created_at, modified_at, due_date, count, ancestor_hash, hash";
pub(super) const TAG_COLS: &str =
    "id, title, color, created_at, modified_at, count, ancestor_hash, hash";
pub(super) const TAG_VERSION_COLS: &str =
    "tag_id, title, color, created_at, modified_at, count, ancestor_hash, hash";

impl SqliteStorage {
    /// Return the bucket index (0-255) for a given entity UUID.
    /// Uses the first byte of the UUID, which for UUIDv4 (random) gives
    /// uniform distribution across all 256 buckets.
    pub(crate) fn bucket_of(id: Uuid) -> u8 {
        id.as_bytes()[0]
    }

    /// Serialize a `Vec<Uuid>` to bytes using postcard.
    pub(super) fn serialize_tags(tags: &[Uuid]) -> Vec<u8> {
        postcard::to_allocvec(tags).expect("tag serialization should not fail")
    }

    /// Deserialize a `Vec<Uuid>` from postcard bytes.
    pub(super) fn deserialize_tags(bytes: &[u8]) -> Vec<Uuid> {
        postcard::from_bytes(bytes).expect("tag deserialization should not fail")
    }

    /// Reconstruct a `blake3::Hash` from a 32-byte slice.
    pub(super) fn hash_from_bytes(bytes: &[u8]) -> Hash {
        let arr: [u8; 32] = bytes.try_into().expect("hash must be 32 bytes");
        Hash::from_bytes(arr)
    }

    /// Build a Card from a rusqlite row.
    pub(super) fn card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
        let id_bytes: Vec<u8> = row.get(0)?;
        let content: String = row.get(1)?;
        let priority_raw: i64 = row.get(2)?;
        let tags_bytes: Vec<u8> = row.get(3)?;
        let blazed: bool = row.get(4)?;
        let created_at_ms: i64 = row.get(5)?;
        let modified_at_ms: i64 = row.get(6)?;
        let due_date_ms: Option<i64> = row.get(7)?;
        let count_raw: i64 = row.get(8)?;
        let ancestor_hash_bytes: Vec<u8> = row.get(9)?;
        let hash_bytes: Vec<u8> = row.get(10)?;

        let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
        let priority = NonNegativeI64::try_from(priority_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })?;
        let count = NonNegativeI64::try_from(count_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })?;
        let tags = Self::deserialize_tags(&tags_bytes);
        let created_at: DateTime<Utc> = DateTime::from_timestamp_millis(created_at_ms).unwrap();
        let modified_at: DateTime<Utc> = DateTime::from_timestamp_millis(modified_at_ms).unwrap();
        let ancestor_hash = Self::hash_from_bytes(&ancestor_hash_bytes);
        let hash = Self::hash_from_bytes(&hash_bytes);
        let due_date = due_date_ms.map(|ms| DateTime::from_timestamp_millis(ms).unwrap());

        Card::from_parts(
            id,
            content,
            priority,
            tags,
            blazed,
            created_at,
            modified_at,
            count,
            ancestor_hash,
            hash,
            due_date,
        )
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
        })
    }

    /// Build a Tag from a rusqlite row.
    pub(super) fn tag_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
        let id_bytes: Vec<u8> = row.get(0)?;
        let title: String = row.get(1)?;
        let color_bytes: Option<Vec<u8>> = row.get(2)?;
        let color = color_bytes.map(|b| RGB8::new(b[0], b[1], b[2]));
        let created_at_ms: i64 = row.get(3)?;
        let modified_at_ms: i64 = row.get(4)?;
        let count_raw: i64 = row.get(5)?;
        let ancestor_hash_bytes: Vec<u8> = row.get(6)?;
        let hash_bytes: Vec<u8> = row.get(7)?;

        let id = Uuid::from_bytes(id_bytes.as_slice().try_into().unwrap());
        let count = NonNegativeI64::try_from(count_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })?;
        let created_at: DateTime<Utc> = DateTime::from_timestamp_millis(created_at_ms).unwrap();
        let modified_at: DateTime<Utc> = DateTime::from_timestamp_millis(modified_at_ms).unwrap();
        let ancestor_hash = Self::hash_from_bytes(&ancestor_hash_bytes);
        let hash = Self::hash_from_bytes(&hash_bytes);

        Tag::from_parts(
            id,
            title,
            color,
            created_at,
            modified_at,
            count,
            ancestor_hash,
            hash,
        )
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
        })
    }

    /// Check if a UUID is in deleted_entities.
    pub(super) fn is_deleted(conn: &Connection, id: Uuid) -> Result<bool, rusqlite::Error> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM deleted_entities WHERE id = ?1",
            params![id.as_bytes().as_slice()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
