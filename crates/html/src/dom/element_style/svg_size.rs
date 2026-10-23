//! Ключевые слова внутренних размеров у внешнего `<svg>` (css-sizing-3 §5.1,
//! css-sizing-4 §4). Размеры min-content и max-content замещаемой коробки — её
//! автоматический размер: природный, либо переданный через соотношение сторон
//! от определённой противоположной оси, иначе умолчание 300×150 (CSS 2.1
//! §10.3.2, §10.6.2). Blink `ComputeReplacedSizeInternal` (length_utils.cc)
//! разрешает `min-content`/`max-content`/`fit-content` замещаемой коробки в
//! этот же размер по обеим осям. Раскладка `<svg>` меряет коробку в точках,
//! поэтому слова становятся длинами здесь, как `canvas_size` для `<canvas>`.

use crate::defaults::{DEFAULT_REPLACED_HEIGHT_PX, DEFAULT_REPLACED_WIDTH_PX};
use crate::style::values::value::px_strict;
use crate::style::{computed::Computed, values::value::Len};

/// `viewBox="x y w h"` as `(w, h)` when both are positive.
pub(crate) fn view_box(raw: &str) -> Option<(f32, f32)> {
    let p: Vec<f32> = raw
        .split([' ', ',', '\t', '\n'])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    (p.len() == 4 && p[2] > 0.0 && p[3] > 0.0).then(|| (p[2], p[3]))
}

fn is_keyword(v: Option<Len>) -> bool {
    matches!(v, Some(Len::MinContent | Len::MaxContent | Len::FitContent))
}

pub(crate) fn resolve(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    if tag != "svg"
        || style.contains_width()
        || style.contains_height()
        || ![
            style.width,
            style.min_width,
            style.max_width,
            style.height,
            style.min_height,
            style.max_height,
        ]
        .into_iter()
        .any(is_keyword)
    {
        return;
    }
    let attr = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
    };
    let px = |v| px_strict(v);
    let (nat_w, nat_h) = (attr("width"), attr("height"));
    let natural_ratio = attrs
        .iter()
        .find(|(k, _)| k == "viewBox")
        .and_then(|(_, v)| view_box(v))
        .map(|(w, h)| w / h)
        .or(match (nat_w, nat_h) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(w / h),
            _ => None,
        });
    let ratio = style
        .aspect_ratio
        .or(natural_ratio)
        .or(style.aspect_ratio_auto)
        .filter(|r| r.is_finite() && *r > 0.0);
    let edges = style.borders();
    let sum = |a, b, c, d| {
        px(a).unwrap_or(0.0) + px(b).unwrap_or(0.0) + px(c).unwrap_or(0.0) + px(d).unwrap_or(0.0)
    };
    // Keywords are content sizes; under `border-box` the length carries the edges.
    let (bx, by) = if style.border_box == Some(true) {
        (
            sum(
                style.padding.left,
                style.padding.right,
                edges.left,
                edges.right,
            ),
            sum(
                style.padding.top,
                style.padding.bottom,
                edges.top,
                edges.bottom,
            ),
        )
    } else {
        (0.0, 0.0)
    };
    let given_w = px(style.width).map(|w| (w - bx).max(0.0));
    let given_h = px(style.height).map(|h| (h - by).max(0.0));
    let auto_h = match (given_w, ratio) {
        (Some(w), Some(r)) => w / r,
        _ => nat_h
            .or_else(|| nat_w.zip(ratio).map(|(w, r)| w / r))
            .unwrap_or(DEFAULT_REPLACED_HEIGHT_PX),
    };
    let auto_w = match (given_h, ratio) {
        (Some(h), Some(r)) => h * r,
        _ => nat_w
            .or_else(|| nat_h.zip(ratio).map(|(h, r)| h * r))
            .unwrap_or(DEFAULT_REPLACED_WIDTH_PX),
    };
    for v in [
        &mut style.height,
        &mut style.min_height,
        &mut style.max_height,
    ] {
        if is_keyword(*v) {
            *v = Some(Len::Px(auto_h + by));
        }
    }
    for (i, v) in [&mut style.width, &mut style.min_width, &mut style.max_width]
        .into_iter()
        .enumerate()
    {
        if is_keyword(*v) && style.fit_arg[i].is_none() {
            *v = Some(Len::Px(auto_w + bx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn block_keywords_without_ratio_use_the_natural_height() {
        let mut s = Computed {
            width: Some(Len::Px(100.0)),
            height: Some(Len::Px(1000.0)),
            max_height: Some(Len::MinContent),
            min_height: Some(Len::MaxContent),
            ..Computed::default()
        };
        resolve(&mut s, "svg", &attrs(&[("width", "0"), ("height", "100")]));
        assert_eq!(s.max_height, Some(Len::Px(100.0)));
        assert_eq!(s.min_height, Some(Len::Px(100.0)));
        assert_eq!(s.height, Some(Len::Px(1000.0)));
    }

    #[test]
    fn keywords_transfer_through_the_view_box_ratio() {
        let mut s = Computed {
            width: Some(Len::Px(80.0)),
            height: Some(Len::MinContent),
            ..Computed::default()
        };
        resolve(&mut s, "svg", &attrs(&[("viewBox", "0 0 2 1")]));
        assert_eq!(s.height, Some(Len::Px(40.0)));
        assert_eq!(view_box("0,0, 4 3"), Some((4.0, 3.0)));
        assert_eq!(view_box("0 0 0 3"), None);
    }

    #[test]
    fn other_tags_are_untouched() {
        let mut s = Computed {
            height: Some(Len::MinContent),
            ..Computed::default()
        };
        resolve(&mut s, "div", &attrs(&[("height", "10")]));
        assert_eq!(s.height, Some(Len::MinContent));
    }
}
