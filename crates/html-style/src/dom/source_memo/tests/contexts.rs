//! Deep shared input spines compare and release without recursive thread stacks.
use super::*;

fn link(parent: Option<Rc<CascadeContext>>, node: u64) -> Rc<CascadeContext> {
    Rc::new(CascadeContext {
        parent,
        node: NodeKey::from_packed(node),
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
