//! Сборка кусков абзаца из узлов (collect_with_empty_metrics) и сдвиги наложений.

mod element_nodes;
#[cfg(test)]
mod tests {
    use crate::dom::{Node, parse};
    use crate::layout::cx::{BuildDoc, LayoutCx};
    use crate::style::computed::BoxStyle;
    use crate::text::inline::{self, Piece};
    use gpui::IntoElement;

    #[test]
    fn positioned_inline_identity_reaches_anchor_and_absolute_atom() {
        let parsed = parse(
            "<span style='position:relative'><span style='anchor-name:--a'>a</span>\
             <span style='position:absolute;left:anchor(--a left)'></span></span>",
            "",
        );
        fn positioned(nodes: &[Node]) -> Option<&crate::dom::Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e)
                    if e.style.used().position
                        == Some(crate::style::computed::Position::Relative) =>
                {
                    Some(e)
                }
                Node::Element(e) => positioned(&e.children),
                Node::Text(_) => None,
            })
        }
        let parent = positioned(&parsed.nodes).expect("positioned inline parent");
        assert_ne!(parent.node_id, 0);
        let cx = LayoutCx {
            values: crate::layout::cx::ValueContext {
                doc: BuildDoc(parsed.doc.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut seen_atom = false;
        let pieces = inline::collect(
            &parsed.nodes,
            &BoxStyle::default(),
            &mut |e, atom| {
                if crate::layout::box_kind::out_of_flow(&e.style.used()) {
                    assert!(atom.in_inline_cb);
                    assert_eq!(atom.parent.as_ref().unwrap().self_node, parent.node_id);
                    seen_atom = true;
                    Some(Piece::Overlay(
                        gpui::Empty.into_any_element(),
                        Default::default(),
                    ))
                } else {
                    None
                }
            },
            &cx,
        );
        assert!(seen_atom);
        let anchor = pieces
            .iter()
            .find_map(|piece| match piece {
                Piece::Overlay(_, how) => how
                    .inline_box_info
                    .as_ref()
                    .filter(|info| !info.names.is_empty()),
                _ => None,
            })
            .expect("named inline anchor");
        assert_eq!(anchor.own_cb, parent.node_id);
    }
}
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
    let mut out = vec![];
    // Место последней распорки зазора за коробкой (см. ниже).
    let mut gap_at: Option<usize> = None;
    for child in children {
        // Куски ЭТОГО ребёнка: по ним ставится зазор на границе с тем, что
        // идёт следом (см. `set_boundary_spacing` ниже).
        let from = out.len();
        match child {
            Node::Text(t) => {
                collect_text(t, inherited, &mut cx.case, &mut out);
            }
            Node::Element(e) => {
                collect_element(
                    e,
                    inherited,
                    atom,
                    has_text,
                    cx,
                    &mut out,
                    &mut gap_at,
                    from,
                );
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
