//! Проверки жизни назначений и изоляции проб.

use super::*;

#[test]
fn closed_destinations_keep_strict_and_fixed_routing_in_host_arena() {
    let p = Positioning::default();
    p.icb_open();
    p.cb_open_with(true);
    p.open_cb(ABS, Some(7));
    let retained = p.retain();
    let strict_host = p.cb_close();
    let fixed_host = p.cb_close();
    let icb_host = p.icb_close();
    let resumed = retained.resume();
    assert!(resumed.can_defer(false, 7, false));
    assert!(resumed.can_defer(true, 4, false));
    assert!(resumed.can_defer(false, 4, true));
    assert_eq!(resumed.cb.borrow()[1].strict, Some(7));
    assert_eq!(resumed.cb.borrow()[0].takes, ABS | FIX);
    use gpui::IntoElement;
    assert!(
        resumed
            .cb_push(Default::default(), gpui::div().into_any_element(), 7)
            .is_none()
    );
    assert!(
        resumed
            .cb_push_fixed(Default::default(), gpui::div().into_any_element())
            .is_none()
    );
    assert!(!resumed.cb.borrow()[1].taken.is_empty());
    assert!(!resumed.cb.borrow()[0].taken.is_empty());
    assert!(
        resumed
            .icb_push(Default::default(), gpui::div().into_any_element())
            .is_none()
    );
    assert!(!resumed.icb.borrow()[0].is_empty());
    drop(resumed);
    drop((strict_host, fixed_host, icb_host));
}

#[test]
fn retained_destinations_do_not_own_closed_queues() {
    let p = Positioning::default();
    p.icb_open();
    p.cb_open_with(true);
    let retained = p.retain();
    p.cb.borrow_mut().clear();
    p.icb.borrow_mut().clear();
    let expired = retained.resume();
    assert!(!expired.cb_active(false));
    assert!(!expired.icb_active());
}

#[test]
fn rejected_escaped_children_never_enter_the_host() {
    use gpui::IntoElement;
    let p = Positioning::default();
    p.icb_open();
    let retained = p.retain();
    let (trial, escaped) = retained.isolate();
    assert!(
        trial
            .icb_push(Default::default(), gpui::div().into_any_element())
            .is_none()
    );
    assert!(!trial.icb.borrow()[0].is_empty());
    assert!(p.icb.borrow()[0].is_empty());
    drop((trial, escaped));
    assert!(p.icb.borrow()[0].is_empty());
}

#[test]
fn selected_escaped_children_and_later_descendants_use_the_host() {
    use gpui::IntoElement;
    let p = Positioning::default();
    p.icb_open();
    let retained = p.retain();
    let (trial, escaped) = retained.isolate();
    let later = trial.retain();
    assert!(
        trial
            .icb_push(Default::default(), gpui::div().into_any_element())
            .is_none()
    );
    escaped.commit();
    assert!(!p.icb.borrow()[0].is_empty());
    assert!(trial.icb.borrow()[0].is_empty());
    let later = later.resume();
    assert!(
        later
            .icb_push(Default::default(), gpui::div().into_any_element())
            .is_none()
    );
    // Both the drained child and a subsequently built descendant reach one host.
    assert_eq!(
        p.icb.borrow()[0]
            .clone()
            .place(super::super::super::spot_place)
            .len(),
        3
    );
}

#[cfg(test)]
mod paint;
