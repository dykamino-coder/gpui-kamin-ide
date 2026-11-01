//! Сборка кадра документа для хоста: опции сборки с этим документом, сборка
//! кадра и блоков, обёртка `Scoped` для фаз GPUI, значения и правила `@page`
//! для листов.

use super::Document;
use super::state::Scoped;
use crate::values::ValueCx;

impl Document {
    /// Документ завершил работу текущего кадра. Читать после завершения кадра
    /// хостом, включая deferred paint; анимационное время хост фиксирует сам.
    /// Это не подтверждение презентации GPU и не разрешение пропустить её.
    pub fn frame_ready(&self) -> bool {
        self.state.values.frame_ready()
    }

    /// Начать кадр и вернуть опции сборки с этим документом: с ними хост
    /// вызывает `render::*` по `nodes()` (стопка страниц) сам.
    pub fn frame_opts(&self, opts: &crate::RenderOpts) -> crate::RenderOpts {
        frame_opts(&self.state, &self.nodes, opts)
    }

    /// Значения кадра документа: ими хост разбирает объявления листа
    /// (`Len::parse` и `Color::parse` по правилам `@page`).
    pub fn values(&self) -> ValueCx<'_> {
        self.state.frame_values()
    }

    /// Обернуть элемент, собранный по `frame_opts`: документ доживает до
    /// краски и отмечается покрашенным.
    pub fn scoped(&self, element: gpui::AnyElement) -> gpui::AnyElement {
        Scoped::wrap(&self.state, element)
    }

    /// Собрать кадр всего документа (`render::render`).
    pub fn render(&self, opts: &crate::RenderOpts) -> Vec<gpui::AnyElement> {
        render(&self.state, &self.nodes, opts)
    }

    /// Собрать один блок верхнего уровня (`render::render_block`).
    pub fn render_block(&self, index: usize, opts: &crate::RenderOpts) -> Option<gpui::AnyElement> {
        crate::render::render_block(&self.nodes, index, &self.frame_opts(opts))
            .map(|e| self.scoped(e))
    }

    /// Правила `@page` документа — по ним хост строит листы
    /// (`css::page_decls_in`, `css::page_margins_in`).
    pub fn page_rules(&self) -> Vec<crate::style::css::PageRule> {
        self.state.page_rules()
    }
}

/// Начать кадр документа `state` с деревом `nodes` (`Document::frame_opts`).
pub(super) fn frame_opts(
    state: &std::rc::Rc<super::state::DocState>,
    nodes: &[crate::dom::Node],
    opts: &crate::RenderOpts,
) -> crate::RenderOpts {
    state.begin_frame();
    crate::values::image_values::refresh(state.frame_values());
    let mut opts = opts.clone();
    opts.cx.clamp.input = None;
    opts.cx.values.inert_clear = false;
    opts.cx.contracts.document_root = Some((nodes.as_ptr() as usize, nodes.len()));
    opts.cx.values.doc = crate::layout::cx::BuildDoc(state.clone());
    opts
}

/// Собрать кадр всего документа (`Document::render`).
pub(super) fn render(
    state: &std::rc::Rc<super::state::DocState>,
    nodes: &[crate::dom::Node],
    opts: &crate::RenderOpts,
) -> Vec<gpui::AnyElement> {
    crate::render::render(nodes, &frame_opts(state, nodes, opts))
        .into_iter()
        .map(|e| Scoped::wrap(state, e))
        .collect()
}
