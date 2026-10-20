//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

mod paragraph_route;
use paragraph_route::paint_inline_step7;
use paragraph_route::paragraph_probed;

mod paint_order;
pub(super) use paint_order::inline_abs_paint_last;
use paint_order::paint_last_ok;
use paint_order::unkeyed_positions;

use crate::dom::Node;
pub(crate) use crate::dom::text_content::{gather_text, split_nodes};
use crate::layout::float::clear::without_inert_clear;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
pub(super) mod content_wrapper;
mod first_line_text;
pub(crate) use content_wrapper::{content_sized, content_sized_wraps};
mod first_letter_descendants;
mod first_letter_scope;
pub(super) mod first_line_descendants;
pub(super) mod pseudo_line_layers;
pub(crate) use crate::layout::block::inline_splits::split_block_in_inline;
pub(super) mod native_paragraph_route;

pub(super) mod box_div;
pub(crate) use crate::layout::box_kind::*;
pub(crate) use crate::render::box_div::*;
pub(super) mod util;
pub use crate::layout::page::names::{PageMarginDecls, PageMarginDeclsFn, first_page_name};
pub use crate::layout::page::paged::{render_paged, render_paged_select};
pub(crate) use crate::render::util::*;
pub(crate) use crate::text::fonts::style_font::measure_font;
pub(crate) use crate::text::paragraph::text_id::text_id;
pub(crate) use crate::text::shadow_text::normalize_for_shadow;
pub(crate) mod paragraph;
pub(crate) use crate::render::paragraph::atom_piece::*;
pub(crate) use crate::render::paragraph::pieces::*;
pub(crate) use crate::render::paragraph::*;
pub(crate) mod blocks;
pub(crate) use crate::render::blocks::*;
pub(super) mod element;
pub(super) mod generic_box;
pub use crate::layout::options::RenderOpts;
pub(crate) use crate::render::element::*;
use crate::render::generic_box::*;

pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let viewport_opts = opts.inner(|cx| cx.paint_viewport = opts.viewport);
    let opts = &*viewport_opts;
    let vcx = opts.cx.vcx();
    let stripped = without_inert_clear(nodes);
    let nodes: &[Node] = stripped.as_deref().unwrap_or(nodes);
    crate::text::metrics::set_doc_family(&opts.text.font_family, vcx);
    let root = opts.root_style();
    crate::interactive::frame::frame_sanitize(vcx);
    crate::paint::effects::mask::collect_mask_defs(nodes, vcx);
    // Корень документа получает ширину области просмотра: от неё цепочка
    // `LayoutCx::avail_w` вычитает поля/рамки/отступы `html` и `body`.
    // Слои содержащих блоков — свои у каждой сборки документа.
    let opts = &*opts.inner(|cx| {
        cx.avail_w = Some(opts.viewport.0).filter(|w| *w > 0.0);
        cx.positioning = Default::default();
        if !cx.in_render {
            cx.vertical_paint = Default::default();
            cx.paint = Default::default();
        }
    });
    // Слой начального содержащего блока: внепоточные элементы без
    // позиционированного предка дописываются последними детьми документа —
    // их края решает область просмотра (§10.1 п.4).
    let icb_layer = crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.positioning);
    // Шаг 8 приложения E: позиционированные `z-index: auto | 0` красятся
    // после потока корневого контекста в порядке разметки — собиратель
    // `gpui::PaintCollect` между парой меток. Внешняя сборка идёт вне краски
    // и сбрасывает собиратели (пойманная паника кадра оставила бы их
    // открытыми); вложенный документ (рамка) собирает своё внутри.
    if !opts.cx.in_render {
        gpui::paint_collect_reset();
    }
    let opts = &*opts.inner(|cx| {
        cx.unkeyed = std::rc::Rc::new(unkeyed_positions(nodes));
        cx.in_render = true;
    });
    let mut out = blocks(nodes, &root, opts);
    out.extend(icb_layer.finish());
    let (open, close) = gpui::PaintCollect::pair();
    out.insert(0, open.into_any_element());
    out.push(close.into_any_element());
    out
}

/// Позиции узлов в прямом обходе документа и позиция ПОСЛЕДНЕГО
/// позиционированного, который ключа краски не получает (см.
/// `unkeyed_positions`).
pub(crate) type Unkeyed = (std::collections::HashMap<u64, usize>, Option<usize>);

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let viewport_opts = opts.inner(|cx| cx.paint_viewport = opts.viewport);
    let opts = &*viewport_opts;
    let vcx = opts.cx.vcx();
    let node = nodes.get(index)?;
    crate::interactive::frame::frame_sanitize(vcx);
    crate::paint::effects::mask::collect_mask_defs(nodes, vcx);
    let root = opts.root_style();
    // Слой ICB закрывается на блок ленты: дальше своего блока внепоточный
    // элемент всё равно не уедет, а без слоя он остался бы на месте.
    let opts = &*opts.inner(|cx| {
        cx.avail_w = Some(opts.viewport.0).filter(|w| *w > 0.0);
        cx.positioning = Default::default();
        if !cx.in_render {
            cx.vertical_paint = Default::default();
            cx.paint = Default::default();
        }
    });
    let icb_layer = crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.positioning);
    let out = blocks(std::slice::from_ref(node), &root, opts);
    let layer = icb_layer.finish();
    let first = out.into_iter().next()?;
    if layer.is_empty() {
        return Some(first);
    }
    // Лента отдаёт РОВНО ОДИН элемент на блок, поэтому слой уходит внутрь
    // обёртки. Содержащим блоком становится она, а не окно: в ленте окна
    // всё равно нет — блок живёт в прокрутке. Без обёртки вынесенный
    // элемент просто пропадал бы с экрана.
    Some(
        div()
            .relative()
            .child(first)
            .children(layer)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests;
