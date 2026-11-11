//! Кэш одного прохода materialize: одинаковый каскад, родитель и query-container input.
//! Ключ держит владельцев групп, поэтому повторное использование адресов исключено.
use super::ComputedSlot;
use crate::style::computed::{Computed, Storage};
use crate::style::publication::{ComputedBuilder, Published};
use crate::values::ValueCx;
use std::rc::Rc;

const CAPACITY: usize = 32;

#[derive(Default)]
pub(super) struct Sharing {
    entries: [Option<Entry>; CAPACITY],
    next: usize,
}

struct Entry {
    own: Storage,
    parent: Rc<Computed>,
    size: (Option<u32>, Option<u32>),
    value: Published,
}

pub(super) fn size_key(size: (Option<f32>, Option<f32>)) -> (Option<u32>, Option<u32>) {
    (size.0.map(f32::to_bits), size.1.map(f32::to_bits))
}

impl Sharing {
    pub(super) fn compute(
        &mut self,
        slot: &ComputedSlot,
        parent: &Rc<Computed>,
        own: &Computed,
        vcx: ValueCx<'_>,
    ) -> Published {
        let size = size_key(slot.1.get());
        let recent = (1..=CAPACITY).map(|back| (self.next + CAPACITY - back) % CAPACITY);
        if let Some(hit) = recent
            .filter_map(|i| self.entries[i].as_ref())
            .find(|entry| {
                entry.size == size
                    && entry.own.matches(own)
                    && super::inherited::parent_eq(&entry.parent, parent, own)
            })
        {
            crate::work::add!(MaterializedSharingHits, 1);
            if hit.value.decor_box == own.decor_box {
                return hit.value.clone();
            }
            // DOM id нужен стадии коробки; computed CSS от него не зависит.
            let mut value = ComputedBuilder::from_cascaded(&hit.value);
            value.decor_box = own.decor_box;
            return value.finish();
        }
        let value = slot.compute(parent, own, vcx);
        self.entries[self.next] = Some(Entry {
            own: Storage::of(own),
            parent: parent.clone(),
            size,
            value: value.clone(),
        });
        self.next = (self.next + 1) % CAPACITY;
        value
    }
}

#[cfg(test)]
mod tests {
    //! Проверки входов, владения и изоляции результата кэша вычисленных свойств.
    use super::*;
    use crate::style::css::Media;
    use crate::values::DocumentValues;

    #[test]
    fn sharing_requires_equal_inherited_inputs_and_container_input() {
        let doc = DocumentValues::detached(Media::default());
        let vcx = doc.values();
        let mut cache = Sharing::default();
        let parent = Rc::new(Computed::default());
        let own = Computed::default();
        let first_slot = ComputedSlot::default();
        let first = cache.compute(&first_slot, &parent, &own, vcx);
        let second = cache.compute(&ComputedSlot::default(), &parent, &own.clone(), vcx);
        assert!(first.handle.ptr_eq(&second.handle));
        let mut changed_parent = (*parent).clone();
        changed_parent.reset_effects.opacity = Some(0.5);
        let distinct_parent = Rc::new(changed_parent);
        let third = cache.compute(&first_slot, &distinct_parent, &own, vcx);
        // Opacity не наследуется: одинаковых наследуемых входов достаточно.
        assert!(first.handle.ptr_eq(&third.handle));
        let equal_parent = Rc::new(Computed::default());
        let equal = cache.compute(&first_slot, &equal_parent, &own, vcx);
        assert!(first.handle.ptr_eq(&equal.handle));
        first_slot.set_container_size((Some(50.0), None));
        let fourth = cache.compute(&first_slot, &parent, &own, vcx);
        assert!(!first.handle.ptr_eq(&fourth.handle));
        let mut changed = own.clone();
        changed.reset_box_layout.width = Some(crate::style::values::value::Len::Px(10.0));
        let fifth = cache.compute(&ComputedSlot::default(), &parent, &changed, vcx);
        assert!(!first.handle.ptr_eq(&fifth.handle));
    }

    #[test]
    fn decoration_identity_keeps_css_groups_shared() {
        let doc = DocumentValues::detached(Media::default());
        let parent = Rc::new(Computed::default());
        let mut own = Computed::default();
        own.decor_box = 10;
        let mut cache = Sharing::default();
        let first = cache.compute(&ComputedSlot::default(), &parent, &own, doc.values());
        own.decor_box = 20;
        let mut cousin_parent = (*parent).clone();
        cousin_parent.decor_box = 99;
        let second = cache.compute(
            &ComputedSlot::default(),
            &Rc::new(cousin_parent),
            &own,
            doc.values(),
        );
        assert_eq!(first.decor_box, 10);
        assert_eq!(second.decor_box, 20);
        assert!(first.shares_storage(&second));
    }

    #[test]
    fn size_key_preserves_float_bits() {
        assert_ne!(size_key((Some(0.0), None)), size_key((Some(-0.0), None)));
    }

    #[test]
    fn explicit_reset_inheritance_never_reuses_a_different_parent_value() {
        for (property, first, second) in [
            ("opacity", ".2", ".8"),
            ("z-index", "2", "8"),
            ("padding-left", "3px", "9px"),
            ("text-overflow", "clip", "ellipsis"),
            ("unicode-bidi", "embed", "isolate"),
            ("box-shadow", "1px 2px red", "3px 4px blue"),
            ("background-blend-mode", "multiply", "screen"),
            ("scrollbar-width", "thin", "none"),
            ("scrollbar-gutter", "auto", "stable"),
            ("scroll-snap-type", "x mandatory", "y proximity"),
        ] {
            let parsed = crate::dom::parse(
                &format!(
                    "<section style=\"{property}:{first}\"><i style=\"{property}:inherit\">x</i></section>\
                     <article style=\"{property}:{second}\">y</article>"
                ),
                "",
            );
            let vcx = parsed.values();
            super::super::materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
            let element = |tag| super::super::tests::element(&parsed.nodes, tag);
            let left = element("section").style.slot().any().unwrap();
            let right = element("article").style.slot().any().unwrap();
            let own = element("i").style.cascaded();
            let slot = ComputedSlot::default();
            let mut cache = Sharing::default();
            let before = cache.compute(&slot, &left, own, vcx);
            let after = cache.compute(&slot, &right, own, vcx);
            let expected = slot.compute(&right, own, vcx);
            assert_ne!(
                *before, *expected,
                "{property}: test must change the result"
            );
            assert_eq!(*after, *expected, "{property}");
            // Тот же контракт защищает уже опубликованный слот после правки родителя.
            *slot.0.borrow_mut() = Some(Rc::new(super::super::Materialized {
                parent: left,
                generation: super::super::inputs(vcx),
                computed: before,
            }));
            assert!(slot.get(own, &right, super::super::inputs(vcx)).is_none());
        }
    }

    #[test]
    fn uniquely_owned_groups_can_repeat_through_a_shared_cascade_header() {
        let doc = DocumentValues::detached(Media::default());
        let mut own = Computed::default();
        own.reset_effects.opacity = Some(0.25);
        let own = Rc::new(own);
        let alias = own.clone();
        let parent = Rc::new(doc.initial_style());
        let mut cache = Sharing::default();
        let result = cache.compute(&ComputedSlot::default(), &parent, &own, doc.values());
        let repeated = cache.compute(&ComputedSlot::default(), &parent, &alias, doc.values());
        assert_eq!(result.reset_effects.opacity, Some(0.25));
        assert!(result.handle.ptr_eq(&repeated.handle));
    }

    #[test]
    fn inherited_changes_miss_and_repeated_restyle_inputs_share_again() {
        let doc = DocumentValues::detached(Media::default());
        let vcx = doc.values();
        let mut own = doc.initial_style();
        own.apply_one("width", "2em", vcx);
        let mut old_parent = doc.initial_style();
        old_parent.apply_one("font-size", "10px", vcx);
        let mut new_parent = old_parent.clone();
        new_parent.apply_one("font-size", "20px", vcx);
        let old_parent = Rc::new(old_parent);
        let new_parent = Rc::new(new_parent);
        let slot = ComputedSlot::default();
        let mut cold = Sharing::default();
        let before = cold.compute(&slot, &old_parent, &own, vcx);
        let mut restyle = Sharing::default();
        let after = restyle.compute(&slot, &new_parent, &own, vcx);
        let hit = restyle.compute(&ComputedSlot::default(), &new_parent, &own, vcx);
        let changed = restyle.compute(&ComputedSlot::default(), &old_parent, &own, vcx);
        assert_eq!(
            changed.reset_box_layout.width,
            before.reset_box_layout.width
        );
        assert!(!after.handle.ptr_eq(&changed.handle));
        assert_eq!(
            before.reset_box_layout.width,
            Some(crate::style::values::value::Len::Px(20.0))
        );
        assert_eq!(
            after.reset_box_layout.width,
            Some(crate::style::values::value::Len::Px(40.0))
        );
        assert!(after.handle.ptr_eq(&hit.handle));
    }
}
