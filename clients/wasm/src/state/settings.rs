/// Device-local settings persisted in `localStorage`.
///
/// These settings are not synced to the server — they stay on the device.

const STORAGE_KEY_SHOW_PREVIEW: &str = "blazelist_show_preview";
const STORAGE_KEY_AUTO_SAVE: &str = "blazelist_auto_save";
const STORAGE_KEY_AUTO_SAVE_DELAY: &str = "blazelist_auto_save_delay";
const STORAGE_KEY_AUTO_SYNC: &str = "blazelist_auto_sync";
const STORAGE_KEY_AUTO_SYNC_INTERVAL: &str = "blazelist_auto_sync_interval";
const STORAGE_KEY_DEBOUNCE_ENABLED: &str = "blazelist_debounce_enabled";
const STORAGE_KEY_DEBOUNCE_DELAY: &str = "blazelist_debounce_delay";
const STORAGE_KEY_KEYBOARD_SHORTCUTS: &str = "blazelist_keyboard_shortcuts";
const STORAGE_KEY_SEARCH_TAGS: &str = "blazelist_search_tags";
const STORAGE_KEY_UI_SCALE: &str = "blazelist_ui_scale";
const STORAGE_KEY_UI_DENSITY: &str = "blazelist_ui_density";
const STORAGE_KEY_TOUCH_SWIPE: &str = "blazelist_touch_swipe";
const STORAGE_KEY_SWIPE_THRESHOLD_RIGHT: &str = "blazelist_swipe_threshold_right";
const STORAGE_KEY_SWIPE_THRESHOLD_LEFT: &str = "blazelist_swipe_threshold_left";
const STORAGE_KEY_CLEAR_TAG_SEARCH: &str = "blazelist_clear_tag_search";
const STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH: &str = "blazelist_default_sidebar_width";
const STORAGE_KEY_DEFAULT_DETAIL_WIDTH: &str = "blazelist_default_detail_width";
const STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH: &str = "blazelist_override_sidebar_width";
const STORAGE_KEY_OVERRIDE_DETAIL_WIDTH: &str = "blazelist_override_detail_width";

/// Default values (used when localStorage has no value and no server override).
pub const DEFAULT_SHOW_PREVIEW: bool = false;
pub const DEFAULT_AUTO_SAVE: bool = false;
pub const DEFAULT_AUTO_SAVE_DELAY: u32 = 5;
pub const DEFAULT_AUTO_SYNC: bool = true;
pub const DEFAULT_AUTO_SYNC_INTERVAL: u32 = 10;
pub const DEFAULT_DEBOUNCE_ENABLED: bool = false;
pub const DEFAULT_DEBOUNCE_DELAY: u32 = 5;
pub const DEFAULT_KEYBOARD_SHORTCUTS: bool = true;
pub const DEFAULT_SEARCH_TAGS: bool = true;
pub const DEFAULT_UI_SCALE: u32 = 100;
pub const DEFAULT_UI_DENSITY: &str = "compact";
pub const DEFAULT_TOUCH_SWIPE: bool = false;
pub const DEFAULT_SWIPE_THRESHOLD_RIGHT: u32 = 100;
pub const DEFAULT_SWIPE_THRESHOLD_LEFT: u32 = 90;
pub const DEFAULT_CLEAR_TAG_SEARCH: bool = true;
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 180;
pub const DEFAULT_DETAIL_WIDTH: u32 = 0;
pub const DEFAULT_OVERRIDE_SIDEBAR_WIDTH: bool = false;
pub const DEFAULT_OVERRIDE_DETAIL_WIDTH: bool = false;

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

/// Read a string setting. Returns `None` if not set.
fn load_string(key: &str) -> Option<String> {
    local_storage().and_then(|s| s.get_item(key).ok()?)
}

/// Save a string setting.
fn save_string(key: &str, value: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
}

// -- "has" checks: true if the user has explicitly set a value in localStorage --

pub fn has_show_preview() -> bool { load_bool(STORAGE_KEY_SHOW_PREVIEW).is_some() }
pub fn has_auto_save() -> bool { load_bool(STORAGE_KEY_AUTO_SAVE).is_some() }
pub fn has_auto_save_delay() -> bool { load_u32(STORAGE_KEY_AUTO_SAVE_DELAY).is_some() }
pub fn has_auto_sync() -> bool { load_bool(STORAGE_KEY_AUTO_SYNC).is_some() }
pub fn has_auto_sync_interval() -> bool { load_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL).is_some() }
pub fn has_debounce_enabled() -> bool { load_bool(STORAGE_KEY_DEBOUNCE_ENABLED).is_some() }
pub fn has_debounce_delay() -> bool { load_u32(STORAGE_KEY_DEBOUNCE_DELAY).is_some() }
pub fn has_keyboard_shortcuts() -> bool { load_bool(STORAGE_KEY_KEYBOARD_SHORTCUTS).is_some() }
pub fn has_search_tags() -> bool { load_bool(STORAGE_KEY_SEARCH_TAGS).is_some() }
pub fn has_ui_scale() -> bool { load_u32(STORAGE_KEY_UI_SCALE).is_some() }
pub fn has_ui_density() -> bool { load_string(STORAGE_KEY_UI_DENSITY).is_some() }
pub fn has_touch_swipe() -> bool { load_bool(STORAGE_KEY_TOUCH_SWIPE).is_some() }
pub fn has_swipe_threshold_right() -> bool { load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT).is_some() }
pub fn has_swipe_threshold_left() -> bool { load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT).is_some() }
pub fn has_clear_tag_search() -> bool { load_bool(STORAGE_KEY_CLEAR_TAG_SEARCH).is_some() }
pub fn has_default_sidebar_width() -> bool { load_u32(STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH).is_some() }
pub fn has_default_detail_width() -> bool { load_u32(STORAGE_KEY_DEFAULT_DETAIL_WIDTH).is_some() }
pub fn has_override_sidebar_width() -> bool { load_bool(STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH).is_some() }
pub fn has_override_detail_width() -> bool { load_bool(STORAGE_KEY_OVERRIDE_DETAIL_WIDTH).is_some() }

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

pub fn load_search_tags() -> bool {
    load_bool(STORAGE_KEY_SEARCH_TAGS).unwrap_or(DEFAULT_SEARCH_TAGS)
}

pub fn save_search_tags(enabled: bool) {
    save_bool(STORAGE_KEY_SEARCH_TAGS, enabled);
}

pub fn load_ui_scale() -> u32 {
    load_u32(STORAGE_KEY_UI_SCALE).unwrap_or(DEFAULT_UI_SCALE)
}

pub fn save_ui_scale(pct: u32) {
    save_u32(STORAGE_KEY_UI_SCALE, pct);
}

pub fn load_ui_density() -> String {
    load_string(STORAGE_KEY_UI_DENSITY).unwrap_or_else(|| DEFAULT_UI_DENSITY.to_string())
}

pub fn save_ui_density(density: &str) {
    save_string(STORAGE_KEY_UI_DENSITY, density);
}

pub fn load_touch_swipe() -> bool {
    load_bool(STORAGE_KEY_TOUCH_SWIPE).unwrap_or(DEFAULT_TOUCH_SWIPE)
}

pub fn save_touch_swipe(enabled: bool) {
    save_bool(STORAGE_KEY_TOUCH_SWIPE, enabled);
}

pub fn load_swipe_threshold_right() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT).unwrap_or(DEFAULT_SWIPE_THRESHOLD_RIGHT)
}

pub fn save_swipe_threshold_right(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT, px);
}

pub fn load_swipe_threshold_left() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT).unwrap_or(DEFAULT_SWIPE_THRESHOLD_LEFT)
}

pub fn save_swipe_threshold_left(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT, px);
}

pub fn load_clear_tag_search() -> bool {
    load_bool(STORAGE_KEY_CLEAR_TAG_SEARCH).unwrap_or(DEFAULT_CLEAR_TAG_SEARCH)
}

pub fn save_clear_tag_search(enabled: bool) {
    save_bool(STORAGE_KEY_CLEAR_TAG_SEARCH, enabled);
}

pub fn load_default_sidebar_width() -> u32 {
    load_u32(STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH).unwrap_or(DEFAULT_SIDEBAR_WIDTH)
}

pub fn save_default_sidebar_width(px: u32) {
    save_u32(STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH, px);
}

pub fn load_default_detail_width() -> u32 {
    load_u32(STORAGE_KEY_DEFAULT_DETAIL_WIDTH).unwrap_or(DEFAULT_DETAIL_WIDTH)
}

pub fn save_default_detail_width(px: u32) {
    save_u32(STORAGE_KEY_DEFAULT_DETAIL_WIDTH, px);
}

pub fn load_override_sidebar_width() -> bool {
    load_bool(STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH).unwrap_or(DEFAULT_OVERRIDE_SIDEBAR_WIDTH)
}

pub fn save_override_sidebar_width(enabled: bool) {
    save_bool(STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH, enabled);
}

pub fn load_override_detail_width() -> bool {
    load_bool(STORAGE_KEY_OVERRIDE_DETAIL_WIDTH).unwrap_or(DEFAULT_OVERRIDE_DETAIL_WIDTH)
}

pub fn save_override_detail_width(enabled: bool) {
    save_bool(STORAGE_KEY_OVERRIDE_DETAIL_WIDTH, enabled);
}

/// Remove all BlazeList settings from localStorage, restoring defaults.
pub fn clear_all_settings() {
    if let Some(storage) = local_storage() {
        let keys = [
            STORAGE_KEY_SHOW_PREVIEW,
            STORAGE_KEY_AUTO_SAVE,
            STORAGE_KEY_AUTO_SAVE_DELAY,
            STORAGE_KEY_AUTO_SYNC,
            STORAGE_KEY_AUTO_SYNC_INTERVAL,
            STORAGE_KEY_DEBOUNCE_ENABLED,
            STORAGE_KEY_DEBOUNCE_DELAY,
            STORAGE_KEY_KEYBOARD_SHORTCUTS,
            STORAGE_KEY_SEARCH_TAGS,
            STORAGE_KEY_UI_SCALE,
            STORAGE_KEY_UI_DENSITY,
            STORAGE_KEY_TOUCH_SWIPE,
            STORAGE_KEY_SWIPE_THRESHOLD_RIGHT,
            STORAGE_KEY_SWIPE_THRESHOLD_LEFT,
            STORAGE_KEY_CLEAR_TAG_SEARCH,
            STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH,
            STORAGE_KEY_DEFAULT_DETAIL_WIDTH,
            STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH,
            STORAGE_KEY_OVERRIDE_DETAIL_WIDTH,
        ];
        for key in keys {
            let _ = storage.remove_item(key);
        }
    }
}
