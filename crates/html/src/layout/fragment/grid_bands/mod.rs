//! Полосы сетки при фрагментации.
// owner: A

use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod auto_rows;
pub(super) use auto_rows::grid_auto_row_bands;
pub(super) use auto_rows::grid_items_spotted;
mod forced;
pub(super) use forced::grid_row_forced;
pub(crate) use forced::grid_stack;
pub(super) use forced::grow_grid_track;

/// Высота сетки по ЯВНЫМ дорожкам рядов: все дорожки в
/// точках, плюс зазоры между ними. `None` — дорожки
/// неизвестны или не все в точках.
pub(crate) fn grid_rows_px(c: &Computed) -> Option<f32> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let mut total = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => total += v,
            _ => return None,
        }
    }
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    Some(total + gap * (rows.len() as f32 - 1.0))
}

/// Зазоры между ЯВНЫМИ рядами сетки от верха содержимого: `(начало, конец)`.
/// Ряды — в точках либо доли `fr` при заданной в точках высоте коробки
/// (остаток после точечных рядов и зазоров делится по долям, css-grid-1
/// §12.7). Иначе — пусто: дорожек не знаем, точек не даём.
pub(super) fn grid_row_gaps(c: &Computed, inner_h: f32) -> Vec<(f32, f32)> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return Vec::new();
    }
    let Some(rows) = c.grid_rows.as_ref() else {
        return Vec::new();
    };
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    if rows.len() < 2 || gap <= 0.0 {
        return Vec::new();
    }
    let mut fixed = 0.0f32;
    let mut fr = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => fixed += v,
            TrackSize::Single(Track::Fr(k)) => fr += k,
            _ => return Vec::new(),
        }
    }
    let per_fr = if fr > 0.0 {
        if !matches!(c.height, Some(Len::Px(_))) {
            return Vec::new();
        }
        (inner_h - fixed - gap * (rows.len() as f32 - 1.0)).max(0.0) / fr
    } else {
        0.0
    };
    let mut out = Vec::new();
    let mut y = 0.0f32;
    for (i, t) in rows.iter().enumerate() {
        y += match t {
            TrackSize::Single(Track::Px(v)) => *v,
            TrackSize::Single(Track::Fr(k)) => k * per_fr,
            _ => 0.0,
        };
        if i + 1 < rows.len() {
            out.push((y, y + gap));
            y += gap;
        }
    }
    out
}

/// Полосы ЯВНЫХ рядов сетки, когда все дорожки и зазор — в точках:
/// `(начало, конец)` каждого ряда от верха содержимого. Тот же путь, что
/// даёт высоту в `grid_rows_px`, только развёрнутый по рядам: границы рядов
/// — точки разреза класса A (css-grid-2 §Fragmenting Grid Layout: «Class A
/// break opportunities occur between rows or columns»). `None` — дорожек
/// нет, они не все в точках или зазор задан не в точках: границ мы не знаем
/// и точек не даём. Строже, чем `grid_rows_px` (тот считает незнакомый
/// зазор нулём) — неверная граница ряда хуже отсутствующей.
fn grid_px_row_bands(c: &Computed) -> Option<Vec<(f32, f32)>> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let gap = match c.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    let mut out = Vec::with_capacity(rows.len());
    let mut y = 0.0f32;
    for t in rows {
        let TrackSize::Single(Track::Px(v)) = t else {
            return None;
        };
        out.push((y, y + v));
        y += v + gap;
    }
    Some(out)
}
