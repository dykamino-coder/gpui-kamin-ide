//! Тесты: физические и логические стороны пишутся целиком или никак,
//! `inherit` — признаком стороны, место в каскаде — местом объявления.

use super::*;
use crate::style::computed::types::DeclOrder;

const KEYS: [&str; 4] = [
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];

fn padding(style: &Computed) -> [Option<Len>; 4] {
    let sides = style.padding;
    [sides.top, sides.right, sides.bottom, sides.left]
}

fn order(pass: u32) -> DeclOrder {
    DeclOrder {
        pass,
        ..DeclOrder::default()
    }
}

#[test]
fn physical_sides_keep_values_and_cascade_order() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    for (index, key) in KEYS.iter().enumerate() {
        style.decl_order = order(index as u32 + 1);
        style.apply_one(key, "25%", vcx);
        let mut expected = [None; 4];
        expected[..=index].fill(Some(Len::Pct(0.25)));
        assert_eq!(padding(&style), expected);
        assert_eq!(style.side_order.padding[index], style.decl_order);
    }
}

#[test]
fn invalid_longhands_keep_the_previous_side_and_order() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (index, key) in KEYS.iter().enumerate() {
        let mut style = Computed::default();
        style.decl_order = order(1);
        style.apply_one("padding", "7px", vcx);
        style.decl_order = order(2);
        for value in [
            "-1px", "-1%", "-1em", "-1ex", "-1ch", "auto", "stretch", "bogus",
        ] {
            style.apply_one(key, value, vcx);
            assert_eq!(padding(&style), [Some(Len::Px(7.0)); 4], "{key}: {value}");
            assert_eq!(style.side_order.padding, [order(1); 4]);
        }
        style.apply_one(key, "inherit", vcx);
        let marked = |s: &Computed| {
            [0, 1, 2, 3].map(|i| {
                s.inherit_longhands
                    .contains(LonghandId::physical(BoxGroup::Padding, i))
            })
        };
        let mut expected = [false; 4];
        expected[index] = true;
        assert_eq!(marked(&style), expected);
        assert_eq!(padding(&style), [Some(Len::Px(7.0)); 4]);
        assert_eq!(style.side_order.padding[index], order(2));
        // A later value clears the side's inheritance.
        style.apply_one(key, "3px", vcx);
        assert_eq!(marked(&style), [false; 4]);
    }
}

#[test]
fn every_padding_longhand_reports_its_parse_result() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for key in KEYS {
        for value in [
            "7px",
            "25%",
            "0%",
            "inherit",
            "calc(50% - 10px)",
            "calc(-5px)",
        ] {
            assert_eq!(
                Computed::declaration_result(key, value, vcx),
                ParseResult::Accepted,
                "{key}: {value}"
            );
        }
        for value in ["-1px", "-1%", "-1em", "-1ex", "-1ch", "invalid", "auto"] {
            assert_eq!(
                Computed::declaration_result(key, value, vcx),
                ParseResult::Invalid,
                "{key}: {value}"
            );
        }
    }
}

#[test]
fn side_grammars_follow_their_properties() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let ok = |key: &str, value: &str| {
        Computed::declaration_result(key, value, vcx) == ParseResult::Accepted
    };
    assert!(ok("margin-top", "auto") && ok("margin-top", "-5px"));
    assert!(ok("margin-top", "calc(10% + 5px)"));
    assert!(!ok("margin-top", "anchor(--a top)") && !ok("margin-top", "min-content"));
    assert!(ok("top", "auto") && ok("top", "anchor(--a top)") && ok("top", "-5px"));
    assert!(ok("inset-block-start", "10%") && !ok("inset-block-start", "fit-content"));
    assert!(ok("max-width", "none") && !ok("width", "none") && ok("width", "stretch"));
    assert!(ok("max-inline-size", "none") && !ok("inline-size", "none"));
    assert!(ok("margin", "1px 2px 3px") && !ok("margin", "1px 2px 3px 4px 5px"));
    assert!(ok("padding-block", "1px 2px") && !ok("padding-block", "1px -2px"));
    assert!(ok("margin", "inherit") && !ok("margin", "inherit 1px"));
}

#[test]
fn logical_sides_store_value_inheritance_and_order() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    style.decl_order = order(3);
    style.apply_one("margin-inline-start", "4px", vcx);
    style.apply_one("inset-block-end", "inherit", vcx);
    style.apply_one("padding-inline-end", "-1px", vcx);
    let logical = style.logical.clone().unwrap_or_default();
    assert_eq!(logical.margin.inline_start, Some(Len::Px(4.0)));
    assert_eq!(logical.margin.order[0], Some(order(3)));
    assert!(logical.inset.inherit[3] && logical.inset.order[3] == Some(order(3)));
    assert_eq!(logical.padding.order[1], None);
}
