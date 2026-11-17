//! Нужен ли кадр окна после события состояния.
//!
//! Каждый `true` — кадр всего окна через насос (`web/pump.rs`). Ошибиться в
//! сторону `true` безопасно (лишний кадр), в сторону `false` — нет (устаревшая
//! картинка), поэтому исключения ЯВНЫЕ и короткие: вывод терминала, доставка
//! страницам, пустые маршруты и повторные признаки жизни вью.

use std::collections::HashSet;

use crate::host::events::{TermEvent, TreeEvent};
use crate::host_link::ShellEvent;

/// `alive` — вью, чей скрипт уже отчитался (`RootView::webviews_alive`).
pub(super) fn needs_frame(event: &ShellEvent, alive: &HashSet<String>) -> bool {
    match event {
        // Страница шлёт признак жизни с КАЖДЫМ своим сообщением (опросы чата,
        // `pty-resize` консоли — около одного в секунду в простое). Состояние
        // меняет только первый: с вью снимается обложка загрузки. Иначе
        // простаивающее окно перерисовывалось целиком на каждое сообщение
        // страницы (INC-2026-0064).
        ShellEvent::Term(TermEvent::WebviewAlive(id)) => !alive.contains(id),
        _ => !matches!(
            event,
            // Насос терминала сам будит грид; событие состояния не меняет.
            ShellEvent::Term(TermEvent::TermWakeup)
                // JS в страницы: исполняется в CEF, поля RootView не трогает.
                | ShellEvent::Term(TermEvent::WvJs { .. })
                // Маршрут «в сторы следующих фаз» — пустой обработчик.
                | ShellEvent::HostEvent { .. }
                // Перечитка листингов: только исходящие запросы, мутации
                // придут отдельными DirListing.
                | ShellEvent::Tree(TreeEvent::RefreshTree)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::needs_frame;
    use crate::host::events::{TermEvent, TreeEvent};
    use crate::host_link::ShellEvent;
    use std::collections::HashSet;

    fn alive(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_the_first_sign_of_life_of_a_view_needs_a_frame() {
        let event = ShellEvent::Term(TermEvent::WebviewAlive("claudeBridgeChat".into()));
        assert!(needs_frame(&event, &alive(&[])));
        assert!(needs_frame(&event, &alive(&["claudeBridgeConsoleView"])));
        assert!(!needs_frame(&event, &alive(&["claudeBridgeChat"])));
    }

    #[test]
    fn idle_traffic_needs_no_frame_and_state_changes_do() {
        let none = alive(&[]);
        assert!(!needs_frame(
            &ShellEvent::Term(TermEvent::TermWakeup),
            &none
        ));
        assert!(!needs_frame(
            &ShellEvent::Tree(TreeEvent::RefreshTree),
            &none
        ));
        assert!(needs_frame(&ShellEvent::WsConnected, &none));
    }
}
