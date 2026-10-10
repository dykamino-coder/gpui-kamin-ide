//! Вложенные ряды: коробка, форма и ширина вложенной стопки рядов.

use super::{group_inline_runs, inline_content};
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx, with_lines};
use crate::layout::multicol::spanner::{has_deep_spanner, multicol_container};
use crate::layout::positioned::predicates::carries_abspos;
use crate::render::{RenderOpts, is_blank, out_of_flow};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Клон поддерева с длинами коробки в ТОЧКАХ: `em`/`ch`/`ex`/`rem` в
/// размерах, полях, отбивках, рамках и вставках разрешены по кеглю своего
/// элемента (тем же `inline::inherit` → `Computed::resolve_em`, что и при
/// отрисовке). `shape_full` читает голый стиль, и `margin-top: 2em` мерился
/// нулём (нестрогий `px_or`), а `height: 4em` — отказом всей стопки, хотя
/// рисунок кладёт их в точках. Наследуемые (`font-size`, `line-height`) не
/// трогаются: число в `line-height` наследуется множителем.
pub(crate) fn resolved_lengths(c: &Element, parent: &Computed) -> Element {
    let m = crate::style::cascade::inherit::inherit(parent, &c.style);
    let mut t = c.clone();
    // Подменяются ТОЛЬКО шрифтовые единицы, разрешённые в точки: прочее
    // (`None`, `auto`, доли, точки) остаётся как было — `inherit` дописывает
    // и умолчания, а на `min-height.is_some()` стоят гейты строк flex
    // (`multi-line-row-flex-fragmentation-018/037`).
    fn fix(own: &mut Option<Len>, res: Option<Len>) {
        if own.is_some_and(|l| !matches!(l, Len::Px(_) | Len::Auto | Len::Pct(_)))
            && matches!(res, Some(Len::Px(_)))
        {
            *own = res;
        }
    }
    fn fix_sides(own: &mut crate::style::computed::Sides, res: &crate::style::computed::Sides) {
        fix(&mut own.top, res.top);
        fix(&mut own.right, res.right);
        fix(&mut own.bottom, res.bottom);
        fix(&mut own.left, res.left);
    }
    fix(&mut t.style.width, m.width);
    fix(&mut t.style.height, m.height);
    fix(&mut t.style.min_width, m.min_width);
    fix(&mut t.style.min_height, m.min_height);
    fix(&mut t.style.max_width, m.max_width);
    fix(&mut t.style.max_height, m.max_height);
    fix_sides(&mut t.style.margin, &m.margin);
    fix_sides(&mut t.style.padding, &m.padding);
    fix_sides(&mut t.style.border_width, &m.border_width);
    fix_sides(&mut t.style.inset, &m.inset);
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => Node::Element(resolved_lengths(k, &m)),
            other => other.clone(),
        })
        .collect();
    t
}

/// Вложенный многоколоночник, который внешняя стопка ведёт РЯДАМИ (`nest_row`):
/// обычный блок с колонками, высотой в точках, без своих рядов, спаннеров,
/// внепоточных и вертикального письма.
pub(crate) fn nested_rows_box(c: &Element) -> bool {
    let s = &c.style;
    multicol_container(s)
        && (s.column_count.is_some_and(|n| n > 1) || s.column_width.is_some())
        && s.column_height.is_none()
        && s.column_wrap.is_none()
        && (matches!(s.height, Some(Len::Px(h)) if h > 0.0)
            || (matches!(s.height, None | Some(Len::Auto))
                && s.min_height.is_none()
                && s.max_height.is_none()))
        && matches!(s.display, None | Some(Display::Block))
        && s.vertical != Some(true)
        && s.float.unwrap_or(0) == 0
        && !has_deep_spanner(c)
        && !carries_abspos(c, 4)
        // Ряды считаются от верха СОДЕРЖИМОГО: блочные рамка и отбивка (и их
        // повтор у `box-decoration-break: clone`) сдвинули бы границы рядов с
        // границ внешних колонок (`box-decoration-break-clone-010`).
        && !s.bdb_clone
        && {
            // Нижние рамка и отбивка у `height: auto` допустимы: они встают
            // после последнего ряда (`multicol-breaking-006`), а у заданной
            // высоты сдвинули бы последний ряд.
            let b = s.borders();
            let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
            let auto = matches!(s.height, None | Some(Len::Auto));
            zero(&s.padding.top)
                && zero(&b.top)
                && (auto || (zero(&s.padding.bottom) && zero(&b.bottom)))
        }
        && c.children.iter().any(|n| !is_blank(n))
        // Внепоточный потомок — содержащий блок и дотяг рядами не выражены
        // (`out-of-flow-in-multicolumn-019`).
        && !oof_descendant(c)
}

pub(super) fn oof_descendant(e: &Element) -> bool {
    e.children
        .iter()
        .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style) || oof_descendant(k)))
}

/// Мера вложенного многоколоночника с `height: auto`, который внешняя стопка
/// ведёт рядами во внешний фрагментаинер `hh` (`nest_row`): блочный размер —
/// полные ряды по `hh` и сбалансированный последний (css-multicol-1 §7.1
/// «only the last fragment is balanced»; Blink `column_layout_algorithm.cc`
/// `LayoutRow` с `ConstrainColumnBlockSize`), плюс нижние рамка и отбивка.
/// Та же укладка (`ColumnStack::measure_rows`) и те же меры детей
/// (`resolved_lengths` + `with_lines`), что у копии через `element()`.
/// Точек разреза нет: внешняя стопка режет коробку краем колонки — по рядам.
pub(crate) fn nested_rows_shape(
    c: &Element,
    parent: &Computed,
    hh: f32,
    cw: f32,
    opts: &RenderOpts,
) -> Option<Shape> {
    let m = inherit(parent, &c.style);
    let w = nested_box_w(c, cw)?;
    let n = match m.column_count {
        Some(n) if n > 1 => n as usize,
        _ => return None,
    };
    if m.column_width.is_some() {
        return None;
    }
    let gap = match m.column_gap {
        Some(Len::Px(v)) => v,
        _ => match m.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        },
    };
    let col_w = ((w + gap) / n as f32 - gap).max(0.0);
    let mut mc = c.clone();
    mc.style.width = Some(Len::Px(w));
    let g = group_inline_runs(&mc).unwrap_or(mc);
    let kids: Vec<crate::layout::fragment::types::Kid> = with_lines(&m, Some(col_w), opts, || {
        g.children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                // Только строчное содержимое (анонимные блоки строк): ряды по
                // строкам у нас сходятся с Blink, а блочные дети с
                // переполнением своей коробки и монолиты выше ряда ведут себя
                // иначе (`multicol-fill-balance-003/030`, `multicol-nested-026/
                // 031` при блочных детях уходили 0.00 → «красное видно»).
                Node::Element(k)
                    if !k.inline
                        && inline_content(k)
                        && !out_of_flow(&k.style)
                        && matches!(
                            k.style.position,
                            None | Some(crate::style::computed::Position::Relative)
                        )
                        && k.style.float.unwrap_or(0) == 0 =>
                {
                    let k = resolved_lengths(k, &m);
                    let sh = shape_full(&k, 4, ShapeCx::COLUMNS)?;
                    Some(crate::layout::fragment::types::Kid {
                        h: sh.0,
                        mt: sh.1,
                        mb: sh.2,
                        monolith: solid_box(&k) && !(inline_content(&k) && !sh.3.is_empty()),
                        cuts: sh.3,
                        force_before: edge_break(&k, false),
                        force_after: edge_break(&k, true),
                        avoid_before: edge_avoid(&k, false),
                        avoid_after: edge_avoid(&k, true),
                        forced: sh.4,
                        solid: sh.5,
                        span: false,
                        over: sh.0,
                        clone_dec: None,
                        overflow_top: false,
                        repeat: Default::default(),
                        par: Default::default(),
                    })
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    })?;
    if kids.is_empty() {
        return None;
    }
    let fixed = (m.column_fill_auto == Some(true)).then_some(hh);
    let rows = crate::layout::fragment::types::Rows {
        h: Some(hh),
        gap: 0.0,
        wrap: true,
        cap: false,
    };
    let content = crate::layout::multicol::column_stack::ColumnStack::measure_rows(
        &kids, n, gap, fixed, rows,
    );
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = c.style.borders();
    let bot = px(&c.style.padding.bottom)? + px(&b.bottom)?;
    let h = content + bot;
    let solid = if bot > 0.0 {
        vec![(content, h)]
    } else {
        Vec::new()
    };
    Some((
        h,
        0.0,
        px(&c.style.margin.bottom)?,
        Vec::new(),
        Vec::new(),
        solid,
    ))
}

/// Ширина коробки (`width` по её `box-sizing`) ребёнка в колонке `cw`.
pub(crate) fn nested_box_w(c: &Element, cw: f32) -> Option<f32> {
    let s = &c.style;
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = s.borders();
    let outer = cw - px(&s.margin.left)? - px(&s.margin.right)?;
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    Some(
        if s.border_box == Some(true) {
            outer
        } else {
            outer - edges
        }
        .max(0.0),
    )
}
