//! Область документа для хоста: вход в неё на сборке кадра, сборка кадра и
//! блоков в ней, обёртка `Scoped` для фаз GPUI, правила `@page` для листов.

use super::Document;
use super::state::{DocScope, Scoped};

impl Document {
    /// Сделать документ текущим до конца жизни guard'а: в этой области
    /// хост вызывает `render::*` по `nodes()` и разбирает значения листа.
    pub fn enter(&self) -> DocScope {
        self.state.begin_frame();
        self.state.enter_frame()
    }

    /// Обернуть элемент, собранный в области документа, чтобы раскладка,
    /// подготовка и краска его поддерева шли в той же области.
    pub fn scoped(&self, element: gpui::AnyElement) -> gpui::AnyElement {
        Scoped::wrap(&self.state, element)
    }

    /// Собрать кадр всего документа (`render::render` в его области).
    pub fn render(&self, opts: &crate::RenderOpts) -> Vec<gpui::AnyElement> {
        let _scope = self.enter();
        crate::render::render(&self.nodes, opts)
            .into_iter()
            .map(|e| self.scoped(e))
            .collect()
    }

    /// Собрать один блок верхнего уровня (`render::render_block`).
    pub fn render_block(&self, index: usize, opts: &crate::RenderOpts) -> Option<gpui::AnyElement> {
        let _scope = self.enter();
        crate::render::render_block(&self.nodes, index, opts).map(|e| self.scoped(e))
    }

    /// Правила `@page` документа — по ним хост строит листы
    /// (`css::page_decls_in`, `css::page_margins_in`).
    pub fn page_rules(&self) -> Vec<crate::style::css::PageRule> {
        self.state.page_rules()
    }
}
