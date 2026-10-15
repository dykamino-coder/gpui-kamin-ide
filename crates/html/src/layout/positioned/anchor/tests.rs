//! Модульные проверки якорного позиционирования: `match-parent`.

use super::settle::match_parent;
use crate::style::computed::PositionAnchor;

#[test]
fn match_parent_takes_named_default_anchor_of_abspos_parent() {
    let p = Some((Some(PositionAnchor::Named("--a".into())), false, None));
    assert_eq!(
        match_parent(&p),
        (PositionAnchor::Named("--a".into()), None)
    );
}

#[test]
fn match_parent_without_abspos_parent_has_no_default_anchor() {
    assert_eq!(match_parent(&None), (PositionAnchor::None, None));
    let none = Some((Some(PositionAnchor::None), false, Some(7)));
    assert_eq!(match_parent(&none), (PositionAnchor::None, None));
}

#[test]
fn match_parent_inherits_implicit_anchor() {
    let auto = Some((Some(PositionAnchor::Auto), false, Some(7)));
    assert_eq!(match_parent(&auto), (PositionAnchor::Auto, Some(7)));
    // `normal` + `position-area` behaves as `auto`.
    let normal = Some((None, true, Some(9)));
    assert_eq!(match_parent(&normal), (PositionAnchor::Auto, Some(9)));
    let plain = Some((None, false, Some(9)));
    assert_eq!(match_parent(&plain), (PositionAnchor::None, None));
}
