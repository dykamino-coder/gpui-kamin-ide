//! Наследование вычисленных длин и действующего zoom через границу SVG/HTML.

use super::tests::element;
use super::*;
use crate::style::values::value::Len;

#[test]
fn foreign_object_preserves_effective_zoom_and_inherited_lengths() {
    let parsed = crate::dom::parse(
        r#"<style>
            :root { zoom: 4; font-size: 16px; line-height: 24px; letter-spacing: 2px }
        </style><svg width="50" height="25"><foreignObject width="50" height="25">
            <div><span style="font-size:16px">a</span>
                <b style="font-size:inherit">b</b><i style="font-size:1em">c</i>
                <p style="zoom:.5;font-size:100%">d</p></div>
        </foreignObject></svg>"#,
        "",
    );
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    // CSS Viewport: действующий zoom — произведение по всему дереву.
    // Граница координат SVG меняет геометрию покраски, а не наследование.
    for (tag, zoom) in [
        ("div", 4.0),
        ("span", 4.0),
        ("b", 4.0),
        ("i", 4.0),
        ("p", 2.0),
    ] {
        let style = element(&parsed.nodes, tag).style.computed();
        assert_eq!(style.zoom_eff, Some(zoom), "{tag}: effective zoom");
        assert_eq!(
            style.font_size,
            Some(Len::Px(16.0 * zoom)),
            "{tag}: font-size"
        );
        assert_eq!(
            style.line_height,
            Some(Len::Px(24.0 * zoom)),
            "{tag}: line-height"
        );
        assert_eq!(
            style.letter_spacing,
            Some(Len::Px(2.0 * zoom)),
            "{tag}: spacing"
        );
    }
}
