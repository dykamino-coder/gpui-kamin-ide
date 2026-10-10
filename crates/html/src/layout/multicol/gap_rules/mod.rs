//! Правила промежутков колонок (`column-rule`, `*-rule`).
// owner: A

use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod layout_spec;
use layout_spec::gap_rule_layout;

/// Пробельный текстовый узел: в подсчёте детей он не участвует.
/// Правила линеек промежутков (css-gaps-1) контейнера — `None`, когда ни
/// одна линейка не задана. Длины (`em`) сводятся в точки здесь: слой знает
/// только геометрию. Цвет по умолчанию — `currentcolor`, ширина — `medium`.
pub(crate) fn gap_rule_spec(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
) -> Option<crate::paint::gap_rules::GapRuleSpec> {
    use crate::paint::gap_rules::GapAxisRule;
    use crate::style::computed::{GapInset, GapList};
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
    let fallback = merged.color.unwrap_or(crate::style::values::value::Color {
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
            double: if column {
                s.column_rule_double
            } else {
                s.row_rule_double
            },
        })
    };
    let col = axis(true);
    let row = axis(false);
    if col.is_none() && row.is_none() {
        return None;
    }
    gap_rule_layout(merged, size, s, col, row)
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
pub(super) fn multicol_gap_rule_spec(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    column_gap: f32,
    row_gap: f32,
) -> Option<crate::paint::gap_rules::GapRuleSpec> {
    let mut probe = merged.clone();
    probe.display = Some(Display::Flex);
    let mut spec = gap_rule_spec(e, &probe, opts)?;
    let vertical = spec.vertical;
    spec.kind = crate::paint::gap_rules::GapLayout::Lines {
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
