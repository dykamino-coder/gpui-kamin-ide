//! Флекс-строки колонки (flex_lines_of) и правила промежутков.

use super::common::{gaps, plain_container, plain_item, zero};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Строки контейнера для `split_flex_lines`: `(сдвиг строки по x, элементы с
/// мерой)`. `None` — контейнер вне гейта.
#[allow(clippy::type_complexity)]
pub(super) fn flex_lines_of(
    c: &Element,
    col_w: Option<f32>,
    vcx: ValueCx<'_>,
) -> Option<Vec<(f32, Vec<(Element, Shape)>)>> {
    use crate::style::computed::FlexDir;
    if flex_gap_rules(c.style.cascaded()) {
        return None;
    }
    let s = c.style.cascaded();
    if !plain_container(c) || s.flex_dir != Some(FlexDir::Col) {
        return None;
    }
    let Some(Len::Px(main)) = s.height else {
        return None;
    };
    let cross = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = gaps(s)?;
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = k.style.cascaded();
        if !plain_item(k) || ks.flex_basis.is_some() {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    // Визуальный порядок (`order`, стабильно), как в `blocks()`.
    items.sort_by_key(|k| k.style.cascaded().order.unwrap_or(0));
    // Мера и внешний поперечный размер элемента; `None` у ширины — `auto`.
    let mut measured: Vec<(Element, Shape, Option<f32>)> = Vec::with_capacity(items.len());
    for k in items {
        let sh = shape_full(k, 4, ShapeCx::COLUMNS, vcx)?;
        let kb = k.style.cascaded().borders();
        let side = |l: &Option<Len>| match l {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(*v),
            _ => None,
        };
        let w = match k.style.cascaded().width {
            Some(Len::Px(w)) => {
                let extra = if k.style.cascaded().border_box == Some(true) {
                    0.0
                } else {
                    side(&k.style.cascaded().padding.left)?
                        + side(&k.style.cascaded().padding.right)?
                        + side(&kb.left)?
                        + side(&kb.right)?
                };
                Some(w + extra)
            }
            None | Some(Len::Auto)
                if zero(&k.style.cascaded().padding.left)
                    && zero(&k.style.cascaded().padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right)
                    && k.children.iter().all(is_blank) =>
            {
                None
            }
            _ => return None,
        };
        measured.push((k.clone(), sh, w));
    }
    // Строки: §9.3 шаг 5.
    let mut lines: Vec<Vec<(Element, Shape, Option<f32>)>> = Vec::new();
    let mut used = 0.0f32;
    for (k, sh, w) in measured {
        let outer = sh.height + sh.margin_top + sh.margin_bottom;
        match lines.last_mut() {
            Some(line) if used + row_gap + outer <= main + 0.01 => {
                used += row_gap + outer;
                line.push((k, sh, w));
            }
            _ => {
                used = outer;
                lines.push(vec![(k, sh, w)]);
            }
        }
    }
    // Одна строка — однострочный по сути контейнер: прежний путь его знает.
    if lines.len() < 2 {
        return None;
    }
    let n = lines.len() as f32;
    let crosses: Vec<f32> = lines
        .iter()
        .map(|l| l.iter().filter_map(|x| x.2).fold(0.0f32, f32::max))
        .collect();
    let free = cross - crosses.iter().sum::<f32>() - col_gap * (n - 1.0);
    let extra = if free > 0.0 { free / n } else { 0.0 };
    let mut out = Vec::with_capacity(lines.len());
    let mut dx = 0.0f32;
    for (line, lc) in lines.into_iter().zip(crosses) {
        let lc = lc + extra;
        let mut items = Vec::with_capacity(line.len());
        for (i, (mut k, mut sh, w)) in line.into_iter().enumerate() {
            // `auto` тянется на строку (§9.4 шаг 11, `align-self: stretch`).
            if w.is_none() {
                k.style.cascaded_mut().width = Some(Len::Px(lc));
            }
            // Зазор между элементами строки — к полю следующего: на разрыве
            // он пропадает вместе с полем (Blink
            // `UpdateOffsetAdjustmentForSuppressedRowGap`, :2486-2500).
            if i > 0 {
                sh.margin_top += row_gap;
            }
            items.push((k, sh));
        }
        out.push((dx, items));
        dx += lc + col_gap;
    }
    Some(out)
}

/// Линейки промежутков у flex-контейнера (css-gaps-1 `column-rule`/`row-rule`).
/// Строки, раскрытые в параллельные потоки (`split_flex_lines`), — отдельные
/// копии без контейнера, и художник линеек (`GapRulePainter`) их не видит:
/// линейки пропадали целиком (`flex-gap-decorations-fragmentation-025/028/029/
/// 030`). Такой контейнер идёт прежним путём — одной копией со своими линейками.
pub(crate) fn flex_gap_rules(s: &Computed) -> bool {
    s.column_rule_visible == Some(true)
        || s.row_rule_visible == Some(true)
        || s.column_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
        || s.row_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
}
