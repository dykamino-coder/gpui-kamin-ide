//! Печатный символ в страницу: ASCII — событием CHAR, остальное — IME-композицией.
//!
//! CEF на Windows собирает CHAR-событие из ОДНОГО поля: `windows_key_code`
//! становится и символом (`ui::KeyEvent::FromCharacter`), и кодом клавиши
//! (`KeyboardCodeForWindowsKeyCode` — `static_cast` в 8-битный `KeyboardCode`);
//! поле `character` при этом не читается. Для ASCII оба прочтения совпадают с
//! настоящим WM_CHAR. Для остального код клавиши — мусор из младшего байта
//! символа: «Л» (U+041B) превращается в VKEY_ESCAPE, и слой Chrome гасит
//! «Escape» до рендерера — страница видела keydown, но не keypress, а заглавная
//! Л не печаталась нигде, от композера чата до консоли на xterm.js. У «Д»
//! (U+0414) младший байт — VK_CAPITAL, безобидный; из русских заглавных
//! страдает только Л, из чешских — ě.
//!
//! Композиция — штатный путь любого нелатинского ввода в OSR (так cefclient
//! отдаёт результат IME): `ime_set_composition` + `ime_commit_text` дают
//! compositionstart/end и input, которые понимают и textarea, и xterm.js. Один
//! `ime_commit_text` без композиции xterm.js 5.5 отбрасывает: `input` после
//! увиденного keydown он считает эхом keypress, которого не будет.

/// Как доставить символ.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// KEYEVENT_CHAR: код клавиши и символ совпадают, keypress приходит.
    Char,
    /// IME-композиция: символ не зависит от 8-битного кода клавиши.
    Composition,
}

/// ASCII — CHAR, остальное — композиция (см. шапку модуля).
pub(crate) fn delivery(ch: char) -> Delivery {
    if ch.is_ascii() {
        Delivery::Char
    } else {
        Delivery::Composition
    }
}

/// Доставить печатный символ в страницу `id` тем путём, что выбрал [`delivery`].
pub(crate) fn send(id: &str, ch: char, mods: u32) {
    match delivery(ch) {
        Delivery::Char => send_char(id, ch, mods),
        Delivery::Composition => commit_composition(id, ch),
    }
}

/// KEYEVENT_CHAR: `windows_key_code` = код символа, как у WM_CHAR — из него CEF
/// и возьмёт текст (поле `character` он не читает, см. шапку).
fn send_char(id: &str, ch: char, mods: u32) {
    use cef::sys::cef_key_event_type_t as T;
    use cef::{ImplBrowserHost, KeyEvent, KeyEventType};
    let code = ch as u32 as u16;
    let event = KeyEvent {
        size: std::mem::size_of::<cef::sys::_cef_key_event_t>(),
        type_: KeyEventType::from(T::KEYEVENT_CHAR),
        modifiers: mods,
        windows_key_code: code as i32,
        native_key_code: 0,
        is_system_key: 0,
        character: code,
        unmodified_character: code,
        focus_on_editable_field: 0,
    };
    super::input::on_browser(id, move |host| host.send_key_event(Some(&event)));
}

/// Вставить символ композицией: поставить и сразу подтвердить. Диапазон
/// замены — `CefRange::InvalidRange()`, а не NULL: C-обёртка CEF проверяет
/// byref-параметры и с NULL молча выходит.
fn commit_composition(id: &str, ch: char) {
    let text = ch.to_string();
    super::input::on_browser(id, move |host| {
        use cef::ImplBrowserHost;
        let s = cef::CefString::from(text.as_str());
        let none = cef::Range {
            from: u32::MAX,
            to: u32::MAX,
        };
        let caret = cef::Range { from: 1, to: 1 };
        host.ime_set_composition(Some(&s), None, Some(&none), Some(&caret));
        host.ime_commit_text(Some(&s), Some(&none), 0);
    });
}

#[cfg(test)]
mod tests {
    use super::{Delivery, delivery};

    #[test]
    fn ascii_keeps_char_event() {
        for ch in ['a', 'K', '7', ' ', '\r', '\t', '~'] {
            assert_eq!(delivery(ch), Delivery::Char, "{ch:?}");
        }
    }

    #[test]
    fn non_ascii_goes_through_composition() {
        // Л (U+041B): младший байт 0x1B = VK_ESCAPE — сам инцидент.
        // ě (U+011B): тот же байт в чешской раскладке.
        // Д (U+0414): печаталась и раньше, но идёт тем же путём — правило
        // не зависит от того, какой именно VK получился из символа.
        for ch in ['Л', 'ě', 'Д', 'л', '€', '中'] {
            assert_eq!(delivery(ch), Delivery::Composition, "{ch:?}");
        }
    }
}
