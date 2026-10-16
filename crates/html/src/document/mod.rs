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
#[cfg(test)]
mod tests;
mod viewport;
mod writing_mode;
use crate::document::canvas_background::any_containment;
use crate::document::canvas_background::any_side;
use crate::document::canvas_background::mark_canvas_background;
use crate::document::canvas_background::viewport_overflow;
use crate::document::embedded::hash_of;
pub use crate::document::embedded::parse_embedded;
use crate::document::embedded::unwrap_document;
pub(crate) use crate::document::embedded::{EmbeddingScheme, parse_embedded_sized};
use crate::document::logical_properties::resolve_logical;
use crate::document::writing_mode::propagate_writing_mode;

mod box_style;

use crate::dom::Node;

/// Документ, разобранный один раз.
pub struct Document {
    nodes: Vec<Node>,
    /// Текстовый стиль `body`: им набирается текст верхнего уровня, у
    /// которого своего элемента нет.
    root: crate::style::computed::Computed,
    /// Хэш разметки и темы: по нему видно, нужен ли повторный разбор.
    key: u64,
    base_url: Option<url::Url>,
}

impl Document {
    pub fn new(html: &str, theme_css: &str) -> Self {
        Self::build(html, theme_css, None)
    }

    /// Parse a document with the URL used to resolve its external resources.
    pub fn new_with_base(html: &str, theme_css: &str, base_uri: &str) -> Self {
        Self::build(html, theme_css, url::Url::parse(base_uri).ok())
    }

    /// Parse a local document, resolving relative resources against its source file.
    pub fn new_with_path(html: &str, theme_css: &str, path: &std::path::Path) -> Self {
        let base = std::path::absolute(path)
            .ok()
            .and_then(|p| url::Url::from_file_path(p).ok());
        Self::build(html, theme_css, base)
    }

    fn build(html: &str, theme_css: &str, base_url: Option<url::Url>) -> Self {
        // Свои шрифты страницы грузятся ДО разбора: иначе первый же замер
        // ширины пойдёт по подстановке, а перерисовки под новый шрифт нет.
        crate::text::fonts::load_faces(html);
        crate::style::values::color_space::load_profiles(html);
        crate::text::paragraph::forget_measures();
        crate::layout::table::paint::forget_row_rects();
        crate::text::vertical::forget_vt_measures();
        let (mut nodes, root) = viewport::prepare(crate::dom::parse(html, theme_css));
        if let Some(base) = &base_url {
            base_url::resolve(&mut nodes, &base_url::document_base(html, base), false);
        }
        Document {
            nodes,
            root,
            key: base_url::key(html, theme_css, base_url.as_ref()),
            base_url,
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
        let (mut nodes, root) = viewport::prepare(crate::dom::parse(html, theme_css));
        if let Some(base) = &self.base_url {
            base_url::resolve(&mut nodes, &base_url::document_base(html, base), false);
        }
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
