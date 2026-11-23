//! Identity reuse shares a namespace without retaining the document owner.
use super::*;
use crate::dom::state_identity::{IdentityNamespace, StateIdentity};
use crate::values::DocumentValues;

#[test]
fn dirty_synthetic_boxes_share_only_the_allocator_and_keep_old_frame_identity() {
    let doc = DocumentValues::detached(Default::default());
    let owner = Rc::downgrade(&doc);
    let namespace = IdentityNamespace(doc.synthetic_ids());
    let input: Children = vec![node("a", vec![node("b", vec![])])].into();
    let mut pass = Pass::new(StateIdentity);
    let old = publish(&mut pass, &input, namespace.clone(), &Admission::default());
    assert_eq!(element(&old[0]).marks.state_id.get().unwrap(), "0");
    assert_eq!(
        element(&element(&old[0]).children[0])
            .marks
            .state_id
            .get()
            .unwrap(),
        "1"
    );
    drop(doc);
    assert!(owner.upgrade().is_none());
    let hit = publish(&mut pass, &input, namespace.clone(), &Admission::default());
    assert_eq!(hit.identity(), old.identity());
    let mut changed = input.clone();
    element_mut(&mut changed[0])
        .children
        .push(node("c", vec![]));
    let prepared = pass
        .prepare(&changed, namespace.clone(), &Admission::default())
        .unwrap();
    assert_eq!(
        element(&element(&prepared.output()[0]).children[1])
            .marks
            .state_id
            .get()
            .unwrap(),
        "2"
    );
    drop(prepared);
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
    let actual = publish(&mut pass, &changed, namespace, &Admission::default());
    assert_eq!(
        element(&actual[0]).marks.state_id.get(),
        element(&old[0]).marks.state_id.get()
    );
    assert_eq!(
        element(&element(&actual[0]).children[1])
            .marks
            .state_id
            .get()
            .unwrap(),
        "2"
    );
    assert_eq!(element(&old[0]).children.len(), 1);
}
