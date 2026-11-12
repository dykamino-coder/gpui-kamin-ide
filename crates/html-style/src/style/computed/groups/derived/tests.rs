use super::*;

struct Push;
struct Other;

#[test]
fn one_edit_of_one_group_is_shared() {
    let cache = DerivedGroups::default();
    let base = StyleGroup::new(vec![1]);
    let first = cache.derive::<Push, _>(&base, 2, |v| v.push(2));
    // Another owner handle of the same group hits the same entry.
    let alias = base.clone();
    let second = cache.derive::<Push, _>(&alias, 2, |v| v.push(2));
    drop(alias);
    assert!(first.ptr_eq(&second));
    assert_eq!(&*first, &[1, 2]);
    assert_eq!(&*base, &[1]);
    let third = cache.derive::<Push, _>(&base, 3, |v| v.push(3));
    assert!(!third.ptr_eq(&first));
    let other = cache.derive::<Other, _>(&base, 2, |v| v.push(2));
    assert!(!other.ptr_eq(&first));
    let distinct = StyleGroup::new(vec![1]);
    assert!(
        !cache
            .derive::<Push, _>(&distinct, 2, |v| v.push(2))
            .ptr_eq(&first)
    );
}

#[test]
fn one_edit_of_one_shared_value_is_shared_and_may_derive_groups() {
    let cache = DerivedGroups::default();
    let base = Rc::new((StyleGroup::new(vec![1]), 0u8));
    let edit = |v: &mut (StyleGroup<Vec<i32>>, u8)| {
        v.0 = cache.derive::<Push, _>(&v.0, 2, |g| g.push(2));
        v.1 = 1;
    };
    let first = cache.derive_shared::<Push, _>(&base, 2, edit);
    let second = cache.derive_shared::<Push, _>(&base.clone(), 2, edit);
    assert!(Rc::ptr_eq(&first, &second));
    assert_eq!((&first.0[..], first.1), (&[1, 2][..], 1));
    assert_eq!(cache.len(), 2, "the value and the group it derived");
    let distinct = Rc::new((StyleGroup::new(vec![1]), 0u8));
    let other = cache.derive_shared::<Push, _>(&distinct, 2, edit);
    assert!(!Rc::ptr_eq(&other, &first));
    assert!(!other.0.ptr_eq(&first.0), "another source group");
}

#[test]
fn frames_release_unused_entries() {
    let cache = DerivedGroups::default();
    let base = StyleGroup::new(vec![1]);
    let kept = cache.derive::<Push, _>(&base, 2, |v| v.push(2));
    let _dropped = cache.derive::<Push, _>(&base, 3, |v| v.push(3));
    cache.begin_frame();
    assert_eq!(cache.len(), 2);
    assert!(
        cache
            .derive::<Push, _>(&base, 2, |v| v.push(2))
            .ptr_eq(&kept)
    );
    cache.begin_frame();
    assert_eq!(cache.len(), 1);
    let other = StyleGroup::new(vec![5]);
    let _ = cache.derive::<Push, _>(&other, 2, |v| v.push(2));
    assert_eq!(cache.len(), 2);
    drop(other);
    cache.release_unowned();
    assert_eq!(cache.len(), 1);
}

#[test]
fn the_last_document_releases_entries_while_values_live() {
    let values = crate::values::DocumentValues::detached(Default::default());
    let base = StyleGroup::new(vec![1]);
    values.hold_document();
    let _ = values.derived.derive::<Push, _>(&base, 2, |v| v.push(2));
    values.hold_document();
    values.release_document();
    assert_eq!(values.derived.len(), 1);
    values.release_document();
    assert_eq!(values.derived.len(), 0);
}
