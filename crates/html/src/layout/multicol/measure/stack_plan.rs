//! Мера стопки до сборки: размер колонки, анонимные блоки, флоаты и clone.
//! Прямые внепоточные дети сохраняют статическое место в потоке.

use super::{column_layout_width, column_measure_width, measure_children};
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::{Shape, with_lines};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed, Position};
use crate::style::values::value::Len;

/// Итог меры стопки: всё, что сборка передаёт `multicol_column_stack`.
pub(crate) struct StackPlan {
    pub(crate) line_col_w: Option<f32>,
    /// Ширина колонки для раскладки копии таблицы-ребёнка до укладки
    /// (`column_layout_width`).
    pub(crate) layout_col_w: Option<f32>,
    pub(crate) direct_oof: Vec<Element>,
    pub(crate) oof_static: Vec<(usize, Element)>,
    pub(crate) nested_auto: std::cell::RefCell<Vec<u64>>,
    pub(crate) nested_whole: std::cell::RefCell<Vec<u64>>,
    pub(crate) measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
    pub(crate) stackable: Option<Vec<(Element, Shape)>>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_plan(
    e: &Element,
    merged: &BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    cols: u16,
    used_gap: f32,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    box_h: Option<f32>,
    col_inline_size: Option<Len>,
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
) -> StackPlan {
    let positioned =
        |s: &Computed| matches!(s.position, Some(Position::Absolute | Position::Fixed));
    let line_col_w = column_measure_width(
        e,
        inherited,
        merged,
        col_inline_size,
        col_vert,
        cols,
        used_gap,
    );
    let prepared = super::prepared(e, merged, opts, col_vert, line_col_w);
    let ge = &prepared;
    // Прямые флоаты legacy-пути — не в стопку,
    // рисуются её соседями.
    let mut direct_oof: Vec<Element> = ge
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(c) if out_of_flow(&c.style.used()) && !positioned(&c.style.used()) => {
                Some(c.clone())
            }
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
            if positioned(&c.style.used()) {
                out.push((at, c.clone()));
            } else if !out_of_flow(&c.style.used()) {
                at += 1;
            }
        }
        out
    };
    // Высота внешнего фрагментаинера для вложенного рядами
    // (`nested_rows_shape`): `column-fill: auto` и блочный размер
    // в точках, без своих рядов.
    let outer_frag = (e.style.used().column_fill_auto == Some(true) && rows.is_none() && !col_vert)
        .then(|| col_h.or(box_h))
        .flatten();
    let layout_col_w = column_layout_width(
        e,
        inherited,
        merged,
        col_inline_size,
        col_vert,
        cols,
        used_gap,
    );
    let first_flow = ge
        .children
        .iter()
        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style.used())))
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
    let whole_ok = e.style.used().column_fill_auto != Some(true)
        && rows.is_none_or(|r| r.cap)
        && nest_rows.is_none()
        && !col_vert;
    // Дети, чью высоту меряет раскладка копии (`StackChild::measure`).
    let measured_kids: std::cell::RefCell<Vec<(u64, f32)>> = Default::default();
    let measure_ok = e.style.used().column_fill_auto == Some(true)
        && rows.is_none()
        && nest_rows.is_none()
        && !col_vert
        && (col_h.is_some() || matches!(e.style.used().height, Some(Len::Px(_))));
    // Layout-driven pagination (`fragment::paginate`): one line of columns,
    // no spanners, horizontal writing.
    let pag_ok = !opts.frag.in_stack_copy
        && rows.is_none_or(|r| r.cap)
        && nest_rows.is_none()
        && !col_vert
        && !ge
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(c) if c.style.used().column_span == Some(true)));
    let last_flow = ge
        .children
        .iter()
        .rev()
        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style.used())))
        .map(|n| n as *const Node);
    let laid = line_col_w
        .filter(|_| !col_vert && nest_rows.is_none())
        .and_then(|w| {
            super::laid::children(ge, merged, opts.cx.vcx(), w, &measured_kids).or_else(|| {
                pag_ok
                    .then(|| {
                        super::native_float::children(ge, merged, opts.cx.vcx(), w, &measured_kids)
                    })
                    .flatten()
            })
        });
    let pending = (line_col_w.is_none() && nest_rows.is_none())
        .then(|| {
            if col_vert {
                super::laid::pending_vertical(ge, merged, opts.cx.vcx(), col_rl)
            } else {
                super::laid::pending(ge, merged, opts.cx.vcx()).or_else(|| {
                    pag_ok
                        .then(|| super::native_float::pending(ge, merged, opts.cx.vcx()))
                        .flatten()
                })
            }
        })
        .flatten();
    let vertical = line_col_w
        .filter(|_| col_vert && nest_rows.is_none())
        .and_then(|w| {
            super::laid::vertical_children(ge, merged, opts.cx.vcx(), w, col_rl, &measured_kids)
        });
    // Native-копии copy_flow уже содержат прямые флоаты в анонимном блоке.
    // Повторный сосед стопки увеличил бы её высоту второй margin-box.
    if laid.is_some() || pending.is_some() || vertical.is_some() {
        direct_oof.clear();
    }
    let stackable: Option<Vec<(Element, Shape)>> = laid.or(pending).or(vertical).or_else(|| {
        with_lines(merged, line_col_w, opts, |opts| {
            measure_children(
                ge,
                merged,
                opts,
                first_flow,
                last_flow,
                outer_frag,
                (line_col_w, layout_col_w),
                &nested_auto,
                whole_ok,
                &nested_whole,
                &measured_kids,
                col_vert,
                col_rl,
                measure_ok,
                pag_ok,
            )
        })
    });
    StackPlan {
        line_col_w,
        layout_col_w,
        direct_oof,
        oof_static,
        nested_auto,
        nested_whole,
        measured_kids,
        stackable,
    }
}
