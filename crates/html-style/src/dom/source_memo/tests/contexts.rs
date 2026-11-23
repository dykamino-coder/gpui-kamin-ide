//! Deep shared input spines compare and release without recursive thread stacks.
use super::*;

fn link(parent: Option<Rc<CascadeContext>>, node: u64) -> Rc<CascadeContext> {
    Rc::new(CascadeContext {
        parent,
        node: NodeKey::from_packed(node),
        tag: "div".into(),
        attrs: Vec::new(),
        selector: Default::default(),
        generated: GeneratedContext {
            counters: Default::default(),
            counter_style: Default::default(),
            content: None,
            content_none: None,
            language: None,
            target_group: false,
        },
    })
}

#[test]
fn deep_spines_compare_and_drop_on_a_bounded_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut left = link(None, 1);
            let mut right = link(None, 1);
            let root = Rc::downgrade(&left);
            for key in 2..20_000 {
                left = link(Some(left), key);
                right = link(Some(right), key);
            }
            assert!(*left == *right);
            drop(left);
            drop(right);
            assert!(root.upgrade().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn releasing_a_branch_preserves_a_shared_prefix_until_its_last_reader() {
    let root = link(None, 1);
    let weak = Rc::downgrade(&root);
    let left = link(Some(root.clone()), 2);
    let right = link(Some(root), 3);
    drop(left);
    assert!(weak.upgrade().is_some());
    assert_eq!(right.parent.as_ref().unwrap().node.packed(), 1);
    drop(right);
    assert!(weak.upgrade().is_none());
}

#[test]
fn ancestor_hints_change_cell_inputs_without_changing_generated_context() {
    use crate::style::values::value::Len;
    let (input, _old_values, before, _) = build(
        "<!doctype html><table id=t cellpadding=10><tr><td id=c>x</td></tr></table><p id=q>y</p>",
    );
    let cell = handle(&input, "c").key();
    let outside = handle(&input, "q").key();
    let table = handle(&input, "t");
    let NodeData::Element { attrs, .. } = &table.data else {
        panic!("table")
    };
    attrs
        .borrow_mut()
        .iter_mut()
        .find(|a| &*a.name.local == "cellpadding")
        .unwrap()
        .value = "20".into();
    let (_new_values, after, _) = recascade(&input);
    let old = before.raw().get(cell).unwrap();
    let new = after.raw().get(cell).unwrap();
    let old_parent = old.inputs.parent.as_ref().unwrap();
    let new_parent = new.inputs.parent.as_ref().unwrap();
    assert!(old_parent.generated == new_parent.generated);
    assert!(old_parent != new_parent);
    assert!(
        before.raw().get(outside).unwrap().inputs.parent
            == after.raw().get(outside).unwrap().inputs.parent
    );
    for (entry, pixels) in [(old, 10.0), (new, 20.0)] {
        let element =
            crate::dom::test_support::find(&entry.raw, &|e| e.node_id == cell.packed()).unwrap();
        assert_eq!(
            element.style.cascaded().reset_box_layout.padding.top,
            Some(Len::Px(pixels))
        );
    }
}

#[test]
fn stable_environment_is_shared_between_sibling_entries() {
    let (input, _values, cache, _) = build("<div><p id=a>x</p><p id=b>y</p></div>");
    let a = cache.raw().get(handle(&input, "a").key()).unwrap();
    let b = cache.raw().get(handle(&input, "b").key()).unwrap();
    assert!(Rc::ptr_eq(&a.inputs.environment, &b.inputs.environment));
}
