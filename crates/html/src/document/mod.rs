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
mod resource_tests;
pub(crate) mod resource_url;
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

    fn parse_nodes(html: &str, theme_css: &str, xml: bool) -> Vec<Node> {
        if xml {
            crate::dom::parse_xml(html, theme_css)
        } else {
            crate::dom::parse(html, theme_css)
        }
    }

    fn build(html: &str, theme_css: &str, base_url: Option<url::Url>, xml: bool) -> Self {
        let effective_base = if xml {
            base_url.clone()
        } else {
            base_url::document_base(html, base_url.as_ref())
        };
        // Свои шрифты страницы грузятся ДО разбора: иначе первый же замер
        // ширины пойдёт по подстановке, а перерисовки под новый шрифт нет.
        let mut sheets = vec![theme_css.to_string()];
        sheets.extend(crate::dom::style_sources::collect(html, xml));
        crate::text::fonts::load_faces_sheets(&sheets, effective_base.as_ref());
        crate::style::values::color_space::load_profiles_sheets(&sheets, effective_base.as_ref());
        crate::text::paragraph::forget_measures();
        crate::layout::table::paint::forget_row_rects();
        crate::text::vertical::forget_vt_measures();
        let (mut nodes, root) = viewport::prepare(Self::parse_nodes(html, theme_css, xml));
        if let Some(base) = &effective_base {
            base_url::resolve(&mut nodes, base, false);
        }
        Document {
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
        let (mut nodes, root) = viewport::prepare(Self::parse_nodes(html, theme_css, self.xml));
        if let Some(base) = &self.base_url {
            let effective = if self.xml {
                Some(base.clone())
            } else {
                base_url::document_base(html, Some(base))
            };
            if let Some(base) = effective {
                base_url::resolve(&mut nodes, &base, false);
            }
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
        let mut pending: Vec<&Node> = self.nodes.iter().collect();
        let mut count = 0;
        while let Some(node) = pending.pop() {
            count += 1;
            if let Node::Element(element) = node {
                pending.extend(&element.children);
            }
        }
        count
    }

    /// Блоки верхнего уровня — единица виртуализации: их можно раздать в
    /// список GPUI, который раскладывает только видимое.
    pub fn top_level_blocks(&self) -> usize {
        self.nodes.len()
    }
}
