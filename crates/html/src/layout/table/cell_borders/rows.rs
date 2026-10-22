//! Кандидаты кромок ряда в сросшейся модели: рамки ряда (и последнего ряда охвата)
//! встречают охватывающую ячейку на её логических концах сетки и уходят пробами в слой
//! кромок. CSS 2.1 §17.6.2.1; Blink `TableBorders::ComputeTableBorders` помечает каждый
//! ряд отдельно.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::ParentElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn collapsed_row_edges(
    row: &Element,
    col_ix: usize,
    span_cols: u16,
    cols: u16,
    row_style: &Computed,
    table_style: &Computed,
    last_row: Option<&Element>,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    px_of: &impl Fn(Option<Len>) -> f32,
    mut d: gpui::Div,
    vcx: ValueCx<'_>,
) -> gpui::Div {
    let axes = super::super::axes::Axes::new(table_style);
    let first = [
        true,
        col_ix >= cols as usize,
        last_row.is_none(),
        col_ix == span_cols as usize,
    ];
    let candidates = std::iter::once((row, first))
        .chain(last_row.map(|last| (last, [false, false, true, false])));
    for (source, active) in candidates {
        let b = source.style.borders();
        let widths = axes.mask(
            [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)],
            active,
        );
        let raw_styles = std::array::from_fn(|i| {
            source.style.border_side_styles[i].unwrap_or(if widths[i] > 0.0 { 9 } else { 0 })
        });
        let logical_styles = axes.logical(raw_styles);
        let styles = axes.physical(std::array::from_fn(|i| {
            if active[i] { logical_styles[i] } else { 0 }
        }));
        if !widths.iter().any(|w| *w > 0.0) && !styles.contains(&1) {
            continue;
        }
        let end_style;
        let computed = if std::ptr::eq(source, row) {
            row_style
        } else {
            end_style = inherit(table_style, &source.style, vcx);
            &end_style
        };
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let colors = std::array::from_fn(|i| {
            source.style.border_colors[i].resolve(computed.color.unwrap_or(black))
        });
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            widths,
            colors,
            styles,
            4,
            source.node_id as u32,
            [0.0; 4],
        ));
    }
    d
}
