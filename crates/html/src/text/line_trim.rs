//! Адресация `text-box-trim` к абзацу, который держит первую/последнюю
//! отформатированную строку срезающего контейнера.
//!
//! Срез строки делится на две части (css-inline-3 §text-box-trim; Blink
//! `InlineLayoutAlgorithm::ApplyTextBoxTrim`): от верха строчной коробки до
//! струта — её знает только замер абзаца (высокий кусок, атом, руби), — и от
//! струта до метрики края корневой строчной коробки — её считает по стилю
//! хвост `blocks_flow` и кладёт отрицательным полем. Первую часть абзац
//! снимает сам (`Paragraph::line_padding`), но ему надо знать, что именно
//! он держит срезаемую строку: строка может лежать в потомке контейнера
//! (css-pseudo-4 «first formatted line»). Контейнер записывает узел-владелец
//! строки (`request`) до раскладки детей, `blocks_flow` владельца забирает
//! запрос (`take`) и на время сборки нужного абзаца выставляет флаги
//! (`scoped`), а сборщик абзаца читает их по своему узлу (`para`).

use std::cell::{Cell, RefCell};

/// Флаги среза абзаца: начало, конец и предел надбавки первой строки над
/// струтом блока (`over`): корневая строчная коробка первой строки берёт
/// стиль `::first-line`, и её край стоит на `over` выше струта блока.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trim {
    pub start: bool,
    pub end: bool,
    pub over: f32,
}

thread_local! {
    /// (узел-владелец строки, сторона: `true` — начало, `false` — конец, `over`).
    static REQ: RefCell<Vec<(u64, bool, f32)>> = const { RefCell::new(Vec::new()) };
    /// Флаги абзаца, который собирается сейчас: (узел, флаги).
    static PARA: Cell<(u64, Trim)> = const {
        Cell::new((0, Trim { start: false, end: false, over: 0.0 }))
    };
}

/// Записать запрос среза стороны `start` для блока `owner`.
pub(crate) fn request(owner: u64, start: bool, over: f32) {
    if owner != 0 {
        REQ.with(|r| r.borrow_mut().push((owner, start, over)));
    }
}

/// Снять запрос, если владелец его так и не забрал (строка ушла другим путём).
pub(crate) fn cancel(owner: u64, start: bool) {
    REQ.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(i) = r.iter().rposition(|q| (q.0, q.1) == (owner, start)) {
            r.remove(i);
        }
    });
}

/// Забрать запросы блока `owner`.
pub(crate) fn take(owner: u64) -> Trim {
    if owner == 0 {
        return Trim::default();
    }
    REQ.with(|r| {
        let mut r = r.borrow_mut();
        let start = r.iter().find(|q| q.0 == owner && q.1);
        let t = Trim {
            start: start.is_some(),
            end: r.iter().any(|q| q.0 == owner && !q.1),
            over: start.map_or(0.0, |q| q.2),
        };
        r.retain(|q| q.0 != owner);
        t
    })
}

/// Собрать абзац блока `owner` с флагами среза его первой/последней строки.
pub(crate) fn scoped<R>(owner: u64, t: Trim, f: impl FnOnce() -> R) -> R {
    let prev = PARA.replace((owner, t));
    let out = f();
    PARA.set(prev);
    out
}

/// Текущие флаги целиком — отложенная сборка абзаца (`::first-line`
/// собирается при раскладке) повторяет их через `scoped`.
pub(crate) fn current() -> (u64, Trim) {
    PARA.get()
}

/// Флаги среза для абзаца блока `owner` (узел сверяется: вложенные атомы
/// собирают свои абзацы внутри того же вызова). `None` — абзац собран не
/// циклом `blocks_flow` своего блока: решает собственный стиль.
pub(crate) fn para(owner: u64) -> Option<Trim> {
    let (o, t) = PARA.get();
    (o == owner && owner != 0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_take_and_scope() {
        request(7, true, 3.0);
        request(7, false, 0.0);
        request(9, true, 0.0);
        let t = take(7);
        assert!(t.start && t.end && t.over == 3.0);
        assert_eq!(take(7), Trim::default());
        cancel(9, true);
        assert_eq!(take(9), Trim::default());
        let on = Trim {
            start: true,
            end: false,
            over: 0.0,
        };
        let inner = scoped(7, on, || (para(7), para(8)));
        assert_eq!(inner, (Some(on), None));
        assert_eq!(para(7), None);
        request(0, true, 0.0);
        assert_eq!(take(0), Trim::default());
    }
}
