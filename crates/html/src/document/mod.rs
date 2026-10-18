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
mod scope;
pub mod state;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
mod viewport;
mod writing_mode;
use crate::document::canvas_background::any_containment;
use crate::document::canvas_background::any_side;
use crate::document::canvas_background::mark_canvas_background;
use crate::document::canvas_background::viewport_overflow;
pub use crate::document::embedded::Embedded;
use crate::document::embedded::hash_of;
#[cfg(test)]
pub(crate) use crate::document::embedded::parse_embedded;
use crate::document::embedded::unwrap_document;
pub(crate) use crate::document::embedded::{EmbeddingScheme, parse_embedded_sized};
use crate::document::logical_properties::resolve_logical;
use crate::document::writing_mode::propagate_writing_mode;

mod box_style;

use crate::dom::Node;
use crate::style::css::Media;
use state::DocState;
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
        Self::new_media(html, theme_css, false)
    }

    /// Parse a document for the screen (`print == false`) or the print
    /// medium (`@media print` matches, viewport = the 5in x 3in page box).
    /// The medium belongs to the view showing the document (Blink
    /// `LocalFrameView::MediaType`), so the host passes it here.
    pub fn new_media(html: &str, theme_css: &str, print: bool) -> Self {
        Self::build(html, theme_css, None, false, Media::for_medium(print))
    }

    /// Parse a document with the URL used to resolve its external resources.
    pub fn new_with_base(html: &str, theme_css: &str, base_uri: &str) -> Self {
        Self::build(
            html,
            theme_css,
            url::Url::parse(base_uri).ok(),
            false,
            Media::default(),
        )
    }

    /// Parse a local document, resolving relative resources against its source file.
    /// A `.svg` file is `image/svg+xml` (the file URL's type comes from its
    /// extension) and is parsed as an XML document.
    pub fn new_with_path(html: &str, theme_css: &str, path: &std::path::Path) -> Self {
        Self::new_with_path_media(html, theme_css, path, false)
    }

    /// `new_with_path` for the screen or the print medium (`new_media`).
    pub fn new_with_path_media(
        html: &str,
        theme_css: &str,
        path: &std::path::Path,
        print: bool,
    ) -> Self {
        let base = std::path::absolute(path)
            .ok()
            .and_then(|p| url::Url::from_file_path(p).ok());
        let xml = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        Self::build(html, theme_css, base, xml, Media::for_medium(print))
    }

    fn build(
        html: &str,
        theme_css: &str,
        base_url: Option<url::Url>,
        xml: bool,
        media: Media,
    ) -> Self {
        let effective_base = if xml {
            base_url.clone()
        } else {
            base_url::document_base(html, base_url.as_ref())
        };
        crate::text::paragraph::forget_measures();
        crate::layout::table::paint::forget_row_rects();
        crate::text::vertical::forget_vt_measures();
        let (state, nodes, root) =
            parse_document(html, theme_css, effective_base.as_ref(), xml, media);
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
        let media = self.state.media();
        *self = Self::build(html, theme_css, self.base_url.clone(), self.xml, media);
        true
    }

    /// Документ разобран для печатного носителя (`new_media`).
    pub fn is_print(&self) -> bool {
        self.state.media().print
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

/// Разбор в СВЕЖЕЕ состояние документа носителем `media`: шрифты и
/// `@color-profile` из листов страницы (до разбора: иначе первый же замер
/// ширины пойдёт по подстановке), разметка (XML — своим разборщиком,
/// `dom::xml`), правка адресов от действующей базы.
fn parse_document(
    html: &str,
    theme_css: &str,
    base: Option<&url::Url>,
    xml: bool,
    media: Media,
) -> (Rc<DocState>, Vec<Node>, crate::style::computed::Computed) {
    let state = DocState::new(media);
    let _scope = state.enter();
    let mut sheets = vec![theme_css.to_string()];
    sheets.extend(crate::dom::style_sources::collect(html, xml));
    crate::text::fonts::load_faces_sheets(&sheets, base);
    crate::style::values::color_space::load_profiles_sheets(&sheets, base);
    let (mut nodes, root) = viewport::prepare(if xml {
        crate::dom::parse_current_xml(html, theme_css, media)
    } else {
        crate::dom::parse_current(html, theme_css, media)
    });
    if let Some(base) = base {
        base_url::resolve(&mut nodes, base, false);
    }
    drop(_scope);
    (state, nodes, root)
}
