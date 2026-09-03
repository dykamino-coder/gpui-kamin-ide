//! Состояние приложения: `RootView` и его методы по доменам.
//!
//! Домены разнесены по файлам, чтобы каждый читался целиком:
//! `splitters` — драг раскладки, `contributed` — вью расширений,
//! `tools` — тело активного тула слота. См. `plan/100-refactor-250.md`.

pub mod canvas_layers;
pub mod chrome;
pub mod columns;
pub mod consts;
pub mod drag;
pub(crate) mod drag_flag;
pub mod drop_hints;
pub mod editor_save;
pub mod editor_tab;
pub mod init;
pub mod metrics;
pub mod model;
pub mod model_customize;
pub mod model_editor;
pub mod model_search;
pub mod model_term;
pub mod overlay_query;
pub mod overlay_stack;
pub mod overlays_main;
pub mod popovers;
pub mod tree_keys;
pub mod tree_select;
pub mod tree_store;
pub mod view;

pub mod contributed;
pub mod contributed_webview;
pub mod events;
pub mod file_tree_body;
pub mod frame;
pub mod frame_ctx;
pub mod fs_ops;
pub mod handlers;
pub mod handlers_mouse;
pub(crate) mod hover_pill;
pub(crate) mod rename_transition;
pub mod splitters;
pub mod term_grid;
pub mod terminal_body;
pub mod tools;
