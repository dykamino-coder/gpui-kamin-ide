//! Размеры рядов и ограничения вложенного многоколоночного потока.

use crate::dom::Element;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn row_plan(
    e: &Element,
    _merged: &Computed,
    opts: &RenderOpts,
    column_width: Option<Len>,
    cols: u16,
    col_vert: bool,
    outer_row: Option<(f32, f32)>,
) -> RowPlan {
    let col_w_px = match column_width {
        Some(Len::Px(w)) if w > 0.0 => Some(w),
        _ => None,
    };
    let want = (cols > 0).then_some(cols as usize);
    // Ряды колонок (css-multicol-2 §ch, §cwr): высота ряда —
    // `column-height`, а при `column-wrap: wrap` без него —
    // высота коробки (Blink `RowHeight()`:
    // `remaining_content_block_size_`, issue 11754 вариант 2);
    // `column-wrap: auto` = `wrap` при заданном
    // `column-height`. `row-gap: normal` в колонках — 1em (§rg).
    let col_h = match e.style.column_height {
        Some(Len::Px(h)) if h >= 0.0 => Some(h),
        _ => None,
    };
    // Блочный размер коробки: в вертикальном письме — ширина.
    let box_h = match if col_vert {
        e.style.width
    } else {
        e.style.height
    } {
        Some(Len::Px(h)) if h > 0.0 => Some(h),
        _ => None,
    };
    let wrap = e.style.column_wrap.unwrap_or(col_h.is_some());
    let em = match e.style.font_size {
        Some(Len::Px(size)) => size,
        _ => opts.base_size(),
    };
    let row_gap = match e.style.gap.and_then(|g| g.0) {
        Some(Len::Px(v)) => v.max(0.0),
        Some(Len::Em(k)) => k * em,
        _ => em,
    };
    let rows = (col_h.is_some() || wrap).then_some(crate::layout::fragment::types::Rows {
        h: col_h.or(if wrap { box_h } else { None }),
        gap: row_gap,
        wrap,
        cap: false,
    });
    // Потолок баланса (`Rows::cap`): СОБСТВЕННАЯ высота коробки в
    // точках. ★ Прежний MC-BALANCE-CAP (float.rs, v164, +3/−9)
    // спускал внешний потолок во ВЛОЖЕННЫЕ многоколоночники без
    // своей высоты — все девять потерь такие; здесь потолок не
    // передаётся вниз (`in_stack`: внутри копии другой стопки —
    // нет), не трогает `column-fill: auto` (там `fixed`) и ряды
    // (`column-height`). `grid-container-fragmentation-004`: 350 в
    // 4 колонках по 100 — баланс уходил в 125, в четвёртой
    // колонке красное 50..100.
    // Потолок баланса — ИСПОЛЬЗУЕМАЯ высота коробки, а не голая `height`:
    // Blink `ConstrainColumnBlockSize` (`column_layout_algorithm.cc:
    // 1774-1793`) берёт `max = min(max-height, height)`, затем
    // `max = max(max, min-height)` («A specified min-block-size may
    // increase the maximum length»; CSS 2.1 §10.7). `multicol-fill-
    // balance-005`: `height:20px; max-height:40px; min-height:100px` —
    // коробка 100, баланс 200/2 = 100 ровно в неё; с потолком 20 колонки
    // выходили по 20, и красный фон 20..100 был виден. `min-height` не в
    // точках (доля, `em`) — потолка нет, как до P5: ниже используемой
    // высоты резать нельзя, а её здесь не знаем.
    let (max_block, min_block) = if col_vert {
        (e.style.max_width, e.style.min_width)
    } else {
        (e.style.max_height, e.style.min_height)
    };
    let cap_h = box_h.and_then(|h| {
        let h = match max_block {
            Some(Len::Px(m)) if m >= 0.0 => h.min(m),
            _ => h,
        };
        match min_block {
            None | Some(Len::Auto) => Some(h),
            Some(Len::Px(m)) => Some(h.max(m)),
            _ => None,
        }
    });
    // Вложенный многоколоночник во внешней колонке (`flow::OUTER_ROW`):
    // ряды высотой во внешний фрагментаинер без зазора — граница
    // ряда совпадает с границей внешней колонки (css-break-4 §2.1;
    // Blink `ConstrainColumnBlockSize`, `LayoutLine` :1009-1026
    // «wrap … if we're participating in an outer fragmentation
    // context»). Последний ряд балансируется (css-multicol-1 §7.1
    // «only the last fragment is balanced»).
    let nest_phase = outer_row.map_or(0.0, |r| r.1);
    let nest_rows = outer_row
        .map(|r| r.0)
        .filter(|_| col_h.is_none() && e.style.column_wrap.is_none() && !col_vert);
    let rows = match nest_rows {
        Some(hh) => Some(crate::layout::fragment::types::Rows {
            h: Some(hh),
            gap: 0.0,
            wrap: true,
            cap: false,
        }),
        None => rows,
    };
    let rows = rows.or_else(|| {
        (cap_h.is_some()
            && e.style.column_fill_auto != Some(true)
            && !crate::layout::fragment::types::in_stack())
        .then_some(crate::layout::fragment::types::Rows {
            h: cap_h,
            gap: row_gap,
            wrap: false,
            cap: true,
        })
    });
    RowPlan {
        col_w_px,
        want,
        col_h,
        box_h,
        row_gap,
        rows,
        nest_phase,
        nest_rows,
    }
}

pub(crate) struct RowPlan {
    pub(super) col_w_px: Option<f32>,
    pub(super) want: Option<usize>,
    pub(super) col_h: Option<f32>,
    pub(super) box_h: Option<f32>,
    pub(super) row_gap: f32,
    pub(super) rows: Option<crate::layout::fragment::types::Rows>,
    pub(super) nest_phase: f32,
    pub(super) nest_rows: Option<f32>,
}
