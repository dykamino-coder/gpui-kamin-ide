//! Строчная коробка как якорь (`anchor-name`) и как содержащий блок абсолюта:
//! метки краёв её содержимого в кусках абзаца и таблица её свойств.
//!
//! css-anchor-position-1 §anchor box: рамка якоря из нескольких фрагментов —
//! объемлющий прямоугольник их border box (Blink `AnchorMap::Set` объединяет
//! фрагменты одного `LayoutObject`, `anchor_map.cc:130`). CSS 2.1 §10.1 п.4:
//! содержащий блок строчной коробки — от начала первого фрагмента до конца
//! последнего. Абзац знает фрагменты только по геометрии строк, поэтому
//! коробка помечается парой пустых кусков (как `mark_inline_cb`), а свойства
//! лежат в таблице кадра по номеру метки; абзац на подготовке кадра пишет
//! рамки в общий реестр якорей (`anchor::register_inline`).

use super::{InlineCb, OverlayAt, Piece};
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::IntoElement;
use std::cell::RefCell;
use std::collections::HashMap;

/// Свойства помеченной строчной коробки.
#[derive(Clone, Debug, Default)]
pub struct InlineBoxInfo {
    /// `node_id` элемента.
    pub node: u64,
    /// `anchor-name` (пусто — коробка только содержащий блок).
    pub names: Vec<String>,
    /// Коробка — содержащий блок абсолютных потомков.
    pub cb: bool,
    /// Её собственный содержащий блок (`Computed::cb_node`).
    pub own_cb: u64,
    /// Порядок сборки (`anchor::next_seq`).
    pub seq: u32,
    /// Отбивка и рамка `[top, right, bottom, left]`.
    pub pad: [f32; 4],
    pub border: [f32; 4],
    /// `visibility: hidden` (§position-visibility: anchors-visible).
    pub hidden: bool,
}

thread_local! {
    /// Таблица кадра: номер метки → свойства коробки. Чистится `clear`
    /// в начале кадра (`anchor::reset`).
    static BOXES: RefCell<HashMap<u32, InlineBoxInfo>> = RefCell::new(HashMap::new());
}

/// Свойства коробки по номеру метки.
pub fn info(id: u32) -> Option<InlineBoxInfo> {
    BOXES.with(|m| m.borrow().get(&id).cloned())
}

/// Очистить таблицу (новый кадр).
pub fn clear() {
    BOXES.with(|m| m.borrow_mut().clear());
}

fn px4(t: Option<Len>, r: Option<Len>, b: Option<Len>, l: Option<Len>) -> [f32; 4] {
    let v = |x: Option<Len>| match x {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    [v(t), v(r), v(b), v(l)]
}

/// Обернуть куски содержимого строчной коробки `e` метками краёв, если она
/// именованный якорь или содержащий блок абсолютных потомков (тогда — только
/// при внепоточных кусках внутри: иначе рамка никому не нужна).
pub(super) fn mark_inline_box(pieces: Vec<Piece>, e: &Element, merged: &Computed) -> Vec<Piece> {
    if e.node_id == 0 {
        return pieces;
    }
    let names = e.style.anchor_name.clone().unwrap_or_default();
    let cb = super::establishes_cb(&e.style)
        && pieces.iter().any(|p| {
            matches!(p, Piece::Overlay(_, h) if h.cb_marker.is_none() && h.box_marker.is_none() && h.inline_box.is_none())
        });
    if names.is_empty() && !cb {
        return pieces;
    }
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let b = e.style.borders();
    let p = &e.style.padding;
    let info = InlineBoxInfo {
        node: e.node_id,
        names,
        cb,
        own_cb: merged.cb_node,
        seq: crate::layout::positioned::anchor::next_seq(),
        pad: px4(p.top, p.right, p.bottom, p.left),
        border: px4(b.top, b.right, b.bottom, b.left),
        hidden: merged.hidden == Some(true),
    };
    BOXES.with(|m| m.borrow_mut().insert(id, info));
    let marker = |start: bool| {
        Piece::Overlay(
            gpui::Empty.into_any_element(),
            OverlayAt {
                box_marker: Some((id, start)),
                ..Default::default()
            },
        )
    };
    let mut out = Vec::with_capacity(pieces.len() + 2);
    out.push(marker(true));
    out.extend(pieces);
    out.push(marker(false));
    out
}

/// Кусок-носитель разрешённых краёв коробки `id`: `s..e` — байтовые края
/// содержимого, `at` — место самого куска.
pub(super) fn carrier(
    id: u32,
    at: usize,
    s: usize,
    e: usize,
) -> (usize, gpui::AnyElement, OverlayAt) {
    (
        at,
        gpui::Empty.into_any_element(),
        OverlayAt {
            inline_box: Some(InlineCb {
                id,
                start: s as isize - at as isize,
                end: e as isize - at as isize,
                pad: [0.0; 4],
                shift: (0.0, 0.0),
            }),
            ..Default::default()
        },
    )
}
