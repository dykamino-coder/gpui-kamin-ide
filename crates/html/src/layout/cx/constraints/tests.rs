//! Проверки записи чтений ширин и базы ширины (`super`).

use super::*;

#[test]
fn only_consumed_outside_widths_are_recorded() {
    let sink = Rc::new(Cell::new(false));
    let mut outer = BoxConstraints::default();
    outer.set_avail_w(Some(800.0));
    outer.set_cb_width(Some(800.0));
    outer.track_widths(sink.clone());
    // A derived width stays outside without a read.
    let mut child = outer.clone();
    child.derive_avail_w(|w| w.map(|w| w - 10.0));
    assert!(!sink.get());
    // A width that does not change the result is not a read.
    assert_eq!(child.with_avail_w(|_| 1), 1);
    assert!(!sink.get());
    // A definite own width is the subtree's own.
    let mut own = child.clone();
    own.derive_avail_w(|_| Some(300.0));
    own.set_cb_width(Some(300.0));
    assert_eq!((own.avail_w(), own.cb_width()), (Some(300.0), Some(300.0)));
    assert!(!sink.get());
    assert_eq!(child.avail_w(), Some(790.0));
    assert!(sink.get());
    assert_eq!(child, {
        let mut plain = BoxConstraints::default();
        plain.set_avail_w(Some(790.0));
        plain.set_cb_width(Some(800.0));
        plain
    });
}

#[test]
fn a_width_basis_records_the_read_only_when_it_is_resolved() {
    let sink = Rc::new(Cell::new(false));
    let mut outer = BoxConstraints::default();
    outer.set_cb_width(Some(640.0));
    outer.track_widths(sink.clone());
    let read = || outer.cb_width();
    let basis = WidthBasis::Read(&read);
    assert!(!sink.get(), "passing the basis down reads nothing");
    assert_eq!(WidthBasis::Known(Some(5.0)).get(), Some(5.0));
    assert!(!sink.get());
    assert_eq!(basis.get(), Some(640.0));
    assert!(sink.get(), "a resolved outside width is recorded");
}
