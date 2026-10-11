//! Согласовать CEF visibility с фактически показанными слотами, а retention —
//! с политикой Customize. Неполный registry не отменяет явные layout hide.

use super::model::RootView;
use super::webview_visibility::{Layout, Slot};

impl RootView {
    pub(super) fn sync_web_visibility(&mut self) {
        let sessions = self.sessions.as_ref();
        let layout = Layout {
            sidebar: self.sidebar_visible,
            main: self.layout.main_visible,
            main_bottom: self.layout.main_bottom_visible,
            file: self.layout.file_panel_visible,
            file_bottom: self.layout.file_panel_bottom_visible,
            right: self.layout.right_panel_visible,
            right_bottom: self.layout.right_panel_bottom_visible,
            has_active: sessions.is_some_and(|s| s.active_session_id.is_some()),
            has_open: sessions.is_some_and(|s| s.sessions.iter().any(|s| s.open)),
        };
        let slots = crate::activity::PanelSlot::ALL
            .into_iter()
            .map(|slot| {
                let tool = self.activity.state(slot).active.clone();
                let views = tool
                    .as_deref()
                    .and_then(crate::activity::dyn_tool)
                    .map(|d| {
                        d.views
                            .into_iter()
                            .filter(|v| v.webview)
                            .map(|v| v.id)
                            .collect()
                    });
                Slot {
                    name: slot.as_str(),
                    tool,
                    views,
                }
            })
            .collect::<Vec<_>>();
        let (visible, retained) = self.webview_slots.collect(
            &layout,
            self.cz.customize_open,
            slots,
            self.layout.file_panel_mode == "web",
            self.cz.customize_contrib.clone(),
        );
        crate::web::mark_visible(visible, retained);
    }
}
