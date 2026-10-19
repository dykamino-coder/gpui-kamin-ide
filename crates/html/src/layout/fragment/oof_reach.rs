//! Continuous containing-block coordinates for parallel positioned flows.
//!
//! Anchored extents belong to the containing block; static extents travel
//! through intervening flow boxes. Neither advances the in-flow cursor.

use crate::dom::Element;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::page::paged::visible_overflow;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::style::computed::Position;

#[derive(Clone, Copy, Default)]
struct Extent {
    anchored: Option<f32>,
    flow: Option<f32>,
}

impl Extent {
    fn merge(&mut self, other: Self, start: f32) {
        let max = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        self.anchored = max(self.anchored, other.anchored);
        self.flow = max(self.flow, other.flow.map(|v| start + v));
    }

    fn end(self) -> f32 {
        self.anchored.unwrap_or(0.0).max(self.flow.unwrap_or(0.0))
    }
}

/// Итог меры коробки для фрагментации (`fragment_size::measure`): форма,
/// досягаемость внепоточных, которую коробка отдаёт содержащему блоку выше,
/// и её собственный размер, когда меру дотянули внепоточные потомки. Blink
/// несёт то же в результате раскладки (`LayoutResult`:
/// `OutOfFlowPositionedDescendants`, собственный block-size фрагмента), а не
/// в общем состоянии.
pub(crate) struct Measure {
    pub(crate) shape: Shape,
    pub(crate) up: Reach,
    /// `(свой размер, мера с дотягом)`; `None` — мера и есть свой размер.
    pub(crate) own: Option<(f32, f32)>,
}

impl Measure {
    /// Мера без внепоточной досягаемости.
    pub(crate) fn plain(shape: Shape) -> Self {
        Measure {
            shape,
            up: Reach::default(),
            own: None,
        }
    }

    /// Собственный размер коробки, чья мера — `full` (дотяг относится только
    /// к той же мере).
    pub(crate) fn own_size(own: Option<(f32, f32)>, full: f32) -> f32 {
        own.filter(|&(_, measured)| (measured - full).abs() < 0.01)
            .map_or(full, |(own, _)| own)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Reach {
    pub(crate) legacy: f32,
    absolute: Extent,
    fixed: Extent,
    overflow: Option<f32>,
}

impl Reach {
    /// Досягаемость внепоточного ребёнка `k`: его собственная протяжённость
    /// плюс то, что его мера (`fragment_size::measure`) отдала наверх (`up`).
    pub(crate) fn positioned(
        k: &Element,
        full: f32,
        top: Option<f32>,
        cx: ShapeCx,
        up: Reach,
    ) -> Self {
        if cx.paged || cx.legacy_oof {
            return Self {
                legacy: if k.style.position == Some(Position::Absolute) {
                    (top.unwrap_or(0.0) + full).max(0.0)
                } else {
                    0.0
                },
                ..Self::default()
            };
        }
        let mut reach = up;
        let extent = if let Some(top) = top {
            Extent {
                anchored: Some(top + full),
                flow: None,
            }
        } else {
            Extent {
                anchored: None,
                flow: Some(full),
            }
        };
        match k.style.position {
            Some(Position::Absolute) => reach.absolute.merge(extent, 0.0),
            Some(Position::Fixed) => reach.fixed.merge(extent, 0.0),
            _ => {}
        }
        reach
    }

    /// Поточный ребёнок `k` мерой `full`: его собственный размер (`own` из
    /// его меры) и досягаемость, которую его мера отдала наверх (`up`).
    pub(crate) fn child(
        k: &Element,
        full: f32,
        cx: ShapeCx,
        up: Reach,
        own: Option<(f32, f32)>,
    ) -> (f32, Self) {
        if cx.paged || cx.legacy_oof || !visible_overflow(&k.style) {
            return (full, Self::default());
        }
        let mut reach = up;
        let own = Measure::own_size(own, full);
        if full > own + 0.01 {
            reach.overflow = Some(full);
        }
        (own, reach)
    }

    pub(crate) fn merge(&mut self, other: Self, start: f32) {
        self.legacy = self.legacy.max(start + other.legacy);
        self.absolute.merge(other.absolute, start);
        self.fixed.merge(other.fixed, start);
        if let Some(end) = other.overflow {
            self.overflow = Some(self.overflow.unwrap_or(0.0).max(start + end));
        }
    }

    /// Закрыть досягаемость на коробке `c`: сколько из неё приходится на
    /// саму `c` (она содержащий блок внепоточных) и что уходит выше — к
    /// содержащему блоку предка (второе значение отдаёт `measure(c)`).
    pub(crate) fn finish(mut self, c: &Element, cx: ShapeCx) -> (f32, Self) {
        if cx.paged || cx.legacy_oof {
            let own = if crate::text::inline::establishes_cb(&c.style) {
                self.legacy
            } else {
                0.0
            };
            return (own, Self::default());
        }
        let mut end = self.overflow.take().unwrap_or(0.0);
        if crate::text::inline::establishes_cb(&c.style) {
            end = end.max(self.absolute.end());
            self.absolute = Extent::default();
        }
        if fixed_cb_layer_box(&c.style) {
            end = end.max(self.fixed.end());
            self.fixed = Extent::default();
        }
        (end, self)
    }
}

#[cfg(test)]
mod tests;
