//! Краска вычисленного цвета элемента: фильтр и заливка текста применяются один раз.

use super::tests::element;
use super::*;

#[test]
fn inherited_element_color_is_filtered_once_per_box() {
    let parsed = crate::dom::parse(
        r#"<body><div style="color:red;filter:brightness(50%)"><span><i>x</i></span></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let mut parent = BoxStyle::default();
    for tag in ["div", "span", "i"] {
        let node = element(&parsed.nodes, tag);
        assert_eq!(node.style.computed().inh.ui.color.unwrap().r, 1.0);
        parent = box_style(&parent, &node.style, vcx);
        assert_eq!(parent.ink().unwrap().r, 0.5);
        assert_eq!(parent.inh.ui.color.unwrap().r, 1.0);
    }
}

#[test]
fn text_clip_fill_does_not_change_element_inheritance() {
    let parsed = crate::dom::parse(
        r#"<body><div style="color:transparent;background:red;background-clip:text"><span><i>x</i></span><b style="position:absolute">y</b></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = box_style(
        &BoxStyle::default(),
        &element(&parsed.nodes, "div").style,
        vcx,
    );
    let span = box_style(&div, &element(&parsed.nodes, "span").style, vcx);
    let i = box_style(&span, &element(&parsed.nodes, "i").style, vcx);
    let b = box_style(&div, &element(&parsed.nodes, "b").style, vcx);
    assert_eq!(div.ink(), span.ink());
    assert_eq!(span.ink(), i.ink());
    assert_eq!(i.ink().unwrap().a, 1.0);
    assert_eq!(b.ink().unwrap().a, 0.0);
    assert!(b.foreground_paint().fill.is_none());
    for style in [&div, &span, &i, &b] {
        assert_eq!(style.inh.ui.color.unwrap().a, 0.0);
    }
}

#[test]
fn paint_operands_preserve_css_and_resolve_currentcolor_from_ink() {
    let parsed = crate::dom::parse(
        r#"<div style="color:red;filter:brightness(50%);background:blue;border:3px solid currentColor;border-radius:7px;outline:2px solid currentColor;box-shadow:1px 2px 3px currentColor">x</div>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let node = element(&parsed.nodes, "div");
    let css = node.style.computed();
    let boxed = box_style(&BoxStyle::default(), &node.style, vcx);
    assert_eq!(boxed.inh.ui.color, css.inh.ui.color);
    assert_eq!(
        boxed.reset_background.background,
        css.reset_background.background
    );
    assert_eq!(
        boxed.reset_border.border_colors,
        css.reset_border.border_colors
    );
    assert_eq!(boxed.reset_border.radius, css.reset_border.radius);
    assert_eq!(boxed.reset_effects.shadows, css.reset_effects.shadows);
    assert_eq!(boxed.reset_effects.filter, css.reset_effects.filter);
    assert_eq!(boxed.ink().unwrap().r, 0.5);
    assert_eq!(boxed.paint_background().color.unwrap().b, 0.5);
    let ink = boxed.ink().unwrap();
    assert_eq!(boxed.used_border_color(0), ink);
    assert_eq!(
        boxed.used_outline_color(boxed.reset_border.outline.as_ref().unwrap()),
        ink
    );
    assert_eq!(boxed.resolved_shadows(false)[0].1, ink);
    let anonymous = super::super::inherit::inherit(&boxed, &Computed::default(), vcx);
    assert_eq!(anonymous.inh.ui.color, css.inh.ui.color);
    assert_eq!(anonymous.ink(), boxed.ink());
    assert_eq!(css.inh.ui.color.unwrap().r, 1.0);
}

#[test]
fn transported_paint_is_applied_once_and_unpainted_input_keeps_its_context() {
    for declaration in ["", "color:red", "color:inherit"] {
        let parsed = crate::dom::parse(
            &format!(
                r#"<div style="color:red;filter:brightness(50%);background:blue;background-clip:text"><span style="{declaration}">x</span></div>"#
            ),
            "",
        );
        let vcx = parsed.values();
        materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
        let parent = box_style(
            &BoxStyle::default(),
            &element(&parsed.nodes, "div").style,
            vcx,
        );
        let own = &element(&parsed.nodes, "span").style;
        let expected = box_style(&parent, own, vcx);
        // Флоаты несут готовый результат; подъём абсолюта — входы до окраски.
        for prepared in [true, false] {
            let source = if prepared {
                box_style(&parent, own, vcx)
            } else {
                box_style_unpainted(&parent, own, vcx)
            };
            for input in [
                crate::style::box_input::BoxStyleInput::from(&source),
                source.clone().into(),
            ] {
                let synthetic = crate::dom::SharedStyle::layout_made(input);
                let rebuilt = box_style(&BoxStyle::default(), &synthetic, vcx);
                assert_eq!(
                    rebuilt.inh.ui.color,
                    own.computed().inh.ui.color,
                    "{declaration}"
                );
                assert_eq!(
                    rebuilt.foreground_paint(),
                    expected.foreground_paint(),
                    "{declaration}: {prepared}"
                );
                assert_eq!(rebuilt.paint_filter(), expected.paint_filter());
                let again = crate::dom::SharedStyle::layout_made(rebuilt);
                let rebuilt = box_style(&BoxStyle::default(), &again, vcx);
                assert_eq!(rebuilt.foreground_paint(), expected.foreground_paint());
            }
        }
    }
}
