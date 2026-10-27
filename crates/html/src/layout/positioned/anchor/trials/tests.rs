//! Выбор, инвалидация последнего успеха и изменение sizing до построения.

use super::*;
use crate::document::state::DocState;
use crate::style::computed::{Computed, Position};
use crate::style::css::parse_decls;
use crate::style::values::position_area::{FLIP_START, parse_try_fallbacks};
use crate::style::values::value::{AnchorSize, Len, anchor_get};

fn base() -> BoxStyle {
    let mut c = BoxStyle::new(Computed {
        position: Some(Position::Absolute),
        position_try_fallbacks: parse_try_fallbacks("--missing, --fit, flip-start"),
        ..Default::default()
    });
    c.anchor_key = 17;
    c
}

#[test]
fn original_indices_survive_missing_rules_and_remember_only_selected_properties() {
    let doc = DocState::new(Default::default());
    doc.try_rules
        .borrow_mut()
        .insert("--fit".into(), parse_decls("height: 71px; width: 29px"));
    let c = base();
    let opts = Options::new(&c, &BoxStyle::default(), doc.values(), &doc.layout);
    assert_eq!(
        opts.styles.iter().map(|s| s.0).collect::<Vec<_>>(),
        [0, 2, 3]
    );
    opts.record(1, doc.values());
    let next = Options::new(&c, &BoxStyle::default(), doc.values(), &doc.layout);
    assert_eq!(next.styles[0].0, 2);
    assert_eq!(next.styles[0].1.height, Some(Len::Px(71.)));
    assert_eq!(next.styles.iter().filter(|s| s.0 == 2).count(), 1);
    assert!(next.styles.iter().any(|s| s.0 == 0));
    let mut changed = c.clone();
    changed.width = Some(Len::Px(101.));
    assert_eq!(
        Options::new(&changed, &c, doc.values(), &doc.layout).styles[0].0,
        0
    );
    doc.try_rules
        .borrow_mut()
        .insert("--fit".into(), parse_decls("height: 72px"));
    assert_eq!(
        Options::new(&c, &c, doc.values(), &doc.layout).styles[0].0,
        0
    );
}

#[test]
fn all_overflow_keeps_current_and_sorting_keeps_source_order_on_ties() {
    let doc = DocState::new(Default::default());
    let mut c = base();
    c.position_try_order = 2;
    let opts = Options::new(&c, &c, doc.values(), &doc.layout);
    let p = |overflow, height| {
        Some(Placement {
            dx: 0.,
            dy: 0.,
            overflow,
            imcb: (100., height),
            cell: (0., 0.),
        })
    };
    assert_eq!(
        opts.choose_placements(&[p(true, 5.), p(true, 9.), p(true, 12.)]),
        0
    );
    assert_eq!(
        opts.choose_placements(&[p(true, 5.), p(false, 9.), p(false, 9.)]),
        1
    );
    assert_eq!(
        opts.choose_placements(&[p(true, 5.), p(false, 9.), p(false, 12.)]),
        2
    );
    assert_eq!(opts.choose_placements(&[p(false, 5.), p(false, 9.)]), 1);
    opts.record(1, doc.values());
    let remembered = Options::new(&c, &c, doc.values(), &doc.layout);
    assert_eq!(
        remembered.choose_placements(&[p(false, 5.), p(false, 9.)]),
        0
    );
}

#[test]
fn flip_start_transposes_sizing_inputs_and_anchor_size_before_layout() {
    let doc = DocState::new(Default::default());
    let mut c = Computed {
        width: Len::parse("anchor-size(--a width, 13px)", doc.values()),
        height: Some(Len::Px(41.)),
        min_width: Some(Len::Px(7.)),
        max_height: Some(Len::Px(83.)),
        ..Default::default()
    };
    c.calc_size[0] = Some((1., 2., 3., 4.));
    c.inherit_longhands
        .set(crate::style::css::LonghandId::Width, true);
    c.stretch_size[2] = true;
    tactics::apply(&mut c, &Computed::default(), FLIP_START, doc.values());
    assert_eq!(c.width, Some(Len::Px(41.)));
    assert_eq!(c.min_height, Some(Len::Px(7.)));
    assert_eq!(c.max_width, Some(Len::Px(83.)));
    assert_eq!(c.calc_size[1], Some((1., 2., 3., 4.)));
    assert!(
        c.inherit_longhands
            .contains(crate::style::css::LonghandId::Height)
    );
    assert!(c.stretch_size[4]);
    let Some(Len::Anchor(id)) = c.height else {
        panic!("anchor-size expected")
    };
    let f = anchor_get(id, doc.values()).expect("owned arena entry");
    assert_eq!(f.size, Some(AnchorSize::Height));
    assert_eq!(f.fallback, Some(Len::Px(13.)));
}

#[test]
fn candidate_style_changes_only_accepted_properties() {
    let mut raw = Computed::default();
    let mut computed = Computed {
        width: Some(Len::Px(42.)),
        ..Default::default()
    };
    computed.font_size = Some(Len::Px(99.));
    use crate::style::css::LonghandId as L;
    computed.inherit_longhands.set(L::JustifySelf, true);
    computed.inherit_longhands.set(L::ColorScheme, true);
    raw.inherit_longhands.set(L::BackgroundColor, true);
    let font = raw.font_size;
    apply_style(&mut raw, &computed);
    assert_eq!(raw.width, computed.width);
    assert_eq!(raw.font_size, font);
    assert!(!raw.inherit_longhands.contains(L::JustifySelf));
    assert!(raw.inherit_longhands.contains(L::BackgroundColor));
    assert!(!raw.inherit_longhands.contains(L::ColorScheme));
}

#[test]
fn transposed_inherit_keeps_the_value_of_the_original_axis() {
    let doc = DocState::new(Default::default());
    doc.try_rules
        .borrow_mut()
        .insert("--fit".into(), parse_decls("width: inherit"));
    let mut c = base();
    c.position_try_fallbacks = parse_try_fallbacks("--fit flip-start");
    let parent = BoxStyle::new(Computed {
        width: Some(Len::Px(73.)),
        height: Some(Len::Px(19.)),
        ..Default::default()
    });
    let opts = Options::new(&c, &parent, doc.values(), &doc.layout);
    let mut raw = Computed::default();
    apply_style(&mut raw, &opts.styles[1].1);
    let own = crate::style::cascade::inherit::inherit(&parent, &raw, doc.values());
    assert_eq!(own.height, Some(Len::Px(73.)));
    assert!(
        !raw.inherit_longhands
            .contains(crate::style::css::LonghandId::Height)
    );
}

#[test]
fn opposite_axis_flips_anchor_percent_but_transpose_preserves_it() {
    use crate::style::values::position_area::FLIP_BLOCK;
    let doc = DocState::new(Default::default());
    let mut c = Computed::default();
    c.inset.top = Len::parse("anchor(--a 25%)", doc.values());
    tactics::apply(&mut c, &Computed::default(), FLIP_BLOCK, doc.values());
    let Some(Len::Anchor(id)) = c.inset.bottom else {
        panic!("anchor inset expected")
    };
    assert_eq!(
        anchor_get(id, doc.values()).expect("arena entry").side,
        crate::style::values::value::AnchorSide::Pct(0.75)
    );
    let mut c = Computed::default();
    c.inset.top = Len::parse("anchor(--a 25%)", doc.values());
    tactics::apply(&mut c, &Computed::default(), FLIP_START, doc.values());
    let Some(Len::Anchor(id)) = c.inset.left else {
        panic!("transposed anchor inset expected")
    };
    assert_eq!(
        anchor_get(id, doc.values()).expect("arena entry").side,
        crate::style::values::value::AnchorSide::Pct(0.25)
    );
}
