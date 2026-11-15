//! Used styles owned by the box tree: one shared value per (computed style, role).
//!
//! Layout derives a box's used style from its computed one (a spacer of a line
//! atom, the cell of a collapsed-border table). Writing through `BoxStyle` or
//! `SharedStyle::used_mut` copies the whole `Computed` for every box and every
//! frame. Boxes of one computed style (style sharing) and one role get the same
//! used style, so the tree keeps one value per pair, as Gecko keeps the
//! inheriting anonymous-box styles on their parent style.
//!
//! Keys are the address of the computed value, kept alive by the entry itself
//! (no address reuse while the entry exists). Entries whose computed value no
//! one else holds any more are dropped when the table is full.

use crate::style::computed::Computed;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What a used style is derived for. Each role is a pure function of the
/// computed style it is derived from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UsedRole {
    /// The spacer text run that holds an atomic inline's place in its line:
    /// no word or letter spacing (`render::paragraph::route::collect`).
    AtomSpacer,
}

const CAPACITY: usize = 4096;

struct Entry {
    base: Rc<Computed>,
    used: Rc<Computed>,
}

#[derive(Default)]
pub(crate) struct UsedStyles(RefCell<HashMap<(usize, UsedRole), Entry>>);

impl UsedStyles {
    /// The used style of `role` over `base`; `derive` runs once per pair.
    pub(crate) fn get(
        &self,
        base: &Rc<Computed>,
        role: UsedRole,
        derive: impl FnOnce(&mut Computed),
    ) -> Rc<Computed> {
        let key = (Rc::as_ptr(base) as usize, role);
        if let Some(entry) = self.0.borrow().get(&key) {
            return entry.used.clone();
        }
        let mut used = (**base).clone();
        derive(&mut used);
        let used = Rc::new(used);
        let mut table = self.0.borrow_mut();
        if table.len() >= CAPACITY {
            table.retain(|_, entry| Rc::strong_count(&entry.base) > 1);
            if table.len() >= CAPACITY {
                table.clear();
            }
        }
        table.insert(
            key,
            Entry {
                base: base.clone(),
                used: used.clone(),
            },
        );
        used
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.borrow().len()
    }

    #[cfg(feature = "heap-report")]
    pub(crate) fn heap_size(&self) -> usize {
        // The table and one header per used value; their groups are shared
        // with the computed styles (written groups are the few the role sets).
        let table = self.0.borrow();
        table.capacity() * size_of::<((usize, UsedRole), Entry)>()
            + table.len() * size_of::<Computed>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    fn spacer(style: &mut Computed) {
        style.inh.text_flow.letter_spacing = Some(Len::Px(0.0));
    }

    #[test]
    fn one_value_per_computed_style_and_role() {
        let table = UsedStyles::default();
        let base = Rc::new(Computed::default());
        let a = table.get(&base, UsedRole::AtomSpacer, spacer);
        let b = table.get(&base, UsedRole::AtomSpacer, |_| panic!("derived twice"));
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.inh.text_flow.letter_spacing, Some(Len::Px(0.0)));
        assert_eq!(base.inh.text_flow.letter_spacing, None);
        let other = Rc::new(Computed::default());
        let c = table.get(&other, UsedRole::AtomSpacer, spacer);
        assert!(!Rc::ptr_eq(&a, &c));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn a_full_table_drops_only_styles_no_box_holds() {
        let table = UsedStyles::default();
        let kept = Rc::new(Computed::default());
        let first = table.get(&kept, UsedRole::AtomSpacer, spacer);
        for _ in 1..CAPACITY {
            table.get(&Rc::new(Computed::default()), UsedRole::AtomSpacer, spacer);
        }
        table.get(&Rc::new(Computed::default()), UsedRole::AtomSpacer, spacer);
        assert_eq!(table.len(), 2, "only the held style and the new one stay");
        let again = table.get(&kept, UsedRole::AtomSpacer, |_| panic!("dropped"));
        assert!(Rc::ptr_eq(&first, &again));
    }
}
