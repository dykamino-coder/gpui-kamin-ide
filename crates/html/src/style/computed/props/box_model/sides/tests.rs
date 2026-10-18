//! Physical padding longhands share inheritance, validation and source ordering.

use super::*;

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

#[test]
fn physical_sides_preserve_values_and_source_order() {
    let mut style = Computed::default();
    for (index, key) in KEYS.iter().enumerate() {
        style.apply_one(key, "25%");
        let mut expected = [None; 4];
        expected[..=index].fill(Some(Len::Pct(0.25)));
        assert_eq!(padding(&style), expected);
        assert_eq!(style.side_seq.padding[index], style.decl_seq);
    }
    for (index, key) in KEYS.iter().enumerate() {
        style.apply_one(key, "0%");
        assert_eq!(padding(&style)[index], Some(Len::Pct(0.0)));
        assert_eq!(style.side_seq.padding[index], style.decl_seq);
    }
}

#[test]
fn negative_longhands_keep_the_previous_side_and_sequence() {
    for (index, key) in KEYS.iter().enumerate() {
        let mut style = Computed::default();
        style.apply_one("padding", "7px");
        let order = style.side_seq.padding;
        for value in ["-1px", "-1%", "-1em", "-1ex", "-1ch"] {
            style.apply_one(key, value);
            assert_eq!(padding(&style), [Some(Len::Px(7.0)); 4], "{key}: {value}");
            assert_eq!(style.side_seq.padding, order);
        }
        style.apply_one(key, "inherit");
        let mut expected = [false; 4];
        expected[index] = true;
        assert_eq!(style.padding_inherit_side, expected);
        assert_eq!(padding(&style), [Some(Len::Px(7.0)); 4]);
        assert_eq!(style.side_seq.padding, order);
    }
}
