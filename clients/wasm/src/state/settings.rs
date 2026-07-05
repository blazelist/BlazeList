// Device-local settings persisted in `localStorage`.
//
// These settings are not synced to the server — they stay on the device.

// On the host test build, only the pure migration/validation logic has
// callers; the load_*/save_*/has_* helpers are reachable only from
// modules gated to `wasm32` (app.rs, store.rs, settings_panel.rs).
// Silence dead-code lints so the test target stays warning-clean.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

const STORAGE_KEY_SHOW_PREVIEW: &str = "blazelist_show_preview";
const STORAGE_KEY_AUTO_SYNC: &str = "blazelist_auto_sync";
const STORAGE_KEY_AUTO_SYNC_INTERVAL_MS: &str = "blazelist_auto_sync_interval_ms";
const STORAGE_KEY_PRIORITY_DEBOUNCE_ENABLED: &str = "blazelist_priority_debounce_enabled";
const STORAGE_KEY_PRIORITY_DEBOUNCE_DELAY_MS: &str = "blazelist_priority_debounce_delay_ms";
const STORAGE_KEY_KEYBOARD_SHORTCUTS: &str = "blazelist_keyboard_shortcuts";
const STORAGE_KEY_SEARCH_TAGS: &str = "blazelist_search_tags";
const STORAGE_KEY_UI_SCALE: &str = "blazelist_ui_scale";
const STORAGE_KEY_UI_DENSITY: &str = "blazelist_ui_density";
const STORAGE_KEY_TOUCH_SWIPE: &str = "blazelist_touch_swipe";
const STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_CYCLE: &str = "blazelist_swipe_threshold_right_cycle";
const STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_LEVELS: &str = "blazelist_swipe_threshold_right_levels";
const STORAGE_KEY_SWIPE_THRESHOLD_LEFT_CYCLE: &str = "blazelist_swipe_threshold_left_cycle";
const STORAGE_KEY_SWIPE_THRESHOLD_LEFT_LEVELS: &str = "blazelist_swipe_threshold_left_levels";
const STORAGE_KEY_SWIPE_UNDO_TIMEOUT_MS: &str = "blazelist_swipe_undo_timeout_ms";
const STORAGE_KEY_SWIPE_LEFT_MODE: &str = "blazelist_swipe_left_mode";
const STORAGE_KEY_SWIPE_LEVELS_ZONE_TODAY_WIDTH: &str = "blazelist_swipe_levels_zone_today_width";
const STORAGE_KEY_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH: &str =
    "blazelist_swipe_levels_zone_tomorrow_width";
const STORAGE_KEY_SWIPE_LEVELS_ZONE_SOON_WIDTH: &str = "blazelist_swipe_levels_zone_soon_width";
const STORAGE_KEY_CLEAR_TAG_SEARCH: &str = "blazelist_clear_tag_search";
const STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH: &str = "blazelist_default_sidebar_width";
const STORAGE_KEY_DEFAULT_DETAIL_WIDTH: &str = "blazelist_default_detail_width";
const STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH: &str = "blazelist_override_sidebar_width";
const STORAGE_KEY_OVERRIDE_DETAIL_WIDTH: &str = "blazelist_override_detail_width";
const STORAGE_KEY_SHOW_DUE_TODAY_BUTTON: &str = "blazelist_show_due_today_button";
const STORAGE_KEY_RECURSIVE_LINKS: &str = "blazelist_recursive_links";
const STORAGE_KEY_SHOW_LIST_LINK_COUNTS: &str = "blazelist_show_list_link_counts";
const STORAGE_KEY_SHOW_CARD_TIME: &str = "blazelist_show_card_time";
const STORAGE_KEY_EXTINGUISH_ON_DUE_SET: &str = "blazelist_extinguish_on_due_set";
const STORAGE_KEY_EXTINGUISH_ON_DUE_CLEAR: &str = "blazelist_extinguish_on_due_clear";
const STORAGE_KEY_CLEAR_DUE_ON_BLAZE: &str = "blazelist_clear_due_on_blaze";
const STORAGE_KEY_DRAG_AND_DROP_ENABLED: &str = "blazelist_drag_and_drop_enabled";
const STORAGE_KEY_DRAG_AND_DROP_MODE: &str = "blazelist_drag_and_drop_mode";
const STORAGE_KEY_CACHE_SCHEMA: &str = "blazelist_cache_schema";

/// Shared prefix on every localStorage key the WASM client owns. The
/// startup sweep only touches keys with this prefix so it cannot
/// clobber state owned by other apps on the same origin.
const BLAZELIST_KEY_PREFIX: &str = "blazelist_";

/// All user-setting keys — every entry the settings panel can read or
/// write. `clear_all_settings` walks this list directly, and the startup
/// migration treats anything else (apart from `NON_SETTING_KEYS` below)
/// as stale and prunes it. **Adding a new setting requires appending
/// here; the migration tests catch omissions.**
const ALL_SETTING_KEYS: &[&str] = &[
    STORAGE_KEY_SHOW_PREVIEW,
    STORAGE_KEY_AUTO_SYNC,
    STORAGE_KEY_AUTO_SYNC_INTERVAL_MS,
    STORAGE_KEY_PRIORITY_DEBOUNCE_ENABLED,
    STORAGE_KEY_PRIORITY_DEBOUNCE_DELAY_MS,
    STORAGE_KEY_KEYBOARD_SHORTCUTS,
    STORAGE_KEY_SEARCH_TAGS,
    STORAGE_KEY_UI_SCALE,
    STORAGE_KEY_UI_DENSITY,
    STORAGE_KEY_TOUCH_SWIPE,
    STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_CYCLE,
    STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_LEVELS,
    STORAGE_KEY_SWIPE_THRESHOLD_LEFT_CYCLE,
    STORAGE_KEY_SWIPE_THRESHOLD_LEFT_LEVELS,
    STORAGE_KEY_SWIPE_UNDO_TIMEOUT_MS,
    STORAGE_KEY_SWIPE_LEFT_MODE,
    STORAGE_KEY_SWIPE_LEVELS_ZONE_TODAY_WIDTH,
    STORAGE_KEY_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH,
    STORAGE_KEY_SWIPE_LEVELS_ZONE_SOON_WIDTH,
    STORAGE_KEY_CLEAR_TAG_SEARCH,
    STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH,
    STORAGE_KEY_DEFAULT_DETAIL_WIDTH,
    STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH,
    STORAGE_KEY_OVERRIDE_DETAIL_WIDTH,
    STORAGE_KEY_SHOW_DUE_TODAY_BUTTON,
    STORAGE_KEY_RECURSIVE_LINKS,
    STORAGE_KEY_SHOW_LIST_LINK_COUNTS,
    STORAGE_KEY_SHOW_CARD_TIME,
    STORAGE_KEY_EXTINGUISH_ON_DUE_SET,
    STORAGE_KEY_EXTINGUISH_ON_DUE_CLEAR,
    STORAGE_KEY_CLEAR_DUE_ON_BLAZE,
    STORAGE_KEY_DRAG_AND_DROP_ENABLED,
    STORAGE_KEY_DRAG_AND_DROP_MODE,
];

/// Non-setting keys the WASM client legitimately owns under the
/// `blazelist_` prefix. These are NOT cleared by `clear_all_settings`
/// (they aren't user settings) but the migration sweep must allow them.
const NON_SETTING_KEYS: &[&str] = &[STORAGE_KEY_CACHE_SCHEMA];

/// Default values (used when localStorage has no value and no server override).
pub const DEFAULT_SHOW_PREVIEW: bool = false;
pub const DEFAULT_AUTO_SYNC: bool = true;
pub const DEFAULT_AUTO_SYNC_INTERVAL_MS: u32 = 10_000;
pub const DEFAULT_PRIORITY_DEBOUNCE_ENABLED: bool = true;
pub const DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS: u32 = 3_000;
pub const DEFAULT_KEYBOARD_SHORTCUTS: bool = true;
pub const DEFAULT_SEARCH_TAGS: bool = true;
pub const DEFAULT_UI_SCALE: u32 = 100;
pub const DEFAULT_UI_DENSITY: &str = "compact";
pub const DEFAULT_TOUCH_SWIPE: bool = false;
/// Trigger distance (px) for swipe-right in `cycle` mode.
pub const DEFAULT_SWIPE_THRESHOLD_RIGHT_CYCLE: u32 = 135;
/// Trigger distance (px) for swipe-right in `levels` mode.
pub const DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS: u32 = 135;
/// Trigger distance (px) for swipe-left in `cycle` mode.
pub const DEFAULT_SWIPE_THRESHOLD_LEFT_CYCLE: u32 = 115;
/// Trigger distance (px) for swipe-left in `levels` mode. Doubles as the
/// start of the Today zone — the additive zone widths extend outward
/// from this point.
pub const DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS: u32 = 95;
pub const DEFAULT_SWIPE_UNDO_TIMEOUT_MS: u32 = 4_000;
/// Swipe-left interaction mode: `"levels"` (zone-based, default) or
/// `"cycle"` (cycles through today/tomorrow/in-2-days/clear on each swipe).
pub const DEFAULT_SWIPE_LEFT_MODE: &str = "levels";
/// Width (px) of the Today zone in levels-mode swipe-left. Zones extend
/// outward from `swipe_threshold_left_levels` and are additive: the
/// Tomorrow zone starts at `threshold_l_levels + zone_today_width`, etc.
pub const DEFAULT_SWIPE_LEVELS_ZONE_TODAY_WIDTH: u32 = 75;
/// Width (px) of the Tomorrow zone in levels-mode swipe-left.
pub const DEFAULT_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH: u32 = 60;
/// Width (px) of the In-2-days ("Soon") zone in levels-mode swipe-left.
/// Beyond this zone the swipe enters the open-ended Clear-due region.
pub const DEFAULT_SWIPE_LEVELS_ZONE_SOON_WIDTH: u32 = 55;
pub const DEFAULT_CLEAR_TAG_SEARCH: bool = true;
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 180;
pub const DEFAULT_DETAIL_WIDTH: u32 = 0;
pub const DEFAULT_OVERRIDE_SIDEBAR_WIDTH: bool = false;
pub const DEFAULT_OVERRIDE_DETAIL_WIDTH: bool = false;
pub const DEFAULT_SHOW_DUE_TODAY_BUTTON: bool = true;
pub const DEFAULT_RECURSIVE_LINKS: bool = true;
pub const DEFAULT_SHOW_LIST_LINK_COUNTS: bool = true;
pub const DEFAULT_SHOW_CARD_TIME: bool = false;
pub const DEFAULT_EXTINGUISH_ON_DUE_SET: bool = true;
pub const DEFAULT_EXTINGUISH_ON_DUE_CLEAR: bool = true;
pub const DEFAULT_CLEAR_DUE_ON_BLAZE: bool = true;
pub const DEFAULT_DRAG_AND_DROP_ENABLED: bool = false;
/// Drag-and-drop activation mode. `"anywhere"` (desktop-friendly,
/// default): pointerdown anywhere on the card row + a small movement
/// threshold starts the drag. `"handle"` (mobile-friendly): only the
/// card's leading number starts the drag, so native scroll and
/// existing swipes still work on the rest of the card.
pub const DEFAULT_DRAG_AND_DROP_MODE: &str = "anywhere";

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

/// Remove a key from localStorage if it exists. Used by the self-healing
/// loaders to evict values that no version of the running build accepts.
fn remove_key(key: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
}

/// Load a string-enum setting, validating against the allowed set. If the
/// stored value is not accepted by `is_valid`, the stale key is removed
/// from localStorage and the compile-time default is returned. This is
/// what lets the app self-heal from values written by older builds with
/// a different enum (e.g. `swipe_left_mode = "toggle"` from before 4.0.0).
fn load_enum_string(key: &str, is_valid: fn(&str) -> bool, default: &str) -> String {
    match load_string(key) {
        Some(v) if is_valid(&v) => v,
        Some(_) => {
            remove_key(key);
            default.to_string()
        }
        None => default.to_string(),
    }
}

/// Companion to [`load_enum_string`]: only counts as "user-set" if the
/// stored value passes `is_valid`. Lets `apply_server_config` overwrite
/// an unrecognised legacy value with the server's default.
fn has_valid_enum_string(key: &str, is_valid: fn(&str) -> bool) -> bool {
    load_string(key).as_deref().is_some_and(is_valid)
}

/// Allowed values for `swipe_left_mode`. Older builds shipped
/// `"toggle"` and later `"distance"`; both are rejected now so the
/// loader can reset to the current default.
pub fn is_valid_swipe_left_mode(s: &str) -> bool {
    matches!(s, "levels" | "cycle")
}

/// Allowed values for `drag_and_drop_mode`. Matches the `<option>`s in
/// the settings panel.
pub fn is_valid_drag_and_drop_mode(s: &str) -> bool {
    matches!(s, "anywhere" | "handle")
}

/// Allowed values for `ui_density`. Matches the `<option>`s in the
/// settings panel.
pub fn is_valid_ui_density(s: &str) -> bool {
    matches!(s, "compact" | "cozy")
}

/// Pure predicate over the allowed-key set. Pulled out so it can be
/// covered by host-runnable tests without touching `web_sys`.
fn is_known_key(key: &str) -> bool {
    ALL_SETTING_KEYS.contains(&key) || NON_SETTING_KEYS.contains(&key)
}

/// Pure-logic helper: given a snapshot of every key currently in
/// localStorage, return the subset to prune. A key is pruned when it
/// carries the `blazelist_` prefix but isn't one this build owns —
/// catches both legacy renames (e.g. `blazelist_swipe_threshold_left`
/// after the 4.0.0 split) and any unknown leftovers from manual
/// tampering or partial migrations. Keys without the prefix belong to
/// other apps on the same origin and are left untouched.
fn keys_to_prune<I, S>(present: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    present
        .into_iter()
        .filter_map(|k| {
            let s = k.as_ref();
            (s.starts_with(BLAZELIST_KEY_PREFIX) && !is_known_key(s)).then(|| s.to_string())
        })
        .collect()
}

/// Startup sweep: enforce the "no lingering data" policy by deleting
/// every `blazelist_*` localStorage entry the running build doesn't
/// recognise. Combined with the self-healing `load_*` helpers below,
/// this guarantees the on-disk surface matches the current code —
/// users who upgrade from a build with different keys or different
/// enum variants get a clean slate instead of a half-broken UI.
///
/// Safe to call repeatedly: idempotent after the first run.
pub fn run_startup_migrations() {
    let Some(storage) = local_storage() else {
        return;
    };

    // Snapshot the key list before mutating: `remove_item` shifts
    // indices, so iterating by index while removing skips entries.
    let len = storage.length().unwrap_or(0);
    let present: Vec<String> = (0..len)
        .filter_map(|i| storage.key(i).ok().flatten())
        .collect();

    for key in keys_to_prune(present) {
        tracing::info!(key = %key, "Pruning stale localStorage key");
        let _ = storage.remove_item(&key);
    }
}

// -- "has" checks: true if the user has explicitly set a value in localStorage --

pub fn has_show_preview() -> bool {
    load_bool(STORAGE_KEY_SHOW_PREVIEW).is_some()
}
pub fn has_auto_sync() -> bool {
    load_bool(STORAGE_KEY_AUTO_SYNC).is_some()
}
pub fn has_auto_sync_interval_ms() -> bool {
    load_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL_MS).is_some()
}
pub fn has_priority_debounce_enabled() -> bool {
    load_bool(STORAGE_KEY_PRIORITY_DEBOUNCE_ENABLED).is_some()
}
pub fn has_priority_debounce_delay_ms() -> bool {
    load_u32(STORAGE_KEY_PRIORITY_DEBOUNCE_DELAY_MS).is_some()
}
pub fn has_keyboard_shortcuts() -> bool {
    load_bool(STORAGE_KEY_KEYBOARD_SHORTCUTS).is_some()
}
pub fn has_search_tags() -> bool {
    load_bool(STORAGE_KEY_SEARCH_TAGS).is_some()
}
pub fn has_ui_scale() -> bool {
    load_u32(STORAGE_KEY_UI_SCALE).is_some()
}
pub fn has_ui_density() -> bool {
    has_valid_enum_string(STORAGE_KEY_UI_DENSITY, is_valid_ui_density)
}
pub fn has_touch_swipe() -> bool {
    load_bool(STORAGE_KEY_TOUCH_SWIPE).is_some()
}
pub fn has_swipe_threshold_right_cycle() -> bool {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_CYCLE).is_some()
}
pub fn has_swipe_threshold_right_levels() -> bool {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_LEVELS).is_some()
}
pub fn has_swipe_threshold_left_cycle() -> bool {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_CYCLE).is_some()
}
pub fn has_swipe_threshold_left_levels() -> bool {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_LEVELS).is_some()
}
pub fn has_swipe_undo_timeout_ms() -> bool {
    load_u32(STORAGE_KEY_SWIPE_UNDO_TIMEOUT_MS).is_some()
}
pub fn has_swipe_left_mode() -> bool {
    has_valid_enum_string(STORAGE_KEY_SWIPE_LEFT_MODE, is_valid_swipe_left_mode)
}
pub fn has_swipe_levels_zone_today_width() -> bool {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TODAY_WIDTH).is_some()
}
pub fn has_swipe_levels_zone_tomorrow_width() -> bool {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH).is_some()
}
pub fn has_swipe_levels_zone_soon_width() -> bool {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_SOON_WIDTH).is_some()
}
pub fn has_clear_tag_search() -> bool {
    load_bool(STORAGE_KEY_CLEAR_TAG_SEARCH).is_some()
}
pub fn has_default_sidebar_width() -> bool {
    load_u32(STORAGE_KEY_DEFAULT_SIDEBAR_WIDTH).is_some()
}
pub fn has_default_detail_width() -> bool {
    load_u32(STORAGE_KEY_DEFAULT_DETAIL_WIDTH).is_some()
}
pub fn has_override_sidebar_width() -> bool {
    load_bool(STORAGE_KEY_OVERRIDE_SIDEBAR_WIDTH).is_some()
}
pub fn has_override_detail_width() -> bool {
    load_bool(STORAGE_KEY_OVERRIDE_DETAIL_WIDTH).is_some()
}
pub fn has_show_due_today_button() -> bool {
    load_bool(STORAGE_KEY_SHOW_DUE_TODAY_BUTTON).is_some()
}
pub fn has_recursive_links() -> bool {
    load_bool(STORAGE_KEY_RECURSIVE_LINKS).is_some()
}
pub fn has_show_list_link_counts() -> bool {
    load_bool(STORAGE_KEY_SHOW_LIST_LINK_COUNTS).is_some()
}
pub fn has_show_card_time() -> bool {
    load_bool(STORAGE_KEY_SHOW_CARD_TIME).is_some()
}
pub fn has_extinguish_on_due_set() -> bool {
    load_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_SET).is_some()
}
pub fn has_extinguish_on_due_clear() -> bool {
    load_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_CLEAR).is_some()
}
pub fn has_clear_due_on_blaze() -> bool {
    load_bool(STORAGE_KEY_CLEAR_DUE_ON_BLAZE).is_some()
}
pub fn has_drag_and_drop_enabled() -> bool {
    load_bool(STORAGE_KEY_DRAG_AND_DROP_ENABLED).is_some()
}
pub fn has_drag_and_drop_mode() -> bool {
    has_valid_enum_string(STORAGE_KEY_DRAG_AND_DROP_MODE, is_valid_drag_and_drop_mode)
}

pub fn load_show_preview() -> bool {
    load_bool(STORAGE_KEY_SHOW_PREVIEW).unwrap_or(DEFAULT_SHOW_PREVIEW)
}

pub fn save_show_preview(enabled: bool) {
    save_bool(STORAGE_KEY_SHOW_PREVIEW, enabled);
}

pub fn load_auto_sync() -> bool {
    load_bool(STORAGE_KEY_AUTO_SYNC).unwrap_or(DEFAULT_AUTO_SYNC)
}

pub fn save_auto_sync(enabled: bool) {
    save_bool(STORAGE_KEY_AUTO_SYNC, enabled);
}

pub fn load_auto_sync_interval_ms() -> u32 {
    load_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL_MS).unwrap_or(DEFAULT_AUTO_SYNC_INTERVAL_MS)
}

pub fn save_auto_sync_interval_ms(ms: u32) {
    save_u32(STORAGE_KEY_AUTO_SYNC_INTERVAL_MS, ms);
}

pub fn load_priority_debounce_enabled() -> bool {
    load_bool(STORAGE_KEY_PRIORITY_DEBOUNCE_ENABLED).unwrap_or(DEFAULT_PRIORITY_DEBOUNCE_ENABLED)
}

pub fn save_priority_debounce_enabled(enabled: bool) {
    save_bool(STORAGE_KEY_PRIORITY_DEBOUNCE_ENABLED, enabled);
}

pub fn load_priority_debounce_delay_ms() -> u32 {
    load_u32(STORAGE_KEY_PRIORITY_DEBOUNCE_DELAY_MS).unwrap_or(DEFAULT_PRIORITY_DEBOUNCE_DELAY_MS)
}

pub fn save_priority_debounce_delay_ms(ms: u32) {
    save_u32(STORAGE_KEY_PRIORITY_DEBOUNCE_DELAY_MS, ms);
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
    load_enum_string(
        STORAGE_KEY_UI_DENSITY,
        is_valid_ui_density,
        DEFAULT_UI_DENSITY,
    )
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

pub fn load_swipe_threshold_right_cycle() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_CYCLE).unwrap_or(DEFAULT_SWIPE_THRESHOLD_RIGHT_CYCLE)
}

pub fn save_swipe_threshold_right_cycle(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_CYCLE, px);
}

pub fn load_swipe_threshold_right_levels() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_LEVELS)
        .unwrap_or(DEFAULT_SWIPE_THRESHOLD_RIGHT_LEVELS)
}

pub fn save_swipe_threshold_right_levels(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_RIGHT_LEVELS, px);
}

pub fn load_swipe_threshold_left_cycle() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_CYCLE).unwrap_or(DEFAULT_SWIPE_THRESHOLD_LEFT_CYCLE)
}

pub fn save_swipe_threshold_left_cycle(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_CYCLE, px);
}

pub fn load_swipe_threshold_left_levels() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_LEVELS).unwrap_or(DEFAULT_SWIPE_THRESHOLD_LEFT_LEVELS)
}

pub fn save_swipe_threshold_left_levels(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_THRESHOLD_LEFT_LEVELS, px);
}

pub fn load_swipe_undo_timeout_ms() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_UNDO_TIMEOUT_MS).unwrap_or(DEFAULT_SWIPE_UNDO_TIMEOUT_MS)
}

pub fn save_swipe_undo_timeout_ms(ms: u32) {
    save_u32(STORAGE_KEY_SWIPE_UNDO_TIMEOUT_MS, ms);
}

pub fn load_swipe_left_mode() -> String {
    load_enum_string(
        STORAGE_KEY_SWIPE_LEFT_MODE,
        is_valid_swipe_left_mode,
        DEFAULT_SWIPE_LEFT_MODE,
    )
}

pub fn save_swipe_left_mode(mode: &str) {
    save_string(STORAGE_KEY_SWIPE_LEFT_MODE, mode);
}

pub fn load_swipe_levels_zone_today_width() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TODAY_WIDTH)
        .unwrap_or(DEFAULT_SWIPE_LEVELS_ZONE_TODAY_WIDTH)
}

pub fn save_swipe_levels_zone_today_width(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TODAY_WIDTH, px);
}

pub fn load_swipe_levels_zone_tomorrow_width() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH)
        .unwrap_or(DEFAULT_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH)
}

pub fn save_swipe_levels_zone_tomorrow_width(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_TOMORROW_WIDTH, px);
}

pub fn load_swipe_levels_zone_soon_width() -> u32 {
    load_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_SOON_WIDTH)
        .unwrap_or(DEFAULT_SWIPE_LEVELS_ZONE_SOON_WIDTH)
}

pub fn save_swipe_levels_zone_soon_width(px: u32) {
    save_u32(STORAGE_KEY_SWIPE_LEVELS_ZONE_SOON_WIDTH, px);
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

pub fn load_show_due_today_button() -> bool {
    load_bool(STORAGE_KEY_SHOW_DUE_TODAY_BUTTON).unwrap_or(DEFAULT_SHOW_DUE_TODAY_BUTTON)
}

pub fn save_show_due_today_button(enabled: bool) {
    save_bool(STORAGE_KEY_SHOW_DUE_TODAY_BUTTON, enabled);
}

pub fn load_recursive_links() -> bool {
    load_bool(STORAGE_KEY_RECURSIVE_LINKS).unwrap_or(DEFAULT_RECURSIVE_LINKS)
}

pub fn save_recursive_links(enabled: bool) {
    save_bool(STORAGE_KEY_RECURSIVE_LINKS, enabled);
}

pub fn load_show_list_link_counts() -> bool {
    load_bool(STORAGE_KEY_SHOW_LIST_LINK_COUNTS).unwrap_or(DEFAULT_SHOW_LIST_LINK_COUNTS)
}

pub fn save_show_list_link_counts(enabled: bool) {
    save_bool(STORAGE_KEY_SHOW_LIST_LINK_COUNTS, enabled);
}

pub fn load_show_card_time() -> bool {
    load_bool(STORAGE_KEY_SHOW_CARD_TIME).unwrap_or(DEFAULT_SHOW_CARD_TIME)
}

pub fn save_show_card_time(enabled: bool) {
    save_bool(STORAGE_KEY_SHOW_CARD_TIME, enabled);
}

pub fn load_extinguish_on_due_set() -> bool {
    load_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_SET).unwrap_or(DEFAULT_EXTINGUISH_ON_DUE_SET)
}

pub fn save_extinguish_on_due_set(enabled: bool) {
    save_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_SET, enabled);
}

pub fn load_extinguish_on_due_clear() -> bool {
    load_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_CLEAR).unwrap_or(DEFAULT_EXTINGUISH_ON_DUE_CLEAR)
}

pub fn save_extinguish_on_due_clear(enabled: bool) {
    save_bool(STORAGE_KEY_EXTINGUISH_ON_DUE_CLEAR, enabled);
}

pub fn load_clear_due_on_blaze() -> bool {
    load_bool(STORAGE_KEY_CLEAR_DUE_ON_BLAZE).unwrap_or(DEFAULT_CLEAR_DUE_ON_BLAZE)
}

pub fn save_clear_due_on_blaze(enabled: bool) {
    save_bool(STORAGE_KEY_CLEAR_DUE_ON_BLAZE, enabled);
}

pub fn load_drag_and_drop_enabled() -> bool {
    load_bool(STORAGE_KEY_DRAG_AND_DROP_ENABLED).unwrap_or(DEFAULT_DRAG_AND_DROP_ENABLED)
}

pub fn save_drag_and_drop_enabled(enabled: bool) {
    save_bool(STORAGE_KEY_DRAG_AND_DROP_ENABLED, enabled);
}

pub fn load_drag_and_drop_mode() -> String {
    load_enum_string(
        STORAGE_KEY_DRAG_AND_DROP_MODE,
        is_valid_drag_and_drop_mode,
        DEFAULT_DRAG_AND_DROP_MODE,
    )
}

pub fn save_drag_and_drop_mode(mode: &str) {
    save_string(STORAGE_KEY_DRAG_AND_DROP_MODE, mode);
}

/// Load the stored cache schema stamp (client-only, e.g. "2.14.0-dev+2.2.0-dev").
///
/// Written by `save_local_state` after a successful write to `blazelist.db`
/// and checked at load time to detect client upgrades that broke the cache
/// format — independently of any server connection.
pub fn load_cache_schema() -> Option<String> {
    load_string(STORAGE_KEY_CACHE_SCHEMA)
}

/// Save the cache schema stamp.
pub fn save_cache_schema(stamp: &str) {
    save_string(STORAGE_KEY_CACHE_SCHEMA, stamp);
}

/// Remove every user-facing setting from localStorage, restoring
/// defaults. The cache-schema stamp in [`NON_SETTING_KEYS`] is left
/// alone — wiping it would force the next launch to re-download the
/// full dataset, which is not what "reset settings" should do.
pub fn clear_all_settings() {
    if let Some(storage) = local_storage() {
        for key in ALL_SETTING_KEYS {
            let _ = storage.remove_item(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -- Pure validators --

    #[test]
    fn swipe_left_mode_accepts_current_values() {
        assert!(is_valid_swipe_left_mode("levels"));
        assert!(is_valid_swipe_left_mode("cycle"));
    }

    #[test]
    fn swipe_left_mode_rejects_pre_4_0_0_values() {
        // Values that existed in earlier builds and would otherwise
        // strand the settings panel with no matching <option>:
        //   * "toggle"   — original cycle-style mode before commit 04b2278
        //   * "distance" — distance-based variant removed by 04b2278
        assert!(!is_valid_swipe_left_mode("toggle"));
        assert!(!is_valid_swipe_left_mode("distance"));
    }

    #[test]
    fn swipe_left_mode_rejects_garbage() {
        assert!(!is_valid_swipe_left_mode(""));
        assert!(!is_valid_swipe_left_mode("Levels")); // case-sensitive
        assert!(!is_valid_swipe_left_mode("cycle ")); // trailing space
        assert!(!is_valid_swipe_left_mode("foobar"));
    }

    #[test]
    fn ui_density_accepts_current_values() {
        assert!(is_valid_ui_density("compact"));
        assert!(is_valid_ui_density("cozy"));
    }

    #[test]
    fn ui_density_rejects_unknown_values() {
        assert!(!is_valid_ui_density(""));
        assert!(!is_valid_ui_density("normal"));
        assert!(!is_valid_ui_density("Compact"));
    }

    #[test]
    fn drag_and_drop_mode_accepts_current_values() {
        assert!(is_valid_drag_and_drop_mode("anywhere"));
        assert!(is_valid_drag_and_drop_mode("handle"));
    }

    #[test]
    fn drag_and_drop_mode_rejects_unknown_values() {
        assert!(!is_valid_drag_and_drop_mode(""));
        assert!(!is_valid_drag_and_drop_mode("Anywhere"));
        assert!(!is_valid_drag_and_drop_mode("handle ")); // trailing space
        assert!(!is_valid_drag_and_drop_mode("desktop"));
        assert!(!is_valid_drag_and_drop_mode("mobile"));
    }

    #[test]
    fn defaults_pass_their_validators() {
        // If the compile-time default ever fails its own validator, the
        // load_* self-heal path would recurse on its own output. Guard.
        assert!(is_valid_swipe_left_mode(DEFAULT_SWIPE_LEFT_MODE));
        assert!(is_valid_ui_density(DEFAULT_UI_DENSITY));
        assert!(is_valid_drag_and_drop_mode(DEFAULT_DRAG_AND_DROP_MODE));
    }

    // -- Migration / key-set hygiene --

    #[test]
    fn every_setting_key_uses_the_blazelist_prefix() {
        for key in ALL_SETTING_KEYS.iter().chain(NON_SETTING_KEYS.iter()) {
            assert!(
                key.starts_with(BLAZELIST_KEY_PREFIX),
                "{key} is missing the {BLAZELIST_KEY_PREFIX:?} prefix — the migration sweep skips it"
            );
        }
    }

    #[test]
    fn no_duplicate_keys_across_categories() {
        let mut seen: HashSet<&str> = HashSet::new();
        for key in ALL_SETTING_KEYS.iter().chain(NON_SETTING_KEYS.iter()) {
            assert!(seen.insert(key), "duplicate key registered: {key}");
        }
    }

    #[test]
    fn keys_to_prune_keeps_recognised_keys() {
        // Every known key, plus an unrelated non-blazelist key, must
        // survive the sweep.
        let mut input: Vec<String> = ALL_SETTING_KEYS
            .iter()
            .chain(NON_SETTING_KEYS.iter())
            .map(|s| (*s).to_string())
            .collect();
        input.push("third_party_app_state".to_string());

        assert!(
            keys_to_prune(input).is_empty(),
            "sweep wrongly flagged a recognised key for removal"
        );
    }

    #[test]
    fn keys_to_prune_drops_renamed_swipe_threshold_keys() {
        // Regression: when the 4.0.0 swipe-threshold split landed
        // (commit 2d3bd24) the old keys were stranded with the user's
        // customisation. The sweep must remove them or the settings
        // panel stays half-broken until the user hits "clear all".
        let input = vec![
            "blazelist_swipe_threshold_right".to_string(),
            "blazelist_swipe_threshold_left".to_string(),
        ];
        let pruned: HashSet<String> = keys_to_prune(input).into_iter().collect();
        assert!(pruned.contains("blazelist_swipe_threshold_right"));
        assert!(pruned.contains("blazelist_swipe_threshold_left"));
    }

    #[test]
    fn keys_to_prune_drops_unknown_blazelist_keys() {
        // The policy is "no lingering data": anything under the
        // blazelist prefix that the current build doesn't recognise is
        // garbage and must be removed, not just the keys we
        // specifically remember renaming.
        let input = vec![
            "blazelist_made_up_setting".to_string(),
            "blazelist_legacy_thing_we_forgot".to_string(),
        ];
        let pruned: HashSet<String> = keys_to_prune(input).into_iter().collect();
        assert!(pruned.contains("blazelist_made_up_setting"));
        assert!(pruned.contains("blazelist_legacy_thing_we_forgot"));
    }

    #[test]
    fn keys_to_prune_leaves_foreign_prefixes_alone() {
        // Another app on the same origin must not be touched, even
        // when its keys look settings-shaped.
        let input = vec![
            "other_app_show_preview".to_string(),
            "totally_unrelated".to_string(),
        ];
        assert!(keys_to_prune(input).is_empty());
    }

    #[test]
    fn keys_to_prune_keeps_cache_schema() {
        // CACHE_SCHEMA isn't a setting but the running build owns it
        // (see sync.rs). Pruning it would force a needless full
        // re-sync on every startup.
        let input = vec![STORAGE_KEY_CACHE_SCHEMA.to_string()];
        assert!(keys_to_prune(input).is_empty());
    }

    #[test]
    fn clear_all_settings_covers_every_setting_key() {
        // `clear_all_settings` walks ALL_SETTING_KEYS directly, so this
        // is really a sanity check that we didn't accidentally split
        // the list and let a key fall through. After a settings reset
        // the migration sweep must have nothing left to do for any
        // setting key.
        let post_clear: Vec<String> = NON_SETTING_KEYS.iter().map(|s| (*s).to_string()).collect();
        assert!(
            keys_to_prune(post_clear).is_empty(),
            "non-setting keys should never be flagged for prune"
        );

        // And every setting key, considered in isolation, is
        // recognised — otherwise clear_all_settings would skip it.
        for key in ALL_SETTING_KEYS {
            assert!(
                is_known_key(key),
                "setting key {key} is not registered as known"
            );
        }
    }
}
