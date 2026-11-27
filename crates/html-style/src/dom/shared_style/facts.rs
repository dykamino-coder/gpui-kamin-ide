//! Факты коробки, созданной или перенесённой раскладкой, вне вычисленных значений.

use super::SharedStyle;
use crate::box_facts::BoxContext;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::computed::{Align, BoxStyle};
use std::rc::Rc;

/// Контекст коробки-родителя, стиль элемента, чей каскад пересчитывают варианты
/// `position-try`, коробка родителя по дереву элементов перенесённой коробки и
/// идентичность украшающей коробки стиля, значения которого её не несут.
#[derive(Clone, Debug, Default)]
pub(super) struct BoxFacts {
    pub(super) context: Option<Rc<BoxContext<WritingMode, Align>>>,
    pub(super) try_source: Option<SharedStyle>,
    pub(super) moved_from: Option<Rc<BoxStyle>>,
    pub(super) decor_box: Option<u64>,
    /// Block layout's used cross-axis placement of the box (`set_flow_align`).
    pub(super) flow_align: Option<Align>,
}

/// Equal box facts: shared parts by identity, the plain values by value. Block
/// layout sets `flow_align` on every layout copy of an element, so the copies
/// of two frames compare equal without sharing one allocation.
pub(super) fn same(a: &Option<Rc<BoxFacts>>, b: &Option<Rc<BoxFacts>>) -> bool {
    fn ptr<T>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            Rc::ptr_eq(a, b)
                || (ptr(&a.context, &b.context)
                    && ptr(&a.moved_from, &b.moved_from)
                    && a.try_source.is_none()
                    && b.try_source.is_none()
                    && a.decor_box == b.decor_box
                    && a.flow_align == b.flow_align)
        }
        _ => false,
    }
}

impl SharedStyle {
    fn edit_facts(&mut self, edit: impl FnOnce(&mut BoxFacts)) {
        let mut facts = self.box_facts.as_deref().cloned().unwrap_or_default();
        edit(&mut facts);
        self.box_facts = Some(Rc::new(facts));
    }

    /// The box of the element's parent in the element tree, when layout moved
    /// the element's box under another parent box (`dom::Marks::box_origin`).
    pub fn moved_from(&self) -> Option<Rc<BoxStyle>> {
        self.box_facts.as_deref().and_then(|f| f.moved_from.clone())
    }

    /// Remember the element-tree parent box of a moved box. A repeated move
    /// (a float lifted through several inline boxes) keeps the first one.
    pub fn set_moved_from(&mut self, tree: Rc<BoxStyle>) {
        self.edit_facts(|facts| {
            facts.moved_from.get_or_insert(tree);
        });
    }

    /// Keeps the facts of an existing synthetic box until A07.
    pub fn set_box_context(&mut self, context: Rc<BoxContext<WritingMode, Align>>) {
        self.edit_facts(|facts| facts.context = Some(context));
    }

    /// The cross-axis placement block layout gave this block-level box, if any.
    ///
    /// Our block container lays its children out as a flex column, so block
    /// layout expresses some CSS 2 placements as the column's cross-axis
    /// alignment: an overflowing fixed-width block in rtl flow hugs the right
    /// edge, an `aspect-ratio` box is fit-content, an orthogonal child stands
    /// at the inline-start. That is a used fact of the box, not `align-self`:
    /// the author's `align-self` stays in the computed values, where
    /// `getComputedStyle` reads it, and does not apply to a block-level box
    /// (css-align-3 §6.1). The box stage takes the placement in place of the
    /// author's value (`style::cascade::inherit::self_align`).
    pub fn flow_align(&self) -> Option<Align> {
        self.box_facts.as_deref().and_then(|f| f.flow_align)
    }

    /// Set the block layout placement (`flow_align`) of this box.
    pub fn set_flow_align(&mut self, align: Align) {
        self.edit_facts(|facts| facts.flow_align = Some(align));
    }
}
