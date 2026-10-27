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
use crate::layout::positioned::containing_block::metrics::{PositionedMetrics, Snapshot};
use crate::style::computed::Position;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

mod block;
use block::{block, zero};

fn measure_fragment_and_own_height(c: &Element, vcx: ValueCx<'_>) -> Option<(Shape, f32)> {
    // TRANSITIONAL(F2-2): Прямой measure смешивает оценку normal size с OOF reach копий held-семей (P08b) и grid-семей (P16); удалить, когда их продолжения рисует нативный источник.
    let m = crate::layout::fragment::fragment_size::measure(c, 4, ShapeCx::COLUMNS, vcx)?;
    let own = Measure::own_size(m.own, m.shape.height);
    Some((m.shape, own))
}

#[allow(clippy::too_many_arguments)]
fn positioned(
    c: &mut Element,
    geometry: &PositionedMetrics,
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
    let used = f32::from(geometry.normal_offset.y);
    let phase = f32::from(geometry.normal_rect().origin.y).rem_euclid(h);
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
    geometry: &Snapshot,
    h: f32,
    count: usize,
    copies: usize,
    vcx: ValueCx<'_>,
) -> bool {
    if !block(c) {
        return false;
    }
    let mut changed = false;
    for n in &mut c.children {
        let Node::Element(k) = n else { continue };
        if out_of_flow(&k.style.used()) {
            if let Some(metrics) = geometry.get(k.node_id) {
                changed |= positioned(k, metrics, h, count, copies, vcx);
            }
        } else {
            changed |= grow_positioned_descendants(k, geometry, h, count, copies, vcx);
        }
    }
    changed
}

pub(super) fn grow(
    kids: &mut [(Element, Shape)],
    h: f32,
    count: usize,
    copies: usize,
    geometry: Option<&Snapshot>,
    vcx: ValueCx<'_>,
) {
    let Some(geometry) = geometry else { return };
    for (c, s) in kids {
        // TRANSITIONAL(F2-2): После роста OOF форма всего ребёнка оценивается заново для копий held-семей (P08b) и grid-семей (P16); удалить после их нативного resume и live Reach.
        if !c.marks.paginate
            && grow_positioned_descendants(c, geometry, h, count, copies, vcx)
            && let Some(next) = shape_full(c, 4, ShapeCx::COLUMNS, vcx)
        {
            *s = crate::layout::fragment::Shape {
                margin_top: s.margin_top,
                margin_bottom: s.margin_bottom,
                ..next
            };
        }
    }
}

#[cfg(test)]
mod tests;
