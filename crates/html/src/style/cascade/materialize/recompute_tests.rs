//! Вычисление заново под родителем-элементом: кадр анимации, источник position-try и
//! стадия zoom (единицы области просмотра, `calc()`, список теней текста).

use super::tests::element;
use super::*;
use crate::style::values::value::Len;

#[test]
fn an_animated_reset_value_reaches_explicit_inherit() {
    let parsed = crate::dom::parse(
        r#"<body><div style="width: 100px"><p style="width: inherit">a</p></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    // A sampled animation frame is a cascade edit of a copy of the element
    // (`animation_frame::sample`); its inherited group stays the same.
    let mut copy = div.clone();
    copy.style.cascaded_mut().width = Some(Len::Px(200.0));
    let copy = rematerialize(&div.style, copy, vcx);
    let p = element(&copy.children, "p");
    assert_eq!(p.style.used().width, Some(Len::Px(200.0)));
}

#[test]
fn an_own_zoom_scales_the_whole_inherited_text_shadow_list() {
    let parsed = crate::dom::parse(
        r#"<body><div style="text-shadow: 1px 1px red, 2px 2px blue"><p style="zoom: 2">a</p></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div").style.used();
    let p = element(&parsed.nodes, "p").style.used();
    assert_eq!(div.text_shadow_rest.len(), 1);
    assert_eq!(p.text_shadow_rest.len(), 1);
    assert_eq!(
        p.text_shadow.as_ref().unwrap().x,
        2.0 * div.text_shadow.as_ref().unwrap().x
    );
    assert_eq!(p.text_shadow_rest[0].x, 2.0 * div.text_shadow_rest[0].x);
}

#[test]
fn position_try_declarations_pass_the_computed_value_stage() {
    let parsed = crate::dom::parse(
        r#"<body><div style="position: absolute; zoom: 2; width: 5px">a</div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let mut c = (*div.style.used()).clone();
    assert_eq!(c.width, Some(Len::Px(10.0)));
    // `@position-try { width: 10px }` under `zoom: 2` is 20px, like any author 10px.
    assert!(recompute_onto(
        &mut c,
        &div.style,
        |declared| declared.width = Some(Len::Px(10.0)),
        vcx,
    ));
    assert_eq!(c.width, Some(Len::Px(20.0)));
}

#[test]
fn zoom_scales_viewport_units_and_the_absolute_part_of_calc() {
    let parsed = crate::dom::parse(
        r#"<body><div style="zoom: 2; width: 1vw; height: calc(10px + 1vw)">a</div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let mut div = (*element(&parsed.nodes, "div").style.used()).clone();
    div.resolve_viewport((1000.0, 800.0), vcx);
    // 1vw of a 1000px window is 10px, 20px under `zoom: 2`, like `10px` is.
    assert_eq!(div.width, Some(Len::Px(20.0)));
    assert_eq!(div.height, Some(Len::Px(40.0)));
}
