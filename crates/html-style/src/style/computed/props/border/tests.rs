//! Тесты рамки и контура по реестру: раскрытие сокращений при разборе,
//! негодное объявление отброшено целиком, наследование вычисленной толщины
//! мимо правила «ноль без рисунка».

use crate::style::cascade::inherit::inherit;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::css::parse_decls_in;
use crate::style::values::value::{ColorOrAuto, Len, StyleColor};

fn doc() -> std::rc::Rc<crate::values::DocumentValues> {
    crate::values::DocumentValues::detached(crate::style::css::Media::default())
}

fn styled(css: &str) -> (Computed, std::rc::Rc<crate::values::DocumentValues>) {
    let doc = doc();
    let mut c = Computed::default();
    c.apply_decls(&parse_decls_in(css, doc.values()), doc.values());
    (c, doc)
}

#[test]
fn inherited_width_is_the_computed_width_whatever_the_parent_style() {
    // `border-width-012`: the zero width of a `hidden` border is its USED
    // value (css-backgrounds-3 §4.3); the computed 2em is inherited.
    for child in [
        "border-width: inherit; border-style: solid",
        "border-style: solid; border-width: inherit",
    ] {
        let doc = doc();
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_decls(
            &parse_decls_in("border-width: 2em; border-style: hidden", vcx),
            vcx,
        );
        let parent = inherit(&BoxStyle::default(), &parent, vcx);
        assert_eq!(parent.borders().top, None);
        let mut own = Computed::default();
        own.apply_decls(&parse_decls_in(child, vcx), vcx);
        let c = inherit(&parent, &own, vcx);
        assert_eq!(c.borders().top, Some(Len::Px(32.0)), "{child}");
    }
}

#[test]
fn shorthand_and_longhands_meet_by_order_of_appearance() {
    let (c, _d) = styled("border: solid gray; border-width: 1px 2px 3px 4px");
    assert_eq!(c.borders().left, Some(Len::Px(4.0)));
    let (c, _d) = styled("border-width: 1px 2px 3px 4px; border: solid gray");
    assert_eq!(c.borders().left, Some(Len::Px(3.0)));
    // `grid-gap-decorations-*`: a side shorthand resets that side's color to
    // `currentColor`, a later `border-color` wins over it.
    let (c, d) = styled("border-right: 6px solid; border-color: pink");
    let pink = StyleColor::parse_value("pink", d.values()).unwrap();
    assert_eq!(c.border_colors[1], pink);
    let (c, _d) = styled("border-color: pink; border-right: 6px solid");
    assert_eq!(c.border_colors[1], StyleColor::Current);
    assert_eq!(c.border_colors[0], pink);
    assert_eq!(c.used_uniform_border_color(), None);
}

#[test]
fn invalid_border_declarations_are_dropped_whole() {
    for css in [
        "border: 2px solid green; border: 1px solid red currentcolor",
        "border: 2px solid green; border: -1px solid red",
        "border: 2px solid green; border-width: 1px 10%",
        "border: 2px solid green; border-top-width: -3px",
        "border: 2px solid green; border-style: solid wavy",
        "border: 2px solid green; border-color: red bogus",
    ] {
        let (c, d) = styled(css);
        assert_eq!(c.borders().top, Some(Len::Px(2.0)), "{css}");
        assert_eq!(
            c.used_uniform_border_color(),
            crate::style::values::value::Color::parse("green", d.values()),
            "{css}"
        );
    }
}

#[test]
fn style_alone_gives_the_initial_medium_width() {
    let (c, _d) = styled("border-style: none solid");
    assert_eq!(c.border_width.top, None);
    assert_eq!(c.borders().top, None);
    assert_eq!(c.borders().right, Some(Len::Px(3.0)));
    let (c, _d) = styled("border-right: 10px dotted; border-color: pink");
    assert_eq!(c.border_dotted, Some(true));
    // `currentColor` stays the keyword (css-color-4 §6.4): the element's
    // color decides it at use.
    let (c, _d) = styled("border: 1px solid currentColor");
    assert_eq!(c.borders().top, Some(Len::Px(1.0)));
    assert!(c.border_colors.iter().all(StyleColor::is_current));
    assert_eq!(c.border_side_styles, [Some(9); 4]);
}

#[test]
fn border_resets_border_image_and_outline_resets_its_parts() {
    let (c, _d) = styled("border-image: url(x.png) 30; border: 1px solid");
    assert!(c.border_image.is_none());
    let (c, _d) = styled("border: 1px solid; border-image: url(x.png) 30");
    assert!(c.border_image.is_some_and(|b| b.src.is_some()));
    let (c, d) = styled("outline-color: red; outline: 2px solid");
    let o = c.outline.expect("outline");
    assert_eq!(
        (o.width, o.style, o.color),
        (Some(Len::Px(2.0)), Some(1), ColorOrAuto::Auto)
    );
    let (c, _d) = styled("outline: 2px solid; outline-color: red");
    assert_eq!(
        c.outline.and_then(|o| o.color.color().cloned()),
        StyleColor::parse_value("red", d.values())
    );
    // `hidden` is not an outline style (css-ui-4 §outline-style).
    let (c, _d) = styled("outline: 2px dotted; outline-style: hidden");
    assert_eq!(c.outline.and_then(|o| o.style), Some(3));
}
