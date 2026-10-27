//! Полный HTML/XML DOM: парсер не обрезает и не сплющивает вложенность.
//!
//! HTML driver и sink используют общую per-revision admission policy.

#[cfg(test)]
mod admission_tests;
mod html;
mod names;
mod overhead;
mod policy;

use crate::document::DocumentFailure;
use markup5ever_rcdom::RcDom;
use std::rc::Rc;

/// Подразборы одной ревизии разделяют work, имена и structural admission.
#[derive(Clone)]
pub(crate) struct Context(Rc<policy::Policy>);

impl Context {
    pub(crate) fn new(budget: crate::budget::RevisionBudget) -> Self {
        Self(Rc::new(policy::Policy::new(budget)))
    }

    pub(crate) fn budget(&self) -> &crate::budget::RevisionBudget {
        &self.0.budget
    }
}

/// Исходное дерево остаётся во владении revision независимо от её представления.
pub(crate) struct Admitted {
    pub dom: RcDom,
    pub xhtml: bool,
    context: Context,
}

impl Admitted {
    #[cfg(test)]
    pub fn parse(source: &str, xml: bool) -> Result<Self, DocumentFailure> {
        let budget = crate::budget::DocumentBudget::new(Default::default(), Default::default())
            .map_err(|error| DocumentFailure::new(crate::document::FailureReason::Budget(error)))?;
        let revision = budget.revision();
        revision
            .source(source, "")
            .map_err(|error| DocumentFailure::new(crate::document::FailureReason::Budget(error)))?;
        Self::with_budget(source, xml, revision)
    }

    pub(crate) fn with_budget(
        source: &str,
        xml: bool,
        revision: crate::budget::RevisionBudget,
    ) -> Result<Self, DocumentFailure> {
        Self::with_context(source, xml, Context::new(revision))
    }

    pub(crate) fn context(&self) -> Context {
        self.context.clone()
    }

    pub(crate) fn with_context(
        source: &str,
        xml: bool,
        context: Context,
    ) -> Result<Self, DocumentFailure> {
        let policy = &context.0;
        let mut dom = if xml {
            super::xml::xml_dom(source)?
        } else {
            html::parse(source, policy).map_err(DocumentFailure::new)?
        };
        let xhtml = xml
            || super::content::is_xhtml(&dom.document, &policy.budget)
                .map_err(DocumentFailure::new)?;
        if xhtml && !xml {
            let (expanded, _storage) =
                super::xhtml::expand_xhtml_self_closing(source, &policy.budget)
                    .map_err(DocumentFailure::new)?;
            if let std::borrow::Cow::Owned(expanded) = expanded {
                drop(dom);
                dom = html::parse(&expanded, policy).map_err(DocumentFailure::new)?;
            }
        }
        Ok(Self {
            dom,
            xhtml,
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Полная вложенность HTML/XML, неявные узлы, templates и малый стек парсера.

    use super::*;
    use markup5ever_rcdom::{Handle, NodeData};

    fn nested(depth: usize, xml: bool) -> String {
        let wrappers = if xml { depth - 1 } else { depth - 3 };
        format!(
            "{}text{}",
            "<div>".repeat(wrappers),
            "</div>".repeat(wrappers)
        )
    }

    fn facts(root: &Handle) -> (usize, String) {
        let mut pending = vec![(root.clone(), 0)];
        let (mut maximum, mut text) = (0, String::new());
        while let Some((node, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            if let NodeData::Text { contents } = &node.data {
                text.push_str(&contents.borrow());
            }
            if let NodeData::Element {
                template_contents, ..
            } = &node.data
                && let Some(root) = template_contents.borrow().as_ref()
            {
                pending.push((root.clone(), depth + 1));
            }
            pending.extend(
                node.children
                    .borrow()
                    .iter()
                    .rev()
                    .map(|n| (n.clone(), depth + 1)),
            );
        }
        (maximum, text)
    }

    #[test]
    fn html_and_xml_preserve_all_three_boundaries_without_flattening() {
        for xml in [false, true] {
            for depth in [63, 64, 65] {
                let input = Admitted::parse(&nested(depth, xml), xml).unwrap();
                assert_eq!(facts(&input.dom.document), (depth, "text".into()));
            }
        }
    }

    #[test]
    fn implied_nodes_and_tree_repair_preserve_the_actual_tree() {
        for depth in [63, 64, 65] {
            let source = format!(
                "{}<table><tr><td>x</table>{}",
                "<div>".repeat(depth - 7),
                "</div>".repeat(depth - 7)
            );
            let input = Admitted::parse(&source, false).unwrap();
            assert_eq!(facts(&input.dom.document), (depth, "x".into()));
        }
        let input = Admitted::parse(&"<p>x".repeat(1000), false).unwrap();
        assert_eq!(facts(&input.dom.document), (4, "x".repeat(1000)));
    }

    #[test]
    fn inert_and_shadow_templates_retain_fragment_and_slot_edges() {
        for shadow in ["", " shadowrootmode='open'"] {
            for depth in [63, 64, 65] {
                let source = format!(
                    "<div><template{shadow}>{}<slot>x</slot>{}</template></div>",
                    "<div>".repeat(depth - 7),
                    "</div>".repeat(depth - 7)
                );
                let input = Admitted::parse(&source, false).unwrap();
                assert_eq!(facts(&input.dom.document), (depth, "x".into()));
            }
        }
    }

    #[test]
    fn xhtml_self_closing_expansion_keeps_siblings_and_text() {
        let source = format!(
            "<html xmlns='http://www.w3.org/1999/xhtml'><body>{}<span>text</span></body></html>",
            "<span/>".repeat(64)
        );
        let input = Admitted::parse(&source, false).unwrap();
        assert_eq!(facts(&input.dom.document), (4, "text".into()));
    }

    #[test]
    fn deep_html_xml_and_templates_parse_and_drop_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                for xml in [false, true] {
                    let input = Admitted::parse(&nested(10_000, xml), xml).unwrap();
                    assert_eq!(facts(&input.dom.document), (10_000, "text".into()));
                }
                let source = format!(
                    "{}x{}",
                    "<template>".repeat(2000),
                    "</template>".repeat(2000)
                );
                let input = Admitted::parse(&source, false).unwrap();
                let (depth, text) = facts(&input.dom.document);
                assert!(depth > 2000);
                assert_eq!(text, "x");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn xml_templates_and_slots_have_no_implied_html_wrappers() {
        for depth in [63, 64, 65] {
            let source = format!(
                "<template>{}<slot>x</slot>{}</template>",
                "<div>".repeat(depth - 3),
                "</div>".repeat(depth - 3)
            );
            let input = Admitted::parse(&source, true).unwrap();
            assert_eq!(facts(&input.dom.document), (depth, "x".into()));
        }
    }
}
