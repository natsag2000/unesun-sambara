//! User-facing plugins built on top of `crate::editor_core::plugin`.
//!
//! Unlike `CoreCommandsPlugin` (which lives in `editor_core::plugin`
//! because it is part of the plugin *substrate*, not a feature), plugins
//! in this module are features implemented *using* that substrate. This
//! is the directory new feature plugins should land in going forward
//! (see `prompt/FUTURE_PLAN.md` P2-03).

pub mod find_replace;
