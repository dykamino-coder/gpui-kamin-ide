//! Один one-shot тик на RootView для видимых лоадеров: retry, подпись и deadline
//! продолжаются без GPUI AnimationElement и не опрашиваются на частоте дисплея.

use super::model::RootView;
use gpui::Context;

impl RootView {
    pub(super) fn loader_slot_visible(&self, slot: crate::activity::PanelSlot) -> bool {
        if self.cz.customize_open {
            return false;
        }
        let sessions = self.sessions.as_ref();
        let open = sessions.is_some_and(|s| s.sessions.iter().any(|s| s.open));
        match slot.as_str() {
            "sidebar" => self.sidebar_visible,
            "main" => {
                self.layout.main_visible && sessions.is_some_and(|s| s.active_session_id.is_some())
            }
            "mainBottom" => self.layout.main_visible && self.layout.main_bottom_visible && open,
            "centralBottom" => {
                self.layout.file_panel_visible && self.layout.file_panel_bottom_visible && open
            }
            "rightTop" => self.layout.right_panel_visible && open,
            "rightBottom" => {
                self.layout.right_panel_visible && self.layout.right_panel_bottom_visible && open
            }
            _ => false,
        }
    }

    pub(super) fn schedule_loader_tick(&mut self, cx: &mut Context<Self>) {
        if self.loader_tick.is_some() {
            return;
        }
        self.loader_tick = Some(cx.spawn(async move |view, cx| {
            smol::Timer::after(super::loader_schedule::TICK).await;
            let _ = view.update(cx, |view, cx| {
                view.loader_tick = None;
                cx.notify();
            });
        }));
    }
}
