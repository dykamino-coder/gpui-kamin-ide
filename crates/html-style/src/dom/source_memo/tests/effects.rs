//! Resuming raw cascade restores only detached effects into the current traversal.
use super::*;
use crate::style::generated::counters::Counters;
use crate::style::values::value::Len;
use std::cell::Cell;

#[test]
fn root_effect_survives_the_old_owner_and_restores_all_root_units() {
    for style in [
        "font-size:2rem;line-height:3;font-family:serif",
        "font-size:50vw;line-height:2",
    ] {
        let source = format!(
            "<!doctype html><html id=r style='{style}'><body><p id=p>text</p></body></html>"
        );
        let (input, old, cache, nodes) = build(&source);
        let effect = cache
            .raw()
            .get(handle(&input, "r").key())
            .unwrap()
            .effects
            .clone();
        assert!(effect.root.is_some());
        assert!(
            cache
                .raw()
                .get(handle(&input, "p").key())
                .unwrap()
                .effects
                .root
                .is_none()
        );
        let owner = Rc::downgrade(&old);
        drop((cache, nodes, old));
        assert!(owner.upgrade().is_none());
        let (_fresh_input, fresh, _fresh_cache, _fresh_nodes) = build(&source);
        let current = DocumentValues::detached(Media::default());
        let mut counters = Counters::default();
        assert!(effect.restore(current.values(), &mut counters, None));
        assert!(current.root.borrow().same_inputs(&fresh.root.borrow()));
        for unit in ["1rem", "1rlh", "1rcap", "1rch", "1rex", "1ric"] {
            assert_eq!(
                Len::parse(unit, current.values()),
                Len::parse(unit, fresh.values())
            );
        }
    }
}

#[test]
fn group_effect_resumes_the_next_sibling_counter_input_in_the_current_group() {
    let (input, values, cache, _) = build(
        "<nav id=g style='scroll-target-group:auto;counter-reset:n'>\
         <a id=a href='#x' style='counter-increment:n'>one</a>\
         <a id=b href='#y'>two</a></nav><p id=x>x</p><p id=y>y</p>",
    );
    let a = cache.raw().get(handle(&input, "a").key()).unwrap();
    let b = cache.raw().get(handle(&input, "b").key()).unwrap();
    let old_cell = Cell::new(false);
    let current_cell = Cell::new(false);
    let mut counters = a.inputs.counters.clone();
    assert!(a.effects.restore(
        values.values(),
        &mut counters,
        Some((handle(&input, "g").key(), &current_cell))
    ));
    assert!(current_cell.get());
    assert!(!old_cell.get());
    assert!(counters == b.inputs.counters);
}

#[test]
fn a_missing_current_group_leaves_all_outputs_untouched() {
    let (input, values, cache, _) =
        build("<!doctype html><html id=r style='font-size:32px'><body><p id=p>x</p></body></html>");
    let mut effects = cache
        .raw()
        .get(handle(&input, "r").key())
        .unwrap()
        .effects
        .clone();
    effects.selects_group = Some(handle(&input, "r").key());
    let current = DocumentValues::detached(Media::default());
    let before = current.root.borrow().clone();
    let mut counters = Counters::default();
    let original = counters.clone();
    let wrong_cell = Cell::new(false);
    assert!(!effects.restore(current.values(), &mut counters, None));
    assert!(!effects.restore(
        current.values(),
        &mut counters,
        Some((handle(&input, "p").key(), &wrong_cell))
    ));
    assert!(!wrong_cell.get());
    assert!(current.root.borrow().same_inputs(&before));
    assert!(counters == original);
    assert!(!current.root.borrow().same_inputs(&values.root.borrow()));
}
