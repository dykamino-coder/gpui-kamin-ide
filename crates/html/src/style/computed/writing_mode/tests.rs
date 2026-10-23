//! Тесты отображения abstract-physical и запросов модели поворота у `WritingMode`.
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
fn block_start_follows_block_flow_direction() {
    assert_eq!(mode(false, true, false).block_start(), Side::Top);
    assert_eq!(mode(true, true, false).block_start(), Side::Right);
    assert_eq!(mode(true, false, false).block_start(), Side::Left);
    assert_eq!(mode(true, true, true).block_start(), Side::Right);
    assert_eq!(mode(true, false, true).block_start(), Side::Left);
}

#[test]
fn turned_line_clone_keeps_its_vertical_mode() {
    let clone = {
        let mut with_inherited = Computed {
            turned_line: true,
            ..Computed::default()
        };
        with_inherited.flat_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, false, true,
        ));
        with_inherited
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
    let flat = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.flat_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, true, true,
        ));
        with_inherited
    };
    let wm = flat.wm();
    assert!(!wm.vertical() && wm.block_rl() && wm.sideways());
    let child = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    assert!(child.wm().is_orthogonal_to(child.cb_wm));
}

#[test]
fn upright_sets_used_direction_ltr_only_in_vertical_typographic_mode() {
    let upright = |wm: WritingMode| WritingMode {
        rtl: true,
        orientation: TextOrientation::Upright,
        ..wm
    };
    assert!(!upright(mode(true, true, false)).used_direction().rtl());
    assert!(upright(mode(true, true, true)).used_direction().rtl());
    assert!(upright(mode(false, false, false)).used_direction().rtl());
}

#[test]
fn reversed_axes_follow_block_and_inline_start() {
    // (mode, rtl, [x reversed, y reversed])
    let rows = [
        (mode(false, false, false), false, [false, false]),
        (mode(false, false, false), true, [true, false]),
        (mode(true, true, false), false, [true, false]),
        (mode(true, false, false), true, [false, true]),
        (mode(true, false, true), false, [false, true]),
        (mode(true, false, true), true, [false, false]),
    ];
    for (m, rtl, axes) in rows {
        assert_eq!(
            WritingMode { rtl, ..m }.reversed_axes(),
            axes,
            "{m:?} rtl={rtl}"
        );
    }
}
