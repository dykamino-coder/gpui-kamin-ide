//! Многоколоночный поток из блочных детей (не только строчных).

use super::column_flow_in;
use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::probe::size_monolith;
use crate::render::{RenderOpts, is_blank};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn non_inline_columns(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    whole: bool,
    stretch: Option<f32>,
    all_inline: bool,
) -> Option<Option<AnyElement>> {
    if !all_inline {
        // Один-единственный блок с текстом — это тот же поток, только в своей
        // коробке: колонки режут его строки, а не обходят стороной. Разметка
        // теста колонок почти всегда такая (`<div class=multicol><div>…`).
        let mut blocks = e.children.iter().filter(|n| !is_blank(n));
        let (Some(Node::Element(only)), None) = (blocks.next(), blocks.next()) else {
            return Some(None);
        };
        if only.style.position.is_some() || only.style.float.is_some_and(|f| f != 0) {
            return Some(None);
        }
        // Вложенный многоколоночник с заданной высотой — не «тот же поток»: его
        // строки идут СВОИМИ колонками, рядами во внешних (`nested_rows_box`,
        // `flow::OUTER_ROW`), а текстовый путь разложил бы их по внешним
        // колонкам (`multicol-breaking-000…006`).
        if nested_rows_box(only) {
            return Some(None);
        }
        let inside = inherit(inherited, &only.style);
        return Some(column_flow_in(
            only,
            &inside,
            opts,
            count,
            col_w,
            whole || size_monolith(only),
            stretch,
        ));
    }
    None
}
