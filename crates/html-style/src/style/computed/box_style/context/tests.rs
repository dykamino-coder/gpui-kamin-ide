//! Факты родителя-коробки переживают копии, не меняя наследование элемента.

use super::*;
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::computed::Computed;
use std::rc::Rc;

#[test]
fn context_uses_the_box_parent_and_keeps_computed_inheritance() {
    for (display, grid, flex_grid) in [("grid", 3, true), ("flex", 0, true), ("block", 0, false)] {
        let parsed = crate::dom::parse(
            &format!(
                r#"<div style="display:{display};writing-mode:vertical-rl;font-size:10px"><section style="font-size:20px"><i>x</i></section></div>"#
            ),
            "",
        );
        let find =
            |tag: &str| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
        materialize(
            &parsed.nodes,
            &Rc::new(Computed::default()),
            parsed.values(),
        );
        let parent = box_style(&BoxStyle::default(), &find("div").style, parsed.values());
        let own = &find("i").style;
        let before = own.computed();
        let boxed = box_style(&parent, own, parsed.values());
        assert_eq!(boxed.context.parent_grid, grid);
        assert_eq!(boxed.context.parent_flex_grid, flex_grid);
        assert_eq!(boxed.context.cb_wm, parent.wm().line());
        assert_eq!(boxed.font_size, before.font_size);
        assert!(Rc::ptr_eq(&before, &own.computed()));
        let ordinary = box_style(&BoxStyle::default(), own, parsed.values());
        assert_eq!(ordinary.context.parent_grid, 0);
        assert!(!ordinary.context.parent_flex_grid);
    }
}

#[test]
fn layout_made_bridge_transports_the_same_context_without_computed_fields() {
    let parsed = crate::dom::parse(
        r#"<div style="display:grid;align-items:end;grid-template-columns:subgrid;writing-mode:vertical-lr"><span>x</span></div>"#,
        "",
    );
    let parent = crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "div").unwrap();
    let child = crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "span").unwrap();
    let parent = box_style(&BoxStyle::default(), &parent.style, parsed.values());
    let boxed = box_style(&parent, &child.style, parsed.values());
    let expected = boxed.context.clone();
    let bridge = crate::dom::SharedStyle::layout_made(boxed);
    let copy = bridge.clone();
    let restored = BoxStyle::shared(copy.used());
    assert_eq!(restored.context.parent_grid, expected.parent_grid);
    assert_eq!(
        restored.context.parent_grid_align_items,
        expected.parent_grid_align_items
    );
    assert_eq!(restored.context.parent_flex_grid, expected.parent_flex_grid);
    assert_eq!(restored.context.parent_lanes, expected.parent_lanes);
    assert_eq!(restored.context.cb_wm, expected.cb_wm);
    assert!(Rc::ptr_eq(restored.style_rc(), &bridge.used()));
    assert!(Rc::ptr_eq(restored.style_rc(), restored.clone().style_rc()));
    assert_eq!(
        restored.restyled(Computed::default()).context.cb_wm,
        expected.cb_wm
    );
}

#[test]
fn contents_keeps_element_inheritance_while_the_box_uses_the_flex_parent() {
    let parsed = crate::dom::parse(
        r#"<!doctype html><div style="display:flex;flex-wrap:wrap-reverse;direction:rtl;font-size:10px"><section style="display:contents;font-size:20px"><i style="align-self:normal">x</i></section></div>"#,
        "",
    );
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    let find =
        |tag: &str| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
    let parent = box_style(&BoxStyle::default(), &find("div").style, parsed.values());
    let own = &find("i").style;
    let boxed = box_style(&parent, own, parsed.values());
    assert!(boxed.context.applicability.flex);
    assert!(boxed.context.applicability.rtl);
    assert!(boxed.context.applicability.wrap_reverse);
    assert_eq!(boxed.font_size, find("section").style.computed().font_size);
    let mut bridge = crate::dom::SharedStyle::layout_made(boxed);
    bridge.used_mut().width = Some(crate::style::values::value::Len::Px(50.0));
    assert!(BoxStyle::shared(bridge.used()).context.applicability.flex);
    assert!(own.computed().width.is_none());
}

#[test]
fn anonymous_wrappers_replace_parent_facts_without_rewriting_the_element() {
    let parsed = crate::dom::parse(
        r#"<!doctype html><div style="display:grid;font-size:20px"><span>x</span></div>"#,
        "",
    );
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    let parent = crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "div").unwrap();
    let child = crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "span").unwrap();
    let parent = box_style(&BoxStyle::default(), &parent.style, parsed.values());
    let identity = child.style.computed();
    for (display, ruby_role) in [
        (Some(D::Table), None),
        (None, Some(crate::style::computed::RubyRole::BaseContainer)),
    ] {
        let anonymous = crate::style::cascade::inherit::inherit(
            &parent,
            &Computed {
                display,
                ruby_role,
                ..Computed::default()
            },
            parsed.values(),
        );
        assert!(anonymous.context.parent_flex_grid);
        let boxed = box_style(&anonymous, &child.style, parsed.values());
        assert!(!boxed.context.parent_flex_grid);
        assert_eq!(boxed.context.parent_grid, 0);
        assert_eq!(boxed.font_size, identity.font_size);
        assert!(Rc::ptr_eq(&child.style.computed(), &identity));
    }
}

#[test]
fn paint_input_copies_keep_the_context_and_shared_computed_allocation() {
    let mut boxed = BoxStyle::default();
    boxed.context.parent_flex_grid = true;
    boxed.context.parent_grid = 3;
    let input = crate::style::box_input::BoxStyleInput::from(&boxed);
    let copied = crate::style::box_input::BoxStyleInput::from(&input);
    let mut restored = BoxStyle::shared(copied);
    assert!(restored.context.parent_flex_grid);
    assert_eq!(restored.context.parent_grid, 3);
    assert!(Rc::ptr_eq(boxed.style_rc(), restored.style_rc()));
    restored.opacity = Some(0.5);
    assert!(restored.context.parent_flex_grid);
    assert_eq!(restored.context.parent_grid, 3);
    assert_eq!(boxed.opacity, None);
}

#[test]
fn quirks_height_preserves_materialized_values_and_box_ancestor_inputs() {
    use crate::style::values::value::Len;
    let parsed = crate::dom::parse(
        r#"<div style="height:200px"><section style="height:auto;direction:rtl"><i style="display:block;height:50%">x</i></section></div>"#,
        "",
    );
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    let find =
        |tag: &str| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
    let outer = box_style(&BoxStyle::default(), &find("div").style, parsed.values());
    let parent = box_style(&outer, &find("section").style, parsed.values());
    let own = &find("i").style;
    let identity = own.computed();
    let boxed = box_style(&parent, own, parsed.values());
    assert_eq!(parent.quirk_pct_base, Some(200.0));
    assert_eq!(boxed.height, Some(Len::Px(100.0)));
    assert_eq!(boxed.context.cb_wm, parent.wm().line());
    assert!(boxed.context.applicability.rtl);
    // Existing DOM quirks fixup resolves this percentage before materialization.
    assert_eq!(identity.height, Some(Len::Px(100.0)));
    assert!(Rc::ptr_eq(&identity, &own.computed()));
}
