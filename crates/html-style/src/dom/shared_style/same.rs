//! Value equality of element styles across tree copies: shared stages compare
//! by pointer first, layout's used layer by value.

use super::SharedStyle;
use crate::style::computed::Computed;
use std::rc::Rc;

fn same_rc<T: PartialEq>(a: &Rc<T>, b: &Rc<T>) -> bool {
    Rc::ptr_eq(a, b) || **a == **b
}

fn same_ptr<T>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl SharedStyle {
    /// Whether every stage layout can read holds the same values. Box facts and
    /// inline decoration compare by identity only: a copy that carries its own
    /// is treated as different.
    pub fn same(&self, other: &Self) -> bool {
        let used = match (&self.used, &other.used) {
            (None, None) => true,
            (Some(a), Some(b)) => same_rc(a, b),
            _ => false,
        };
        used && same_rc(&self.cascaded, &other.cascaded)
            && same_rc(&self.base(), &other.base())
            && same_ptr(&self.box_facts, &other.box_facts)
            && same_ptr(&self.decoration, &other.decoration)
    }

    /// Like `same`, but the stages may differ in paint-only values
    /// (`Computed::same_layout`): layout reads the same values from both.
    pub fn same_layout(&self, other: &Self) -> bool {
        let layout = |a: &Rc<Computed>, b: &Rc<Computed>| Rc::ptr_eq(a, b) || a.same_layout(b);
        let used = match (&self.used, &other.used) {
            (None, None) => true,
            (Some(a), Some(b)) => layout(a, b),
            _ => false,
        };
        // A rebuilt tree carries its own decoration inputs: by value, as layout reads them.
        let decoration = match (&self.decoration, &other.decoration) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b) || a.same_layout(b),
            _ => false,
        };
        used && layout(&self.cascaded, &other.cascaded)
            && layout(&self.base(), &other.base())
            && same_ptr(&self.box_facts, &other.box_facts)
            && decoration
    }

    /// Whether the paint of the box resolves its colour in any stage or in
    /// its decoration input (`Computed::paint_reads_color`).
    pub fn paint_reads_color(&self) -> bool {
        let base = self.base();
        self.cascaded.paint_reads_color()
            || base.paint_reads_color()
            || self.used.as_ref().is_some_and(|u| u.paint_reads_color())
            || self
                .decoration
                .as_ref()
                .is_some_and(|d| d.paint_reads_color(&base))
    }

    /// Like `same`, but the stages may differ in `color` where the paint of
    /// the box does not read it (`Computed::same_unread_color`), its
    /// decoration input's ink included (`DecorationInput::same_unread_color`).
    pub fn same_unread_color(&self, other: &Self) -> bool {
        let unread =
            |a: &Rc<Computed>, b: &Rc<Computed>| Rc::ptr_eq(a, b) || a.same_unread_color(b);
        let used = match (&self.used, &other.used) {
            (None, None) => true,
            (Some(a), Some(b)) => unread(a, b),
            _ => false,
        };
        let base = self.base();
        let decoration = match (&self.decoration, &other.decoration) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b) || a.same_unread_color(b, &base),
            _ => false,
        };
        used && unread(&self.cascaded, &other.cascaded)
            && unread(&base, &other.base())
            && same_ptr(&self.box_facts, &other.box_facts)
            && decoration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    use crate::style::values::value::Len;

    #[test]
    fn copies_and_equal_used_layers_are_the_same_style() {
        let base = SharedStyle::from(Computed::default());
        assert!(base.same(&base.clone()));
        let (mut a, mut b) = (base.clone(), base.clone());
        a.used_mut().reset_box_layout.margin.top = Some(Len::Px(3.0));
        b.used_mut().reset_box_layout.margin.top = Some(Len::Px(3.0));
        assert!(a.same(&b));
        assert!(!a.same(&base));
    }
}
