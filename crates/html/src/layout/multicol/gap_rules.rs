//! Правила промежутков колонок (`column-rule`, `*-rule`).
// owner: A

use crate::render::*;

/// Пробельный текстовый узел: в подсчёте детей он не участвует.
/// Правила линеек промежутков (css-gaps-1) контейнера — `None`, когда ни
/// одна линейка не задана. Длины (`em`) сводятся в точки здесь: слой знает
/// только геометрию. Цвет по умолчанию — `currentcolor`, ширина — `medium`.
pub(crate) fn gap_rule_spec(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
) -> Option<crate::interact::GapRuleSpec> {
    use crate::computed::{FlexDir, GapInset, GapList};
    use crate::interact::{GapAxisRule, GapLayout};
    if !matches!(
        merged.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
    ) {
        return None;
    }
    let size = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let px = |l: &Len| match l {
        Len::Px(v) => *v,
        Len::Em(k) => k * size,
        _ => 3.0,
    };
    let fallback = merged.color.unwrap_or(crate::value::Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    let s = &e.style;
    let axis = |column: bool| -> Option<GapAxisRule> {
        let (vis, w, c, ws, ss, cs, brk, inset, visibility) = if column {
            (
                s.column_rule_visible,
                &s.column_rule_width,
                s.column_rule_color,
                &s.column_rule_widths,
                &s.column_rule_styles,
                &s.column_rule_colors,
                s.column_rule_break,
                s.column_rule_inset,
                s.column_rule_visibility,
            )
        } else {
            (
                s.row_rule_visible,
                &s.row_rule_width,
                s.row_rule_color,
                &s.row_rule_widths,
                &s.row_rule_styles,
                &s.row_rule_colors,
                s.row_rule_break,
                s.row_rule_inset,
                s.row_rule_visibility,
            )
        };
        let styles = ss
            .clone()
            .unwrap_or_else(|| GapList::single(vis == Some(true)));
        if !styles.any(|v| *v) {
            return None;
        }
        let widths = ws
            .as_ref()
            .map(|l| l.map(px))
            .unwrap_or_else(|| GapList::single(w.as_ref().map(px).unwrap_or(3.0)));
        let colors = cs
            .as_ref()
            .map(|l| l.map(|c| c.unwrap_or(fallback)))
            .unwrap_or_else(|| GapList::single(c.unwrap_or(fallback)));
        let inset = inset
            .unwrap_or([GapInset::Len(Len::Px(0.0)); 4])
            .map(|i| match i {
                GapInset::Len(Len::Em(k)) => GapInset::Len(Len::Px(k * size)),
                other => other,
            });
        Some(GapAxisRule {
            widths,
            colors,
            styles,
            brk: brk.unwrap_or(1),
            inset,
            visibility: visibility.unwrap_or(0),
            double: if column { s.column_rule_double } else { s.row_rule_double },
        })
    };
    let col = axis(true);
    let row = axis(false);
    if col.is_none() && row.is_none() {
        return None;
    }
    let vertical = merged.vertical == Some(true);
    // Лунки идут путём сетки (`dom::lanes_to_grid`: `display: grid` с
    // пометкой `lanes_taffy`), но промежутки у них — ленты: главные между
    // лентами через всё поле содержимого, поперечные — между элементами
    // ленты (Blink `GridLanesGapAccumulator`).
    let lanes = merged.lanes_taffy || merged.display == Some(Display::GridLanes);
    let kind = match merged.display {
        _ if merged.lanes_taffy => GapLayout::Lines {
            stacked_vertically: crate::dom::lanes_row_dir(merged) != vertical,
        },
        Some(Display::Grid) | Some(Display::InlineGrid) => GapLayout::Grid,
        Some(Display::GridLanes) => {
            // Направление лент — как в `lanes()`: явное или по той оси, где
            // объявлены дорожки.
            let row_tracks = merged.grid_rows.is_some()
                || merged.auto_repeat_rows.is_some()
                || merged.grid_auto_fill_row.is_some();
            let col_tracks = merged.grid_tracks.is_some()
                || merged.auto_repeat_cols.is_some()
                || merged.grid_auto_fill_min.is_some();
            GapLayout::Lines {
                stacked_vertically: merged.lanes_row.unwrap_or(row_tracks && !col_tracks),
            }
        }
        _ => {
            // Строки гибкого контейнера уложены поперёк главной оси; в
            // вертикальном письме `row` идёт по вертикали.
            let row_dir = !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            GapLayout::Lines {
                stacked_vertically: row_dir != vertical,
            }
        }
    };
    let gap_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(k)) => Some(k * size),
        _ => None,
    };
    let (row_gap, col_gap) = match s.gap {
        Some((r, c)) => (gap_px(r), gap_px(c)),
        None => (None, None),
    };
    let (gap_x, gap_y) = if vertical {
        (row_gap, col_gap)
    } else {
        (col_gap, row_gap)
    };
    // Дорожки шаблона в точках. Берётся ТОЛЬКО целиком точечный список: доли
    // `fr`, проценты и дорожки по содержимому разрешает раскладка, а здесь
    // использованного размера контейнера ещё нет. Строки и ленты
    // (`GapLayout::Lines`) шаблона не имеют вовсе: у каждой строки свои
    // промежутки между элементами (css-gaps-1 §gap-flex).
    let track_px = |list: Option<&Vec<crate::computed::TrackSize>>| -> Option<Vec<f32>> {
        use crate::computed::{Track, TrackSize};
        let list = list?;
        if list.len() < 2 {
            return None;
        }
        list.iter()
            .map(|t| match t {
                TrackSize::Single(Track::Px(v)) => Some(*v),
                _ => None,
            })
            .collect()
    };
    let (tpl_rows, tpl_cols) = match kind {
        GapLayout::Grid => (
            track_px(merged.grid_rows.as_ref()),
            track_px(merged.grid_tracks.as_ref()),
        ),
        GapLayout::Lines { .. } => (None, None),
    };
    // В вертикальном письме колонки сетки идут по y — тем же поворотом, что и
    // `gap_x`/`gap_y` строкой выше.
    let (tracks_x, tracks_y) = if vertical {
        (tpl_rows, tpl_cols)
    } else {
        (tpl_cols, tpl_rows)
    };
    // Логическое начало осей: в горизонтальном письме колонки (x) идут
    // справа налево при `rtl`; в вертикальном ряды (x) — справа налево при
    // `*-rl`, колонки (y) — снизу вверх при `sideways-lr`, и `rtl` это
    // переворачивает (эталоны `grid-gap-decorations-multi-value-writing-mode`).
    let rtl = merged.rtl == Some(true);
    let sideways_lr =
        vertical && merged.sideways == Some(true) && merged.vertical_rl != Some(true);
    let (rev_x, rev_y) = if vertical {
        (merged.vertical_rl == Some(true), sideways_lr != rtl)
    } else {
        (rtl, false)
    };
    // Поля контейнера: художник занимает его паддинг-бокс, а протяжённость
    // главных промежутков строк и лент считается от поля содержимого.
    let pad_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * size,
        _ => 0.0,
    };
    let pad = [
        pad_px(s.padding.top),
        pad_px(s.padding.right),
        pad_px(s.padding.bottom),
        pad_px(s.padding.left),
    ];
    let lines_extent = match merged.display {
        _ if lanes => 2,
        Some(Display::Grid) | Some(Display::InlineGrid) => 0,
        _ => 1,
    };
    Some(crate::interact::GapRuleSpec {
        pad,
        lines_extent,
        lanes_content_aligned: lanes
            && if crate::dom::lanes_row_dir(merged) {
                merged.justify_content.is_some()
            } else {
                merged.align_content.is_some()
            },
        rev_x,
        rev_y,
        col,
        row,
        kind,
        vertical,
        rtl: merged.rtl == Some(true),
        column_over_row: s.rule_column_over_row == Some(true),
        gap_x,
        gap_y,
        tracks_x,
        tracks_y,
    })
}

/// Линейки промежутков многоколоночника с рядами (css-gaps-1 §gap-multicol:
/// «A column gap is the gutter between adjacent column boxes … A row gap is
/// the gutter between the rows of column boxes established by
/// column-height»; «column gaps in multicol containers do not overlap row
/// gaps, similar to flex»). Геометрия — строки гибкого контейнера: колонки
/// ряда — элементы строки, ряды разделены сквозным `row-gap`; сами
/// прямоугольники кладёт `ColumnStack::prepaint`. `gap_rule_spec` гейтится
/// `display`, поэтому стиль линеек берётся через пробу с `display: flex`:
/// правила осей общие, а вид укладки и зазоры задаются здесь.
/// `column-rule-break: normal` у multicol = `intersection`, `row-rule-break:
/// normal` = `none` (§break) — колонки рвутся в зазоре ряда, ряды идут
/// сквозь (эталоны `multicol-gap-decorations-001/024`).
pub(crate) fn multicol_gap_rule_spec(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    column_gap: f32,
    row_gap: f32,
) -> Option<crate::interact::GapRuleSpec> {
    let mut probe = merged.clone();
    probe.display = Some(Display::Flex);
    let mut spec = gap_rule_spec(e, &probe, opts)?;
    let vertical = spec.vertical;
    spec.kind = crate::interact::GapLayout::Lines {
        stacked_vertically: !vertical,
    };
    if let Some(c) = spec.col.as_mut()
        && c.brk == 1
    {
        c.brk = 2;
    }
    if let Some(r) = spec.row.as_mut()
        && r.brk == 1
    {
        r.brk = 0;
    }
    // Многоколонник: протяжённость — по колонкам стопки, как прежде.
    spec.lines_extent = 0;
    spec.gap_x = Some(if vertical { row_gap } else { column_gap });
    spec.gap_y = Some(if vertical { column_gap } else { row_gap });
    Some(spec)
}
