//! Detached source nodes and child lists share the parser's admission policy.

use super::{Admitted, Context, DocumentFailure};

impl Context {
    pub fn child_list(
        &self,
        children: impl Iterator<Item = markup5ever_rcdom::Handle>,
    ) -> Result<markup5ever_rcdom::admission::ChildList, DocumentFailure> {
        markup5ever_rcdom::admission::ChildList::prepare(children, &*self.0)
            .map_err(DocumentFailure::new)
    }
}

impl Admitted {
    pub fn create_element(
        &self,
        name: html5ever::QualName,
        context: &Context,
    ) -> Result<markup5ever_rcdom::Handle, DocumentFailure> {
        context.0.name(&name.local).map_err(DocumentFailure::new)?;
        let mut flags = html5ever::interface::ElementFlags::default();
        flags.template = name.ns == html5ever::ns!(html) && &*name.local == "template";
        self.dom
            .try_create_element(name, Vec::new(), flags, &*context.0)
            .map_err(DocumentFailure::new)
    }

    pub fn create_text(
        &self,
        text: &str,
        context: &Context,
    ) -> Result<markup5ever_rcdom::Handle, DocumentFailure> {
        let mut contents = html5ever::tendril::StrTendril::new();
        context
            .0
            .buffer(&mut contents, text.len())
            .map_err(DocumentFailure::new)?;
        context.0.work(text.len()).map_err(DocumentFailure::new)?;
        contents.push_slice(text);
        self.dom
            .try_create_text(contents, &*context.0)
            .map_err(DocumentFailure::new)
    }
}
