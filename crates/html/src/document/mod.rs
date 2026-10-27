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

#[cfg(test)]
mod admission_tests;
mod base_url;
#[cfg(test)]
mod budget_tests;
mod builder;
#[cfg(feature = "engine-bench")]
mod timing;
#[cfg(feature = "engine-bench")]
pub use timing::BuildTimings;
mod outcome;
mod revision;
pub use builder::DocumentBuilder;
pub use outcome::{
    BuildReport, DocumentFailure, FailureReason, FailureScope, RevisionStatus, TraversalKind,
    TraversalLimit,
};
mod canvas_background;
mod embedded;
pub(crate) mod image_values;
mod logical_properties;
mod prepare;
#[cfg(test)]
mod resource_tests;
pub(crate) mod resource_url;
mod scope;
use prepare::parse_document;
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
    last_failure: Option<DocumentFailure>,
    #[cfg(feature = "engine-bench")]
    timings: BuildTimings,
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
        fonts: &crate::text::metrics::FontContext,
        revision: crate::budget::RevisionBudget,
    ) -> Result<Self, DocumentFailure> {
        let key = base_url::key(html, theme_css, base_url.as_ref());
        #[cfg(feature = "engine-bench")]
        let parse_started = std::time::Instant::now();
        let input = crate::dom::input::Admitted::with_budget(html, xml, revision.clone()).map_err(
            |mut failure| {
                failure.report.revision = Some(key);
                failure
            },
        )?;
        #[cfg(feature = "engine-bench")]
        let mut timings = BuildTimings {
            parse: parse_started.elapsed(),
            ..Default::default()
        };
        let effective_base = if xml {
            base_url.clone()
        } else {
            base_url::document_base(&input.dom.document, base_url.as_ref())
        };
        let (state, nodes, root) = parse_document(
            &input,
            theme_css,
            effective_base.as_ref(),
            media,
            fonts,
            revision,
            #[cfg(feature = "engine-bench")]
            &mut timings,
        );
        state.traversal.retain(input);
        Ok(Document {
            state,
            nodes,
            root,
            key,
            base_url,
            xml,
            last_failure: None,
            #[cfg(feature = "engine-bench")]
            timings,
        })
    }

    /// Платформенная конфигурация для нового документа того же хоста.
    pub fn font_context(&self) -> crate::text::metrics::FontContext {
        self.state.metrics.font_context()
    }

    /// Текстовый стиль страницы (`html`/`body`): им набирается текст, у
    /// которого своего элемента нет.
    pub fn root_style(&self) -> &crate::style::computed::Computed {
        &self.root
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
