//! Каскад сокращений раскладки: `flex`, `place-*`, `grid-area`, `columns`,
//! `column-rule` раскрыты разбором, и итог не зависит от порядка сокращения и
//! его длинного свойства в блоке.

use crate::document::state::DocState;
use crate::style::computed::{Align, Computed, Placement};
use crate::style::css::{DeclBlock, Media, parse_decls_in};
use crate::style::values::value::Len;

fn style(css: &str) -> Computed {
    let doc = DocState::new(Media::default());
    let block: DeclBlock = parse_decls_in(css, doc.values());
    let mut c = Computed::default();
    c.apply_decls(&block, doc.values());
    c
}

#[test]
fn shorthand_and_longhand_order_decides_by_position_only() {
    let a = style("flex: 1; flex-basis: 30px");
    assert_eq!(a.flex_basis, Some(Len::Px(30.0)));
    assert_eq!(a.flex_grow, Some(1.0));
    let b = style("flex-basis: 30px; flex: 1");
    assert_eq!(b.flex_basis, Some(Len::Pct(0.0)));
    let c = style("align-self: end; place-self: center");
    assert_eq!(c.align_self, Some(Align::Center));
    let d = style("place-self: center; align-self: end");
    assert_eq!(d.align_self, Some(Align::End));
    assert_eq!(d.justify_self, Some(Align::Center));
}

#[test]
fn grid_area_ident_is_four_longhands_and_names_the_area() {
    let c = style("grid-area: main");
    assert_eq!(c.grid_area_name.as_deref(), Some("main"));
    let d = style("grid-row: main; grid-column: main");
    assert_eq!(d.grid_area_name.as_deref(), Some("main"));
    let e = style("grid-area: main; grid-column-end: 3");
    assert_eq!(e.grid_area_name, None);
    assert_eq!(e.grid_col, Some((Placement::Auto, Placement::Line(3))));
    let f = style("grid-area: 2 / 1");
    assert_eq!(f.grid_row, Some((Placement::Line(2), Placement::Auto)));
    assert_eq!(f.grid_col, Some((Placement::Line(1), Placement::Auto)));
    assert_eq!(style("grid-row: auto").grid_row, None);
}

#[test]
fn columns_resets_height_and_wrap_and_rule_sets_both_axes() {
    let c = style("column-height: 10px; column-wrap: wrap; columns: 3");
    assert_eq!(c.column_count, Some(3));
    assert_eq!(c.column_height, None);
    assert_eq!(c.column_wrap, None);
    let r = style("rule: 4px solid; column-rule-width: 1px");
    assert_eq!(r.column_rule_width, Some(Len::Px(1.0)));
    assert_eq!(r.row_rule_width, Some(Len::Px(4.0)));
    assert_eq!(r.row_rule_visible, Some(true));
}

#[test]
fn grid_shorthand_resets_auto_flow_but_not_gaps() {
    let c = style("grid-auto-flow: column; gap: 5px; grid: 10px / 20px");
    assert_eq!(
        c.grid_auto_flow,
        Some(crate::style::computed::AutoFlow::Row)
    );
    assert_eq!(c.gap, Some((Some(Len::Px(5.0)), Some(Len::Px(5.0)))));
    let t = style("grid-template: 'a b' 20px / 1fr 1fr");
    assert_eq!(
        t.grid_areas,
        Some(vec![vec!["a".to_string(), "b".to_string()]])
    );
}
