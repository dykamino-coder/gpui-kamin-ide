//! Тесты рамки и контура по реестру: раскрытие сокращений при разборе,
//! негодное объявление отброшено целиком, наследование вычисленной толщины
//! мимо правила «ноль без рисунка», логические кромки по каскаду.

use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::css::parse_decls_in;
use crate::style::values::value::{ColorOrAuto, Len, StyleColor};

fn doc() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.attrs.iter().any(|(k, v)| k == "id" && v == id) => Some(e),
        Node::Element(e) => find(&e.children, id),
        _ => None,
    })
}

fn styled(css: &str) -> (Computed, std::rc::Rc<crate::document::state::DocState>) {
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
        let parent = inherit(&Computed::default(), &parent, vcx);
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

#[test]
fn logical_border_sides_compete_with_physical_ones_by_cascade() {
    let doc = Document::builder(
        r#"<style>
        #a { border-block-start: 4px solid; border-top-width: 1px }
        .b { border-top-width: 1px }
        #b { border-block-start-width: 5px }
        #v { border-inline-start: 7px solid red }
        #p { border-width: 2px; --w: bogus }
        #i { border-block-width: inherit; border-inline: var(--w) solid }
        </style><body><div id="a"></div><div id="b" class="b"></div>
        <div style="writing-mode: vertical-rl"><div id="v"></div></div>
        <div id="p"><div id="i"></div></div></body>"#,
        "",
    )
    .build();
    let a = find(doc.nodes(), "a").expect("a");
    assert_eq!(a.style.border_width.top, Some(Len::Px(1.0)));
    let b = find(doc.nodes(), "b").expect("b");
    assert_eq!(b.style.border_width.top, Some(Len::Px(5.0)));
    // `vertical-rl`: inline-start is the top.
    let v = find(doc.nodes(), "v").expect("v");
    assert_eq!(v.style.borders().top, Some(Len::Px(7.0)));
    assert_eq!(v.style.borders().left, None);
    let i = find(doc.nodes(), "i").expect("i");
    assert!(i.style.border_inherit_w[0] && i.style.border_inherit_w[2]);
    // Invalid after substitution: every longhand of the shorthand is unset.
    assert_eq!(i.style.border_side_styles[3], Some(0));
}
