//! OPFS-backed local storage for offline-first operation.
//!
//! Persists cards, tags, deleted entities, and root state in the browser's
//! [Origin Private File System (OPFS)](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system)
//! for persistence across sessions. Data is serialized with postcard.
//!
//! OPFS is required — the app will refuse to start on browsers that don't
//! support it (e.g., insecure contexts or very old browsers).

use blazelist_protocol::{Card, DeletedEntity, RootState, SequenceHistoryEntry, Tag};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

const DB_FILENAME: &str = "blazelist.db";
const HISTORY_FILENAME: &str = "blazelist-history.db";

/// Check that OPFS is available. Panics with a user-visible alert if not.
pub async fn require_opfs() {
    if let Err(e) = opfs_check().await {
        let msg = format!(
            "BlazeList requires Origin Private File System (OPFS) support.\n\n\
             Your browser or context does not support it: {e:?}\n\n\
             Please use a modern browser over HTTPS or localhost."
        );
        if let Some(window) = web_sys::window() {
            let _ = window.alert_with_message(&msg);
        }
        panic!("OPFS unavailable: {e:?}");
    }
}

/// Complete local state persisted in OPFS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalState {
    pub cards: Vec<Card>,
    pub tags: Vec<Tag>,
    pub deleted_entities: Vec<DeletedEntity>,
    pub root: RootState,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            tags: Vec::new(),
            deleted_entities: Vec::new(),
            root: RootState::empty(),
        }
    }
}

/// Cached version histories persisted separately from the main state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryCache {
    pub card_histories: HashMap<Uuid, Vec<Card>>,
    pub tag_histories: HashMap<Uuid, Vec<Tag>>,
    pub sequence_history: Vec<SequenceHistoryEntry>,
}

thread_local! {
    static HISTORY: RefCell<HistoryCache> = RefCell::new(HistoryCache::default());
}

/// Load the local state from OPFS.
///
/// Returns [`LocalState::default()`] if the database doesn't exist, is empty,
/// or cannot be deserialized (e.g., after a schema change).
pub async fn load() -> LocalState {
    match opfs_read(DB_FILENAME).await {
        Ok(data) => {
            if data.is_null() || data.is_undefined() {
                return LocalState::default();
            }
            let array = js_sys::Uint8Array::new(&data);
            let bytes = array.to_vec();
            if bytes.is_empty() {
                return LocalState::default();
            }
            match postcard::from_bytes(&bytes) {
                Ok(state) => state,
                Err(e) => {
                    log::warn!("Local database corrupted, starting fresh: {e}");
                    LocalState::default()
                }
            }
        }
        Err(e) => {
            log::info!("No local database found (first run): {e:?}");
            LocalState::default()
        }
    }
}

/// Save the local state to OPFS.
pub async fn save(state: &LocalState) {
    match postcard::to_allocvec(state) {
        Ok(bytes) => {
            if let Err(e) = opfs_write(DB_FILENAME, &bytes).await {
                log::error!("Failed to save local database: {e:?}");
            }
        }
        Err(e) => {
            log::error!("Failed to serialize local state: {e}");
        }
    }
}

/// Clear the local database (e.g., on `RootHashMismatch`).
pub async fn clear() {
    if let Err(e) = opfs_delete(DB_FILENAME).await {
        log::warn!("Failed to clear local database: {e:?}");
    }
}

/// Request persistent storage from the browser to reduce eviction risk.
///
/// Returns `true` if the browser granted persistent storage.
pub async fn request_persistent_storage() -> bool {
    match request_persistence().await {
        Ok(val) => val.as_bool().unwrap_or(false),
        Err(_) => false,
    }
}

// -- History cache API -------------------------------------------------------

/// Load the history cache from OPFS into memory.
pub async fn load_history_cache() {
    match opfs_read(HISTORY_FILENAME).await {
        Ok(data) => {
            if data.is_null() || data.is_undefined() {
                return;
            }
            let array = js_sys::Uint8Array::new(&data);
            let bytes = array.to_vec();
            if bytes.is_empty() {
                return;
            }
            match postcard::from_bytes::<HistoryCache>(&bytes) {
                Ok(cache) => {
                    HISTORY.with(|h| *h.borrow_mut() = cache);
                }
                Err(e) => {
                    log::warn!("History cache corrupted, starting fresh: {e}");
                }
            }
        }
        Err(_) => {}
    }
}

/// Persist the in-memory history cache to OPFS.
pub async fn save_history_cache() {
    let cache = HISTORY.with(|h| h.borrow().clone());
    match postcard::to_allocvec(&cache) {
        Ok(bytes) => {
            if let Err(e) = opfs_write(HISTORY_FILENAME, &bytes).await {
                log::error!("Failed to save history cache: {e:?}");
            }
        }
        Err(e) => {
            log::error!("Failed to serialize history cache: {e}");
        }
    }
}

/// Clear the history cache from memory and OPFS.
pub async fn clear_history_cache() {
    HISTORY.with(|h| *h.borrow_mut() = HistoryCache::default());
    if let Err(e) = opfs_delete(HISTORY_FILENAME).await {
        log::warn!("Failed to clear history cache: {e:?}");
    }
}

/// Get cached card version history (empty vec if not cached).
pub fn get_cached_card_history(id: Uuid) -> Vec<Card> {
    HISTORY.with(|h| h.borrow().card_histories.get(&id).cloned().unwrap_or_default())
}

/// Update cached card version history in memory.
pub fn update_cached_card_history(id: Uuid, history: Vec<Card>) {
    HISTORY.with(|h| {
        h.borrow_mut().card_histories.insert(id, history);
    });
}

/// Get cached tag version history (empty vec if not cached).
pub fn get_cached_tag_history(id: Uuid) -> Vec<Tag> {
    HISTORY.with(|h| h.borrow().tag_histories.get(&id).cloned().unwrap_or_default())
}

/// Update cached tag version history in memory.
pub fn update_cached_tag_history(id: Uuid, history: Vec<Tag>) {
    HISTORY.with(|h| {
        h.borrow_mut().tag_histories.insert(id, history);
    });
}

/// Get cached sequence history (empty vec if not cached).
pub fn get_cached_sequence_history() -> Vec<SequenceHistoryEntry> {
    HISTORY.with(|h| h.borrow().sequence_history.clone())
}

/// Update cached sequence history in memory.
pub fn update_cached_sequence_history(history: Vec<SequenceHistoryEntry>) {
    HISTORY.with(|h| {
        h.borrow_mut().sequence_history = history;
    });
}

// -- OPFS bindings via inline JavaScript ------------------------------------
//
// The Origin Private File System API is accessed through a small set of
// async JS helpers. This avoids depending on unstable web-sys features and
// keeps the interop surface minimal.

#[wasm_bindgen(inline_js = "
export async function opfs_check() {
    await navigator.storage.getDirectory();
}

export async function opfs_read(filename) {
    try {
        const root = await navigator.storage.getDirectory();
        const fileHandle = await root.getFileHandle(filename);
        const file = await fileHandle.getFile();
        const buffer = await file.arrayBuffer();
        return new Uint8Array(buffer);
    } catch (e) {
        return null;
    }
}

export async function opfs_write(filename, data) {
    const root = await navigator.storage.getDirectory();
    const fileHandle = await root.getFileHandle(filename, { create: true });
    const writable = await fileHandle.createWritable();
    await writable.write(data);
    await writable.close();
}

export async function opfs_delete(filename) {
    try {
        const root = await navigator.storage.getDirectory();
        await root.removeEntry(filename);
    } catch (e) {
        // File may not exist — that is fine.
    }
}

export async function request_persistence() {
    if (navigator.storage && navigator.storage.persist) {
        return await navigator.storage.persist();
    }
    return false;
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn opfs_check() -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn opfs_read(filename: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn opfs_write(filename: &str, data: &[u8]) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn opfs_delete(filename: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn request_persistence() -> Result<JsValue, JsValue>;
}
