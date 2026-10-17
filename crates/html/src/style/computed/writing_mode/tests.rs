//! Abstract-physical mapping and rotation-model queries of `WritingMode`.
use super::*;

fn mode(vertical: bool, rl: bool, sideways: bool) -> WritingMode {
    WritingMode::new(vertical, rl, sideways, false)
}

#[test]
fn inline_start_follows_line_left_and_direction() {
    // (mode, inline-start ltr, inline-start rtl)
    let rows = [
        (mode(false, false, false), Side::Left, Side::Right),
        (mode(true, true, false), Side::Top, Side::Bottom),
        (mode(true, false, false), Side::Top, Side::Bottom),
        (mode(true, true, true), Side::Top, Side::Bottom),
        (mode(true, false, true), Side::Bottom, Side::Top),
    ];
    for (m, ltr, rtl) in rows {
        assert_eq!(m.inline_start(), ltr, "{m:?}");
        assert_eq!(WritingMode { rtl: true, ..m }.inline_start(), rtl, "{m:?}");
    }
}

#[test]
fn turned_line_clone_keeps_its_vertical_mode() {
    let clone = Computed {
        flat_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, false, true,
        )),
        turned_line: true,
        ..Computed::default()
    };
    assert!(clone.wm().turned() && clone.wm().sideways());
    assert_eq!(clone.wm().line().line_turn(), Turn::CounterClockwise);
    assert_eq!(clone.wm().line_turn(), Turn::None);
}

#[test]
fn turn_and_line_order_follow_the_abstract_physical_mapping() {
    // (mode, turn, lines fed bottom-up)
    let rows = [
        (mode(false, false, false), Turn::None, false),
        (mode(true, true, false), Turn::Clockwise, false),
        (mode(true, false, false), Turn::Clockwise, true),
        (mode(true, true, true), Turn::Clockwise, false),
        (mode(true, false, true), Turn::CounterClockwise, false),
    ];
    for (m, turn, reversed) in rows {
        assert_eq!(m.line_turn(), turn, "{m:?}");
        assert_eq!(m.lines_reversed(), reversed, "{m:?}");
    }
}

#[test]
fn laid_flat_clone_keeps_block_flow_but_is_not_vertical() {
    let flat = Computed {
        flat_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, true, true,
        )),
        ..Computed::default()
    };
    let wm = flat.wm();
    assert!(!wm.vertical() && wm.block_rl() && wm.sideways());
    let child = Computed {
        writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, false, false,
        )),
        ..Computed::default()
    };
    assert!(child.wm().is_orthogonal_to(child.cb_wm));
}
