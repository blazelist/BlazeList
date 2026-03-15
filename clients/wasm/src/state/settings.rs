/// Device-local settings persisted in `localStorage`.
///
/// These settings are not synced to the server — they stay on the device.

const STORAGE_KEY_DRAG_DROP: &str = "blazelist_drag_drop_reorder";
const STORAGE_KEY_SHOW_PREVIEW: &str = "blazelist_show_preview";
const STORAGE_KEY_AUTO_SAVE: &str = "blazelist_auto_save";
const STORAGE_KEY_AUTO_SAVE_DELAY: &str = "blazelist_auto_save_delay";
const STORAGE_KEY_AUTO_SYNC: &str = "blazelist_auto_sync";
const STORAGE_KEY_AUTO_SYNC_INTERVAL: &str = "blazelist_auto_sync_interval";
const STORAGE_KEY_DEBOUNCE_ENABLED: &str = "blazelist_debounce_enabled";
const STORAGE_KEY_DEBOUNCE_DELAY: &str = "blazelist_debounce_delay";
const STORAGE_KEY_KEYBOARD_SHORTCUTS: &str = "blazelist_keyboard_shortcuts";

/// Default values (used when localStorage has no value and no server override).
pub const DEFAULT_DRAG_DROP: bool = false;
pub const DEFAULT_SHOW_PREVIEW: bool = false;
pub const DEFAULT_AUTO_SAVE: bool = true;
pub const DEFAULT_AUTO_SAVE_DELAY: u32 = 5;
pub const DEFAULT_AUTO_SYNC: bool = true;
pub const DEFAULT_AUTO_SYNC_INTERVAL: u32 = 10;
pub const DEFAULT_DEBOUNCE_ENABLED: bool = false;
pub const DEFAULT_DEBOUNCE_DELAY: u32 = 5;
pub const DEFAULT_KEYBOARD_SHORTCUTS: bool = true;

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Read a bool setting. Returns `None` if not set (caller decides default).
fn load_bool(key: &str) -> Option<bool> {
    local_storage()
        .and_then(|s| s.get_item(key).ok()?)
        .map(|v| v == "true")
}

/// Save a bool setting.
fn save_bool(key: &str, value: bool) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, if value { "true" } else { "false" });
    }
}

/// Read a u32 setting. Returns `None` if not set.
fn load_u32(key: &str) -> Option<u32> {
    local_storage()
        .and_then(|s| s.get_item(key).ok()?)
        .and_then(|v| v.parse().ok())
}

/// Save a u32 setting.
fn save_u32(key: &str, value: u32) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, &value.to_string());
    }
}

// -- "has" checks: true if the user has explicitly set a value in localStorage --

pub fn has_drag_drop_reorder() -> bool { load_bool(STORAGE_KEY_DRAG_DROP).is_some() }
pub fn has_show_preview() -> bool { load_bool(STORAGE_KEY_SHOW_PREVIEW).is_some() }
pub fn has_auto_save() -> bool { load_bool(STORAGE_KEY_AUTO_SAVE).is_some() }
pub fn has_auto_save_delay() -> bool { load_u32(STORAGE_KEY_AUTO_SAVE_DELAY).is_some() }
pub fn has_auto_sync() -> bool { load_bool(STORAGE_KEY_AUTO_SYNC).is_some() }
pub fn has_auto_sync_interval() -> bool { load_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL).is_some() }
pub fn has_debounce_enabled() -> bool { load_bool(STORAGE_KEY_DEBOUNCE_ENABLED).is_some() }
pub fn has_debounce_delay() -> bool { load_u32(STORAGE_KEY_DEBOUNCE_DELAY).is_some() }
pub fn has_keyboard_shortcuts() -> bool { load_bool(STORAGE_KEY_KEYBOARD_SHORTCUTS).is_some() }

pub fn load_drag_drop_reorder() -> bool {
    load_bool(STORAGE_KEY_DRAG_DROP).unwrap_or(DEFAULT_DRAG_DROP)
}

pub fn save_drag_drop_reorder(enabled: bool) {
    save_bool(STORAGE_KEY_DRAG_DROP, enabled);
}

pub fn load_show_preview() -> bool {
    load_bool(STORAGE_KEY_SHOW_PREVIEW).unwrap_or(DEFAULT_SHOW_PREVIEW)
}

pub fn save_show_preview(enabled: bool) {
    save_bool(STORAGE_KEY_SHOW_PREVIEW, enabled);
}

pub fn load_auto_save() -> bool {
    load_bool(STORAGE_KEY_AUTO_SAVE).unwrap_or(DEFAULT_AUTO_SAVE)
}

pub fn save_auto_save(enabled: bool) {
    save_bool(STORAGE_KEY_AUTO_SAVE, enabled);
}

pub fn load_auto_save_delay() -> u32 {
    load_u32(STORAGE_KEY_AUTO_SAVE_DELAY).unwrap_or(DEFAULT_AUTO_SAVE_DELAY)
}

pub fn save_auto_save_delay(secs: u32) {
    save_u32(STORAGE_KEY_AUTO_SAVE_DELAY, secs);
}

pub fn load_auto_sync() -> bool {
    load_bool(STORAGE_KEY_AUTO_SYNC).unwrap_or(DEFAULT_AUTO_SYNC)
}

pub fn save_auto_sync(enabled: bool) {
    save_bool(STORAGE_KEY_AUTO_SYNC, enabled);
}

pub fn load_auto_sync_interval() -> u32 {
    load_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL).unwrap_or(DEFAULT_AUTO_SYNC_INTERVAL)
}

pub fn save_auto_sync_interval(secs: u32) {
    save_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL, secs);
}

pub fn load_debounce_enabled() -> bool {
    load_bool(STORAGE_KEY_DEBOUNCE_ENABLED).unwrap_or(DEFAULT_DEBOUNCE_ENABLED)
}

pub fn save_debounce_enabled(enabled: bool) {
    save_bool(STORAGE_KEY_DEBOUNCE_ENABLED, enabled);
}

pub fn load_debounce_delay() -> u32 {
    load_u32(STORAGE_KEY_DEBOUNCE_DELAY).unwrap_or(DEFAULT_DEBOUNCE_DELAY)
}

pub fn save_debounce_delay(secs: u32) {
    save_u32(STORAGE_KEY_DEBOUNCE_DELAY, secs);
}

pub fn load_keyboard_shortcuts() -> bool {
    load_bool(STORAGE_KEY_KEYBOARD_SHORTCUTS).unwrap_or(DEFAULT_KEYBOARD_SHORTCUTS)
}

pub fn save_keyboard_shortcuts(enabled: bool) {
    save_bool(STORAGE_KEY_KEYBOARD_SHORTCUTS, enabled);
}
