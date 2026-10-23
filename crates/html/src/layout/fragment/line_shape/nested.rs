//! Вложенные ряды: коробка, форма и ширина вложенной стопки рядов.

use super::nested_kid::{oof_in_plain_kids, plain_block_kid};
use super::resolved_lengths::resolved_lengths;
use super::{group_inline_runs, inline_content};
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx, with_lines};
use crate::layout::multicol::spanner::{has_deep_spanner, multicol_container};
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::carries_abspos;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

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
        && !s.wm().vertical()
        && s.float.unwrap_or(0) == 0
        && !has_deep_spanner(c)
        && (!carries_abspos(c, 4) || oof_in_plain_kids(c))
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
        && (!oof_descendant(c) || oof_in_plain_kids(c))
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
    let vcx = opts.cx.vcx();
    let m = box_style(parent, &c.style, vcx);
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
        Some(Len::Pct(k)) => super::layout_unit(k * w), // как `column_plan`
        _ => match m.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        },
    };
    let col_w = ((w + gap) / n as f32 - gap).max(0.0);
    let mut mc = c.clone();
    mc.style.width = Some(Len::Px(w));
    let g = group_inline_runs(&mc).unwrap_or(mc);
    let kids: Vec<crate::layout::fragment::types::Kid> =
        with_lines(&m, Some(col_w), opts, |opts| {
            let vcx = opts.cx.vcx();
            g.children
                .iter()
                .filter(|n| !is_blank(n))
                .map(|n| match n {
                    // Только строчное содержимое (анонимные блоки строк): ряды по
                    // строкам у нас сходятся с Blink, а блочные дети с
                    // переполнением своей коробки и монолиты выше ряда ведут себя
                    // иначе (`multicol-fill-balance-003/030`, `multicol-nested-026/
                    // 031` с блочными детьми расходятся с эталоном).
                    Node::Element(k)
                        if !k.inline
                            && (inline_content(k) || plain_block_kid(k))
                            && !out_of_flow(&k.style)
                            && matches!(
                                k.style.position,
                                None | Some(crate::style::computed::Position::Relative)
                            )
                            && k.style.float.unwrap_or(0) == 0 =>
                    {
                        let k = resolved_lengths(k, &m, vcx);
                        let sh = shape_full(&k, 4, ShapeCx::COLUMNS, vcx)?;
                        if !inline_content(&k) && sh.solid.iter().any(|&(a, b)| b - a > hh + 0.01) {
                            return None;
                        }
                        Some(crate::layout::fragment::types::Kid {
                            h: sh.height,
                            mt: sh.margin_top,
                            mb: sh.margin_bottom,
                            monolith: solid_box(&k) && (!inline_content(&k) || sh.cuts.is_empty()),
                            cuts: sh.cuts,
                            force_before: edge_break(&k, false, vcx),
                            force_after: edge_break(&k, true, vcx),
                            avoid_before: edge_avoid(&k, false, vcx),
                            avoid_after: edge_avoid(&k, true, vcx),
                            forced: sh.forced,
                            solid: sh.solid,
                            avoid: Vec::new(),
                            span: false,
                            over: sh.height,
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
    Some(crate::layout::fragment::Shape {
        height: h,
        margin_top: 0.0,
        margin_bottom: px(&c.style.margin.bottom)?,
        cuts: Vec::new(),
        forced: Vec::new(),
        solid,
    })
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
