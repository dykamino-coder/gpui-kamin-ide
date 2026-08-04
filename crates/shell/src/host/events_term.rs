//! Вариантты `ShellEvent` домена `events_term`
//! (`plan/100-refactor-250.md`).

#[derive(Clone)]
pub enum TermEvent {
    /// HTML вью готов — кладём в стор, страницу отдаёт `web/scheme.rs`.
    WebviewHtml(String, String),
    /// probe: исчерпать бюджет resolve вью — терминальная карточка Retry.
    /// Иначе состояние не снять кадром: оно приходит через 45 попыток.
    ForceViewLoadError(String),
    /// Кнопка Retry на терминальной ошибке загрузки вебвью-панели: сбросить
    /// бюджет resolve и запросить вью заново (`onRetry` оригинала).
    RetryView(String),
    /// Терминал: новые данные в grid (перерисовать).
    TermWakeup,
    /// Байты в PTY извне UI (probe-верификация терминала)
    TermInput(String),
    /// Новый шелл по id профиля («+» дропдаун)
    TermNew(String),
    /// Скролл вьюпорта (probe-верификация скроллбэка)
    TermScroll(i32),
    /// Первый ipc-inbound от вебвью: скрипт жив → показать wv2 (chat-cover).
    WebviewAlive(String),
    /// Шевроны тулбара терминала: сдвиг окна видимых табов (+1/-1).
    TermTabScroll(i32),
    /// Star в пикере профилей: сделать шелл дефолтным (persist).
    TermSetDefaultShell(String),
    TermSelect(usize),
    TermClose(usize),
    ToggleTermMenu,
    /// Выполнить JS в composition-вью (probe wvjs → UI-поток: COM STA).
    WvJs(String, String),
}
