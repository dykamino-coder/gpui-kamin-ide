//! Сегменты многоколоночника между охватывающими элементами (column-span: all).

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::multicol::column_flow::column_flow;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

mod row;

#[allow(clippy::too_many_arguments)]
pub(crate) fn multicol_spanner_segments(
    mut d: gpui::Div,
    e: &Element,
    merged: BoxStyle,
    opts: &RenderOpts,
    col_w_px: Option<f32>,
    want: Option<usize>,
    lone_span: bool,
    rest_h: Option<f32>,
    mut span_prev: Option<Option<u64>>,
    mut seg_open: bool,
    is_span: impl Fn(&Node) -> bool,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    // `<fieldset>`: отрисованная легенда стоит ВНЕ колонок —
    // многоколоночность получает анонимная коробка содержимого
    // fieldset (HTML §15.3.13 «The fieldset and legend
    // elements», пересказ; Blink `LayoutFieldset` +
    // `FieldsetContentBox`). Прежде легенда падала в первый ряд и
    // занимала колонку (`multicol-span-all-fieldset-001…003`:
    // эталон — `fieldset > legend + div.inner` с колонками).
    // Отрисованная — первая `legend` в потоке.
    let (legend, kids) = legend_and_kids(e);
    if let Some(legend) = legend {
        let band = crate::layout::block::legend::legend_band(legend, &merged);
        d = d.children(opts.cx.contracts.build.blocks(&[band], &merged, opts));
    }
    // Свой исходный OOF-источник у каждого ряда (`row_sources`).
    let row_geometry = opts
        .cx
        .position
        .row_geometry
        .clone()
        .filter(|(host, _)| *host == merged.self_node);
    let mut row = 0usize;
    for chunk in kids.split_inclusive(&is_span) {
        let (body, span) = match chunk.split_last() {
            Some((last, head)) if is_span(last) => (head, Some(last)),
            _ => (chunk, None),
        };
        if body.iter().any(|n| !is_blank(n)) {
            // Ряд колонок между спаннерами — новый контекст
            // форматирования: поля спаннеров сквозь него не
            // схлопываются (§column-span: «margins on elements
            // inside a column box will not collapse with the margin
            // of a spanner»). Сам ряд для taffy — лист или
            // не-блок, насквозь его поле не проходит.
            span_prev = None;
            seg_open = true;
            // Ряд получает собственный остаток, декорации остаются на хосте.
            let sub = row::segment(e, body, if span.is_none() { rest_h } else { None });
            // Одна колонка со спаннером идёт обычным блоком: column_flow
            // спускается в единственного ребёнка и теряет коробку фрагмента
            // (multicol-span-all-children-height-005/008).
            let row_style = box_style(&merged, &sub.style, vcx);
            let mut row_opts = opts.clone();
            row_opts.cx.position.row_geometry = None;
            if let Some((_, rows)) = &row_geometry {
                row_opts.cx.position.geometry = rows.get(row).cloned().flatten();
            }
            row += 1;
            let opts = &row_opts;
            let flow = (!lone_span)
                .then(|| column_flow(&sub, &row_style, opts, want, col_w_px, None))
                .flatten();
            if let Some(el) = flow {
                d = d.child(el);
            } else {
                d = d.child(div().children(opts.cx.contracts.build.blocks(
                    &[Node::Element(sub)],
                    &merged,
                    opts,
                )));
            }
        }
        if let Some(Node::Element(sp)) = span {
            // Хост сегментов — блок taffy (`Display::Block`,
            // `vendor/gpui/src/style.rs:862`), и поля соседних детей
            // он схлопывает сам (`vendor/taffy/src/compute/
            // block.rs:186-210`): соседние спаннеры ОДНОГО предка —
            // верно («the margins of two adjacent spanners will
            // collapse with each other»; `multicol-span-all-margin-
            // 003`). Запрещённое схлопывание гасит нулевая гибкая
            // сторожка — её taffy насквозь не проходит
            // (`has_styles_preventing_being_collapsed_through`:
            // `!style.is_block()`): (1) перед ПЕРВЫМ куском-спаннером
            // — многоколоночник сам контекст форматирования, и
            // верхнее поле спаннера сквозь его верх не уходит;
            // (2) между спаннерами РАЗНЫХ исходных предков (метка
            // `Marks::span_parent`, `spanner_parts`).
            let key = sp.marks.span_parent;
            if !seg_open || span_prev.is_some_and(|k| k != key) {
                d = d.child(div().flex());
            }
            seg_open = true;
            span_prev = Some(key);
            let inner = box_style(&merged, &sp.style, vcx);
            // Спаннер — независимый контекст форматирования
            // (§column-span). Голый `styled_div_with` — блок taffy,
            // и тот схлопывал поле первого/последнего ребёнка
            // сквозь край спаннера (`vendor/taffy/src/compute/
            // block.rs:186`): в `multicol-span-all-margin-nested-
            // firstchild-001` `<span style="margin: 2em 0">` уводил
            // `<h6>` вниз, и чёрного фона не было видно вовсе.
            // Оболочка — гибкая колонка, ровно как у блока общего
            // пути (`d.flex().flex_col()` при пустом `display`), где
            // поля детей сводит сам `blocks()` — а он с предикатом
            // `own_context_style` поле наружу больше не отдаёт.
            let shell = opts.cx.contracts.build.styled_box(sp, &inner, &opts.cx);
            let shell = if matches!(inner.reset_box_layout.display, None | Some(Display::Block))
                && !inner.wm().vertical()
            {
                shell.flex().flex_col()
            } else {
                shell
            };
            d = d.child(shell.children(opts.cx.contracts.build.blocks(&sp.children, &inner, opts)));
        }
    }
    // Нижняя сторожка: нижнее поле последнего спаннера остаётся
    // внутри многоколоночника — он независимый контекст
    // форматирования (CSS 2.1 §8.3.1).
    d = d.child(div().flex());
    return d.into_any_element();
}

/// Отрисованная легенда `<fieldset>` (первая `legend` в потоке; HTML §15.3.13)
/// и остальные дети хоста — содержимое рядов и спаннеры.
pub(crate) fn legend_and_kids(e: &Element) -> (Option<Node>, Vec<Node>) {
    let mut kids = e.children.to_vec();
    let legend = (e.tag == "fieldset")
        .then(|| {
            kids.iter().position(
                |n| matches!(n, Node::Element(c) if c.tag == "legend" && !out_of_flow(&c.style.used())),
            )
        })
        .flatten()
        .map(|i| kids.remove(i));
    (legend, kids)
}

/// Непустые ряды колонок между спаннерами в порядке рядов
/// `multicol_spanner_segments`.
pub(crate) fn row_bodies(e: &Element, is_span: impl Fn(&Node) -> bool) -> Vec<Vec<Node>> {
    let (_, kids) = legend_and_kids(e);
    kids.split_inclusive(&is_span)
        .map(|chunk| match chunk.split_last() {
            Some((last, head)) if is_span(last) => head,
            _ => chunk,
        })
        .filter(|body| body.iter().any(|n| !is_blank(n)))
        .map(|body| body.to_vec())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::row_bodies;
    use crate::dom::{Element, Node};
    use crate::style::computed::Computed;

    fn el(tag: &str) -> Node {
        Node::Element(Element::synthetic(
            tag,
            Computed::default(),
            Default::default(),
            vec![],
        ))
    }

    #[test]
    fn rows_skip_the_rendered_legend_and_split_at_spanners() {
        let host = Element::synthetic(
            "fieldset",
            Computed::default(),
            Default::default(),
            vec![
                el("legend"),
                el("div"),
                el("h3"),
                el("p"),
                el("p"),
                el("h3"),
            ],
        );
        let rows = row_bodies(&host, |n| matches!(n, Node::Element(e) if e.tag == "h3"));
        let tags: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .filter_map(|n| match n {
                        Node::Element(e) => Some(e.tag.to_string()),
                        _ => None,
                    })
                    .collect()
            })
            .collect();
        assert_eq!(tags, vec![vec!["div"], vec!["p", "p"]]);
    }
}
