//! Оформление inline переживает мост синтетических коробок без записи в CSS.

use super::*;
use crate::dom::SharedStyle;
use crate::style::box_input::BoxStyleInput;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Computed};
use std::rc::Rc;

#[test]
fn box_input_round_trip_preserves_decoration_and_shared_css() {
    let mut original = BoxStyle::default();
    let css = original.style_rc().clone();
    original.inline_decoration = InlineDecoration {
        band_color: Some(Color {
            r: 1.0,
            a: 1.0,
            ..Default::default()
        }),
        border_stroke: Some((Color::default(), [2.0; 4])),
        physical_padding: Some([1.0, 2.0, 3.0, 4.0]),
        corner_radius: Some(5.0),
    };
    let borrowed = BoxStyle::shared(BoxStyleInput::from(&original));
    let moved = BoxStyle::shared(BoxStyleInput::from(original.clone()));
    for boxed in [borrowed, moved] {
        assert_eq!(boxed.inline_decoration, original.inline_decoration);
        assert!(Rc::ptr_eq(&css, boxed.style_rc()));
        assert_eq!(**boxed.style_rc(), Computed::default());
    }
}

#[test]
fn synthetic_input_overrides_parent_band_without_projecting_css() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut parent = BoxStyle::default();
    parent.inline_decoration.physical_padding = Some([3.0; 4]);
    parent.inline_decoration.corner_radius = Some(7.0);
    parent.inline_decoration.band_color = Color::parse("red", doc.values());
    let mut child = BoxStyle::default();
    child.inline_decoration.band_color = Color::parse("blue", doc.values());
    let input = SharedStyle::layout_made(&child);
    let boxed = box_style(&parent, &input, doc.values());
    assert_eq!(
        boxed.inline_decoration.band_color,
        child.inline_decoration.band_color
    );
    assert_eq!(
        boxed.inline_decoration.physical_padding,
        parent.inline_decoration.physical_padding
    );
    assert_eq!(
        boxed.inline_decoration.corner_radius,
        parent.inline_decoration.corner_radius
    );
    assert!(boxed.background.is_none());
    assert!(input.used().background.is_none());
    assert_eq!(*input.computed(), Computed::default());
}
