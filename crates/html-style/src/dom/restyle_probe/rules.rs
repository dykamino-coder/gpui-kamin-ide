//! The rules of a probe (`RestyleProbe`): built from the admitted source of a
//! revision, or taken over from its build.

use super::{RestyleProbe, scheme};
use crate::dom::input::Admitted;
use crate::dom::outcome::DocumentFailure;
use crate::dom::shadow::Scope;
use crate::style::css::{Media, Rule};
use crate::style::select::has::HasArg;
use crate::values::DocumentValues;
use std::rc::Rc;

impl RestyleProbe {
    pub(super) fn new(
        input: &Admitted,
        extra_css: &str,
        asked: Media,
        values: &Rc<DocumentValues>,
    ) -> Result<Option<Self>, DocumentFailure> {
        let mut media = asked;
        let admission = crate::budget::admission::Admission::new(input.context().budget().clone());
        let document = &input.dom.document;
        if crate::dom::shadow::has_declarative_shadow(document, &admission)? {
            return Ok(None);
        }
        if let Some(dark) = crate::dom::page_scheme::preference(document, media.dark, &admission)? {
            media.dark = dark;
        }
        let vcx = values.values().with_scheme(scheme(media));
        let rules =
            crate::dom::document_rules::document_rules(input, extra_css, media, &admission, vcx)?;
        Ok(Some(Self::built(
            rules.doc,
            rules.has_args,
            rules.reads_attr,
            (media, asked),
            extra_css,
        )))
    }

    /// Проба из правил сборки документа без теней (`convert_restyling`):
    /// `media` — носитель со схемой страницы и носитель, с которым звали
    /// сборку.
    pub(in crate::dom) fn built(
        doc: Rc<Scope>,
        has_args: Vec<HasArg>,
        reads_attr: bool,
        (media, asked): (Media, Media),
        extra_css: &str,
    ) -> Self {
        Self {
            doc,
            has_args,
            media,
            asked,
            extra_css: extra_css.into(),
            reads_attr,
        }
    }

    /// Лист документа или тема читают атрибуты элементов вне селекторов.
    pub fn reads_attr(&self) -> bool {
        self.reads_attr
    }

    /// Правила области документа (агент, тема, листы страницы).
    pub fn rules(&self) -> &[Rule] {
        &self.doc.rules
    }
}
