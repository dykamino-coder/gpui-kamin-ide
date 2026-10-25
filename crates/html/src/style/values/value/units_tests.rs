//! Тесты математических функций calc(), констант и единиц окна/контейнера.

use super::*;
use crate::document::state::ValueCx;

fn px(s: &str, vcx: ValueCx<'_>) -> f32 {
    match Len::parse(s, vcx) {
        Some(Len::Px(v)) => v,
        other => panic!("{s}: ожидались точки, вышло {other:?}"),
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn folded(s: &str, vertical: bool, container: Option<(f32, f32)>, vcx: ValueCx<'_>) -> f32 {
    let Some(Len::Calc(i)) = Len::parse(s, vcx) else {
        panic!("{s}: ожидалась сумма в арене");
    };
    let mut sum = calc_get(i, vcx);
    assert!(sum.fold_viewport((800.0, 600.0), vertical, container));
    sum.px
}

#[test]
fn round_strategies_follow_values_4() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(px("round(17px, 5px)", vcx), 15.0);
    assert_eq!(px("round(17.5px, 5px)", vcx), 20.0);
    assert_eq!(px("round(up, 11px, 5px)", vcx), 15.0);
    assert_eq!(px("round(down, 14px, 5px)", vcx), 10.0);
    assert_eq!(px("round(to-zero, -13px, 5px)", vcx), -10.0);
    assert_eq!(number("round(2.5)", vcx), Some(3.0));
    assert_eq!(number("round(-2.5)", vcx), Some(-2.0));
    assert_eq!(Len::parse("round(1em, 3px)", vcx), None);
}

#[test]
fn mod_and_rem_differ_in_sign() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(px("mod(-18px, 5px)", vcx), 2.0);
    assert_eq!(px("rem(-18px, 5px)", vcx), -3.0);
    assert_eq!(px("calc(mod(18px, 5px) * 2)", vcx), 6.0);
}

#[test]
fn trigonometry_takes_angles_and_numbers() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert!(near(number("sin(90deg)", vcx).unwrap(), 1.0));
    assert!(near(number("cos(pi)", vcx).unwrap(), -1.0));
    assert!(near(number("tan(0.5turn)", vcx).unwrap(), 0.0));
    assert!(near(px("calc(100px * sin(30deg))", vcx), 50.0));
    assert!(near(px("calc(100px * cos(acos(0.5)))", vcx), 50.0));
    assert!(near(px("calc(10px * sin(atan2(1px, 0px)))", vcx), 10.0));
    assert!(near(px("calc(1px * tan(atan(2)))", vcx), 2.0));
    assert_eq!(Len::parse("calc(45deg)", vcx), None);
}

#[test]
fn exponential_functions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(number("pow(2, 10)", vcx), Some(1024.0));
    assert_eq!(number("calc(sqrt(16) + 1)", vcx), Some(5.0));
    assert!(near(number("log(e)", vcx).unwrap(), 1.0));
    assert!(near(number("log(8, 2)", vcx).unwrap(), 3.0));
    assert_eq!(number("exp(0)", vcx), Some(1.0));
    assert_eq!(px("hypot(3px, 4px)", vcx), 5.0);
    assert_eq!(Len::parse("hypot(3px, 4)", vcx), None);
}

#[test]
fn sign_related_functions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(px("abs(-5px)", vcx), 5.0);
    assert_eq!(px("calc(1px * (1 + sign(16px - 10px)))", vcx), 2.0);
    assert_eq!(px("calc(10px * sign(-2em))", vcx), -10.0);
    assert_eq!(px("calc(10px * sign(0px))", vcx), 0.0);
}

#[test]
fn constants_are_censored_at_top_level() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert!(px("calc(infinity * 1px)", vcx) > 3.0e7);
    assert!(px("calc(-infinity * 1px)", vcx) < -3.0e7);
    assert_eq!(px("calc(NaN * 1px)", vcx), 0.0);
    assert_eq!(px("calc(1px / 0)", vcx), px("calc(infinity * 1px)", vcx));
    assert!(near(number("calc(pi)", vcx).unwrap(), std::f32::consts::PI));
    assert!(near(
        number("calc(e * 2)", vcx).unwrap(),
        2.0 * std::f32::consts::E
    ));
    assert_eq!(number("calc(inf)", vcx), None);
}

#[test]
fn viewport_size_variants() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(folded("10vmin", false, None, vcx), 60.0);
    assert_eq!(folded("10vmax", false, None, vcx), 80.0);
    assert_eq!(folded("10vi", false, None, vcx), 80.0);
    assert_eq!(folded("10vi", true, None, vcx), 60.0);
    assert_eq!(folded("10svb", false, None, vcx), 60.0);
    assert_eq!(folded("calc(10vmin + 5px)", false, None, vcx), 65.0);
    assert_eq!(Len::parse("50svh", vcx), Some(Len::Vh(0.5)));
    assert_eq!(Len::parse("5dvw", vcx), Some(Len::Vw(0.05)));
    assert_eq!(Len::parse("5LVH", vcx), Some(Len::Vh(0.05)));
    assert_eq!(Len::parse("5xvh", vcx), None);
}

#[test]
fn container_units_resolve_against_the_container() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let c = Some((400.0, 300.0));
    assert_eq!(folded("10cqw", false, c, vcx), 40.0);
    assert_eq!(folded("10cqh", false, c, vcx), 30.0);
    assert_eq!(folded("10cqi", true, c, vcx), 30.0);
    assert_eq!(folded("10cqb", false, c, vcx), 30.0);
    assert_eq!(folded("10cqmin", false, c, vcx), 30.0);
    assert_eq!(folded("10cqmax", false, c, vcx), 40.0);
    // Без контейнера — малое окно.
    assert_eq!(folded("10cqw", false, None, vcx), 80.0);
}

#[test]
fn size_container_exposes_its_content_box() {
    use crate::style::computed::Computed;
    let mut c = Computed {
        container_type: crate::style::computed::ContainerType::Size,
        contain_size: Some(true),
        width: Some(Len::Px(400.0)),
        height: Some(Len::Px(300.0)),
        cq_size: (Some(350.0), Some(250.0)),
        ..Computed::default()
    };
    assert_eq!(c.container_box_for_children(), (Some(350.0), Some(250.0)));
    c.contain_size = None;
    c.container_type = crate::style::computed::ContainerType::InlineSize;
    c.contain_inline_size = Some(true);
    assert_eq!(c.container_box_for_children(), (Some(350.0), None));
    c.container_type = crate::style::computed::ContainerType::Normal;
    assert_eq!(c.container_box_for_children(), (None, None));
}
