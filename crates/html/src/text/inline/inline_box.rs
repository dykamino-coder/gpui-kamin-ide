//! Строчная коробка как якорь (`anchor-name`) и как содержащий блок абсолюта:
//! метки краёв её содержимого и принадлежащие потоку свойства коробки.
//!
//! css-anchor-position-1 §anchor box: рамка якоря из нескольких фрагментов —
//! объемлющий прямоугольник их border box (Blink `AnchorMap::Set` объединяет
//! фрагменты одного `LayoutObject`, `anchor_map.cc:130`). CSS 2.1 §10.1 п.4:
//! содержащий блок строчной коробки — от начала первого фрагмента до конца
//! последнего. Абзац знает фрагменты только по геометрии строк, поэтому
//! коробка помечается парой пустых кусков (как `mark_inline_cb`), а свойства
//! передаются конечной меткой и куском-носителем; абзац перед покраской пишет
//! рамки в общий реестр якорей (`anchor::register_inline`).

use super::{InlineCb, OverlayAt, Piece};
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::IntoElement;
use std::rc::Rc;

/// Свойства помеченной строчной коробки.
#[derive(Clone, Debug, Default, PartialEq)]
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

fn px4(t: Option<Len>, r: Option<Len>, b: Option<Len>, l: Option<Len>) -> [f32; 4] {
    [t, r, b, l].map(px_lossy_zero)
}

/// Обернуть куски содержимого строчной коробки `e` метками краёв, если она
/// именованный якорь или содержащий блок абсолютных потомков (тогда — только
/// при внепоточных кусках внутри: иначе рамка никому не нужна).
pub(super) fn mark_inline_box(
    pieces: Vec<Piece>,
    e: &Element,
    merged: &Computed,
    vcx: crate::document::state::ValueCx<'_>,
) -> Vec<Piece> {
    if e.node_id == 0 {
        return pieces;
    }
    let names = e.style.computed().anchor_name.clone().unwrap_or_default();
    let cb = super::establishes_cb(&e.style.computed())
        && pieces.iter().any(|p| {
            matches!(p, Piece::Overlay(_, h) if h.cb_marker.is_none() && h.box_marker.is_none() && h.inline_box.is_none())
        });
    if names.is_empty() && !cb {
        return pieces;
    }
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let b = e.style.computed().borders();
    let p = &e.style.computed().padding;
    let info = Rc::new(InlineBoxInfo {
        node: e.node_id,
        names,
        cb,
        own_cb: merged.cb_node,
        seq: crate::layout::positioned::anchor::next_seq(vcx),
        pad: px4(p.top, p.right, p.bottom, p.left),
        border: px4(b.top, b.right, b.bottom, b.left),
        hidden: merged.hidden == Some(true),
    });
    let marker = |start: bool| {
        Piece::Overlay(
            gpui::Empty.into_any_element(),
            OverlayAt {
                box_marker: Some((id, start)),
                inline_box_info: (!start).then(|| info.clone()),
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
    info: Rc<InlineBoxInfo>,
) -> (usize, gpui::AnyElement, OverlayAt) {
    (
        at,
        gpui::Empty.into_any_element(),
        OverlayAt {
            inline_box_info: Some(info),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_piece_keeps_its_properties_across_other_paragraphs() {
        let properties = Rc::new(InlineBoxInfo {
            node: 1,
            names: vec!["first".into()],
            ..Default::default()
        });
        let marker = |id, start, info| {
            Piece::Overlay(
                gpui::Empty.into_any_element(),
                OverlayAt {
                    box_marker: Some((id, start)),
                    inline_box_info: info,
                    ..Default::default()
                },
            )
        };
        let first = super::super::spacers::overlays(vec![
            marker(1, true, None),
            Piece::Text {
                text: "one".into(),
                style: Default::default(),
            },
            marker(1, false, Some(properties.clone())),
        ]);
        let other = super::super::spacers::overlays(vec![
            marker(2, true, None),
            marker(
                2,
                false,
                Some(Rc::new(InlineBoxInfo {
                    node: 2,
                    ..Default::default()
                })),
            ),
        ]);
        let (_, _, how) = &first[0];
        assert!(Rc::ptr_eq(
            how.inline_box_info.as_ref().unwrap(),
            &properties
        ));
        assert_eq!(how.inline_box_info.as_ref().unwrap().names, ["first"]);
        assert_eq!(how.inline_box.unwrap().start, -3);
        assert_eq!(other[0].2.inline_box_info.as_ref().unwrap().node, 2);
    }
}
