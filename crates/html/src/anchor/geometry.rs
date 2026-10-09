//! Resolve anchor placement from logical geometry before device pixel snapping.

use super::{AnchorPlace, Bounds, CHOSEN, LayoutId, Pixels, Placement, Window, px};

pub(super) fn logical_own(
    layout: LayoutId,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) -> Bounds<Pixels> {
    let origin = window.layout_origin_unrounded(layout);
    let half = px(0.5 / window.scale_factor() + 1e-4);
    // A manually placed root no longer shares its original layout coordinates.
    if (origin.x - bounds.origin.x).abs() > half || (origin.y - bounds.origin.y).abs() > half {
        return bounds;
    }
    Bounds {
        origin,
        size: window.layout_size_unrounded(layout),
    }
}

impl AnchorPlace {
    /// Выбор варианта (§fallback; Blink `OutOfFlowLayoutPart`, цикл
    /// `TryCalculateOffset`): сперва последний удачный (`CHOSEN`), пока он
    /// не переполняет; иначе первый непереполняющий по порядку списка, а при
    /// `position-try-order` — с наибольшим IMCB по заданной оси в письме
    /// СОДЕРЖАЩЕГО блока (устойчивая сортировка); ни один не влез — база с
    /// пометкой переполнения. Размер коробки у всех кандидатов — текущий:
    /// правило, меняющее размер, довозит его следующей сборкой.
    pub(super) fn choose(&self, own: Bounds<Pixels>, window: &Window) -> (usize, Placement) {
        let places: Vec<Placement> = self.plans.iter().map(|p| p.compute(own, window)).collect();
        if places.len() == 1 {
            return (0, places[0]);
        }
        if let Some(k) = CHOSEN.with(|m| m.borrow().get(&self.key).copied())
            && k < places.len()
            && !places[k].overflow
        {
            return (k, places[k]);
        }
        let mut fit: Vec<usize> = (0..places.len()).filter(|i| !places[*i].overflow).collect();
        let cb_vertical = self.plans[0].cb_vertical;
        let size = |i: usize| -> f32 {
            let (w, h) = places[i].imcb;
            match self.order {
                1 => w,
                2 => h,
                3 => {
                    if cb_vertical {
                        w
                    } else {
                        h
                    }
                }
                4 => {
                    if cb_vertical {
                        h
                    } else {
                        w
                    }
                }
                _ => 0.0,
            }
        };
        if self.order != 0 {
            fit.sort_by(|a, b| size(*b).total_cmp(&size(*a)));
        }
        let k = fit.first().copied().unwrap_or(0);
        (k, places[k])
    }
}
