//! Retained cascade checkpoints keep counter and quote scopes independent.
use super::*;
#[test]
fn snapshots_keep_nested_counters_and_containment_boundaries() {
    let mut c = Counters::default();
    c.enter();
    c.reset("outer", 4);
    c.enter();
    c.reset("inner", 9);
    let mut before = c.clone();
    let scope = c.enter_style_scope();
    c.enter();
    c.update("outer", 20, false);
    c.reset("inner", 2);
    assert_eq!(c.value_of("outer"), 20);
    assert_eq!(c.chain_of("inner"), vec![9, 2]);
    assert_eq!(before.value_of("outer"), 4);
    assert_eq!(before.value_of("inner"), 9);
    let mut contained = c.clone();
    c.leave();
    c.leave_style_scope(scope);
    assert_eq!(c.value_of("outer"), 4);
    assert_eq!(c.value_of("inner"), 9);
    contained.update("inner", 5, false);
    assert_eq!(contained.value_of("inner"), 7);
    assert_eq!(before.value_of("inner"), 9);
    c.leave();
    c.enter();
    assert_eq!(c.value_of("inner"), 9);
}
#[test]
fn snapshots_keep_quote_depth_language_and_overrides() {
    let mut c = Counters::default();
    c.enter();
    c.set_quote_language("fr");
    assert_eq!(c.quote(true, true, None), "«");
    let mut old = c.clone();
    c.enter();
    c.set_quotes(Some(vec![("[".into(), "]".into())]));
    assert_eq!(c.quote(true, true, None), "[");
    assert_eq!(old.quote(false, true, None), "»");
    assert_eq!(c.quote(false, true, None), "]");
    c.leave();
    assert_eq!(c.quote(false, true, None), "»");
    assert_eq!(old.quote(false, true, None), "");
}
