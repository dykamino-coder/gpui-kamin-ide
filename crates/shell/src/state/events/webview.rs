//! Обработка событий: Вебвью и браузер-панель: жизненный цикл, JS-мост, ошибки загрузки.
//!
//! Вызывается диспетчером `state/events/dispatch.rs` — он и решает,
//! чьё это событие, по варианту `ShellEvent`. Тела армов перенесены
//! из `root.rs` дословно.

use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Вебвью и браузер-панель: жизненный цикл, JS-мост, ошибки загрузки.
    pub(crate) fn apply_webview(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            // Доставка постов расширения (бывший WebviewDeliver) ушла в
            // WS-поток (ws_events/views.rs): main loop её не касается.
            ShellEvent::Term(TermEvent::WebviewHtml(view_id, html)) => {
                // Загрузка/создание — в render (нужен window): pending → кадр.
                self.pending_html.insert(view_id, html);
            }
            ShellEvent::Term(TermEvent::WvJs(id, js)) => {
                if crate::web::enabled() {
                    crate::web::execute_script(&id, &js);
                }

                #[cfg(not(windows))]
                let _ = (id, js);
            }
            ShellEvent::Term(TermEvent::WebviewAlive(id)) => {
                // Только признак «скрипт вью жив» (снимает load-cover вью).
                // Шторку переключения чата гасит НЕ любой inbound, а протокол
                // bridgeShowing false→true в снапшотах сессий (apply_sessions):
                // случайное сообщение чата приходило раньше, чем чат реально
                // перерисовывал новую сессию, и шторка гасла над чужим чатом.
                self.webviews_alive.insert(id);
            }
            ShellEvent::Term(TermEvent::ForceViewLoadError(id)) => {
                // Ветка загрузки живёт, только пока html НЕТ; 45 — порог
                // `tries >= 45`, за которым вместо скелета карточка ошибки
                crate::ui::chat_webview::drop_html(&id);
                self.pending_html.remove(&id);
                self.view_resolve_start
                    .insert(id.clone(), std::time::Instant::now());
                self.view_resolve_tries.insert(id, 45);
            }
            ShellEvent::Term(TermEvent::RetryView(id)) => {
                // Сброс бюджета: следующий кадр отправит resolve заново
                self.view_resolve_tries.remove(&id);
                self.view_resolve_at.remove(&id);
                self.view_resolve_start.remove(&id);
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
