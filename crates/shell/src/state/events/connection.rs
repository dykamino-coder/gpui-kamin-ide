//! Обработка событий: Связь с хостом: готовность, подключение и обрыв WS.
//!
//! Вызывается диспетчером `state/events/dispatch.rs` — он и решает,
//! чьё это событие, по варианту `ShellEvent`. Тела армов перенесены
//! из `root.rs` дословно.

use crate::host_link::ShellEvent;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Связь с хостом: готовность, подключение и обрыв WS.
    pub(crate) fn apply_connection(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::HostReady(ep) => self.host_port = Some(ep.port),
            ShellEvent::WsConnected => {
                self.ws_connected = true;
                self.push_syslog("info", "shell", "Connected to kamin-host");
                // Префы нужны ДО Customize: `skipDeleteConfirm` спрашивают на
                // первом же удалении сессии, а панель могут не открыть вовсе.
                crate::host_link::request_app_prefs(self.tx.clone());
            }
            ShellEvent::WsDisconnected => {
                self.ws_connected = false;
                self.push_syslog("warning", "shell", "Disconnected from kamin-host");
            }
            ShellEvent::HostEvent(_, _) => {} // маршрутизация в сторы — след. фазы
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
