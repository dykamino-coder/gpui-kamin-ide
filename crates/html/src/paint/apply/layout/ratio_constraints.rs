//! Применение к GPUI: перенос определённых ограничений `min-*`/`max-*` через
//! `aspect-ratio` на другую ось (CSS Sizing 4, size transfers). Заданные автором
//! ограничения и предпочтительный размер оси-получателя старше перенесённых;
//! при автоматическом минимуме по содержимому жёсткий перенесённый максимум не
//! вставляется (CSS Sizing 4 §5.2). Возвращает копию стиля только при изменении.

use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

pub(super) fn resolve(style: &BoxStyle) -> Option<BoxStyle> {
    let ratio = style.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0)?;
    let sizes = [style.width, style.height];
    let mins = [style.min_width, style.min_height];
    let maxs = [style.max_width, style.max_height];
    let mut resolved_min = mins;
    let mut resolved_max = maxs;
    for axis in 0..2 {
        let factor = if axis == 0 { ratio } else { 1.0 / ratio };
        // CSS Sizing 4, size transfers: authored destination constraints and
        // preferred sizes take precedence over constraints from the other axis.
        if matches!(mins[axis], None | Some(Len::Auto))
            && let Some(value) = pixels(mins[1 - axis])
        {
            let mut value = value * factor;
            for cap in [sizes[axis], maxs[axis]].into_iter().filter_map(pixels) {
                value = value.min(cap);
            }
            resolved_min[axis] = Some(Len::Px(value));
        }
        // The content-based automatic minimum can exceed a transferred max.
        // Its adapter keeps the dependent preferred size auto; inserting a
        // hard maximum here would defeat that minimum (CSS Sizing 4 §5.2).
        if !super::ratio_as_auto_min(style)
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
    if resolved_min == mins && resolved_max == maxs {
        return None;
    }
    let mut resolved = style.clone();
    [resolved.min_width, resolved.min_height] = resolved_min;
    [resolved.max_width, resolved.max_height] = resolved_max;
    Some(resolved)
}

fn pixels(value: Option<Len>) -> Option<f32> {
    match value {
        Some(Len::Px(value)) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn transferred_minimum_does_not_override_explicit_maximum() {
        let style = Computed {
            aspect_ratio: Some(1.0),
            min_height: Some(Len::Px(200.0)),
            max_width: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        let resolved = resolve(&BoxStyle::from(style)).unwrap();
        assert_eq!(resolved.min_width, Some(Len::Px(100.0)));
        assert_eq!(resolved.max_height, Some(Len::Px(200.0)));
    }

    #[test]
    fn transferred_maximum_does_not_override_preferred_size() {
        let style = Computed {
            aspect_ratio: Some(2.0),
            width: Some(Len::Px(300.0)),
            min_height: Some(Len::Px(0.0)),
            max_height: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        assert_eq!(
            resolve(&BoxStyle::from(style)).unwrap().max_width,
            Some(Len::Px(300.0))
        );
    }

    #[test]
    fn transferred_maximum_does_not_cap_content_based_automatic_minimum() {
        let style = Computed {
            aspect_ratio: Some(2.0),
            width: Some(Len::Px(200.0)),
            max_width: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        assert!(resolve(&BoxStyle::from(style)).is_none());
    }

    #[test]
    fn transferred_constraint_preserves_content_box_units() {
        let mut style = Computed {
            aspect_ratio: Some(1.25),
            max_height: Some(Len::Px(80.0)),
            ..Computed::default()
        };
        style.padding.bottom = Some(Len::Px(20.0));
        assert_eq!(
            resolve(&BoxStyle::from(style)).unwrap().max_width,
            Some(Len::Px(100.0))
        );
    }
}
