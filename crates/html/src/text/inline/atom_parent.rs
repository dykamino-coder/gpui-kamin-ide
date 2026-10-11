//! Стиль строчного предка для атома: наследование идёт от ближайшей коробки.
//!
//! Атомарная строчная коробка (`inline-block`, `inline-table`, замещаемый
//! элемент) — ребёнок того `<span>`, внутри которого стоит, и наследует от
//! него (CSS Cascade 4 §7: «the inherited value of a property on an element
//! is the computed value of the property on the element's parent»). Атом
//! строится замыканием абзаца со стилем БЛОКА, поэтому сбор кусков держит
//! стек вычисленных стилей открытых строчных предков (без служебных полей
//! окраски строчной коробки — только `inherit`): `<b>a <span
//! style="display:inline-block">b</span></b>` терял полужирный у «b»
//! (`run-in-contains-inline-block-001`).

use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;

thread_local! {
    static STACK: std::cell::RefCell<Vec<Computed>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Открыть строчного предка: его стиль — наследование от верха стека
/// (или от `base`, стиля абзаца, когда стек пуст).
pub(crate) fn open(own: &Computed, base: &Computed) {
    STACK.with(|s| {
        let mut s = s.borrow_mut();
        let parent = s.last().unwrap_or(base);
        let style = inherit(parent, own);
        s.push(style);
    });
}

/// Закрыть строчного предка, открытого `open`.
pub(crate) fn close() {
    STACK.with(|s| {
        s.borrow_mut().pop();
    });
}

/// Стиль ближайшего открытого строчного предка, если он есть.
pub(crate) fn current() -> Option<Computed> {
    STACK.with(|s| s.borrow().last().cloned())
}

/// Спрятать стек на время постройки атома: его содержимое — свой абзац со
/// своими строчными предками. Вернуть — `resume`.
pub(crate) fn suspend() -> Vec<Computed> {
    STACK.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

/// Вернуть стек, спрятанный `suspend`.
pub(crate) fn resume(saved: Vec<Computed>) {
    STACK.with(|s| *s.borrow_mut() = saved);
}
