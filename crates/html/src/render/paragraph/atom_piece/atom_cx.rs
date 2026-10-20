//! What the paragraph hands to the build of one inline atom (`LayoutCx`):
//! its ruby annotation collection, the line-box slot of an out-of-flow atom
//! and the offer of a positioned inline ancestor as its containing block.

use crate::dom::Element;
use crate::layout::box_kind::replaced_tag;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::LineBoxSlot;
use crate::style::computed::Display;
use crate::text::inline::InlineCbOffer;
use crate::text::paragraph::RubyExtents;
use std::cell::RefCell;
use std::rc::Rc;

/// Ruby collection of the atom being built: its annotation stacks
/// (`LayoutCx::ruby_extents`, closed when the atom is built) and whether it is
/// an independent inline context (atomic inline: no ruby base layers or sink).
pub(crate) struct AtomRuby {
    extents: Rc<RefCell<Option<RubyExtents>>>,
    independent: bool,
}

impl AtomRuby {
    /// A fresh collection for atom `e`.
    pub(crate) fn for_atom(e: &Element) -> Self {
        AtomRuby {
            extents: Rc::new(RefCell::new(Some(RubyExtents::default()))),
            independent: replaced_tag(e)
                || matches!(
                    e.style.display,
                    Some(
                        Display::InlineBlock
                            | Display::InlineFlex
                            | Display::InlineGrid
                            | Display::InlineTable
                    )
                ),
        }
    }

    /// Close the collection and take what it gathered.
    pub(crate) fn take_extents(&self) -> RubyExtents {
        self.extents.borrow_mut().take().unwrap_or_default()
    }
}

/// Options for building the atom.
pub(super) fn atom_opts(
    opts: &RenderOpts,
    ruby: &AtomRuby,
    line_box: &Option<LineBoxSlot>,
    offer: &Option<Rc<InlineCbOffer>>,
) -> Box<RenderOpts> {
    opts.inner(|cx| {
        cx.ruby_extents = Some(ruby.extents.clone());
        if ruby.independent {
            cx.ruby_layers = None;
            cx.ruby_base = None;
        }
        if let Some(slot) = line_box {
            cx.line_box = Some(slot.clone());
        }
        cx.inline_cb = offer.clone();
    })
}
