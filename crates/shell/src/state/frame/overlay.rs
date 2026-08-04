//! Синхронизация overlay-окна и зоны вебвью на кадр.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::state::model::RootView;
use gpui::{Context, Window};

impl RootView {
    pub(crate) fn frame_overlay(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // Ф6: второго окна нет — синхронизировать и показывать нечего.
        // Оверлеи рисует слой главного окна (`state/overlay_stack.rs`).
    }
}
