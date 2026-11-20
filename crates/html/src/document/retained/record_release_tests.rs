//! A changed block gives up its record before the new subtree is built.

use super::reuse_tests::{rows, settled};
use gpui::TestAppContext;

#[gpui::test]
fn a_changed_block_releases_its_record_before_it_is_built(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let recorded = store.recorded();
    assert!(recorded.len() > 400, "settled blocks were not recorded");
    let block = recorded.into_iter().find(|e| e.tag == "div").unwrap();
    assert!(
        store.same_tree(&block).is_some(),
        "an unchanged block lost its record"
    );
    // The same block after `Document::update` lost its text: the record can
    // never fit it, so it must not keep the previous subtree alive while the
    // new one is built.
    let mut changed = block.clone();
    changed.children = Vec::new().into();
    assert!(
        store.same_tree(&changed).is_none(),
        "a changed block fit its record"
    );
    assert!(
        store.same_tree(&block).is_none(),
        "the record of a changed block stayed in the store"
    );
}
