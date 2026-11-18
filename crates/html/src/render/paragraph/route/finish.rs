//! The last step of a paragraph's build, also taken by a kept paragraph
//! (`layout::reuse::paragraph`): its place in this frame's scroll frame.

use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::paragraph::Paragraph;
use gpui::{AnyElement, IntoElement, SharedString};

/// The paragraph's text: the retained flow text when it is the same, so the
/// paragraph shares its buffer (`BoxTree::flow_text`); a copy otherwise.
pub(super) fn own_text(text: String, shared: Option<SharedString>) -> SharedString {
    match shared {
        Some(shared) if *shared == *text => shared,
        _ => SharedString::from(text),
    }
}

/// The paragraph with its scroll owner and projection of this frame.
pub(crate) fn finish(para: Paragraph, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    let projection = crate::interactive::scroll_extent::projection(
        opts.cx.scroll.frame.as_ref(),
        inherited.self_node,
        None,
        opts.cx.scroll.sticky.as_ref(),
    );
    let sticky = projection
        .as_ref()
        .map(|p| p.link().clone())
        .or_else(|| opts.cx.scroll.sticky.clone());
    crate::interactive::scroll_extent::project(
        para.scroll_owner(inherited.self_node, sticky, opts.cx.scroll.frame.clone())
            .into_any_element(),
        projection,
    )
}

/// The element of atom `e` of a kept paragraph, built as the paragraph's
/// build built it (`collect`, `layout::reuse::paragraph::AtomBuilder`). An
/// inert atom takes keys none of its output reaches: the frame's counters
/// stay where they were.
pub(crate) fn atom_builder(
    inherited: &BoxStyle,
    opts: &RenderOpts,
    flow_text: bool,
) -> crate::text::paragraph::AtomFn {
    use crate::render::paragraph::atom_piece::{AtomRuby, atom_piece};
    let (inherited, opts) = (inherited.clone(), opts.clone());
    Box::new(move |e| {
        let counters = crate::layout::reuse::Counters::read(&opts.cx);
        let ruby = AtomRuby::for_atom(e);
        let cx = crate::text::inline::atom_parent::AtomCx::default();
        let piece = atom_piece(e, &inherited, &opts, flow_text, &cx, &ruby);
        counters.write(&opts.cx);
        match piece? {
            crate::text::inline::Piece::Atom(el) => Some(el),
            _ => None,
        }
    })
}
