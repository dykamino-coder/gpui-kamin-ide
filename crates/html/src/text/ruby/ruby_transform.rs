//! CSS Transforms 1 §transformable-element excludes non-replaced inline boxes.
//! Ruby units use block wrappers internally; that must not make them transformable.

use crate::style::computed::{Computed, Display, inh};
use crate::dom::Element;

pub(crate) fn used(e: &Element) -> Option<Element> {
    // A block ruby principal box remains transformable. Author display changes
    // that remove a ruby role likewise retain ordinary transform behavior.
    if crate::text::ruby::ruby_role(e).is_none() || e.style.display == Some(Display::Block) {
        return None;
    }
    let mut e = e.clone();
    clear(&mut e.style);
    Some(e)
}

pub(super) fn clear(style: &mut Computed) {
    style.transform = None;
    style.transform_raw = None;
    style.translate = None;
    style.rotate_prop = None;
    style.scale_prop = None;
    style.perspective = None;
    style.preserve_3d = None;
    style.inherit_bits &= !(inh::TRANSFORM | inh::TRANSFORM_ORIGIN);
}
