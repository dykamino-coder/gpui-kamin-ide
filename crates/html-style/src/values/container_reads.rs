//! Whether the document's values can read the size of a query container.
//!
//! A size container's content box (css-conditional-5 §6.1) reaches a value
//! only through a container-relative length (`cqw`, `cqh`, `cqi`, `cqb`,
//! `cqmin`, `cqmax`, §6.1.1); this engine evaluates no `@container` rule. Such
//! a length lives as a term of a `calc` sum in the document's arena
//! (`style::values::value::units`), and every sum enters the arena through
//! `calc_store`, which notes the term here (`note`). Until a document holds
//! one, the layout of its containers publishes no size (`any`): a container
//! that changes size then restyles nothing and lays nothing out again. Blink
//! decides the same per container: `ContainerQueryEvaluator::SizeContainerChanged`
//! reports no change unless a query or a container-relative unit depends on
//! the size (`CSSToLengthConversionData::ContainerSizes` marks the unit read).
//! The fact is sticky for the document: a removed length keeps it.

use super::{DocumentValues, ValueCx};

/// A sum stored in the document holds a container-relative term.
pub fn note(vcx: ValueCx<'_>) {
    vcx.doc.traversal.container_lengths.set(true);
}

/// Some value of the document may read a query container's size.
pub fn any(doc: &DocumentValues) -> bool {
    doc.traversal.container_lengths.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn only_a_container_relative_length_marks_the_document() {
        let doc = DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        for other in ["calc(10px + 2vw)", "3vmin", "calc(50% - 1em)"] {
            assert!(Len::parse_mixed(other, vcx).is_some(), "{other}");
        }
        assert!(!any(&doc));
        assert!(Len::parse("10cqw", vcx).is_some());
        assert!(any(&doc));
    }
}
