//! Полоса inline берётся из результата коробки; CSS фон и радиусы не наследуются.

use super::*;
use crate::style::computed::Computed;
use crate::text::inline::runs::text_and_runs;
use std::rc::Rc;

#[test]
fn nested_inline_runs_keep_ancestor_band_and_original_css() {
    let parsed = crate::dom::parse(
        r#"<span style="background:red;padding:2px;border-radius:5px">a<b>b</b></span>"#,
        "",
    );
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        vcx,
    );
    let find = |tag| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
    let span = find("span");
    let bold = find("b");
    let original = span.style.computed();
    let (parent, painted) = element_style(span, &BoxStyle::default(), vcx);
    let (child, _) = element_style(bold, &parent, vcx);
    assert!(painted);
    assert_eq!(child.inline_decoration, parent.inline_decoration);
    assert!(child.background.is_none());
    assert!(child.radius.tl.is_none());
    assert_eq!(*span.style.computed(), *original);
    assert_eq!(parent.radius, original.radius);
    assert_eq!(parent.background, original.background);
    assert_eq!(parent.inline_decoration.corner_radius, Some(5.0));
    let pieces = vec![
        Piece::Text {
            text: "a".into(),
            style: parent,
        },
        Piece::Text {
            text: "b".into(),
            style: child,
        },
    ];
    let (text, runs) = text_and_runs(&pieces, &Default::default(), vcx).unwrap();
    assert_eq!(text, "ab");
    assert_eq!(runs[0].background_color, runs[1].background_color);
    assert!(runs[0].background_color.is_some());
    assert_eq!(runs[0].background_radius, gpui::px(5.0));
    assert_eq!(runs[0].background_pad, [gpui::px(2.0); 4]);
}

#[test]
fn inline_border_result_does_not_replace_currentcolor_css() {
    let parsed = crate::dom::parse(
        r#"<span style="color:blue;border:2px solid currentColor;border-radius:4px">ab</span>"#,
        "",
    );
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        vcx,
    );
    let span = crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == "span").unwrap();
    let before = span.style.computed();
    let (boxed, _) = element_style(span, &BoxStyle::default(), vcx);
    assert_eq!(
        boxed.inline_decoration.border_stroke,
        Some((Color::parse("blue", vcx).unwrap(), [2.0; 4]))
    );
    assert_eq!(boxed.border_colors, before.border_colors);
    assert_eq!(boxed.radius, before.radius);
    assert_eq!(*span.style.computed(), *before);
    // Прозрачная полоса сохраняет идентичность рамки без добавления CSS фона.
    assert_eq!(boxed.inline_decoration.band_color.unwrap().a, 0.0);
    assert!(boxed.background.is_none());
}

#[test]
fn splitting_a_run_keeps_band_geometry_independent_of_css() {
    use crate::style::inline_decoration::InlineDecoration;
    use crate::style::values::value::Len;
    use crate::text::inline::{Piece, style_first_line};
    let doc = crate::document::state::DocState::new(Default::default());
    let mut original = BoxStyle::default();
    original.background = Color::parse("red", doc.values());
    original.radius.tl = Some(Len::Px(11.0));
    original.inline_decoration = InlineDecoration {
        band_color: Color::parse("blue", doc.values()),
        physical_padding: Some([2.0; 4]),
        corner_radius: Some(5.0),
        ..Default::default()
    };
    let css = original.style_rc().clone();
    let pieces = style_first_line(
        vec![Piece::Text {
            text: "abcd".into(),
            style: original.clone(),
        }],
        2,
        &Computed::default(),
        &Computed::default(),
    );
    assert_eq!(pieces.len(), 2);
    for piece in &pieces {
        let Piece::Text { style, .. } = piece else {
            panic!("text split")
        };
        assert_eq!(style.inline_decoration, original.inline_decoration);
        assert_eq!(style.background, css.background);
        assert_eq!(style.radius, css.radius);
    }
    let (text, runs) =
        crate::text::inline::text_and_runs(&pieces, &Default::default(), doc.values()).unwrap();
    assert_eq!(text, "abcd");
    assert_eq!(runs.len(), 2);
    for run in runs {
        assert_eq!(
            run.background_color,
            original.inline_decoration.band_color.map(Color::to_hsla)
        );
        assert_eq!(run.background_radius, gpui::px(5.0));
        assert_eq!(run.background_pad, [gpui::px(2.0); 4]);
    }
    assert_eq!(original.radius.tl, Some(Len::Px(11.0)));
}

#[test]
fn filtered_inline_border_uses_box_colors_once_in_filter_order() {
    for (filter, expected) in [
        ("brightness(50%)", [0.5, 0.0, 0.0]),
        ("brightness(50%) brightness(50%)", [0.25, 0.0, 0.0]),
        ("brightness(50%) invert(100%)", [0.5, 1.0, 1.0]),
        ("invert(100%) brightness(50%)", [0.0, 0.5, 0.5]),
    ] {
        for ancestor in [false, true] {
            for border in ["red", "currentColor"] {
                let parent_filter = if ancestor { filter } else { "none" };
                let own_filter = if ancestor { "none" } else { filter };
                let parsed = crate::dom::parse(
                    &format!(
                        r#"<div style="filter:{parent_filter}"><span style="color:red;filter:{own_filter};border:2px solid {border};background:blue">ab</span></div>"#
                    ),
                    "",
                );
                let vcx = parsed.values();
                crate::style::cascade::materialize::materialize(
                    &parsed.nodes,
                    &Rc::new(Computed::default()),
                    vcx,
                );
                let find =
                    |tag| crate::dom::test_support::find(&parsed.nodes, &|e| e.tag == tag).unwrap();
                let parent = box_style(&BoxStyle::default(), &find("div").style, vcx);
                let span = find("span");
                let before = span.style.computed();
                let (boxed, _) = element_style(span, &parent, vcx);
                let ink = Color {
                    r: expected[0],
                    g: expected[1],
                    b: expected[2],
                    a: 1.0,
                };
                assert_eq!(
                    boxed.inline_decoration.border_stroke,
                    Some((ink, [2.0; 4])),
                    "{filter}, ancestor={ancestor}, border={border}"
                );
                assert_eq!(*span.style.computed(), *before);
                assert_eq!(boxed.borders(), before.borders());
            }
        }
    }
}
