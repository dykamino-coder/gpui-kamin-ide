//! Адаптация CSS-краски для consumers фаз раскладки и текста.

use gpui::{AnyElement, Div, IntoElement, Window};

pub(crate) fn paint_root(child: AnyElement) -> AnyElement {
    crate::paint::coordinator::roots(vec![child])
        .pop()
        .expect("один корень")
}

pub(crate) fn paint_last(child: AnyElement, key: u64) -> AnyElement {
    crate::paint::coordinator::PaintLast::new(child)
        .key(key)
        .into_any_element()
}

pub(crate) fn paint_atomic(div: Div) -> Div {
    crate::paint::coordinator::atomic(div)
}

pub(crate) fn paint_reopen<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    crate::paint::coordinator::reopen(window, f)
}

#[cfg(test)]
pub(crate) fn paint_inline(child: AnyElement) -> AnyElement {
    crate::paint::coordinator::PaintInline::new(child).into_any_element()
}

#[cfg(test)]
pub(crate) fn paint_collect() -> (AnyElement, AnyElement) {
    let (open, close) = crate::paint::coordinator::PaintCollect::pair();
    (open.into_any_element(), close.into_any_element())
}
