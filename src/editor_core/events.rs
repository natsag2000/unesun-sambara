//! Editor event bus.
//!
//! See `prompt/FUTURE_PLAN.md` (Phase P1-01) and `prompt/PLUGIN_API.md`.
//!
//! The event bus is the mechanism plugins use to observe editor state
//! changes without directly editing the WASM entry points every time a new
//! feature ships. Dispatch points live in `WasmEditor` (keyboard, mouse,
//! orientation toggle, settings, text-changing methods) and in
//! `EditorState` (settings updates).
//!
//! Design notes
//! ------------
//! * Subscribers are stored as `Box<dyn EventSubscriber>` on the
//!   `EventBus`. The bus lives on `EditorState`. `PluginRegistry` in
//!   `plugin.rs` does not itself store subscribers; instead plugins are
//!   dispatched to via the registry's own iteration path. The event bus is
//!   also open for non-plugin subscribers (e.g. future in-process
//!   listeners), which is why the two are separate.
//! * `EditorEvent` is a plain owned enum ÔÇö no lifetimes ÔÇö so subscribers
//!   don't have to juggle borrows against `EditorState`. Payloads for
//!   heavier events (like `TextChanged`) can be added incrementally.
//!
//! Some of the enum variants, helpers, and fields below are not yet
//! consumed by any plugin (nothing subscribes to
//! `SelectionChanged` today, for example). They are the documented
//! surface plugins will use in P2+ and are intentionally kept live
//! rather than being trimmed and re-added later. The
//! `#[allow(dead_code)]` on the module is scoped to that reality.

#![allow(dead_code)]

use std::mem;

/// Modifier + key snapshot dispatched with `EditorEvent::KeyPressed`.
///
/// Kept separate from `web_sys::KeyboardEvent` so the event enum stays
/// usable outside a browser context (unit tests, headless tools).
#[derive(Clone, Debug, Default)]
pub struct KeyInfo {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyInfo {
    /// True when the ctrl (or meta / cmd on macOS) modifier is held.
    pub fn ctrl_or_meta(&self) -> bool {
        self.ctrl || self.meta
    }
}

/// Events dispatched by the editor core.
///
/// New variants MUST be additive; existing plugins should keep working
/// when new variants are introduced. Match arms in plugins are expected
/// to use `_ => {}` for anything they do not care about.
#[derive(Clone, Debug)]
pub enum EditorEvent {
    /// The document text was mutated (insert, delete, paste, ÔÇª).
    TextChanged,
    /// The cursor position moved without changing text.
    CursorMoved,
    /// The selection range changed (including cleared to `Selection::None`).
    SelectionChanged,
    /// User-facing settings (fonts, colors, orientation, ÔÇª) were replaced
    /// or reset.
    SettingsChanged,
    /// A key was pressed in the editor. Fired before Rust performs its
    /// own default handling, so plugins can observe raw input.
    KeyPressed(KeyInfo),
    /// A document was loaded from disk / paste / initial greeting.
    DocumentLoaded { name: Option<String> },
    /// A document was written out (`.txt`, `.uns`, PNG export, ÔÇª).
    DocumentSaved { name: Option<String> },
    /// Text orientation was toggled between vertical and horizontal.
    OrientationToggled,
}

/// Trait implemented by anything that wants to listen on the bus.
pub trait EventSubscriber {
    fn on_event(&mut self, event: &EditorEvent);
}

/// A trivial function-pointer subscriber, useful for tests and small
/// diagnostic sinks. Plugins should prefer implementing `Plugin` in
/// `crate::editor_core::plugin` so they can also expose commands.
pub struct FnSubscriber<F: FnMut(&EditorEvent)> {
    pub f: F,
}

impl<F: FnMut(&EditorEvent)> EventSubscriber for FnSubscriber<F> {
    fn on_event(&mut self, event: &EditorEvent) {
        (self.f)(event);
    }
}

/// Fan-out event bus.
///
/// The bus uses interior iteration (`fn dispatch`) instead of exposing
/// the subscriber list, so the backing store can change later
/// (per-event-topic index, ordered fan-out with priorities, ÔÇª) without
/// touching call sites.
///
/// Subscribers see events in registration order. `dispatch` takes
/// `&mut self`; subscribers do not receive a bus handle, so subscribing
/// from within `on_event` is not possible in this iteration. If a
/// future subscriber needs to install more subscribers dynamically it
/// should route through `EditorState::events` from a command handler
/// (which runs outside the dispatch call), not from `on_event` itself.
pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Register a subscriber. Not callable from inside `dispatch`
    /// because the bus is exclusively borrowed for the duration of a
    /// dispatch cycle.
    pub fn subscribe(&mut self, sub: Box<dyn EventSubscriber>) {
        self.subscribers.push(sub);
    }

    /// Fan an event out to every registered subscriber, in registration
    /// order.
    pub fn dispatch(&mut self, event: &EditorEvent) {
        // Move subscribers out so `self` is fully free during callbacks.
        // This mirrors `PluginRegistry::dispatch_event` in `plugin.rs`
        // and keeps future extension (e.g. subscribers that want to
        // schedule work back onto the bus via an intermediate queue)
        // straightforward.
        let mut subs = mem::take(&mut self.subscribers);
        for sub in subs.iter_mut() {
            sub.on_event(event);
        }
        self.subscribers = subs;
    }

    /// Number of currently active subscribers. Handy in tests.
    pub fn len(&self) -> usize {
        self.subscribers.len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn dispatch_calls_every_subscriber_in_order() {
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let mut bus = EventBus::new();

        let log_a = log.clone();
        bus.subscribe(Box::new(FnSubscriber {
            f: move |_| log_a.borrow_mut().push("a"),
        }));
        let log_b = log.clone();
        bus.subscribe(Box::new(FnSubscriber {
            f: move |_| log_b.borrow_mut().push("b"),
        }));

        assert_eq!(bus.len(), 2);
        bus.dispatch(&EditorEvent::TextChanged);
        bus.dispatch(&EditorEvent::CursorMoved);

        assert_eq!(*log.borrow(), vec!["a", "b", "a", "b"]);
    }

    #[test]
    fn empty_bus_dispatch_is_noop() {
        let mut bus = EventBus::new();
        bus.dispatch(&EditorEvent::TextChanged);
        assert_eq!(bus.len(), 0);
    }
}
