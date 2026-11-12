//! Сборка кусков абзаца из узлов (collect_with_empty_metrics) и сдвиги наложений.

mod element_nodes;
#[cfg(test)]
mod tests;
use crate::text::inline::collect::element_nodes::collect_element;

mod boundary_spacing;
mod overlays;
pub(crate) use overlays::to_rotated_frame;
mod text_nodes;
use crate::text::inline::collect::boundary_spacing::set_boundary_spacing;
pub(super) use crate::text::inline::collect::overlays::overlay_in_row;
use crate::text::inline::collect::text_nodes::collect_text;

use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use crate::text::inline::*;

/// What the piece collection carries across the inline boxes it descends:
/// the text-transform word state (CSS Text 3 §2.1) and the layout context of
/// the paragraph block (`LayoutCx`; its available width is the containing
/// block of inline-box percentages).
pub(crate) struct CollectCx {
    pub(super) case: text_case::Context,
    pub(super) layout: crate::layout::cx::LayoutCx,
    /// BoxStyle styles of the open inline ancestors (`atom_parent`): an atom
    /// inherits from the nearest one.
    pub(super) inline_parents: Vec<BoxStyle>,
    /// How many of them establish a containing block (positioned inlines).
    pub(super) inline_cb_depth: usize,
    /// A ruby container just closed on collapsible white space: that space
    /// swallows the leading collapsible space after the container
    /// (css-text-3 §4.1.1, across inline boundaries) and is itself discarded
    /// at the container's end (css-ruby-1 §2.4).
    pub(super) ruby_space: bool,
}

impl CollectCx {
    /// What an atom built at the current point learns from the collection.
    pub(super) fn atom_cx(&self) -> crate::text::inline::atom_parent::AtomCx {
        crate::text::inline::atom_parent::AtomCx {
            parent: self.inline_parents.last().cloned(),
            in_inline_cb: self.inline_cb_depth > 0,
        }
    }
}

pub(super) fn collect_with_empty_metrics(
    children: &[Node],
    inherited: &BoxStyle,
    atom: &mut dyn FnMut(&Element, &crate::text::inline::atom_parent::AtomCx) -> Option<Piece>,
    has_text: bool,
    cx: &mut CollectCx,
) -> Vec<Piece> {
    // Every paragraph collects its pieces on a stack of its own; a stack
    // keeps its last block once empty, so the build reuses them instead of
    // allocating two blocks per paragraph. Nested paragraphs (an inline block
    // inside a paragraph) take another stack from the pool.
    let mut stack = STACKS
        .with(|stacks| stacks.borrow_mut().pop())
        .unwrap_or_else(reblessive::Stack::new);
    let pieces = stack
        .enter(|stk| collect_stack(stk, children, inherited, atom, has_text, cx))
        .finish();
    STACKS.with(|stacks| stacks.borrow_mut().push(stack));
    pieces
}

thread_local! {
    /// Empty stacks of finished collections on this thread.
    static STACKS: std::cell::RefCell<Vec<reblessive::Stack>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

// Продолжения строчных предков хранятся в heap, как у блочного построителя.
async fn collect_stack(
    stk: &mut reblessive::Stk,
    children: &[Node],
    inherited: &BoxStyle,
    atom: &mut dyn FnMut(&Element, &crate::text::inline::atom_parent::AtomCx) -> Option<Piece>,
    has_text: bool,
    cx: &mut CollectCx,
) -> Vec<Piece> {
    let mut out = vec![];
    // Место последней распорки зазора за коробкой (см. ниже).
    let mut gap_at: Option<usize> = None;
    for child in children {
        // Куски ЭТОГО ребёнка: по ним ставится зазор на границе с тем, что
        // идёт следом (см. `set_boundary_spacing` ниже).
        let from = out.len();
        match child {
            Node::Text(t) => {
                let after_ruby_space = std::mem::take(&mut cx.ruby_space);
                let collapsible = inherited.inh.text_flow.keep_spaces != Some(true)
                    && inherited.inh.text_flow.preserve_newlines != Some(true);
                let t = if after_ruby_space && collapsible {
                    t.trim_start_matches([' ', '\t', '\n', '\r'])
                } else {
                    t
                };
                collect_text(t, inherited, &mut cx.case, &mut out);
            }
            Node::Element(e) => {
                stk.run(|stk| {
                    collect_element(
                        stk,
                        e,
                        inherited,
                        atom,
                        has_text,
                        cx,
                        &mut out,
                        &mut gap_at,
                        from,
                    )
                })
                .await;
                cx.ruby_space = if crate::text::ruby::ruby_role(e)
                    == Some(crate::style::computed::RubyRole::Container)
                {
                    matches!(e.children.last(), Some(Node::Text(t)) if t.ends_with(crate::dom::whitespace::collapsible))
                } else {
                    cx.ruby_space && out.len() == from
                };
            }
        }
    }
    // За последним куском границы внутри ЭТОГО узла нет: зазор там задаст тот
    // предок, у которого дальше идёт своё содержимое. Он и поставит его на
    // этот же кусок, когда сборка вернётся к нему.
    set_boundary_spacing(&mut out, None);
    if gap_at.is_some_and(|at| at + 1 == out.len()) {
        out.pop();
    }
    out
}
