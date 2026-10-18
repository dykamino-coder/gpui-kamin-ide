//! Share pseudo styles between DOM copies, isolating later used-value edits.

use crate::style::computed::Computed;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// A cascaded style with copy-on-write ownership for layout adjustments.
#[derive(Clone, Debug, Default)]
pub struct SharedStyle(Rc<Computed>);

impl From<Computed> for SharedStyle {
    fn from(style: Computed) -> Self {
        Self(Rc::new(style))
    }
}

impl Deref for SharedStyle {
    type Target = Computed;

    fn deref(&self) -> &Computed {
        &self.0
    }
}

impl DerefMut for SharedStyle {
    fn deref_mut(&mut self) -> &mut Computed {
        Rc::make_mut(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn layout_edits_do_not_change_a_shared_cascade() {
        let original = SharedStyle::from(Computed {
            width: Some(Len::Pct(0.5)),
            ..Computed::default()
        });
        let mut copy = original.clone();
        assert!(Rc::ptr_eq(&original.0, &copy.0));
        copy.width = Some(Len::Px(100.0));
        assert_eq!(original.width, Some(Len::Pct(0.5)));
        assert_eq!(copy.width, Some(Len::Px(100.0)));
        assert!(!Rc::ptr_eq(&original.0, &copy.0));
    }

    #[test]
    fn element_copies_share_pseudo_layers_and_keyframes() {
        let mut node = crate::layout::table::anon::anon_element("div", vec![]);
        node.hover = Some(Computed::default().into());
        node.first_letter = node.hover.clone();
        node.first_line = node.hover.clone();
        node.anim = Some(Rc::new(vec![(0.0, Computed::default())]));
        node.style.width = Some(Len::Pct(0.5));
        let mut copy = node.clone();
        copy.style.width = Some(Len::Px(100.0));
        assert_eq!(node.style.width, Some(Len::Pct(0.5)));
        assert!(Rc::ptr_eq(
            node.anim.as_ref().unwrap(),
            copy.anim.as_ref().unwrap()
        ));
        copy.hover.as_mut().unwrap().font_size = Some(Len::Px(32.0));
        assert!(node.hover.as_ref().unwrap().font_size.is_none());
        assert!(copy.first_letter.as_ref().unwrap().font_size.is_none());
        assert!(std::mem::size_of::<crate::dom::Element>() < 512);
    }
}
