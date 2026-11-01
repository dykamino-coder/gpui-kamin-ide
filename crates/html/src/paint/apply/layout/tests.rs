//! Контракт paint: прежняя нормализация не меняет опубликованный стиль.

use super::*;
use crate::style::computed::BoxStyle;
use crate::style::css::parse_decls;

#[test]
fn legacy_block_alignment_matches_normalized_input_without_style_writes() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    for display in ["flex", "grid"] {
        for mode in ["horizontal-tb", "vertical-rl"] {
            let mut input = Computed::default();
            input.apply_decls(
                &parse_decls(&format!(
                    "display:{display};writing-mode:{mode};align-items:stretch;justify-content:center"
                )),
                doc.values(),
            );
            input.reset_flex.webkit_box = Some(true);
            input.inh.text_flow.webkit_box_vertical = Some(true);
            input.reset_fragmentation.line_clamp = Some(2);
            let mut normalized = input.clone();
            normalized.reset_flex.align_items = None;
            normalized.reset_flex.justify_content = None;
            let input = BoxStyle::from(input);
            let mut actual = apply_layout(gpui::div(), &input, doc.values());
            let mut expected = apply_layout(gpui::div(), &BoxStyle::from(normalized), doc.values());
            assert_eq!(actual.style().align_items, expected.style().align_items);
            assert_eq!(
                actual.style().justify_content,
                expected.style().justify_content
            );
            assert_eq!(input.reset_flex.align_items, Some(Align::Stretch));
            assert_eq!(input.reset_flex.justify_content, Some(Justify::Center));
        }
    }
}

#[test]
fn transferred_constraints_reach_native_style_without_replacing_css_inputs() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let input = BoxStyle::from(Computed {
        reset_shapes: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetShapes {
                aspect_ratio: Some(2.0),
                ..Default::default()
            },
        ),
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                min_height: Some(Len::Px(20.0)),
                max_width: Some(Len::Px(200.0)),
                ..Default::default()
            },
        ),
        ..Computed::default()
    });
    let mut actual = apply_layout(gpui::div(), &input, doc.values());
    assert_eq!(actual.style().min_size.width, Some(px(40.0).into()));
    assert_eq!(actual.style().max_size.height, Some(px(100.0).into()));
    assert_eq!(input.reset_box_layout.min_width, None);
    assert_eq!(input.reset_box_layout.max_height, None);
}

#[test]
fn natural_probe_drops_height_constraints_before_ratio_transfer() {
    let doc = crate::document::state::DocState::new(Default::default());
    let css = BoxStyle::from(Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                width: Some(Len::Px(80.)),
                height: Some(Len::Px(100.)),
                min_height: Some(Len::Px(40.)),
                max_height: Some(Len::Px(200.)),
                ..Default::default()
            },
        ),
        reset_shapes: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetShapes {
                aspect_ratio: Some(2.),
                ..Default::default()
            },
        ),
        ..Default::default()
    });
    let sizes = BoxDimensions::from_computed(&css).unconstrained_height();
    let mut actual = apply_layout_with_dimensions(gpui::div(), &css, sizes, doc.values());
    let mut natural = css.clone();
    natural.reset_box_layout.height = None;
    natural.reset_box_layout.min_height = None;
    natural.reset_box_layout.max_height = None;
    let mut expected = apply_layout(gpui::div(), &natural, doc.values());
    assert_eq!(actual.style().size, expected.style().size);
    assert_eq!(actual.style().min_size, expected.style().min_size);
    assert_eq!(actual.style().max_size, expected.style().max_size);
    assert_eq!(actual.style().min_size.width, None);
    assert_eq!(css.reset_box_layout.min_height, Some(Len::Px(40.)));
    assert_eq!(css.reset_box_layout.max_height, Some(Len::Px(200.)));
}

#[test]
fn explicit_content_dimensions_choose_compensation_and_native_percentages() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut css = BoxStyle::from(Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                height: Some(Len::Px(200.)),
                min_height: Some(Len::Px(150.)),
                ..Default::default()
            },
        ),
        reset_border: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBorder {
                border_box: Some(true),
                ..Default::default()
            },
        ),
        ..Default::default()
    });
    css.reset_box_layout.padding.top = Some(Len::Px(4.));
    css.reset_box_layout.padding.bottom = Some(Len::Px(6.));
    let mut sizes = BoxDimensions::from_computed(&css).unconstrained_height();
    sizes.border_box = false;
    sizes.preferred[1] = Some(Len::Px(80.));
    let mut actual = apply_layout_with_dimensions(gpui::div(), &css, sizes, doc.values());
    assert_eq!(actual.style().size.height, Some(px(90.).into()));
    assert_eq!(actual.style().min_size.height, None);
    sizes.preferred[1] = Some(Len::Pct(0.5));
    let mut native = apply_layout_with_dimensions(gpui::div(), &css, sizes, doc.values());
    assert_eq!(native.style().content_box, Some(true));
    assert_eq!(css.reset_box_layout.height, Some(Len::Px(200.)));
    assert_eq!(css.reset_border.border_box, Some(true));
}

#[test]
fn explicit_box_sizing_controls_ratio_minimum() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut css = BoxStyle::from(Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                width: Some(Len::Px(80.)),
                height: Some(Len::Px(200.)),
                min_height: Some(Len::Px(150.)),
                ..Default::default()
            },
        ),
        reset_shapes: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetShapes {
                aspect_ratio: Some(2.),
                ..Default::default()
            },
        ),
        reset_border: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBorder {
                border_box: Some(true),
                ..Default::default()
            },
        ),
        ..Default::default()
    });
    css.reset_box_layout.padding.top = Some(Len::Px(4.));
    css.reset_box_layout.padding.bottom = Some(Len::Px(6.));
    let mut sizes = BoxDimensions::from_computed(&css).unconstrained_height();
    sizes.border_box = false;
    let mut actual = apply_layout_with_dimensions(gpui::div(), &css, sizes, doc.values());
    assert_eq!(actual.style().min_size.height, Some(px(50.).into()));
    assert_eq!(css.reset_box_layout.height, Some(Len::Px(200.)));
    assert_eq!(css.reset_border.border_box, Some(true));
}

#[test]
fn explicit_content_box_controls_calc_size_and_gap_basis() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut css = Computed::default();
    css.apply_decls(
        &parse_decls("width:calc-size(max-content, size + 10px);height:200px;min-height:150px;box-sizing:border-box;padding-top:4px;padding-bottom:6px;gap:calc(50% + 1px)"),
        doc.values(),
    );
    assert!(css.reset_box_layout.calc_size.iter().any(Option::is_some));
    let css = BoxStyle::from(css);
    let mut sizes = BoxDimensions::from_computed(&css).unconstrained_height();
    sizes.border_box = false;
    sizes.preferred[1] = Some(Len::Px(80.));
    let mut actual = apply_layout_with_dimensions(gpui::div(), &css, sizes, doc.values());
    assert_eq!(actual.style().calc_size_content_box, Some(true));
    assert_eq!(actual.style().gap.height, Some(px(41.).into()));
    assert_eq!(css.reset_box_layout.height, Some(Len::Px(200.)));
    assert_eq!(css.reset_box_layout.min_height, Some(Len::Px(150.)));
    assert_eq!(css.reset_border.border_box, Some(true));
}
