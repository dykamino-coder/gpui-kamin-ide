//! Проверки условий текстового листа.
use super::*;
use crate::style::computed::{Computed, Display, ResetBoxLayout, ResetPositioned, StyleGroup};

fn opts() -> RenderOpts {
    let mut opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.viewport = opts.viewport;
    opts
}

fn block(tag: &str, style: Computed, children: Vec<Node>) -> Element {
    Element::synthetic(tag, style, Default::default(), children)
}

fn text() -> Vec<Node> {
    vec![Node::Text("Row 1 alpha".into()), Node::Text(" ".into())]
}

fn with_display(display: Display) -> Computed {
    Computed {
        reset_box_layout: StyleGroup::new(ResetBoxLayout {
            display: Some(display),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn a_plain_block_of_text_is_a_leaf() {
    let parent = BoxStyle::default();
    let e = block("div", Computed::default(), text());
    assert!(candidate(&e, &parent, &opts()));
    assert!(straight(&e, &BoxStyle::default()));
    let empty = block("p", Computed::default(), vec![]);
    assert!(candidate(&empty, &parent, &opts()));
    assert!(straight(&empty, &BoxStyle::default()));
}

#[test]
fn element_children_special_tags_and_own_display_keep_the_general_route() {
    let parent = BoxStyle::default();
    let span = block("span", Computed::default(), vec![]);
    let mixed = block("div", Computed::default(), vec![Node::Element(span)]);
    assert!(!candidate(&mixed, &parent, &opts()));
    for tag in ["li", "table", "body", "textarea", "hr"] {
        assert!(!candidate(
            &block(tag, Computed::default(), text()),
            &parent,
            &opts()
        ));
    }
    let flex = block("div", with_display(Display::Flex), text());
    assert!(!candidate(&flex, &parent, &opts()));
}

#[test]
fn build_contexts_that_wrap_or_count_the_box_keep_the_general_route() {
    let parent = BoxStyle::default();
    let e = block("div", Computed::default(), text());
    let mut deep = opts();
    deep.cx.contracts.reflow_depth = crate::layout::depth::MAX_REFLOW_DEPTH;
    assert!(!candidate(&e, &parent, &deep));
    let mut other_viewport = opts();
    other_viewport.viewport = (640.0, 480.0);
    assert!(!candidate(&e, &parent, &other_viewport));
    let mut slots = opts();
    slots.cx.position.continuation_slots = Some(std::rc::Rc::from([7u64]));
    assert!(!candidate(&e, &parent, &slots));
}

#[test]
fn a_blank_node_that_ends_the_inline_run_keeps_the_general_route() {
    let merged = BoxStyle::default();
    let leading = block(
        "div",
        Computed::default(),
        vec![Node::Text("\n".into()), Node::Text("alpha".into())],
    );
    assert!(!straight(&leading, &merged));
    let trailing = block(
        "div",
        Computed::default(),
        vec![Node::Text("alpha".into()), Node::Text("\n".into())],
    );
    assert!(!straight(&trailing, &merged));
}

#[test]
fn merged_styles_with_their_own_layer_or_axis_keep_the_general_route() {
    let e = block("div", Computed::default(), text());
    let absolute = BoxStyle::from(Computed {
        reset_positioned: StyleGroup::new(ResetPositioned {
            position: Some(Position::Absolute),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(!straight(&e, &absolute));
    assert!(!straight(&e, &BoxStyle::from(with_display(Display::Grid))));
    let mut contained = Computed::default();
    contained.reset_context.contain_paint = Some(true);
    assert!(!straight(&e, &BoxStyle::from(contained)));
    let mut trimmed = Computed::default();
    trimmed.reset_text_box.text_box_trim_start = true;
    assert!(!straight(&e, &BoxStyle::from(trimmed)));
}
