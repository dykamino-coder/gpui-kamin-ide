//! Перерасчёт стиля нескольких элементов исходного DOM ревизии без сборки
//! дерева коробок (`walk::probe`): те же правила, что у полной сборки
//! (`document_rules`), те же отметки `:has()` и тот же вход в элемент.
//!
//! Правила собираются один раз на пробу и служат обоим состояниям DOM пакета
//! мутаций: до записи и после неё. Документ с тенями не пробуется: области и
//! слоты считаются отдельным предпроходом по всему документу.

use super::input::Admitted;
use super::outcome::DocumentFailure;
use super::shadow::Scope;
use super::tree_index::TreeIndex;
use super::walk::{Probed, probe};
use crate::style::css::{Media, Rule};
use crate::style::select::has::{HasArg, mark_has};
use crate::values::DocumentValues;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// Правила документа одной ревизии для проб его элементов.
pub struct RestyleProbe {
    doc: Rc<Scope>,
    has_args: Vec<HasArg>,
    media: Media,
}

impl RestyleProbe {
    /// `None` — документ с декларативными тенями.
    pub fn new(
        input: &Admitted,
        extra_css: &str,
        mut media: Media,
        values: &Rc<DocumentValues>,
    ) -> Result<Option<Self>, DocumentFailure> {
        let admission = crate::budget::admission::Admission::new(input.context().budget().clone());
        let document = &input.dom.document;
        if super::shadow::has_declarative_shadow(document, &admission)? {
            return Ok(None);
        }
        if let Some(dark) = super::page_scheme::preference(document, media.dark, &admission)? {
            media.dark = dark;
        }
        let vcx = values.values().with_scheme(scheme(media));
        let rules =
            super::document_rules::document_rules(input, extra_css, media, &admission, vcx)?;
        Ok(Some(Self {
            doc: rules.doc,
            has_args: rules.has_args,
            media,
        }))
    }

    /// Правила области документа (агент, тема, листы страницы).
    pub fn rules(&self) -> &[Rule] {
        &self.doc.rules
    }

    /// Вход в каждый элемент `targets` при текущем состоянии исходного DOM.
    pub fn styles(
        &self,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        targets: &[Handle],
    ) -> Result<Vec<Option<Probed>>, DocumentFailure> {
        let mut tree = TreeIndex::new(values.clone(), input.xhtml);
        let document = &input.dom.document;
        if !self.has_args.is_empty() {
            mark_has(document, &self.has_args, &mut vec![], &mut tree)?;
        }
        let vcx = values.values().with_scheme(scheme(self.media));
        Ok(probe(document, &tree, &self.doc, targets, vcx)?)
    }
}

fn scheme(media: Media) -> crate::style::values::value::Scheme {
    crate::style::values::value::Scheme::new(media.dark)
}
