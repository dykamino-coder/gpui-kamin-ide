//! Which values make the paint of a box read its `color` (`paint_reads_color`).

use super::*;
use crate::style::computed::{Outline, Shadow};
use crate::style::values::value::{Color, ColorOrAuto};

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

fn recoloured(a: &Computed) -> Computed {
    let mut b = a.clone();
    b.inh.ui.color = Some(RED);
    b
}

/// `a` paints like its recolouring exactly when it reads no `color`.
fn unread(a: &Computed) -> bool {
    let b = recoloured(a);
    assert!(a != &b);
    a.same_unread_color(&b)
}

#[test]
fn a_box_without_colour_values_paints_alike_in_any_colour() {
    let a = Computed::default();
    assert!(!a.paint_reads_color());
    assert!(unread(&a));
    // Default border colours are `currentcolor`, but no side is visible.
    assert!(
        a.reset_border
            .border_colors
            .iter()
            .all(StyleColor::is_current)
    );
    let mut other = recoloured(&a);
    other.reset_box_layout.margin.top = Some(crate::style::values::value::Len::Px(1.0));
    assert!(!a.same_unread_color(&other), "more than colour differs");
}

#[test]
fn visible_current_colour_borders_outlines_and_shadows_read_colour() {
    let mut border = Computed::default();
    border.reset_border.border_visible[2] = Some(true);
    assert!(!unread(&border), "a visible currentcolor side");
    border.reset_border.border_colors[2] = StyleColor::Absolute(RED);
    assert!(unread(&border), "the visible side has its own colour");

    let mut outline = Computed::default();
    outline.reset_border.outline = Some(Outline {
        style: Some(1),
        ..Outline::default()
    });
    assert!(!unread(&outline), "an outline in currentcolor");
    outline.reset_border.outline = Some(Outline {
        style: Some(1),
        color: ColorOrAuto::Color(StyleColor::Absolute(RED)),
        ..Outline::default()
    });
    assert!(unread(&outline));
    outline.reset_border.outline = Some(Outline::default());
    assert!(unread(&outline), "outline-style: none");

    let shadow = |color| Shadow {
        x: 1.0,
        y: 1.0,
        blur: 0.0,
        spread: 0.0,
        color,
    };
    let mut boxed = Computed::default();
    boxed.reset_effects.shadows = vec![shadow(StyleColor::Current)];
    assert!(!unread(&boxed), "a box-shadow without a colour");
    boxed.reset_effects.shadows = vec![shadow(StyleColor::Absolute(RED))];
    assert!(unread(&boxed));
}

#[test]
fn paint_resolved_colour_values_read_colour() {
    let mut background = Computed::default();
    background.reset_background.background_color = Some(StyleColor::Current);
    assert!(!unread(&background));

    let mut svg = Computed::default();
    svg.reset_svg_math.svg_fill = Some("currentColor".into());
    assert!(!unread(&svg), "fill: currentColor");
    svg.reset_svg_math.svg_fill = Some("#00f".into());
    svg.reset_svg_math.svg_stroke = Some("CURRENTCOLOR".into());
    assert!(!unread(&svg), "stroke: currentColor");

    let mut line = Computed::default();
    line.reset_text_box.td_lines = Some(1);
    assert!(!unread(&line), "text-decoration-color: currentcolor");

    let mut rule = Computed::default();
    rule.reset_fragmentation.column_rule_visible = Some(true);
    assert!(!unread(&rule), "a visible column rule");

    let mut scrollbar = Computed::default();
    scrollbar.reset_box_layout.scroll.colors =
        Some(Some([StyleColor::Current, StyleColor::Absolute(RED)]));
    assert!(!unread(&scrollbar), "scrollbar-color: currentcolor");

    let mut control = Computed::default();
    control.reset_context.appearance = Some(crate::style::computed::Appearance::Auto);
    assert!(!unread(&control), "a native control");
}
