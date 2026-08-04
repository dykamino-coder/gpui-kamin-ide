//! Клавиши терминала: keystroke → байты для PTY.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// Клавиша gpui → байты PTY (минимальный набор + printable).
pub fn keystroke_bytes(ks: &gpui::Keystroke) -> Option<Vec<u8>> {
    let ctrl = ks.modifiers.control;
    let k = ks.key.as_str();
    // printable: key_char учитывает раскладку/шифт
    if !ctrl
        && let Some(ch) = &ks.key_char
        && !ch.is_empty()
    {
        return Some(ch.as_bytes().to_vec());
    }
    let seq: &[u8] = match k {
        "enter" => b"\r",
        "backspace" => b"\x7f",
        "tab" => b"\t",
        "escape" => b"\x1b",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "delete" => b"\x1b[3~",
        "space" if ctrl => b"\x00",
        _ if ctrl && k.len() == 1 => {
            let c = k.as_bytes()[0].to_ascii_lowercase();
            if c.is_ascii_lowercase() {
                return Some(vec![c - b'a' + 1]); // Ctrl+A..Z
            }
            return None;
        }
        _ => return None,
    };
    Some(seq.to_vec())
}
