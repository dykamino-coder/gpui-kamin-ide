//! Сборка кадра документа для хоста: опции сборки с этим документом, сборка
//! кадра и блоков, обёртка `Scoped` для фаз GPUI, значения и правила `@page`
//! для листов.

use super::Document;
use super::state::Scoped;
use crate::document::state::ValueCx;

impl Document {
    /// Начать кадр и вернуть опции сборки с этим документом: с ними хост
    /// вызывает `render::*` по `nodes()` (стопка страниц) сам.
    pub fn frame_opts(&self, opts: &crate::RenderOpts) -> crate::RenderOpts {
        self.state.begin_frame();
        let mut opts = opts.clone();
        opts.cx.clamp_plans = crate::text::clamp::ClampPlans::snapshot(&self.state.clamp);
        opts.cx.clamp_input = None;
        opts.cx.doc = crate::layout::cx::BuildDoc(self.state.clone());
        opts
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
        crate::render::render(&self.nodes, &self.frame_opts(opts))
            .into_iter()
            .map(|e| self.scoped(e))
            .collect()
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
