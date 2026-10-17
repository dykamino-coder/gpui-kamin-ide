//! Разобранный документ: то, что живёт между кадрами.
//!
//! Ключевое требование к переводчику — не быть дороже нативной вёрстки. В
//! GPUI элементы пересоздаются каждый кадр, и это нормально: сборка `div` с
//! готовыми числами дёшева. Дорого другое — разбор разметки, каскад и
//! растеризация рисунков. Всё это обязано случиться ОДИН раз на документ, а
//! не на кадр.
//!
//! Поэтому вызывающий держит у себя `Document`, а на кадре зовёт только
//! `render`. Пересборка происходит лишь когда сменилась сама разметка — что
//! проверяется по хэшу, а не по строке целиком.

mod base_url;
mod canvas_background;
mod embedded;
mod logical_properties;
pub mod state;
#[cfg(test)]
mod tests;
mod viewport;
mod writing_mode;
use crate::document::canvas_background::any_containment;
use crate::document::canvas_background::any_side;
use crate::document::canvas_background::mark_canvas_background;
use crate::document::canvas_background::viewport_overflow;
use crate::document::embedded::hash_of;
use crate::document::embedded::unwrap_document;
pub use crate::document::embedded::{Embedded, parse_embedded};
pub(crate) use crate::document::embedded::{EmbeddingScheme, parse_embedded_sized};
use crate::document::logical_properties::resolve_logical;
use crate::document::writing_mode::propagate_writing_mode;

mod box_style;

use crate::dom::Node;
use state::{DocScope, DocState, Scoped};
use std::rc::Rc;

/// Документ, разобранный один раз.
pub struct Document {
    /// Правила, метрики корня и режим документа (`state`): живут вместе с
    /// деревом и подменяются вместе с ним на `update`.
    state: Rc<DocState>,
    nodes: Vec<Node>,
    /// Текстовый стиль `body`: им набирается текст верхнего уровня, у
    /// которого своего элемента нет.
    root: crate::style::computed::Computed,
    /// Хэш разметки и темы: по нему видно, нужен ли повторный разбор.
    key: u64,
    base_url: Option<url::Url>,
    /// An XML document (`image/svg+xml`, `crate::dom::xml`).
    xml: bool,
}

impl Document {
    pub fn new(html: &str, theme_css: &str) -> Self {
        Self::build(html, theme_css, None, false)
    }

    /// Parse a document with the URL used to resolve its external resources.
    pub fn new_with_base(html: &str, theme_css: &str, base_uri: &str) -> Self {
        Self::build(html, theme_css, url::Url::parse(base_uri).ok(), false)
    }

    /// Parse a local document, resolving relative resources against its source file.
    /// A `.svg` file is `image/svg+xml` (the file URL's type comes from its
    /// extension) and is parsed as an XML document.
    pub fn new_with_path(html: &str, theme_css: &str, path: &std::path::Path) -> Self {
        let base = std::path::absolute(path)
            .ok()
            .and_then(|p| url::Url::from_file_path(p).ok());
        let xml = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        Self::build(html, theme_css, base, xml)
    }

    fn build(html: &str, theme_css: &str, base_url: Option<url::Url>, xml: bool) -> Self {
        // Свои шрифты страницы грузятся ДО разбора: иначе первый же замер
        // ширины пойдёт по подстановке, а перерисовки под новый шрифт нет.
        crate::text::fonts::load_faces(html);
        crate::text::paragraph::forget_measures();
        crate::layout::table::paint::forget_row_rects();
        crate::text::vertical::forget_vt_measures();
        let (state, nodes, root) = parse_document(html, theme_css, base_url.as_ref(), xml);
        Document {
            state,
            nodes,
            root,
            key: base_url::key(html, theme_css, base_url.as_ref()),
            base_url,
            xml,
        }
    }

    /// Текстовый стиль страницы (`html`/`body`): им набирается текст, у
    /// которого своего элемента нет.
    pub fn root_style(&self) -> &crate::style::computed::Computed {
        &self.root
    }

    /// Разобрать заново, только если разметка действительно изменилась.
    ///
    /// Для стриминга (текст дописывается по кусочку) это и есть главный
    /// рубеж: пока пришедший кусок не изменил разметку, дерево остаётся тем
    /// же, и кадр стоит ровно столько же, сколько нативный.
    pub fn update(&mut self, html: &str, theme_css: &str) -> bool {
        let key = base_url::key(html, theme_css, self.base_url.as_ref());
        if key == self.key {
            return false;
        }
        let (state, nodes, root) =
            parse_document(html, theme_css, self.base_url.as_ref(), self.xml);
        self.state = state;
        self.nodes = nodes;
        self.root = root;
        self.key = key;
        true
    }

    /// Хэш содержимого — соль для `RenderOpts::doc_salt`.
    pub fn salt(&self) -> u64 {
        self.key
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Сделать документ текущим до конца жизни guard'а: в этой области
    /// хост вызывает `render::*` по `nodes()` и разбирает значения листа.
    pub fn enter(&self) -> DocScope {
        self.state.enter()
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

    /// Сколько узлов в документе — для решения о виртуализации: раскладка в
    /// GPUI считается заново каждый кадр, поэтому длинный документ обязан
    /// рисоваться по видимым блокам, а не целиком.
    pub fn node_count(&self) -> usize {
        fn walk(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|n| match n {
                    Node::Text(_) => 1,
                    Node::Element(e) => 1 + walk(&e.children),
                })
                .sum()
        }
        walk(&self.nodes)
    }

    /// Блоки верхнего уровня — единица виртуализации: их можно раздать в
    /// список GPUI, который раскладывает только видимое.
    pub fn top_level_blocks(&self) -> usize {
        self.nodes.len()
    }
}

/// Разбор в СВЕЖЕЕ состояние документа: `@color-profile`, разметка (XML —
/// своим разборщиком, `dom::xml`), правка адресов. Шрифты страницы (`load_faces`) — процессный реестр гарнитур.
fn parse_document(
    html: &str,
    theme_css: &str,
    base_url: Option<&url::Url>,
    xml: bool,
) -> (Rc<DocState>, Vec<Node>, crate::style::computed::Computed) {
    let media = crate::style::css::Media::default();
    let state = DocState::new(media);
    let _scope = state.enter();
    crate::style::values::color_space::load_profiles(html);
    let (mut nodes, root) = viewport::prepare(if xml {
        crate::dom::parse_current_xml(html, theme_css, media)
    } else {
        crate::dom::parse_current(html, theme_css, media)
    });
    if let Some(base) = base_url {
        base_url::resolve(&mut nodes, &base_url::document_base(html, base), false);
    }
    drop(_scope);
    (state, nodes, root)
}
