//! Тесты calc(): свёртка, сравнение, смешанные единицы.

use super::*;

#[test]
fn calc_adds_homogeneous_operands() {
    assert_eq!(Len::parse("calc(100px + 20px)"), Some(Len::Px(120.0)));
    assert_eq!(Len::parse("calc(100% - 25%)"), Some(Len::Pct(0.75)));
}

#[test]
fn mixed_calc_with_percent_is_still_dropped() {
    // Долю и точки сложить пока нечем (замерено на gap-003);
    // шрифтовая смесь живёт.
    assert_eq!(Len::parse("calc(100% - 24px)"), None);
    let Some(Len::Calc(idx)) = Len::parse("calc(120px + 3.1ch)") else {
        panic!("шрифтовая смесь обязана дожить как Calc");
    };
    let s = calc_get(idx);
    assert_eq!((s.px, s.ch), (120.0, 3.1));
}

#[test]
fn mixed_calc_survives_for_paint_consumers() {
    // Потребители с известным размером коробки просят смесь ЯВНО.
    let Some(Len::Calc(idx)) = Len::parse_mixed("calc(100% - 24px)") else {
        panic!("процентная смесь обязана дожить как Calc для parse_mixed");
    };
    assert_eq!(calc_get(idx).pct_px(), Some((1.0, -24.0)));
    // Однородные записи сворачиваются как и прежде.
    assert_eq!(Len::parse_mixed("calc(25% + 25%)"), Some(Len::Pct(0.5)));
    let pair = Len::parse_mixed("calc(200% / 2 - 40px)").and_then(|l| match l {
        Len::Calc(i) => calc_get(i).pct_px(),
        _ => None,
    });
    assert_eq!(pair, Some((1.0, -40.0)));
    // Третья природа в смеси парой не отдаётся.
    let Some(Len::Calc(idx)) = Len::parse_mixed("calc(50% + 1vw)") else {
        panic!("смесь с vw обязана дожить как Calc");
    };
    assert_eq!(calc_get(idx).pct_px(), None);
}

#[test]
fn calc_sums_many_terms_with_precedence() {
    // Слагаемые сокращаются: `6em - 12em/2` даёт ноль, остаётся чистая доля.
    assert_eq!(
        Len::parse("calc(100% + 6em + 50%*4 - 12em/2)"),
        Some(Len::Pct(3.0))
    );
    assert_eq!(Len::parse("calc(25% + 0px)"), Some(Len::Pct(0.25)));
    assert_eq!(Len::parse("calc((2 + 3) * 4px)"), Some(Len::Px(20.0)));
    assert_eq!(Len::parse("calc(-1em * 2)"), Some(Len::Em(-2.0)));
    assert_eq!(Len::parse("calc(2 * 3)"), None);
    assert_eq!(Len::parse("calc(4px / 0)"), None);
}

#[test]
fn spacing_calc_folds_percent_into_em() {
    // Для интервалов доля и `em` считаются от кегля — их можно сложить.
    assert_eq!(Len::parse_spacing("calc(400% + 1em)"), Some(Len::Em(5.0)));
    // Ширина такой свободы не имеет: доля там от контейнера.
    assert_eq!(Len::parse("calc(400% + 1em)"), None);
}

#[test]
fn viewport_units_survive_parsing() {
    assert_eq!(Len::parse("50vw"), Some(Len::Vw(0.5)));
    assert_eq!(Len::parse("100vh"), Some(Len::Vh(1.0)));
}
