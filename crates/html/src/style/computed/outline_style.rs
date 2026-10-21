//! Вычисленный стиль: разбор `outline-style` в код рисунка (CSS UI 4
//! §outline-style использует узоры линий рамки). `double` хранится отдельным
//! кодом, чтобы краска оставила зазор между линиями прозрачным.

pub(crate) const DOUBLE: u8 = 5;

pub(super) fn parse(value: &str) -> Option<u8> {
    match value {
        "none" | "hidden" => Some(0),
        "auto" => Some(2),
        "dotted" => Some(3),
        "dashed" => Some(4),
        "double" => Some(DOUBLE),
        "solid" | "groove" | "ridge" | "inset" | "outset" => Some(1),
        _ => None,
    }
}
