//! Проверки поведения доменного модуля.
use super::*;

#[test]
fn float_exclusion_survives_float_layout_clearing_the_property() {
    let nodes = crate::document::parsed::parse(
        "<!DOCTYPE quirks-mode><style>body { float: left }</style><body></body>",
        "",
    );
    let crate::dom::Node::Element(html) = &nodes[0] else {
        panic!("html")
    };
    let body = html
        .children
        .iter()
        .find_map(|n| match n {
            crate::dom::Node::Element(e) if e.tag == "body" => Some(e),
            _ => None,
        })
        .expect("body");
    let mut body = body.clone();
    body.style.used_mut().reset_box_layout.float = None;
    let mut st = Computed::default();
    fill(&body, &mut st, &BoxStyle::default(), &opts());
    assert_eq!(st.reset_box_layout.min_height, None);
}

fn opts() -> RenderOpts {
    RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    }
}

#[test]
fn fill_is_a_content_minimum_and_does_not_make_auto_height_definite() {
    let mut body = crate::layout::table::anon::anon_element("body", vec![]);
    body.marks.quirks_fill = true;
    let mut st = Computed::default();
    st.reset_box_layout.margin.top = Some(Len::Px(10.0));
    st.reset_box_layout.margin.bottom = Some(Len::Px(12.0));
    st.reset_box_layout.padding.top = Some(Len::Px(5.0));
    let parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                height: Some(Len::Px(200.0)),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    fill(&body, &mut st, &BoxStyle::from(parent), &opts());
    assert_eq!(st.reset_box_layout.min_height, Some(Len::Px(173.0)));
    assert_eq!(st.reset_box_layout.height, None);
    assert_eq!(
        crate::layout::block::available_height::inner(&st, Some(200.0), Some(800.0), 600.0,),
        None
    );
}

#[test]
fn inline_floating_and_positioned_bodies_keep_their_normal_size() {
    let mut body = crate::layout::table::anon::anon_element("body", vec![]);
    body.marks.quirks_fill = true;
    for mut st in [
        Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::InlineBlock),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::InlineTable),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    float: Some(1),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        Computed {
            reset_positioned: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetPositioned {
                    position: Some(Position::Absolute),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        Computed {
            reset_positioned: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetPositioned {
                    position: Some(Position::Fixed),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
    ] {
        body.style = st.clone().into();
        fill(&body, &mut st, &BoxStyle::default(), &opts());
        assert_eq!(st.reset_box_layout.min_height, None);
    }
}

#[test]
fn vertical_fill_uses_width_and_preserves_border_box_minimum() {
    let mut body = crate::layout::table::anon::anon_element("body", vec![]);
    body.marks.quirks_fill = true;
    let doc = crate::document::state::DocState::new(Default::default());
    let mut st = Computed::default();
    st.apply_one("writing-mode", "vertical-lr", doc.values());
    st.apply_one("box-sizing", "border-box", doc.values());
    st.apply_one("padding", "5px", doc.values());
    st.apply_one("margin", "10px", doc.values());
    let parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                width: Some(Len::Px(200.0)),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    fill(&body, &mut st, &BoxStyle::from(parent), &opts());
    assert_eq!(st.reset_box_layout.min_width, Some(Len::Px(180.0)));
    assert_eq!(st.reset_box_layout.width, None);
}

#[test]
fn calc_size_auto_does_not_trigger_either_fill_axis() {
    let mut body = crate::layout::table::anon::anon_element("body", vec![]);
    body.marks.quirks_fill = true;
    let doc = crate::document::state::DocState::new(Default::default());
    for (mode, property) in [("horizontal-tb", "height"), ("vertical-lr", "width")] {
        let mut st = Computed::default();
        st.apply_one("writing-mode", mode, doc.values());
        st.apply_one(property, "calc-size(auto, size)", doc.values());
        fill(&body, &mut st, &BoxStyle::default(), &opts());
        assert_eq!(st.reset_box_layout.min_width, None);
        assert_eq!(st.reset_box_layout.min_height, None);
    }
}
