//! Тесты calc(): свёртка, сравнение, смешанные единицы.

use super::*;

#[test]
fn calc_adds_homogeneous_operands() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(Len::parse("calc(100px + 20px)", vcx), Some(Len::Px(120.0)));
    assert_eq!(Len::parse("calc(100% - 25%)", vcx), Some(Len::Pct(0.75)));
}

#[test]
fn mixed_calc_with_percent_is_still_dropped() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Долю и точки сложить пока нечем (замерено на gap-003);
    // шрифтовая смесь живёт.
    assert_eq!(Len::parse("calc(100% - 24px)", vcx), None);
    let Some(Len::Calc(idx)) = Len::parse("calc(120px + 3.1ch)", vcx) else {
        panic!("шрифтовая смесь обязана дожить как Calc");
    };
    let s = calc_get(idx, vcx);
    assert_eq!((s.px, s.ch), (120.0, 3.1));
}

#[test]
fn mixed_calc_survives_for_paint_consumers() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Потребители с известным размером коробки просят смесь ЯВНО.
    let Some(Len::Calc(idx)) = Len::parse_mixed("calc(100% - 24px)", vcx) else {
        panic!("процентная смесь обязана дожить как Calc для parse_mixed");
    };
    assert_eq!(calc_get(idx, vcx).pct_px(), Some((1.0, -24.0)));
    // Однородные записи сворачиваются как и прежде.
    assert_eq!(
        Len::parse_mixed("calc(25% + 25%)", vcx),
        Some(Len::Pct(0.5))
    );
    let pair = Len::parse_mixed("calc(200% / 2 - 40px)", vcx).and_then(|l| match l {
        Len::Calc(i) => calc_get(i, vcx).pct_px(),
        _ => None,
    });
    assert_eq!(pair, Some((1.0, -40.0)));
    // Третья природа в смеси парой не отдаётся.
    let Some(Len::Calc(idx)) = Len::parse_mixed("calc(50% + 1vw)", vcx) else {
        panic!("смесь с vw обязана дожить как Calc");
    };
    assert_eq!(calc_get(idx, vcx).pct_px(), None);
}

#[test]
fn calc_sums_many_terms_with_precedence() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Слагаемые сокращаются: `6em - 12em/2` даёт ноль, остаётся чистая доля.
    assert_eq!(
        Len::parse("calc(100% + 6em + 50%*4 - 12em/2)", vcx),
        Some(Len::Pct(3.0))
    );
    assert_eq!(Len::parse("calc(25% + 0px)", vcx), Some(Len::Pct(0.25)));
    assert_eq!(Len::parse("calc((2 + 3) * 4px)", vcx), Some(Len::Px(20.0)));
    assert_eq!(Len::parse("calc(-1em * 2)", vcx), Some(Len::Em(-2.0)));
    assert_eq!(Len::parse("calc(2 * 3)", vcx), None);
    // Деление на ноль — бесконечность, зажатая на верхнем уровне
    // (css-values-4 §10.9, §10.13), а не недействительная запись.
    assert_eq!(
        Len::parse("calc(4px / 0)", vcx),
        Some(Len::Px(33_554_428.0))
    );
}

#[test]
fn spacing_calc_folds_percent_into_em() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Для интервалов доля и `em` считаются от кегля — их можно сложить.
    assert_eq!(
        Len::parse_spacing("calc(400% + 1em)", vcx),
        Some(Len::Em(5.0))
    );
    // Ширина такой свободы не имеет: доля там от контейнера.
    assert_eq!(Len::parse("calc(400% + 1em)", vcx), None);
}

#[test]
fn viewport_units_survive_parsing() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(Len::parse("50vw", vcx), Some(Len::Vw(0.5)));
    assert_eq!(Len::parse("100vh", vcx), Some(Len::Vh(1.0)));
}

#[test]
fn pure_percentage_calculation_handles_variadic_and_typed_functions() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(
        calc_pct_px("calc(min(30%, 20%, 10%, 40%) + hypot(3px, 4px, 12px))", vcx),
        Some((0.1, 13.0))
    );
    assert_eq!(
        calc_pct_px("calc(clamp(5px, 2px, 3px) + round(up, 1%, 2%))", vcx),
        Some((0.02, 5.0))
    );
    assert_eq!(
        calc_pct_px("calc(1cap - 1cap + 1svw - 1svw + 6px)", vcx),
        Some((0.0, 6.0))
    );
    assert_eq!(calc_pct_px("calc(2cap + 50%)", vcx), None);
    assert_eq!(calc_pct_px("calc(min(0px, 100%))", vcx), Some((0.0, 0.0)));
    assert_eq!(calc_pct_px("calc(min(0, 100%))", vcx), None);
    assert_eq!(
        calc_pct_px("calc(1rcap)", vcx),
        Len::parse("1rcap", vcx).and_then(|l| match l {
            Len::Px(px) => Some((0.0, px)),
            _ => None,
        })
    );
    assert_eq!(Len::parse("min(0em, 2em)", vcx), Some(Len::Px(0.0)));
    assert_eq!(Len::parse("round(3em, 2em)", vcx), Some(Len::Em(4.0)));
    assert_eq!(number("hypot(3, 4, 12)", vcx), Some(13.0));
    assert_eq!(number("sin(90DEG)", vcx), Some(1.0));
    assert_eq!(Len::parse("1e2PX", vcx), Some(Len::Px(100.0)));
}
