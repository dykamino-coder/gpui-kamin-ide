//! Every allocating sink callback crosses admission before receiving ownership.

use super::{Admission, Attribute, NodeOrText, QualName, TreeBuilder, TreeSink};
use crate::admission::{storage::Storage, Operation};
use crate::tendril::StrTendril;
use markup5ever::{local_name, ns};
use std::borrow::Cow;

impl<H: Clone, S: TreeSink<Handle = H>, A: Admission<S>> TreeBuilder<H, S, A> {
    fn checked_name(&self, name: &QualName) -> Result<(), A::Error> {
        self.admission.check(Operation::Name(&name.local))?;
        self.admission.check(Operation::Name(&name.ns))?;
        if let Some(prefix) = &name.prefix {
            self.admission.check(Operation::Name(prefix))?;
        }
        Ok(())
    }

    fn checked_attribute_names(&self, attrs: &[Attribute]) -> Result<(), A::Error> {
        for attr in attrs {
            self.checked_name(&attr.name)?;
        }
        Ok(())
    }

    pub(super) fn checked_create_element(
        &self,
        name: QualName,
        attrs: Storage<Attribute>,
        duplicate: bool,
    ) -> Result<H, A::Error> {
        self.result()?;
        self.checked_name(&name)?;
        self.checked_attribute_names(&attrs)?;
        self.admission
            .check(Operation::CreateElement(&name, &attrs))?;
        // Same flag calculation as markup5ever's create_element_with_flags.
        let mut flags = super::ElementFlags::default();
        match name.expanded() {
            markup5ever::expanded_name!(html "template") => flags.template = true,
            markup5ever::expanded_name!(mathml "annotation-xml") => {
                flags.mathml_annotation_xml_integration_point = attrs.iter().any(|attr| {
                    attr.name.expanded() == markup5ever::expanded_name!("", "encoding")
                        && (attr.value.eq_ignore_ascii_case("text/html")
                            || attr.value.eq_ignore_ascii_case("application/xhtml+xml"))
                });
            },
            _ => {},
        }
        flags.had_duplicate_attributes = duplicate;
        attrs.with_vec(|attrs| {
            self.admission
                .create_element(&self.sink, name, attrs, flags)
        })
    }

    pub(super) fn checked_create_element_default(
        &self,
        name: QualName,
        attrs: Storage<Attribute>,
    ) -> Result<H, A::Error> {
        self.checked_create_element(name, attrs, false)
    }

    pub(super) fn checked_create_comment(&self, text: StrTendril) -> Result<H, A::Error> {
        self.result()?;
        self.admission.check(Operation::CreateComment(&text))?;
        self.admission.create_comment(&self.sink, text)
    }

    pub(super) fn checked_create_pi(
        &self,
        target: StrTendril,
        data: StrTendril,
    ) -> Result<H, A::Error> {
        self.result()?;
        self.admission.check(Operation::CreatePi(&target, &data))?;
        self.admission.create_pi(&self.sink, target, data)
    }

    pub(super) fn checked_append(&self, parent: &H, child: NodeOrText<H>) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Append(parent, &child))?;
        self.admission.append(&self.sink, parent, child)
    }

    pub(super) fn checked_append_before_sibling(
        &self,
        sibling: &H,
        child: NodeOrText<H>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission
            .check(Operation::BeforeSibling(sibling, &child))?;
        self.admission.before_sibling(&self.sink, sibling, child)
    }

    pub(super) fn checked_append_based_on_parent_node(
        &self,
        element: &H,
        previous: &H,
        child: NodeOrText<H>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission
            .check(Operation::FosterParent(element, previous, &child))?;
        self.admission
            .foster_parent(&self.sink, element, previous, child)
    }

    pub(super) fn checked_append_doctype_to_document(
        &self,
        name: StrTendril,
        public: StrTendril,
        system: StrTendril,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission
            .check(Operation::Doctype(&name, &public, &system))?;
        self.admission.doctype(&self.sink, name, public, system)
    }

    pub(super) fn checked_add_attrs_if_missing(
        &self,
        target: &H,
        attrs: Storage<Attribute>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.checked_attribute_names(&attrs)?;
        self.admission
            .check(Operation::Attributes(target, &attrs))?;
        attrs.with_vec(|attrs| self.admission.attributes(&self.sink, target, attrs))
    }

    pub(super) fn checked_remove(&self, node: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Remove(node))?;
        self.admission.remove(&self.sink, node)
    }

    pub(super) fn checked_reparent_children(&self, node: &H, parent: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Reparent(node, parent))?;
        self.admission.reparent(&self.sink, node, parent)
    }

    pub(super) fn checked_pop(&self, node: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Pop(node))?;
        self.admission.pop(&self.sink, node)
    }

    pub(super) fn checked_attach_declarative_shadow(
        &self,
        host: &H,
        template: &H,
        attrs: &[Attribute],
    ) -> Result<bool, A::Error> {
        self.result()?;
        self.checked_attribute_names(attrs)?;
        self.admission
            .check(Operation::Shadow(host, template, attrs))?;
        self.admission.shadow(&self.sink, host, template, attrs)
    }

    pub(super) fn checked_parse_error(&self, message: Cow<'static, str>) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::ParseError(&message))?;
        self.admission.parse_error(&self.sink, message)
    }
    pub(super) fn checked_parse_error_fmt(
        &self,
        message: std::fmt::Arguments<'_>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::ParseError(""))?;
        self.admission.parse_error_fmt(&self.sink, message)
    }
    pub(super) fn checked_selectedcontent(&self, node: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Selectedcontent(node))?;
        self.admission.selectedcontent(&self.sink, node)
    }
}
