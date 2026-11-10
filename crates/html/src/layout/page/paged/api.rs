//! Публичные точки входа постраничного построителя.

use crate::{
    dom::Node,
    layout::{options::RenderOpts, page::names::PageMarginDeclsFn},
};
use gpui::AnyElement;

/// Постраничная отрисовка (css-page-3): блоки верхнего уровня документа —
/// дети стопки страниц, page area каждой страницы — фрагментаинер.
///
/// Обёртки `html`/`body` снимаются здесь (сборщик оставляет их при
/// собственной коробке, `doc.rs::has_box_style`): их фон — канвас документа
/// (§painting, слой 2), боковые поля/рамки/отступы — сдвиг содержимого на
/// КАЖДОЙ странице, верхнее — только на первой: коробка тела режется по
/// страницам вместе с содержимым, а её `page` — умолчание имени для детей.
pub fn render_paged(
    nodes: &[Node],
    opts: &RenderOpts,
    geom_for: crate::layout::page::page_stack::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
) -> AnyElement {
    super::render_paged_for::<false>(nodes, opts, geom_for, margin_decls, None)
}

/// `render_paged`, показывающий только листы `select` (номера с нуля).
pub fn render_paged_select(
    nodes: &[Node],
    opts: &RenderOpts,
    geom_for: crate::layout::page::page_stack::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
    select: Option<Vec<usize>>,
) -> AnyElement {
    super::render_paged_for::<false>(nodes, opts, geom_for, margin_decls, select)
}
