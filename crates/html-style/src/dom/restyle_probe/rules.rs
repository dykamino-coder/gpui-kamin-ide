//! The rules of a probe (`RestyleProbe`): built from the admitted source of a
//! revision, or taken over from its build.

use super::RestyleProbe;
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
    ) -> Result<Self, DocumentFailure> {
        let media = asked;
        let admission = crate::budget::admission::Admission::new(input.context().budget().clone());
        let document = &input.dom.document;
        let shadows = Rc::new(crate::dom::shadow::shadow_facts(document, &admission)?);
        let page = crate::dom::page_scheme::used_dark(document, media.dark, &admission)?;
        let scheme =
            crate::style::values::value::Scheme::new(media.dark).with_page(page.unwrap_or(false));
        let vcx = values.values().with_scheme(scheme);
        let rules =
            crate::dom::document_rules::document_rules(input, extra_css, media, &admission, vcx)?;
        let built = Self::built(
            rules.doc,
            rules.has_args,
            rules.reads_attr,
            (scheme, asked),
            extra_css,
        );
        Ok(Self { shadows, ..built })
    }

    /// Rules from a build, retaining page support separately from media preference.
    pub(in crate::dom) fn built(
        doc: Rc<Scope>,
        has_args: Vec<HasArg>,
        reads_attr: crate::dom::attr_reads::AttrReads,
        (scheme, asked): (crate::style::values::value::Scheme, Media),
        extra_css: &str,
    ) -> Self {
        Self {
            doc,
            has_args,
            scheme,
            asked,
            extra_css: extra_css.into(),
            reads_attr,
            shadows: Default::default(),
        }
    }

    /// The shadow trees of the document's light tree (`ShadowFacts`).
    pub fn shadows(&self) -> &crate::dom::shadow::ShadowFacts {
        &self.shadows
    }

    /// Лист документа или тема читают атрибуты элементов вне селекторов.
    pub fn reads_attr(&self) -> &crate::dom::attr_reads::AttrReads {
        &self.reads_attr
    }

    /// Правила области документа (агент, тема, листы страницы).
    pub fn rules(&self) -> &[Rule] {
        &self.doc.rules
    }
}
