//! Блок потока в полосах (place_flow) и внутренняя ширина последовательности.

use super::kid::{Kid, Kind};
use super::{COORD_EPS, Probe, Slot, clearance, intrinsic, probe};
use crate::layout::float::bands::FloatBands;

#[allow(clippy::too_many_arguments)]
pub(super) async fn place_flow(
    x0: f32,
    x1: f32,
    y0: f32,
    adjoining0: bool,
    bands: &mut FloatBands,
    p: &mut Probe<'_>,
    cbw: f32,
    slots: &mut [Slot],
    y: &mut f32,
    k: usize,
    kid: &Kid,
    mt: f32,
    mr: f32,
    mb: f32,
    ml: f32,
    stack: &mut gpui::PaintStack,
) {
    // Коробка во всю ширину содержащего блока: флоаты её
    // перекрывают, а строки получают вырезы полос от её верха
    // (Servo `place_line_among_floats`, `inline/mod.rs:1466`;
    // Blink `ComputeLineLayoutOpportunity`,
    // `layout_opportunity.cc:164-189`). Вырезы меряются от краёв
    // САМОЙ коробки: стенки на время — её border-box.
    let top = clearance::top(
        bands,
        kid.clear,
        *y,
        mt,
        kid.start_open && adjoining0 && (*y - y0).abs() < COORD_EPS,
    );
    let avail = (cbw - ml - mr).max(0.0);
    // CSS 2.1 §9.5: the line breaker moves individual lines below floats.
    // A first-word min-content probe uses the widest hyphenated fragment,
    // which can incorrectly clear a float even when the first fragment fits.
    let walls = bands.set_walls(x0 + ml, x1 - mr);
    let shapes = bands.shapes(top);
    bands.set_walls(walls.0, walls.1);
    let (bw, bh) = probe(kid, cbw, avail, Some(shapes.clone()), p, stack).await;
    slots[k] = Slot {
        prepared: p.last_prepared.clone(),
        // CSS 2.1 §10.3.3: an over-constrained box drops its end margin; in an
        // rtl containing block it stands at the right edge.
        x: inline_start(x0 + ml, x1 - mr, bw, kid.rtl),
        y: top,
        avail,
        shapes: Some(shapes),
        geometry: p.last_geometry.clone(),
        clamp: p.last_clamp.clone(),
        b: bh,
        i: bw,
        ..Slot::default()
    };
    *y = top + bh + mb;
}

/// Start of a box `width` wide between the edges `x0`..`x1` of its
/// containing block (CSS 2.1 §10.3.3: over-constrained margins drop the end
/// one, so the box stands at the inline-start — the right edge in rtl).
pub(super) fn inline_start(x0: f32, x1: f32, width: f32, rtl: bool) -> f32 {
    if rtl { x1 - width } else { x0 }
}

/// Внутренний размер контекста: min-content / max-content.
///
/// По Blink `BlockLayoutAlgorithm::ComputeMinMaxSizes`
/// (`block_layout_algorithm.cc:409-575`): флоаты копят инлайн-размер на
/// одной «строке» по сторонам, `clear` (у флоата или коробки своего
/// контекста) обрывает строку своей стороны, всякий не-флоат — обе; коробка
/// своего контекста прибавляет к себе отступы от флоатов рядом (поле
/// заменяет флоат, если больше); min-content — максимум по детям, каждый на
/// своей строке. Без обрыва на `clear` плавающий контейнер из дюжины
/// абзацев с `clear: left` мерился суммой всех (`letter-spacing-206-ref`).
pub(super) async fn intrinsic_sizes(
    kids: &[Kid],
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    intrinsic_sizes_async(kids, async |kid| intrinsic(kid, p, stack).await).await
}

#[derive(Clone, Copy, Default)]
struct IntrinsicSum {
    fl: f32,
    fr: f32,
    max_size: f32,
    min_size: f32,
}
impl IntrinsicSum {
    fn finish(self) -> (f32, f32) {
        (self.min_size, self.max_size.max(self.min_size))
    }
    fn include(&mut self, kid: &Kid, sizes: (f32, f32)) {
        let [_, mr, _, ml] = kid.margin.map(|e| e.at(0.0));
        let (mn, mx) = sizes;
        let Self {
            mut fl,
            mut fr,
            mut max_size,
            mut min_size,
        } = *self;
        let is_float = matches!(kid.kind, Kind::Float { .. });
        let is_fc = matches!(kid.kind, Kind::Piece { .. });
        let clear = match kid.kind {
            Kind::Float { clear, .. } => clear,
            _ => kid.clear,
        };
        if is_float || is_fc {
            if clear.is_some() {
                max_size = max_size.max(fl + fr);
            }
            if matches!(clear, Some(0) | Some(-1)) {
                fl = 0.0;
            }
            if matches!(clear, Some(0) | Some(1)) {
                fr = 0.0;
            }
        }
        let contribution = match kid.kind {
            Kind::Float { side, .. } => {
                // Флоат целиком за краем содержимого (отрицательные поля) в
                // размер не входит.
                let f = mx + ml + mr;
                if f > 0.0 {
                    if side < 0 {
                        fl += f;
                    } else {
                        fr += f;
                    }
                }
                fl + fr
            }
            Kind::Piece { .. } => {
                let li = if ml > 0.0 { fl.max(ml) } else { fl + ml };
                let ri = if mr > 0.0 { fr.max(mr) } else { fr + mr };
                mx + li + ri
            }
            // Анонимный прогон строк — строчный контекст САМОГО хоста: флоаты
            // перед ним стоят в той же строке, и max-content строки — их сумма
            // с текстом (Blink `InlineNode::ComputeMinMaxSizes`, флоаты в
            // списке строчных элементов). Без суммы хост ужимался до ширины
            // флоата, текст уходил под него, и хост выходил вдвое выше
            // (`floats-122`: флоат `X` и `X` за ним — 50 вместо 100).
            _ if kid.anon => fl + fr + mx + ml + mr,
            _ => mx + ml + mr,
        };
        max_size = max_size.max(contribution);
        min_size = min_size.max(mn + ml + mr);
        if !is_float {
            fl = 0.0;
            fr = 0.0;
        }
        *self = Self {
            fl,
            fr,
            max_size,
            min_size,
        };
    }
}

async fn intrinsic_sizes_async<'a>(
    kids: &'a [Kid],
    mut measure: impl AsyncFnMut(&'a Kid) -> (f32, f32),
) -> (f32, f32) {
    let mut pending = vec![(kids.iter(), IntrinsicSum::default(), None::<&Kid>)];
    loop {
        let (iter, sum, _) = pending
            .last_mut()
            .expect("intrinsic root remains until return");
        let Some(kid) = iter.next() else {
            let (_, sum, owner) = pending.pop().unwrap();
            let (mn, mx) = sum.finish();
            let Some(owner) = owner else {
                return (mn, mx);
            };
            let nest = owner.nest.as_ref().unwrap();
            let [_, ir, _, il] = nest.inset.map(|e| e.at(0.0));
            let sizes = (mn + il + ir, mx + il + ir);
            owner.intrinsic.set(Some(sizes));
            pending.last_mut().unwrap().1.include(owner, sizes);
            continue;
        };
        if matches!(kid.kind, Kind::Strut(_)) {
            continue;
        }
        if let Some(sizes) = kid.intrinsic.get() {
            sum.include(kid, sizes);
        } else if let Some(nest) = &kid.nest {
            pending.push((nest.kids.iter(), IntrinsicSum::default(), Some(kid)));
        } else {
            sum.include(kid, measure(kid).await);
        }
    }
}

#[cfg(test)]
fn intrinsic_sizes_with(kids: &[Kid], measure: &mut impl FnMut(&Kid) -> (f32, f32)) -> (f32, f32) {
    reblessive::Stack::new()
        .enter(|_| intrinsic_sizes_async(kids, async |kid| measure(kid)))
        .finish()
}

#[cfg(test)]
mod tests;
