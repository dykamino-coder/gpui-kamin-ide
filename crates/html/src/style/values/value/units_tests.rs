//! Тесты математических функций calc(), констант и единиц окна/контейнера.

use super::*;

fn px(s: &str) -> f32 {
    match Len::parse(s) {
        Some(Len::Px(v)) => v,
        other => panic!("{s}: ожидались точки, вышло {other:?}"),
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn folded(s: &str, vertical: bool, container: Option<(f32, f32)>) -> f32 {
    let Some(Len::Calc(i)) = Len::parse(s) else {
        panic!("{s}: ожидалась сумма в арене");
    };
    let mut sum = calc_get(i);
    assert!(sum.fold_viewport((800.0, 600.0), vertical, container));
    sum.px
}

#[test]
fn round_strategies_follow_values_4() {
    assert_eq!(px("round(17px, 5px)"), 15.0);
    assert_eq!(px("round(17.5px, 5px)"), 20.0);
    assert_eq!(px("round(up, 11px, 5px)"), 15.0);
    assert_eq!(px("round(down, 14px, 5px)"), 10.0);
    assert_eq!(px("round(to-zero, -13px, 5px)"), -10.0);
    assert_eq!(number("round(2.5)"), Some(3.0));
    assert_eq!(number("round(-2.5)"), Some(-2.0));
    assert_eq!(Len::parse("round(1em, 3px)"), None);
}

#[test]
fn mod_and_rem_differ_in_sign() {
    assert_eq!(px("mod(-18px, 5px)"), 2.0);
    assert_eq!(px("rem(-18px, 5px)"), -3.0);
    assert_eq!(px("calc(mod(18px, 5px) * 2)"), 6.0);
}

#[test]
fn trigonometry_takes_angles_and_numbers() {
    assert!(near(number("sin(90deg)").unwrap(), 1.0));
    assert!(near(number("cos(pi)").unwrap(), -1.0));
    assert!(near(number("tan(0.5turn)").unwrap(), 0.0));
    assert!(near(px("calc(100px * sin(30deg))"), 50.0));
    assert!(near(px("calc(100px * cos(acos(0.5)))"), 50.0));
    assert!(near(px("calc(10px * sin(atan2(1px, 0px)))"), 10.0));
    assert!(near(px("calc(1px * tan(atan(2)))"), 2.0));
    assert_eq!(Len::parse("calc(45deg)"), None);
}

#[test]
fn exponential_functions() {
    assert_eq!(number("pow(2, 10)"), Some(1024.0));
    assert_eq!(number("calc(sqrt(16) + 1)"), Some(5.0));
    assert!(near(number("log(e)").unwrap(), 1.0));
    assert!(near(number("log(8, 2)").unwrap(), 3.0));
    assert_eq!(number("exp(0)"), Some(1.0));
    assert_eq!(px("hypot(3px, 4px)"), 5.0);
    assert_eq!(Len::parse("hypot(3px, 4)"), None);
}

#[test]
fn sign_related_functions() {
    assert_eq!(px("abs(-5px)"), 5.0);
    assert_eq!(px("calc(1px * (1 + sign(16px - 10px)))"), 2.0);
    assert_eq!(px("calc(10px * sign(-2em))"), -10.0);
    assert_eq!(px("calc(10px * sign(0px))"), 0.0);
}

#[test]
fn constants_are_censored_at_top_level() {
    assert!(px("calc(infinity * 1px)") > 3.0e7);
    assert!(px("calc(-infinity * 1px)") < -3.0e7);
    assert_eq!(px("calc(NaN * 1px)"), 0.0);
    assert_eq!(px("calc(1px / 0)"), px("calc(infinity * 1px)"));
    assert!(near(number("calc(pi)").unwrap(), std::f32::consts::PI));
    assert!(near(
        number("calc(e * 2)").unwrap(),
        2.0 * std::f32::consts::E
    ));
    assert_eq!(number("calc(inf)"), None);
}

#[test]
fn viewport_size_variants() {
    assert_eq!(folded("10vmin", false, None), 60.0);
    assert_eq!(folded("10vmax", false, None), 80.0);
    assert_eq!(folded("10vi", false, None), 80.0);
    assert_eq!(folded("10vi", true, None), 60.0);
    assert_eq!(folded("10svb", false, None), 60.0);
    assert_eq!(folded("calc(10vmin + 5px)", false, None), 65.0);
    assert_eq!(Len::parse("50svh"), Some(Len::Vh(0.5)));
    assert_eq!(Len::parse("5dvw"), Some(Len::Vw(0.05)));
    assert_eq!(Len::parse("5LVH"), Some(Len::Vh(0.05)));
    assert_eq!(Len::parse("5xvh"), None);
}

#[test]
fn container_units_resolve_against_the_container() {
    let c = Some((400.0, 300.0));
    assert_eq!(folded("10cqw", false, c), 40.0);
    assert_eq!(folded("10cqh", false, c), 30.0);
    assert_eq!(folded("10cqi", true, c), 30.0);
    assert_eq!(folded("10cqb", false, c), 30.0);
    assert_eq!(folded("10cqmin", false, c), 30.0);
    assert_eq!(folded("10cqmax", false, c), 40.0);
    // Без контейнера — малое окно.
    assert_eq!(folded("10cqw", false, None), 80.0);
}

#[test]
fn size_container_exposes_its_content_box() {
    use crate::style::computed::Computed;
    let mut c = Computed {
        container_size_query: true,
        contain_size: Some(true),
        width: Some(Len::Px(400.0)),
        height: Some(Len::Px(300.0)),
        ..Computed::default()
    };
    assert_eq!(c.container_box_for_children(), (Some(400.0), Some(300.0)));
    c.contain_size = None;
    assert_eq!(c.container_box_for_children(), (Some(400.0), None));
    c.container_size_query = false;
    assert_eq!(c.container_box_for_children(), (None, None));
}
