//! Размеры и края коробки: входные CSS-длины и результат переноса ограничений.
//! Результат принадлежит раскладке; вычисленный стиль остаётся входом.

use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;
mod auto_min;
pub(crate) use auto_min::ratio_as_auto_min;

/// Физические оси x/y. Неразрешённые проценты и auto сохраняют прежний смысл.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BoxDimensions {
    pub(crate) preferred: [Option<Len>; 2],
    pub(crate) minimum: [Option<Len>; 2],
    pub(crate) maximum: [Option<Len>; 2],
}

impl BoxDimensions {
    pub(crate) fn from_computed(style: &Computed) -> Self {
        Self {
            preferred: [style.width, style.height],
            minimum: [style.min_width, style.min_height],
            maximum: [style.max_width, style.max_height],
        }
    }

    pub(crate) fn transfer_ratio(self, style: &Computed) -> Self {
        let Some(ratio) = style.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0) else {
            return self;
        };
        let sizes = self.preferred;
        let mins = self.minimum;
        let maxs = self.maximum;
        let mut resolved_min = mins;
        let mut resolved_max = maxs;
        for axis in 0..2 {
            let factor = if axis == 0 { ratio } else { 1.0 / ratio };
            // CSS Sizing 4, size transfers: собственные ограничения и preferred
            // size оси имеют приоритет перед переносом с противоположной оси.
            if matches!(mins[axis], None | Some(Len::Auto))
                && let Some(value) = pixels(mins[1 - axis])
            {
                let mut value = value * factor;
                for cap in [sizes[axis], maxs[axis]].into_iter().filter_map(pixels) {
                    value = value.min(cap);
                }
                resolved_min[axis] = Some(Len::Px(value));
            }
            // Content-based автоминимум может превышать перенесённый максимум.
            // Адаптер сохраняет auto зависимой оси; жёсткий максимум здесь
            // подавил бы её автоминимум (CSS Sizing 4 §5.2).
            if !ratio_as_auto_min(style, &self)
                && matches!(maxs[axis], None | Some(Len::Auto))
                && let Some(value) = pixels(maxs[1 - axis])
            {
                let mut value = value * factor;
                for floor in [sizes[axis], resolved_min[axis]]
                    .into_iter()
                    .filter_map(pixels)
                {
                    value = value.max(floor);
                }
                resolved_max[axis] = Some(Len::Px(value));
            }
        }
        Self {
            preferred: sizes,
            minimum: resolved_min,
            maximum: resolved_max,
        }
    }
}

fn pixels(value: Option<Len>) -> Option<f32> {
    match value {
        Some(Len::Px(value)) => Some(value),
        _ => None,
    }
}

/// Края входят в расчёт размеров отдельно от свойств коробки.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxEdges {
    pub(crate) outer: Sides,
    pub(crate) inner: Sides,
    pub(crate) stroke: Sides,
}

impl BoxEdges {
    pub(crate) fn from_computed(style: &Computed) -> Self {
        Self {
            outer: style.margin,
            inner: style.padding,
            stroke: style.borders(),
        }
    }

    pub(crate) fn content_compensation(self, border_box: bool, native: bool) -> [f32; 2] {
        let extra = |sides: [Option<Len>; 4]| {
            if border_box || native {
                return 0.0;
            }
            sides
                .into_iter()
                .filter_map(|s| match s {
                    Some(Len::Px(v)) => Some(v),
                    _ => None,
                })
                .sum()
        };
        [
            extra([
                self.inner.left,
                self.inner.right,
                self.stroke.left,
                self.stroke.right,
            ]),
            extra([
                self.inner.top,
                self.inner.bottom,
                self.stroke.top,
                self.stroke.bottom,
            ]),
        ]
    }
}

#[cfg(test)]
mod tests;
