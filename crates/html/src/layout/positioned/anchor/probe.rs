//! Проба якорного элемента: ключ, неявный якорь, регистрация (probe_for).

use super::{AnchorRec, frame::note_checked, reframe_if_stale, tf_map};
use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::style::computed::{Computed, PositionAnchor};
use crate::style::values::value::px_lossy_zero;
use gpui::{AnyElement, App, Bounds, IntoElement, Pixels, Styled, Window, px};

/// Ключ коробки в реестрах между кадрами: `node_id`; у псевдоэлемента он 0 —
/// ключ от хозяина (`implicit_anchor`) и вида.
pub fn key_of(e: &crate::dom::Element) -> u64 {
    if e.node_id != 0 {
        return e.node_id;
    }
    (1u64 << 63) | (e.style.implicit_anchor.unwrap_or(0) << 1) | u64::from(e.tag == "::after")
}

/// Ссылается ли коробка на НЕЯВНЫЙ якорь: `position-anchor: auto`, либо
/// `normal` (в том числе не задано) при непустой `position-area`
/// (§position-anchor: «normal: If position-area is none, behaves as none.
/// Otherwise, behaves as auto»).
pub(super) fn wants_implicit(c: &Computed) -> bool {
    match &c.position_anchor {
        Some(PositionAnchor::Auto) => true,
        Some(PositionAnchor::Normal) | None => c.position_area.is_some(),
        _ => false,
    }
}

/// Проба якоря для коробки `e`: нужна, когда у неё есть `anchor-name` или
/// её псевдоэлемент ссылается на неё как на неявный якорь. Канвас во всю
/// коробку (`absolute` + `size_full`) — та же форма, что у `edge_probe`.
/// `hidden` — `visibility: hidden` коробки (§position-visibility).
pub fn probe_for(
    e: &crate::dom::Element,
    c: &Computed,
    hidden: bool,
    vcx: ValueCx<'_>,
) -> Option<AnyElement> {
    let names: Vec<String> = e.style.anchor_name.clone().unwrap_or_default();
    let implicit = e.node_id != 0
        && e.children.iter().any(|n| {
            matches!(n, Node::Element(k) if k.tag.starts_with("::") && wants_implicit(&k.style))
        });
    // Содержащий блок абсолюта — каждая коробка с `establishes_cb`: её
    // padding box читает сетка `position-area` (§position-area-grid-resolution),
    // связь с её собственным содержащим блоком — приемлемость якоря (§target).
    let cb = e.node_id != 0 && crate::text::inline::establishes_cb(&e.style);
    if names.is_empty() && !implicit && !cb {
        return None;
    }
    let id = e.node_id;
    let seq = c.anchor_seq;
    let own_cb = c.cb_node;
    // CSS Anchor Positioning 1 resolves logical border-box edges before snapping.
    // The absolute probe covers the padding box; expand it by the CSS borders.
    // The containing-block record remains the padding box (CSS 2.1 section 10.1).
    let bw = px_lossy_zero;
    let b = e.style.borders();
    let border = [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)];
    let values = vcx.keep();
    Some(
        gpui::canvas_with_unrounded_bounds(
            move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
                let vcx = values.vcx();
                if cb {
                    vcx.doc.anchor_frame.cb.borrow_mut().insert(id, bounds);
                    vcx.doc
                        .anchor_frame
                        .cb_parent
                        .borrow_mut()
                        .insert(id, own_cb);
                }
                if names.is_empty() && !implicit {
                    return;
                }
                let outer = Bounds {
                    origin: gpui::point(
                        bounds.origin.x - px(border[3]),
                        bounds.origin.y - px(border[0]),
                    ),
                    size: gpui::size(
                        bounds.size.width + px(border[1] + border[3]),
                        bounds.size.height + px(border[0] + border[2]),
                    ),
                };
                record(
                    &names,
                    implicit,
                    Entry {
                        id,
                        cb: own_cb,
                        seq,
                        hidden,
                    },
                    outer,
                    window,
                    vcx,
                );
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// Кто пишет запись в реестр якорей.
#[derive(Clone, Copy)]
pub(crate) struct Entry {
    pub id: u64,
    pub cb: u64,
    pub seq: u32,
    pub hidden: bool,
}

/// Записать рамку якоря `outer` (border box в точках окна, до трансформов)
/// под именами `names` и, при `implicit`, как неявный якорь `who.id`.
pub(crate) fn record(
    names: &[String],
    implicit: bool,
    who: Entry,
    outer: Bounds<Pixels>,
    window: &mut Window,
    vcx: ValueCx<'_>,
) {
    let Entry {
        id,
        cb: own_cb,
        seq,
        hidden,
    } = who;
    // Маска обрезки в точке пробы — пересечение `overflow`-обрезок
    // всех предков: по ней `AnchorPlace` решает, обрезан ли якорь
    // промежуточными коробками (§position-visibility).
    // Рамка раскладки — до трансформов: `Transformed` матрицу
    // применяет только на отрисовке. Отображённую снимаем здесь же
    // по стеку предков (`tf_map`), выбирает её цель (`lookup`):
    // `transform-001/002/009` — якорь с `translate(-200px, -100px)
    // scale(2)` обязан стоять там, где нарисован.
    let (rect_tf, tf_top) = tf_map(outer, vcx);
    let rec = AnchorRec {
        rect: outer,
        clip: window.content_mask().bounds,
        hidden,
        id,
        cb: own_cb,
        rect_tf,
        tf_top,
    };
    {
        let m = &vcx.doc.anchor_frame.named;
        let mut m = m.borrow_mut();
        for n in names {
            m.insert(n.clone(), rec);
        }
    };
    {
        let v = &vcx.doc.anchor_frame.named_seq;
        let mut v = v.borrow_mut();
        for n in names {
            v.push((n.clone(), seq, rec));
        }
    };
    if implicit {
        vcx.doc.anchor_frame.implicit.borrow_mut().insert(id, rec);
    }
    if vcx.doc.anchor_frame.used_last.get() {
        let same = |r: &AnchorRec| r.rect == rec.rect && r.rect_tf == rec.rect_tf;
        let stale = names.iter().any(|n| !{
            let v = &vcx.doc.anchor_frame.last_named;
            v.borrow()
                .iter()
                .any(|(k, s, r)| k == n && *s == seq && same(r))
        }) || (implicit
            && !vcx
                .doc
                .anchor_frame
                .last_implicit
                .borrow()
                .get(&id)
                .is_some_and(same));
        if stale {
            reframe_if_stale(window, vcx);
        } else {
            note_checked(vcx);
        }
    }
}
