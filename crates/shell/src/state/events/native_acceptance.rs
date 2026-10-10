//! Debug UI controls use the same dispatch/domain path as ordinary shell events.

use crate::host_link::ShellEvent;
use crate::native_acceptance::{Action, LOADERS, TOOL, VIEW};
use crate::root::RootView;
use gpui::Context;

pub(crate) const HTML: &str = r#"<!doctype html><html><head><style>
body{background:#18202d;color:white;font:18px sans-serif} .spin{display:inline-block;animation:spin 1s linear infinite!important}
@keyframes spin{to{transform:rotate(360deg)}}
</style></head><body><p>Native acceptance fixture</p><span class="spin">◌</span>
<input placeholder="Type here"><button onclick="this.textContent='Clicked'">Click</button>
<select><option>Popup one</option><option>Popup two</option></select></body></html>"#;

impl RootView {
    #[expect(
        clippy::single_match,
        reason = "The routing checker parses domain match arms"
    )]
    pub(crate) fn apply_native_acceptance(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        match event {
            ShellEvent::NativeAcceptance(action) => {
                crate::host_link::register_dynamic_webview(VIEW);
                let slot = crate::activity::PanelSlot::Sidebar;
                self.sidebar_visible = true;
                match action {
                    Action::Loaders => {
                        self.activity.pin(slot, LOADERS);
                        self.activity.set_active(slot, LOADERS);
                    }
                    Action::Waiting => {
                        crate::web::reset_fixture();
                        crate::ui::chat_webview::drop_html(VIEW);
                        self.pending_html.remove(VIEW);
                        self.webviews_alive.remove(VIEW);
                        self.webview_cover.remove(VIEW);
                        self.view_resolve_tries.remove(VIEW);
                        self.view_resolve_start.remove(VIEW);
                        self.view_resolve_at.remove(VIEW);
                        self.activity.pin(slot, TOOL);
                        self.activity.set_active(slot, TOOL);
                    }
                    Action::Switch => {
                        self.activity.pin(slot, TOOL);
                        self.activity.set_active(slot, TOOL);
                        self.chat_cover = Some((std::time::Instant::now(), None));
                    }
                    Action::Ready => {
                        self.pending_html.insert(VIEW.into(), HTML.into());
                        self.activity.pin(slot, TOOL);
                        self.activity.set_active(slot, TOOL);
                    }
                    Action::Error => {
                        crate::web::reset_fixture();
                        crate::ui::chat_webview::drop_html(VIEW);
                        self.pending_html.remove(VIEW);
                        self.webviews_alive.remove(VIEW);
                        self.view_resolve_tries.insert(VIEW.into(), 45);
                        self.activity.pin(slot, TOOL);
                        self.activity.set_active(slot, TOOL);
                    }
                    Action::Clear => {
                        self.activity.unpin(slot, TOOL);
                        self.activity.unpin(slot, LOADERS);
                        crate::ui::chat_webview::drop_html(VIEW);
                        crate::web::reset_fixture();
                        self.pending_html.remove(VIEW);
                        self.webviews_alive.remove(VIEW);
                        self.webview_cover.remove(VIEW);
                        self.view_resolve_tries.remove(VIEW);
                        self.view_resolve_start.remove(VIEW);
                        self.view_resolve_at.remove(VIEW);
                        self.chat_cover = None;
                    }
                    Action::DeviceLoss => {
                        #[cfg(windows)]
                        crate::native_acceptance::device::request();
                    }
                }
                cx.notify();
            }
            _ => {}
        }
    }
}
