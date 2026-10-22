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
mod builder;
pub use builder::DocumentBuilder;
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
    /// Настроить источник и носитель до единственного разбора документа.
    pub fn builder<'a>(html: &'a str, theme_css: &'a str) -> DocumentBuilder<'a> {
        DocumentBuilder::new(html, theme_css)
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

    /// Документ разобран для печатного носителя (`DocumentBuilder::print`).
    pub fn is_print(&self) -> bool {
        self.state.media().print
    }

    /// Хэш исходного содержимого, таблицы стилей и базового URL.
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
    state.images.borrow_mut().set_base(base.cloned());
    let vcx = state.values();
    let mut sheets = vec![theme_css.to_string()];
    sheets.extend(crate::dom::style_sources::collect(html, xml));
    crate::text::fonts::load_faces_sheets(&sheets, base, vcx);
    crate::style::values::color_space::load_profiles_sheets(&sheets, base, vcx);
    let (mut nodes, root) = viewport::prepare(
        if xml {
            crate::dom::parse_current_xml(html, theme_css, media, &state)
        } else {
            crate::dom::parse_current(html, theme_css, media, &state)
        },
        vcx,
    );
    if let Some(base) = base {
        base_url::resolve(&mut nodes, base, false);
    }
    state.masks.collect(&nodes, vcx);
    (state, nodes, root)
}
