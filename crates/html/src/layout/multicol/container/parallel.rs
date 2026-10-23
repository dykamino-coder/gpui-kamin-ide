//! Независимый рост на разрывах для позиционированных потоков в непрерывных блочных
//! координатах: абсолютная коробка авто-высоты внутри ребёнка стопки получает распорки
//! фрагментации по фазе своего содержащего блока в сетке колонок.
//!
//! Исходная позиция в содержащем блоке разрешается ДО вставки места под разрывы: рост
//! абсолютной коробки авто-высоты не должен заново разрешать её нижнюю вставку.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::oof_reach::Measure;
use crate::layout::fragment::push::grow_pushed;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::Position;
use crate::style::values::value::Len;
use crate::text::inline::establishes_cb;

mod block;
use block::{block, zero};

fn measure_fragment_and_own_height(c: &Element, vcx: ValueCx<'_>) -> Option<(Shape, f32)> {
    let m = crate::layout::fragment::fragment_size::measure(c, 4, ShapeCx::COLUMNS, vcx)?;
    let own = Measure::own_size(m.own, m.shape.height);
    Some((m.shape, own))
}

#[allow(clippy::too_many_arguments)]
fn positioned(
    c: &mut Element,
    cb_height: f32,
    static_y: f32,
    origin: f32,
    h: f32,
    count: usize,
    copies: usize,
    vcx: ValueCx<'_>,
) -> bool {
    if c.style.cascaded().position != Some(Position::Absolute)
        || !block(c)
        || !matches!(c.style.cascaded().height, None | Some(Len::Auto))
        || !zero(c.style.cascaded().margin.top)
        || !zero(c.style.cascaded().margin.bottom)
    {
        return false;
    }
    let Some((s, _)) = measure_fragment_and_own_height(c, vcx) else {
        return false;
    };
    if s.cuts.is_empty() || (s.forced.is_empty() && s.solid.is_empty()) {
        return false;
    }
    let used = match (
        c.style.cascaded().inset.top,
        c.style.cascaded().inset.bottom,
    ) {
        (Some(Len::Px(top)), None | Some(Len::Auto)) => top,
        (None | Some(Len::Auto), Some(Len::Px(bottom))) => cb_height - bottom - s.height,
        (None | Some(Len::Auto), None | Some(Len::Auto)) => static_y,
        _ => return false,
    };
    let phase = (origin + used).rem_euclid(h);
    let original = c.style.clone();
    let mut probe = c.clone();
    probe.style.cascaded_mut().position = Some(Position::Relative);
    probe.style.cascaded_mut().inset.top = None;
    probe.style.cascaded_mut().inset.bottom = None;
    probe.style.cascaded_mut().margin.top = Some(Len::Px(phase));
    let mut shape = s.clone();
    shape.margin_top = phase;
    let planned = copies.max(((phase + s.height) / h).ceil() as usize + count);
    let mut grown = grow_pushed(
        vec![(probe, shape)],
        count,
        Some(h),
        None,
        planned,
        &[],
        &|_| false,
        vcx,
    );
    let (mut probe, after) = grown.remove(0);
    if after.height <= s.height + 0.01 {
        return false;
    }
    probe.style = original;
    probe.style.cascaded_mut().inset.top = Some(Len::Px(used));
    probe.style.cascaded_mut().inset.bottom = Some(Len::Auto);
    *c = probe;
    true
}

fn grow_positioned_descendants(
    c: &mut Element,
    origin: f32,
    h: f32,
    count: usize,
    copies: usize,
    vcx: ValueCx<'_>,
) -> bool {
    if !block(c) {
        return false;
    }
    let Some((_, own)) = measure_fragment_and_own_height(c, vcx) else {
        return false;
    };
    let cb = establishes_cb(c.style.cascaded());
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    let mut changed = false;
    for n in &mut c.children {
        let Node::Element(k) = n else { continue };
        if out_of_flow(k.style.cascaded()) {
            if cb {
                changed |= positioned(k, own, cursor, origin, h, count, copies, vcx);
            }
            continue;
        }
        let Some((s, height)) = measure_fragment_and_own_height(k, vcx) else {
            continue;
        };
        let start = cursor + previous.max(s.margin_top);
        changed |= grow_positioned_descendants(k, origin + start, h, count, copies, vcx);
        cursor = start + height;
        previous = s.margin_bottom;
    }
    changed
}

pub(super) fn grow(
    kids: &mut [(Element, Shape)],
    h: f32,
    count: usize,
    copies: usize,
    vcx: ValueCx<'_>,
) {
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    for (c, s) in kids {
        let own = measure_fragment_and_own_height(c, vcx).map_or(s.height, |(_, own)| own);
        let start = cursor + previous.max(s.margin_top);
        if !c.marks.paginate
            && grow_positioned_descendants(c, start, h, count, copies, vcx)
            && let Some(next) = shape_full(c, 4, ShapeCx::COLUMNS, vcx)
        {
            *s = crate::layout::fragment::Shape {
                margin_top: s.margin_top,
                margin_bottom: s.margin_bottom,
                ..next
            };
        }
        if !out_of_flow(c.style.cascaded()) {
            cursor = start + own;
            previous = s.margin_bottom;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Fragment growth preserves the original CB position and its normal-flow size.

    use super::*;
    use crate::style::computed::Computed;

    fn element(style: Computed, children: Vec<Node>) -> Element {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);
        Element {
            marks: Default::default(),
            node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            tag: "div".into(),
            style: style.into(),
            children,
            attrs: Vec::new(),
            inline: false,
            list_item: None,
            anim: None,
            hover: None,
            first_letter: None,
            first_line: None,
        }
    }

    fn containing_block() -> Element {
        let children = [(90.0, false), (50.0, true), (50.0, false)]
            .into_iter()
            .map(|(h, avoid)| {
                Node::Element(element(
                    Computed {
                        height: Some(Len::Px(h)),
                        break_inside_avoid: avoid,
                        ..Default::default()
                    },
                    Vec::new(),
                ))
            })
            .collect();
        let mut abs = element(
            Computed {
                position: Some(Position::Absolute),
                ..Default::default()
            },
            children,
        );
        abs.style.cascaded_mut().inset.bottom = Some(Len::Px(0.0));
        element(
            Computed {
                height: Some(Len::Px(190.0)),
                position: Some(Position::Relative),
                ..Default::default()
            },
            vec![Node::Element(abs)],
        )
    }

    #[test]
    fn bottom_position_is_resolved_before_auto_height_grows() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut cb = containing_block();
        assert!(grow_positioned_descendants(&mut cb, 0.0, 100.0, 2, 4, vcx));
        let Node::Element(abs) = &cb.children[0] else {
            panic!()
        };
        assert_eq!(abs.style.cascaded().inset.top, Some(Len::Px(0.0)));
        assert_eq!(
            measure_fragment_and_own_height(abs, vcx).unwrap().0.height,
            200.0
        );
        let (shape, own) = measure_fragment_and_own_height(&cb, vcx).unwrap();
        assert_eq!(shape.height, 200.0);
        assert_eq!(own, 190.0);
    }

    #[test]
    fn independent_breaks_use_the_containing_blocks_fragmentainer_phase() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut cb = containing_block();
        assert!(grow_positioned_descendants(&mut cb, 80.0, 100.0, 2, 4, vcx));
        let Node::Element(abs) = &cb.children[0] else {
            panic!()
        };
        assert_eq!(abs.style.cascaded().inset.top, Some(Len::Px(0.0)));
        assert_eq!(
            measure_fragment_and_own_height(abs, vcx).unwrap().0.height,
            220.0
        );
        assert_eq!(measure_fragment_and_own_height(&cb, vcx).unwrap().1, 190.0);
    }
}
