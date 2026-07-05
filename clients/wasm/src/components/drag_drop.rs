//! Drag-and-drop card reordering for the WASM client.
//!
//! Gated on the `drag_and_drop_enabled` setting and disabled when a
//! non-default sort is active (same gate as the Shift+J / Shift+K
//! keyboard reorder). Two activation modes selectable in Settings:
//!
//! - `anywhere` (desktop-friendly, default): `pointerdown` anywhere
//!   on the card row + a 5 px movement threshold initiates the drag.
//!   Mouse and pen only; touch events on the body keep their native
//!   behaviour (scroll, swipe).
//! - `handle` (mobile-friendly): the card's leading number is the only
//!   draggable surface. Touch goes through `TouchEvent` directly
//!   because mobile browsers throttle pointermove for touch.
//!
//! Drops flow into the existing `apply_move_placement` pipeline so a
//! reorder coalesces into one history entry per priority-debounce
//! burst, identical to the keyboard path.

use crate::components::card_detail::apply_move_placement;
use crate::state::drag::{DropEdge, drop_position};
use crate::state::store::AppState;
use blazelist_client_lib::priority::move_card;
use blazelist_protocol::Entity;
use leptos::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Event, PointerEvent, TouchEvent, TouchList};

/// Pixels the pointer / finger must move from the down position before
/// a drag activates. Same value for mouse and touch — the handle in
/// touch mode is a deliberate target so the threshold only swallows
/// accidental micro-movements.
const DRAG_ACTIVATION_PX: f64 = 5.0;
/// Distance from the scroll container's visible top/bottom edge where
/// in-drag auto-scroll begins.
const AUTO_SCROLL_EDGE_PX: f64 = 60.0;
/// Maximum pixels-per-frame the container scrolls at the very edge.
const AUTO_SCROLL_MAX_PX_PER_FRAME: f64 = 14.0;
/// Class added to `<body>` while a drag is in flight. Disables text
/// selection across the whole document — a mouse drag from one card
/// to another would otherwise paint a selection sweep.
const BODY_DRAGGING_CLASS: &str = "blazelist-dragging";
/// Time window for the post-drag click swallower. iOS Safari and
/// some desktops fire a synthetic click on whichever element is under
/// the pointer at release; we suppress exactly one for this window so
/// it can't slip past and toggle an unrelated card's selection.
const CLICK_SWALLOW_TIMEOUT_MS: i32 = 300;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setTimeout")]
    fn js_set_timeout(handler: &js_sys::Function, timeout: i32) -> i32;
}

/// Type-erased document event listener storage. Each entry is the
/// event name (for cleanup via `removeEventListener`) and the owned
/// `Closure` keeping the JS shim alive.
type EventListenerEntry = (&'static str, Closure<dyn FnMut(JsValue)>);
/// rAF callback storage — the browser hands the closure a timestamp.
type FrameCallback = Closure<dyn FnMut(f64)>;

thread_local! {
    /// Single in-flight session. `start_*` paths return early if this
    /// slot is already populated, preventing parallel sessions when a
    /// second pointer or finger lands mid-drag.
    static ACTIVE_SESSION: RefCell<Option<Rc<DragSession>>> = const { RefCell::new(None) };
}

/// Locate a touch by identifier in either `touches` (active list) or
/// `changedTouches` (changed-since-last-event). Touch events arrive
/// with the full list rather than a single touch, so we scan rather
/// than assume order.
fn find_touch(list: &TouchList, id: i32) -> Option<web_sys::Touch> {
    for i in 0..list.length() {
        let t = list.get(i)?;
        if t.identifier() == id {
            return Some(t);
        }
    }
    None
}

/// Hit-test the pointer position against rendered card rows.
///
/// Returns `(target_id, edge)` if the pointer sits over a row other
/// than the source. Every inter-card gap canonicalises to
/// `Above(next-card)`: the lower half of card N and the upper half
/// of card N+1 resolve to the same value, so a single indicator
/// lights up per gap. `Below` is emitted only for the last card's
/// lower half. When the pointer is carried past the first or last row
/// (or off the viewport), the target clamps to that end of the list
/// rather than clearing — so a drag toward the top/bottom resolves to a
/// top/bottom drop without the user having to hover precisely on the
/// edge card.
fn hit_test_drop_target(client_x: f64, client_y: f64, source_id: Uuid) -> Option<(Uuid, DropEdge)> {
    if !client_x.is_finite() || !client_y.is_finite() {
        return None;
    }
    let document = web_sys::window()?.document()?;

    // Fast path: a card row sits directly under the pointer. Match
    // `.card-item-wrapper` rather than bare `[data-card-id]` (which also
    // tags detail-panel cross-links) so only real rows resolve.
    if let Some(hit) = document.element_from_point(client_x as f32, client_y as f32)
        && let Some(wrapper) = hit.closest(".card-item-wrapper").ok().flatten()
        && let Some(id) = wrapper
            .get_attribute("data-card-id")
            .and_then(|s| Uuid::parse_str(&s).ok())
    {
        let rect = wrapper.get_bounding_client_rect();
        let mid_y = rect.top() + rect.height() / 2.0;
        if client_y < mid_y {
            if id == source_id {
                return None;
            }
            return Some((id, DropEdge::Above));
        }
        // Bottom half — collapse with the next card's "Above" slot. Filter
        // by `data-card-id` so a future non-card sibling (skeleton row,
        // banner) can't silently mis-route the drop.
        let mut next = wrapper.next_element_sibling();
        while let Some(sib) = next {
            if let Some(next_id) = sib
                .get_attribute("data-card-id")
                .and_then(|s| Uuid::parse_str(&s).ok())
            {
                if next_id == source_id {
                    return None;
                }
                return Some((next_id, DropEdge::Above));
            }
            next = sib.next_element_sibling();
        }
        // No subsequent card — pointer is in the bottom half of the last
        // card; the only valid drop is past the end.
        if id == source_id {
            return None;
        }
        return Some((id, DropEdge::Below));
    }

    // Pointer is past every row (above the first, below the last, or off
    // the viewport): clamp to the nearest list end.
    clamp_to_list_end(&document, client_y, source_id)
}

/// Resolve a drop target when the pointer is past every card row — above
/// the first, below the last, or off the viewport. Clamps to the nearest
/// end so a drag carried past the edge still lands at the top / bottom
/// instead of the indicator vanishing. Scans `.card-item-wrapper`
/// specifically: `[data-card-id]` also matches detail-panel cross-links,
/// which must not be mistaken for rows.
fn clamp_to_list_end(
    document: &web_sys::Document,
    client_y: f64,
    source_id: Uuid,
) -> Option<(Uuid, DropEdge)> {
    let rows = document.query_selector_all(".card-item-wrapper").ok()?;
    let len = rows.length();
    if len == 0 {
        return None;
    }
    let id_of = |el: &web_sys::Element| {
        el.get_attribute("data-card-id")
            .and_then(|s| Uuid::parse_str(&s).ok())
    };

    // Above the first row → drop at the very top.
    let first = rows.item(0)?.dyn_into::<web_sys::Element>().ok()?;
    if client_y <= first.get_bounding_client_rect().top() {
        let id = id_of(&first)?;
        return (id != source_id).then_some((id, DropEdge::Above));
    }

    // Below the last row → drop at the very bottom.
    let last = rows.item(len - 1)?.dyn_into::<web_sys::Element>().ok()?;
    if client_y >= last.get_bounding_client_rect().bottom() {
        let id = id_of(&last)?;
        return (id != source_id).then_some((id, DropEdge::Below));
    }

    None
}

/// Walk up from the source card looking for the nearest ancestor
/// that is currently scrollable (`overflow-y: auto|scroll` AND
/// `scrollHeight > clientHeight`). Falls back to the document's
/// scrolling element if no ancestor qualifies.
fn find_scroll_container(start: &web_sys::Element) -> Option<web_sys::Element> {
    let window = web_sys::window()?;
    let mut node = start.parent_element()?;
    loop {
        if let Ok(style) = window.get_computed_style(&node)
            && let Some(style) = style
            && let Ok(overflow_y) = style.get_property_value("overflow-y")
            && (overflow_y == "auto" || overflow_y == "scroll")
            && node.scroll_height() > node.client_height()
        {
            return Some(node);
        }
        match node.parent_element() {
            Some(parent) => node = parent,
            None => break,
        }
    }
    let document = window.document()?;
    document
        .scrolling_element()
        .or_else(|| document.document_element())
}

fn toggle_body_dragging(on: bool) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    let list = body.class_list();
    let _ = if on {
        list.add_1(BODY_DRAGGING_CLASS)
    } else {
        list.remove_1(BODY_DRAGGING_CLASS)
    };
}

/// Session state shared across the drag's closures. One per in-flight
/// drag; lives in `ACTIVE_SESSION` until teardown.
struct DragSession {
    state: AppState,
    source_id: Uuid,
    /// `pointer_id` for the pointer path, `touch.identifier()` for touch.
    pointer_id: i32,
    start_x: f64,
    start_y: f64,
    active: Cell<bool>,
    /// All document listeners installed for this session, paired with
    /// their event name for cleanup. Type-erased to `JsValue` so one
    /// vec holds both pointer and touch closures.
    listeners: RefCell<Vec<EventListenerEntry>>,
    pending_xy: Cell<Option<(f64, f64)>>,
    last_xy: Cell<Option<(f64, f64)>>,
    raf_handle: Cell<i32>,
    raf_cb: RefCell<Option<FrameCallback>>,
    scroll_container: RefCell<Option<web_sys::Element>>,
}

impl DragSession {
    fn activate(&self) {
        if self.active.get() {
            return;
        }
        self.active.set(true);
        self.state.drag_active_card.set(Some(self.source_id));
        toggle_body_dragging(true);
        self.ensure_scroll_container();
    }

    fn ensure_scroll_container(&self) {
        if self.scroll_container.borrow().is_some() {
            return;
        }
        if let Some(document) = web_sys::window().and_then(|w| w.document())
            && let Some(source_el) = document
                .query_selector(&format!(
                    "[data-card-id=\"{}\"]",
                    self.source_id.as_hyphenated()
                ))
                .ok()
                .flatten()
        {
            *self.scroll_container.borrow_mut() = find_scroll_container(&source_el);
        }
    }

    fn set_drop_target(&self, target: Option<(Uuid, DropEdge)>) {
        if self.state.drag_drop_target.get_untracked() != target {
            self.state.drag_drop_target.set(target);
        }
    }

    /// Scroll the cached container if the pointer is inside the
    /// top/bottom edge band. Returns true when scroll_top actually
    /// changed so the rAF loop knows to keep running.
    fn auto_scroll_step(&self, client_y: f64) -> bool {
        let container = match self.scroll_container.borrow().clone() {
            Some(c) => c,
            None => return false,
        };
        let rect = container.get_bounding_client_rect();
        let viewport_h = web_sys::window()
            .and_then(|w| w.inner_height().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(f64::INFINITY);
        let top = rect.top().max(0.0);
        let bottom = rect.bottom().min(viewport_h);
        let above = AUTO_SCROLL_EDGE_PX - (client_y - top);
        let below = AUTO_SCROLL_EDGE_PX - (bottom - client_y);
        let delta = if above > 0.0 {
            -(above / AUTO_SCROLL_EDGE_PX) * AUTO_SCROLL_MAX_PX_PER_FRAME
        } else if below > 0.0 {
            (below / AUTO_SCROLL_EDGE_PX) * AUTO_SCROLL_MAX_PX_PER_FRAME
        } else {
            0.0
        };
        if delta.abs() < 0.5 {
            return false;
        }
        let before = container.scroll_top();
        container.set_scroll_top(before + delta);
        (container.scroll_top() - before).abs() > 0.5
    }

    fn finish(&self) {
        let raf = self.raf_handle.replace(0);
        if raf != 0
            && let Some(window) = web_sys::window()
        {
            window.cancel_animation_frame(raf).ok();
        }
        self.raf_cb.borrow_mut().take();
        self.pending_xy.set(None);
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            for (name, cb) in self.listeners.borrow_mut().drain(..) {
                let _ =
                    document.remove_event_listener_with_callback(name, cb.as_ref().unchecked_ref());
            }
        } else {
            self.listeners.borrow_mut().clear();
        }
        if self.active.replace(false) {
            self.state.drag_active_card.set(None);
            self.state.drag_drop_target.set(None);
            toggle_body_dragging(false);
        }
    }
}

/// Public: is any drag session currently armed (or active)? Other
/// gesture handlers (Shift+J / Shift+K reorder shortcuts, body swipe) check this
/// to bail when a drag would otherwise compose on top of them.
pub fn is_drag_in_flight() -> bool {
    ACTIVE_SESSION.with(|s| s.borrow().is_some())
}

/// Cancel any in-flight drag session. Safe to call when nothing is
/// active. Used by the settings toggle so disabling the feature
/// mid-drag aborts the gesture instead of leaving phantom listeners.
pub fn cancel_active_drag() {
    let session = ACTIVE_SESSION.with(|s| s.borrow_mut().take());
    if let Some(s) = session {
        s.finish();
    }
}

/// Install a one-shot capturing-phase listener that consumes the next
/// `click` event on the document. Used after a real drag so the
/// synthesised post-drag click can't toggle selection on whichever
/// card the pointer happened to be over.
///
/// `{ once: true }` lets the browser handle removal after the first
/// click. A safety timeout removes the listener if no click ever
/// arrives so a later deliberate click on a different control
/// isn't swallowed.
fn install_click_swallower() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    opts.set_once(true);

    let cb = Closure::wrap(Box::new(move |ev: Event| {
        ev.stop_propagation();
        ev.prevent_default();
    }) as Box<dyn FnMut(Event)>);
    if document
        .add_event_listener_with_callback_and_add_event_listener_options(
            "click",
            cb.as_ref().unchecked_ref(),
            &opts,
        )
        .is_err()
    {
        return;
    }
    let doc = document.clone();
    let func = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let timeout_cb = Closure::once_into_js(move || {
        let _ = doc.remove_event_listener_with_callback_and_bool("click", &func, true);
        drop(cb);
    });
    let _ = js_set_timeout(timeout_cb.unchecked_ref(), CLICK_SWALLOW_TIMEOUT_MS);
}

/// Drain `pending_xy` into one hit test per animation frame, then run
/// one auto-scroll step. Re-schedules while the container actually
/// scrolls so a finger held still at the edge keeps the list moving.
fn schedule_raf(session: Rc<DragSession>) {
    if session.raf_handle.get() != 0 {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let raf_session = session.clone();
    let cb = Closure::wrap(Box::new(move |_ts: f64| {
        raf_session.raf_handle.set(0);
        // Settings toggle off mid-drag: bail rather than commit.
        if !raf_session.state.drag_and_drop_enabled.get_untracked() {
            finish_session(&raf_session);
            return;
        }
        if let Some((x, y)) = raf_session.pending_xy.take() {
            let target = hit_test_drop_target(x, y, raf_session.source_id);
            raf_session.set_drop_target(target);
        }
        if raf_session.active.get()
            && let Some((_, y)) = raf_session.last_xy.get()
            && raf_session.auto_scroll_step(y)
        {
            // Container scrolled — cards under the pointer moved.
            // Re-hit-test from the same coords and keep the loop alive.
            if let Some((x, y)) = raf_session.last_xy.get() {
                let target = hit_test_drop_target(x, y, raf_session.source_id);
                raf_session.set_drop_target(target);
            }
            schedule_raf(raf_session.clone());
        }
    }) as Box<dyn FnMut(f64)>);
    match window.request_animation_frame(cb.as_ref().unchecked_ref()) {
        Ok(handle) => {
            session.raf_handle.set(handle);
            *session.raf_cb.borrow_mut() = Some(cb);
        }
        Err(_) => drop(cb),
    }
}

/// Build a session, leaving `ACTIVE_SESSION` empty until
/// [`register_session`] is called after the listeners are wired.
fn build_session(
    state: AppState,
    source_id: Uuid,
    pointer_id: i32,
    x: f64,
    y: f64,
) -> Option<Rc<DragSession>> {
    if ACTIVE_SESSION.with(|s| s.borrow().is_some()) {
        return None;
    }
    Some(Rc::new(DragSession {
        state,
        source_id,
        pointer_id,
        start_x: x,
        start_y: y,
        active: Cell::new(false),
        listeners: RefCell::new(Vec::with_capacity(3)),
        pending_xy: Cell::new(None),
        last_xy: Cell::new(None),
        raf_handle: Cell::new(0),
        raf_cb: RefCell::new(None),
        scroll_container: RefCell::new(None),
    }))
}

fn register_session(session: &Rc<DragSession>) {
    ACTIVE_SESSION.with(|s| *s.borrow_mut() = Some(session.clone()));
}

fn finish_session(session: &Rc<DragSession>) {
    session.finish();
    ACTIVE_SESSION.with(|s| {
        let mut slot = s.borrow_mut();
        if let Some(current) = slot.as_ref()
            && Rc::ptr_eq(current, session)
        {
            slot.take();
        }
    });
}

/// Feature gates shared by both entry points.
fn drag_allowed(state: &AppState) -> bool {
    state.drag_and_drop_enabled.get_untracked()
        && state.reorder_allowed()
        && !state.editing().get_untracked()
        && !state.creating_new().get_untracked()
}

/// Commit the move at session end. Re-checks the feature gates so a
/// mid-drag toggle doesn't sneak through.
fn commit_drop_if_eligible(session: &Rc<DragSession>) {
    if !session.state.drag_and_drop_enabled.get_untracked() || !session.state.reorder_allowed() {
        return;
    }
    let Some((target_id, edge)) = session.state.drag_drop_target.get_untracked() else {
        return;
    };
    let filtered = session.state.filtered_cards().get_untracked();
    let Some(current) = filtered
        .iter()
        .find(|c| c.id() == session.source_id)
        .cloned()
    else {
        return;
    };
    let Some(pos) = drop_position(&filtered, session.source_id, target_id, edge) else {
        return;
    };
    let placement = move_card(&filtered, session.source_id, pos);
    apply_move_placement(placement, &current, &filtered, session.state);
}

fn add_listener(
    document: &web_sys::Document,
    session: &Rc<DragSession>,
    name: &'static str,
    cb: Closure<dyn FnMut(JsValue)>,
    passive: Option<bool>,
) {
    let res = match passive {
        Some(p) => {
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(p);
            document.add_event_listener_with_callback_and_add_event_listener_options(
                name,
                cb.as_ref().unchecked_ref(),
                &opts,
            )
        }
        None => document.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref()),
    };
    if res.is_ok() {
        session.listeners.borrow_mut().push((name, cb));
    } else {
        drop(cb);
    }
}

/// Mouse / pen drag-init. Called from the card wrapper's
/// `on:pointerdown` when mode is `anywhere`. Returns early for touch
/// (the touch path goes through `start_touch_drag` instead).
pub fn start_pointer_drag(state: AppState, card_id: Uuid, ev: PointerEvent) {
    if !drag_allowed(&state) {
        return;
    }
    let pointer_type = ev.pointer_type();
    if pointer_type != "mouse" && pointer_type != "pen" {
        return;
    }
    if ev.button() != 0 {
        return;
    }
    let Some(session) = build_session(
        state,
        card_id,
        ev.pointer_id(),
        ev.client_x(),
        ev.client_y(),
    ) else {
        return;
    };
    // Resolve `document` first so the early-return path doesn't leave
    // pointer capture stranded with no listeners to consume it.
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(target) = ev.target()
        && let Ok(element) = target.dyn_into::<web_sys::Element>()
    {
        let _ = element.set_pointer_capture(ev.pointer_id());
    }

    let move_session = session.clone();
    let on_move = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: PointerEvent = ev.unchecked_into();
        if ev.pointer_id() != move_session.pointer_id {
            return;
        }
        let x = ev.client_x();
        let y = ev.client_y();
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        if !move_session.active.get() {
            let dx = x - move_session.start_x;
            let dy = y - move_session.start_y;
            if dx * dx + dy * dy < DRAG_ACTIVATION_PX * DRAG_ACTIVATION_PX {
                return;
            }
            move_session.activate();
        }
        ev.prevent_default();
        move_session.pending_xy.set(Some((x, y)));
        move_session.last_xy.set(Some((x, y)));
        schedule_raf(move_session.clone());
    }) as Box<dyn FnMut(JsValue)>);

    let up_session = session.clone();
    let on_up = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: PointerEvent = ev.unchecked_into();
        if ev.pointer_id() != up_session.pointer_id {
            return;
        }
        let was_active = up_session.active.get();
        if was_active {
            commit_drop_if_eligible(&up_session);
        }
        finish_session(&up_session);
        if was_active {
            install_click_swallower();
        }
    }) as Box<dyn FnMut(JsValue)>);

    let cancel_session = session.clone();
    let on_cancel = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: PointerEvent = ev.unchecked_into();
        if ev.pointer_id() != cancel_session.pointer_id {
            return;
        }
        finish_session(&cancel_session);
    }) as Box<dyn FnMut(JsValue)>);

    add_listener(&document, &session, "pointermove", on_move, None);
    add_listener(&document, &session, "pointerup", on_up, None);
    add_listener(&document, &session, "pointercancel", on_cancel, None);
    register_session(&session);
}

/// Touch drag-init. Called from the card number's `on:touchstart` in
/// `handle` mode. Touch events go direct (not via PointerEvent)
/// because mobile browsers throttle / drop pointermove for touch.
pub fn start_touch_drag(state: AppState, card_id: Uuid, ev: TouchEvent) {
    if !drag_allowed(&state) {
        return;
    }
    if ev.touches().length() != 1 {
        return;
    }
    let Some(touch) = ev.touches().get(0) else {
        return;
    };
    let Some(session) = build_session(
        state,
        card_id,
        touch.identifier(),
        touch.client_x() as f64,
        touch.client_y() as f64,
    ) else {
        return;
    };
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    let move_session = session.clone();
    let on_touch_move = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: TouchEvent = ev.unchecked_into();
        let Some(touch) = find_touch(&ev.touches(), move_session.pointer_id) else {
            return;
        };
        let x = touch.client_x() as f64;
        let y = touch.client_y() as f64;
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        if !move_session.active.get() {
            let dx = x - move_session.start_x;
            let dy = y - move_session.start_y;
            if dx * dx + dy * dy < DRAG_ACTIVATION_PX * DRAG_ACTIVATION_PX {
                return;
            }
            move_session.activate();
        }
        ev.prevent_default();
        move_session.pending_xy.set(Some((x, y)));
        move_session.last_xy.set(Some((x, y)));
        schedule_raf(move_session.clone());
    }) as Box<dyn FnMut(JsValue)>);

    let up_session = session.clone();
    let on_touch_end = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: TouchEvent = ev.unchecked_into();
        if find_touch(&ev.changed_touches(), up_session.pointer_id).is_none() {
            return;
        }
        let was_active = up_session.active.get();
        if was_active {
            commit_drop_if_eligible(&up_session);
        }
        finish_session(&up_session);
        if was_active {
            install_click_swallower();
        }
    }) as Box<dyn FnMut(JsValue)>);

    let cancel_session = session.clone();
    let on_touch_cancel = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev: TouchEvent = ev.unchecked_into();
        if find_touch(&ev.changed_touches(), cancel_session.pointer_id).is_none() {
            return;
        }
        finish_session(&cancel_session);
    }) as Box<dyn FnMut(JsValue)>);

    // Document-level `touchmove` is `passive: true` by default in
    // modern browsers, which silently voids our `preventDefault`.
    // Pass `{ passive: false }` so we can suppress browser scroll
    // once the drag is live.
    add_listener(&document, &session, "touchmove", on_touch_move, Some(false));
    add_listener(&document, &session, "touchend", on_touch_end, None);
    add_listener(&document, &session, "touchcancel", on_touch_cancel, None);
    register_session(&session);
}
