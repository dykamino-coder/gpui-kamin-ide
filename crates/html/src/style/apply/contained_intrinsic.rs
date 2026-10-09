//! Empty containment sizes retain the box geometry while excluding descendant contributions.

use crate::style::computed::{Computed, Display, Track, TrackSize};
use crate::style::values::value::Len;
use gpui::{Div, Styled};

/// Внутренний размер ПУСТОЙ коробки с `contain: size` по оси (без отступов).
///
/// css-contain-2 §3.1: коробка меряется «as if it had no contents» —
/// выбрасывается вклад СОДЕРЖИМОГО, но не собственная геометрия коробки:
/// явные дорожки сетки со щелями (css-grid-2 §11/§12: у пустой сетки дорожка
/// `auto`/`fr`/по содержимому — ноль, фиксированная — своя длина) и
/// «число × ширина колонки + щели» многоколонника (css-multicol-1 §3.4).
/// Шрифтовые дорожки и `repeat(auto-*)` без раскладки не посчитать — ноль, как
/// было.
///
/// `inline` — строчная ось (ширина при горизонтальном письме).
pub(super) fn empty_contained_size(c: &Computed, inline: bool) -> f32 {
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // `gap` хранит пару (ряды, колонки); `column-gap` пишет и пару, и своё
    // поле (`computed.rs`, ветки `"gap"`/`"column-gap"`).
    let gap = if inline {
        c.gap.and_then(|g| g.1).or(c.column_gap)
    } else {
        c.gap.and_then(|g| g.0)
    };
    if inline
        && let (Some(count), Some(Len::Px(w))) = (c.column_count, c.column_width)
        && count > 0
        && w > 0.0
    {
        // `column-gap: normal` — кегль (css-align-3 §8.3), как в блочном пути.
        let g = match c.column_gap {
            Some(Len::Px(v)) => v,
            _ => match c.font_size {
                Some(Len::Px(size)) => size,
                _ => 16.0,
            },
        };
        return count as f32 * w + (count as f32 - 1.0) * g;
    }
    let grid = matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid));
    if !grid {
        return 0.0;
    }
    let tracks = if inline {
        c.grid_tracks.as_ref()
    } else {
        c.grid_rows.as_ref()
    };
    let Some(tracks) = tracks.filter(|t| !t.is_empty()) else {
        return 0.0;
    };
    let one = |t: &Track| -> Option<f32> {
        match t {
            Track::Px(v) => Some(*v),
            Track::Font(_) => None,
            _ => Some(0.0),
        }
    };
    let sum: Option<f32> = tracks.iter().try_fold(0.0f32, |acc, t| match t {
        TrackSize::Single(x) => one(x).map(|v| acc + v),
        // У пустой дорожки `minmax(a, b)` размер по max-content — верхняя
        // граница, если она фиксирована, иначе нижняя.
        TrackSize::MinMax(lo, hi) => match (one(lo), hi) {
            (_, Track::Px(v)) => Some(acc + v),
            (Some(v), _) => Some(acc + v),
            (None, _) => None,
        },
        _ => None,
    });
    match sum {
        Some(total) => total + px_of(gap) * tracks.len().saturating_sub(1) as f32,
        None => 0.0,
    }
}

pub(super) fn apply(d: &mut Div, c: &Computed) {
    // CSS Containment 2 §3.1: auto block widths still fill definite space,
    // but their intrinsic contributions exclude real descendants.
    d.style().contained_intrinsic_size = Some([
        c.contains_width().then(|| {
            c.contain_intrinsic
                .0
                .unwrap_or_else(|| empty_contained_size(c, true))
        }),
        c.contains_height().then(|| {
            c.contain_intrinsic
                .1
                .unwrap_or_else(|| empty_contained_size(c, false))
        }),
    ]);
}
