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
use crate::tolerance::COORD_EPS;

mod block;
use block::{block, zero};

fn measure_fragment_and_own_height(c: &Element, vcx: ValueCx<'_>) -> Option<(Shape, f32)> {
    // TRANSITIONAL(F2-2): Прямой measure смешивает оценку normal size с OOF reach; удалить в P08 после исходного CB/static rect и emitted OOF child fragments.
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
    if c.style.used().position != Some(Position::Absolute)
        || !block(c)
        || !matches!(c.style.used().height, None | Some(Len::Auto))
        || !zero(c.style.used().margin.top)
        || !zero(c.style.used().margin.bottom)
    {
        return false;
    }
    let Some((s, _)) = measure_fragment_and_own_height(c, vcx) else {
        return false;
    };
    if s.cuts.is_empty() || (s.forced.is_empty() && s.solid.is_empty()) {
        return false;
    }
    let used = match (c.style.used().inset.top, c.style.used().inset.bottom) {
        (Some(Len::Px(top)), None | Some(Len::Auto)) => top,
        (None | Some(Len::Auto), Some(Len::Px(bottom))) => cb_height - bottom - s.height,
        (None | Some(Len::Auto), None | Some(Len::Auto)) => static_y,
        _ => return false,
    };
    let phase = (origin + used).rem_euclid(h);
    let original = c.style.clone();
    let mut probe = c.clone();
    probe.style.used_mut().position = Some(Position::Relative);
    probe.style.used_mut().inset.top = None;
    probe.style.used_mut().inset.bottom = None;
    probe.style.used_mut().margin.top = Some(Len::Px(phase));
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
    if after.height <= s.height + COORD_EPS {
        return false;
    }
    probe.style = original;
    probe.style.used_mut().inset.top = Some(Len::Px(used));
    probe.style.used_mut().inset.bottom = Some(Len::Auto);
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
    let cb = establishes_cb(&c.style.used());
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    let mut changed = false;
    for n in &mut c.children {
        let Node::Element(k) = n else { continue };
        if out_of_flow(&k.style.used()) {
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
    if !kids
        .iter()
        .any(|(c, _)| crate::layout::positioned::predicates::carries_abspos(c, 4))
    {
        return;
    }
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    for (c, s) in kids {
        let own = measure_fragment_and_own_height(c, vcx).map_or(s.height, |(_, own)| own);
        let start = cursor + previous.max(s.margin_top);
        // TRANSITIONAL(F2-2): После роста OOF форма всего ребёнка оценивается заново; удалить в P08 после независимого OOF resume и live Reach с исходным CB/static rect.
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
        if !out_of_flow(&c.style.used()) {
            cursor = start + own;
            previous = s.margin_bottom;
        }
    }
}

#[cfg(test)]
mod tests;
