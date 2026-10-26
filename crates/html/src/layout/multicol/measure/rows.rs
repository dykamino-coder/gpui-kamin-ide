//! Размеры рядов и ограничения вложенного многоколоночного потока.

use crate::dom::Element;
use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(crate) fn row_plan(
    e: &Element,
    _merged: &Computed,
    opts: &RenderOpts,
    column_width: Option<Len>,
    cols: u16,
    col_vert: bool,
    outer_space: Option<Rc<Nested>>,
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
    let col_h = match e.style.used().column_height {
        Some(Len::Px(h)) if h >= 0.0 => Some(h),
        _ => None,
    };
    // Блочный размер коробки: в вертикальном письме — ширина.
    let box_h = match if col_vert {
        e.style.used().width
    } else {
        e.style.used().height
    } {
        Some(Len::Px(h)) if h > 0.0 => Some(h),
        _ => None,
    };
    let wrap = e.style.used().column_wrap.unwrap_or(col_h.is_some());
    let em = match e.style.used().font_size {
        Some(Len::Px(size)) => size,
        _ => opts.base_size(),
    };
    let row_gap = match e.style.used().gap.and_then(|g| g.0) {
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
    // точках. Внешний потолок во ВЛОЖЕННЫЕ многоколоночники без
    // своей высоты не спускать — это ломает их баланс; здесь потолок
    // не передаётся вниз (`in_stack`: внутри копии другой стопки —
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
        (e.style.used().max_width, e.style.used().min_width)
    } else {
        (e.style.used().max_height, e.style.used().min_height)
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
    // Вложенный многоколоночник во внешней колонке (`FragCx::outer_space`):
    // ряды высотой во внешний фрагментаинер без зазора — граница
    // ряда совпадает с границей внешней колонки (css-break-4 §2.1;
    // Blink `ConstrainColumnBlockSize`, `LayoutLine` :1009-1026
    // «wrap … if we're participating in an outer fragmentation
    // context»). Последний ряд балансируется (css-multicol-1 §7.1
    // «only the last fragment is balanced»).
    let outer_space = outer_space
        .filter(|_| col_h.is_none() && e.style.used().column_wrap.is_none() && !col_vert);
    let nest_phase = outer_space.as_ref().map_or(0., |r| r.offset);
    let nest_rows = outer_space.as_ref().map(|r| {
        r.outer_at(0)
            .expect("validated outer Space")
            .block_size
            .unwrap()
    });
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
            && e.style.used().column_fill_auto != Some(true)
            && !opts.frag.in_stack_copy)
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
        outer_space,
    }
}

pub(crate) struct RowPlan {
    pub(crate) col_w_px: Option<f32>,
    pub(crate) want: Option<usize>,
    pub(crate) col_h: Option<f32>,
    pub(crate) box_h: Option<f32>,
    pub(crate) row_gap: f32,
    pub(crate) rows: Option<crate::layout::fragment::types::Rows>,
    pub(crate) outer_space: Option<Rc<Nested>>,
    pub(crate) nest_phase: f32,
    pub(crate) nest_rows: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::row_plan;
    use crate::dom::{Element, Node};
    use crate::layout::fragment::token::{Space, space::sequence::Sequence};
    use crate::layout::options::RenderOpts;
    use crate::style::computed::Computed;
    use crate::style::values::value::Len;

    fn host(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("host")
    }

    #[test]
    fn addressed_budget_keeps_first_remainder_and_later_constraints() {
        let tree = crate::dom::parse("<div style='height:500px; min-height:600px'></div>", "");
        let e = host(&tree);
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let sequence = Sequence::new(|index| {
            Ok(Space {
                block_size: Some(if index == 0 { 100. } else { 40. }),
                ..Space::pages()
            })
        })
        .nested(0, 75.)
        .unwrap();
        let plan = row_plan(
            e,
            &Computed::default(),
            &opts,
            None,
            2,
            false,
            Some(sequence.clone()),
        );
        let outer = plan.outer_space.unwrap();
        assert_eq!(outer.at(0).unwrap().block_size, Some(25.));
        assert_eq!(outer.at(1).unwrap().block_size, Some(40.));
        assert_eq!((plan.nest_rows, plan.nest_phase), (Some(100.), 75.));
        assert_eq!(e.style.used().min_height, Some(Len::Px(600.)));
        for property in ["column-height:30px", "column-wrap:nowrap"] {
            let tree = crate::dom::parse(&format!("<div style='{property}'></div>"), "");
            let plan = row_plan(
                host(&tree),
                &Computed::default(),
                &opts,
                None,
                2,
                false,
                Some(sequence.clone()),
            );
            assert!(plan.outer_space.is_none());
        }
    }
}
