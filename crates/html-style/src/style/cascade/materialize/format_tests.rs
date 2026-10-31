//! Проекция строчного контекста не меняет computed значения элементов;
//! размер окна участвует в вычислении кегля и в валидности материализации.

use super::tests::element;
use super::*;
use crate::style::computed::InlineFormat;
use crate::style::values::value::Len;

#[test]
fn inline_projection_stops_at_atomic_and_positioned_boxes() {
    for format in [
        InlineFormat::Turned,
        InlineFormat::Upright,
        InlineFormat::Combined,
    ] {
        let parsed = crate::dom::parse(
            r#"<div style="writing-mode:vertical-rl;font-size:20px;line-height:2"><span>x</span><b style="display:inline-block">y</b><i style="position:absolute">z</i></div>"#,
            "",
        );
        let vcx = parsed.values();
        materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
        let div = element(&parsed.nodes, "div");
        let mut parent = box_style(&BoxStyle::default(), &div.style, vcx);
        parent.inline_format = Some(format);
        parent.flat_mode = parent.writing_mode;
        parent.writing_mode = None;
        let span = element(&parsed.nodes, "span");
        let inline = box_style(&parent, &span.style, vcx);
        assert!(!inline.wm().vertical());
        assert!(span.style.computed().wm().vertical());
        assert_eq!(span.style.computed().line_height, Some(Len::Pct(2.0)));
        if format != InlineFormat::Turned {
            assert_eq!(inline.line_height, Some(Len::Px(20.0)));
        }
        for tag in ["b", "i"] {
            let own = element(&parsed.nodes, tag);
            assert!(box_style(&parent, &own.style, vcx).wm().vertical());
        }
    }
}

#[test]
fn ruby_text_indent_is_a_ua_default_and_author_values_still_cascade() {
    let parsed = crate::dom::parse(
        r#"<div style="text-indent:30px"><ruby>a<rt><span>b</span></rt></ruby><ruby style="text-indent:7px"><rb>c</rb></ruby></div>"#,
        "",
    );
    materialize(
        &parsed.nodes,
        &Rc::new(Computed::default()),
        parsed.values(),
    );
    assert_eq!(
        element(&parsed.nodes, "ruby").style.computed().text_indent,
        Some(Len::Px(0.0))
    );
    assert_eq!(
        element(&parsed.nodes, "span").style.computed().text_indent,
        Some(Len::Px(0.0))
    );
    assert_eq!(
        element(&parsed.nodes, "rb").style.computed().text_indent,
        Some(Len::Px(7.0))
    );
}

#[test]
fn viewport_font_size_is_absolute_before_inheritance_and_resize_invalidates_it() {
    for (css, expected) in [("10vh", 60.0), ("calc(10vh + 1em)", 80.0)] {
        let parsed = crate::dom::parse(
            &format!(
                r#"<div style="font-size:20px"><section style="font-size:{css}"><i style="font-size:2em">x</i></section></div>"#
            ),
            "",
        );
        let root = Rc::new(Computed::default());
        materialize(
            &parsed.nodes,
            &root,
            parsed.values().with_viewport((800.0, 600.0)),
        );
        let section = element(&parsed.nodes, "section");
        let i = element(&parsed.nodes, "i");
        assert_eq!(section.style.computed().font_size, Some(Len::Px(expected)));
        assert_eq!(i.style.computed().font_size, Some(Len::Px(2.0 * expected)));
        materialize(
            &parsed.nodes,
            &root,
            parsed.values().with_viewport((800.0, 800.0)),
        );
        assert_eq!(
            section.style.computed().font_size,
            Some(Len::Px(expected + 20.0))
        );
        assert_eq!(
            i.style.computed().font_size,
            Some(Len::Px(2.0 * (expected + 20.0)))
        );
    }
}
