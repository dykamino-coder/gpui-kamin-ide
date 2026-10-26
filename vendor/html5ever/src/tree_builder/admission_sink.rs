//! Every allocating sink callback crosses admission before receiving ownership.

use super::{Admission, Attribute, NodeOrText, QualName, TreeBuilder, TreeSink};
use crate::admission::Operation;
use crate::tendril::StrTendril;
use std::borrow::Cow;

impl<H: Clone, S: TreeSink<Handle = H>, A: Admission<H>> TreeBuilder<H, S, A> {
    pub(super) fn checked_create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        duplicate: bool,
    ) -> Result<H, A::Error> {
        self.result()?;
        self.admission
            .check(Operation::CreateElement(&name, &attrs))?;
        Ok(super::create_element_with_flags(
            &self.sink, name, attrs, duplicate,
        ))
    }

    pub(super) fn checked_create_element_default(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
    ) -> Result<H, A::Error> {
        self.checked_create_element(name, attrs, false)
    }

    pub(super) fn checked_create_comment(&self, text: StrTendril) -> Result<H, A::Error> {
        self.result()?;
        self.admission.check(Operation::CreateComment(&text))?;
        Ok(self.sink.create_comment(text))
    }

    pub(super) fn checked_create_pi(
        &self,
        target: StrTendril,
        data: StrTendril,
    ) -> Result<H, A::Error> {
        self.result()?;
        self.admission.check(Operation::CreatePi(&target, &data))?;
        Ok(self.sink.create_pi(target, data))
    }

    pub(super) fn checked_append(&self, parent: &H, child: NodeOrText<H>) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Append(parent, &child))?;
        self.sink.append(parent, child);
        Ok(())
    }

    pub(super) fn checked_append_before_sibling(
        &self,
        sibling: &H,
        child: NodeOrText<H>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission
            .check(Operation::BeforeSibling(sibling, &child))?;
        self.sink.append_before_sibling(sibling, child);
        Ok(())
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
        self.sink
            .append_based_on_parent_node(element, previous, child);
        Ok(())
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
        self.sink.append_doctype_to_document(name, public, system);
        Ok(())
    }

    pub(super) fn checked_add_attrs_if_missing(
        &self,
        target: &H,
        attrs: Vec<Attribute>,
    ) -> Result<(), A::Error> {
        self.result()?;
        self.admission
            .check(Operation::Attributes(target, &attrs))?;
        self.sink.add_attrs_if_missing(target, attrs);
        Ok(())
    }

    pub(super) fn checked_reparent_children(&self, node: &H, parent: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Reparent(node, parent))?;
        self.sink.reparent_children(node, parent);
        Ok(())
    }

    pub(super) fn checked_pop(&self, node: &H) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::Pop(node))?;
        self.sink.pop(node);
        Ok(())
    }

    pub(super) fn checked_attach_declarative_shadow(
        &self,
        host: &H,
        template: &H,
        attrs: &[Attribute],
    ) -> Result<bool, A::Error> {
        self.result()?;
        self.admission
            .check(Operation::Shadow(host, template, attrs))?;
        Ok(self.sink.attach_declarative_shadow(host, template, attrs))
    }

    pub(super) fn checked_parse_error(&self, message: Cow<'static, str>) -> Result<(), A::Error> {
        self.result()?;
        self.admission.check(Operation::ParseError(&message))?;
        self.sink.parse_error(message);
        Ok(())
    }
}
