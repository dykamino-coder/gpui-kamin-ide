//! Подготовка и выбор стопки колонок для блочных детей.

mod floats;
pub(super) use floats::block_floats;

mod width;
pub(super) use width::column_measure_width;

mod measurement;
pub(super) use measurement::measure_children;

use crate::dom::{Element, Node};
use crate::layout::fragment::clone::clone_wrapper_item;
use crate::layout::fragment::line_shape::group_inline_runs;
use crate::layout::fragment::{Shape, with_lines};
use crate::layout::multicol::container::multicol_column_stack;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_box(
    mut d: gpui::Div,
    e: &Element,
    merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    cols: u16,
    column_width: Option<Len>,
    used_gap: f32,
    row_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    box_h: Option<f32>,
    col_inline_size: Option<Len>,
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
) -> Result<(gpui::Div, Computed), AnyElement> {
    let positioned = |s: &Computed| {
        matches!(
            s.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    };
    let line_col_w = column_measure_width(
        e,
        inherited,
        &merged,
        col_inline_size,
        col_vert,
        cols,
        used_gap,
    );
    // Строчные прогоны среди блоков — анонимными блоками (CSS 2.1
    // §9.2.1.1), только когда строки можно измерить.
    let grouped_e = line_col_w.and_then(|_| group_inline_runs(e));
    let ge: &Element = grouped_e.as_ref().unwrap_or(e);
    let floats_blocked = block_floats(ge, positioned, line_col_w);
    let ge: &Element = floats_blocked.as_ref().unwrap_or(ge);
    // Обёртка flex/сетки с ЕДИНСТВЕННЫМ элементом-`clone`
    // (`clone_wrapper_item`): по блочной оси такая обёртка
    // раскладывается ровно как блок с этим ребёнком, и фрагменты
    // клонированного украшения строятся у самого элемента.
    let unwrapped = ge
        .children
        .iter()
        .any(|n| matches!(n, Node::Element(c) if clone_wrapper_item(c).is_some()))
        .then(|| {
            let mut g = ge.clone();
            for n in g.children.iter_mut() {
                if let Node::Element(c) = n
                    && let Some(item) = clone_wrapper_item(c)
                {
                    *c = item;
                }
            }
            g
        });
    let ge: &Element = unwrapped.as_ref().unwrap_or(ge);
    // Плавающие прямые дети — как прежде: не в стопку,
    // рисуются её соседями.
    let direct_oof: Vec<Element> = ge
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(c) if out_of_flow(&c.style) && !positioned(&c.style) => Some(c.clone()),
            _ => None,
        })
        .collect();
    // Позиционированные прямые дети несут МЕСТО В ПОТОКЕ —
    // номер среди детей, ушедших в стопку: точка статической
    // позиции лежит В КОЛОНКЕ, а не под стопкой, и взять её
    // больше неоткуда — раскладка под нами про колонки не
    // знает. Счёт идёт по тем же детям, что отбирает
    // `stackable` ниже: непустые и не внепоточные.
    let oof_static: Vec<(usize, Element)> = {
        let mut at = 0usize;
        let mut out: Vec<(usize, Element)> = Vec::new();
        for n in ge.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(c) = n else {
                at += 1;
                continue;
            };
            if positioned(&c.style) {
                out.push((at, c.clone()));
            } else if !out_of_flow(&c.style) {
                at += 1;
            }
        }
        out
    };
    // Высота внешнего фрагментаинера для вложенного рядами
    // (`nested_rows_shape`): `column-fill: auto` и блочный размер
    // в точках, без своих рядов.
    let outer_frag = (e.style.column_fill_auto == Some(true) && rows.is_none() && !col_vert)
        .then(|| col_h.or(box_h))
        .flatten();
    let first_flow = ge
        .children
        .iter()
        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
        .map(|n| n as *const Node);
    // Чью меру дала `nested_rows_shape` — только их копии рядами.
    let nested_auto: std::cell::RefCell<Vec<u64>> = Default::default();
    // Вложенный многоколоночник `height: auto` под БАЛАНСОМ внешнего
    // (`nested_whole`): одна сбалансированная строка колонок целиком.
    // css-multicol-1 §2: вложенный многоколоночник — сам мультиколонный
    // контейнер, его колонки и линейки рисуются внутри внешней колонки
    // (Blink `ColumnLayoutAlgorithm` для внутреннего — та же раскладка).
    // Копия стопки шла узкой веткой `styled_div_with` и рисовала его
    // плоско — без колонок и линеек (`multicol-rule-color-inherit-001`).
    let nested_whole: std::cell::RefCell<Vec<u64>> = Default::default();
    let whole_ok = e.style.column_fill_auto != Some(true)
        && rows.is_none_or(|r| r.cap)
        && nest_rows.is_none()
        && !col_vert;
    // Дети, чью высоту меряет раскладка копии (`StackChild::measure`).
    let measured_kids: std::cell::RefCell<Vec<(u64, f32)>> = Default::default();
    let measure_ok = e.style.column_fill_auto == Some(true)
        && rows.is_none()
        && nest_rows.is_none()
        && !col_vert
        && (col_h.is_some() || matches!(e.style.height, Some(Len::Px(_))));
    let last_flow = ge
        .children
        .iter()
        .rev()
        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
        .map(|n| n as *const Node);
    let stackable: Option<Vec<(Element, Shape)>> = with_lines(&merged, line_col_w, opts, || {
        measure_children(
            ge,
            &merged,
            opts,
            first_flow,
            last_flow,
            outer_frag,
            line_col_w,
            &nested_auto,
            whole_ok,
            &nested_whole,
            &measured_kids,
            col_vert,
            col_rl,
            measure_ok,
        )
    });
    if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
        return Err(multicol_column_stack(
            d,
            e,
            inherited,
            merged,
            opts,
            kids,
            cols,
            column_width,
            used_gap,
            row_gap,
            col_axis,
            col_vert,
            col_rl,
            col_h,
            line_col_w,
            rows,
            nest_rows,
            nest_phase,
            direct_oof,
            oof_static,
            nested_auto,
            nested_whole,
            measured_kids,
        ));
    }
    let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
    let rows = count.div_ceil(cols as usize).max(1) as u16;
    let gap = used_gap;
    d = d
        .grid()
        .grid_template_cols((0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect())
        .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
        .gap_x(px(gap));
    d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
    Ok((d, merged))
}
