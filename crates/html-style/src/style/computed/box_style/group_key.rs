//! The probe key of a box style as an input of conversion (`StyleConversions`).

use super::*;

impl BoxStyle {
    /// The identity of the shared groups of the computed style, with its flags: boxes
    /// whose groups are the same allocations have one key, and their computed values are
    /// equal but for `decor_box` (`same_values`). A probe key, not an equality: equal
    /// values in other allocations have another key, so a caller still compares on a hit
    /// and may look further on a miss. Stylo keys its rule cache by the interned rule node
    /// and Blink its matched properties cache by the identities of the matched
    /// declarations, for the same reason: a hash of identities instead of a walk of values.
    pub fn group_key(&self) -> u64 {
        crate::style::computed::identity::Storage::key(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_of_shared_groups_share_a_key_and_a_written_group_parts_them() {
        let shared = Rc::new(Computed::default());
        let a = BoxStyle::shared(shared.clone());
        let mut own = (*shared).clone();
        own.decor_box = 7;
        let b = BoxStyle::new(own.clone());
        assert_eq!(
            a.group_key(),
            b.group_key(),
            "decor_box is the box's identity"
        );
        own.reset_box_layout.margin.top = Some(crate::style::values::value::Len::Px(1.0));
        assert_ne!(a.group_key(), BoxStyle::new(own).group_key());
    }
}
