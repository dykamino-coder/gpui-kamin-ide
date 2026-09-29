//! Обработка событий: Связь с хостом: готовность, подключение и обрыв WS.
//!
//! Вызывается диспетчером `state/events/dispatch.rs` — он и решает,
//! чьё это событие, по варианту `ShellEvent`. Тела армов перенесены
//! из `root.rs` дословно.

use crate::host_link::ShellEvent;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Повторно открыть в хосте все документы открытых табов.
    ///
    /// Зовётся после перезапуска ребёнка extension-host: у него пустое
    /// зеркало документов, и без посева расширения не видят ни одного
    /// открытого файла — hover, definition и прочие языковые провайдеры
    /// молчат до переоткрытия таба вручную.
    ///
    /// Текст берётся из самого таба, а не с диска: в буфере могут быть
    /// несохранённые правки, и расширение обязано видеть именно их.
    fn reseed_exthost_documents(&mut self, cx: &mut Context<Self>) {
        let docs: Vec<(String, String)> = self
            .ed
            .editor_tabs
            .iter()
            .map(|tab| (tab.path.clone(), tab.input.read(cx).value().to_string()))
            .collect();
        if docs.is_empty() {
            return;
        }
        self.push_syslog(
            "info",
            "shell",
            &format!(
                "exthost respawned: reseeding {} open document(s)",
                docs.len()
            ),
        );
        for (path, text) in docs {
            let lang = crate::file_names::editor_lang(&path);
            crate::editor_lsp::HostLsp::new(&path, lang).open(&text);
        }
    }

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
            // Ребёнок extension-host перезапустился: его зеркала документов и
            // редакторов начались с нуля. Вью возвращает разбор канала
            // (`ws_events/views.rs`), а открытые документы знает только
            // модель — она и засевает их заново (BR-04).
            ShellEvent::HostEvent(channel, _) if channel == "kamin:exthost:respawned" => {
                self.reseed_exthost_documents(cx);
            }
            ShellEvent::HostEvent(_, _) => {} // маршрутизация в сторы — след. фазы
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
