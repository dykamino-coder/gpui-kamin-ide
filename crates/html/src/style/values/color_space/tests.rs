//! Тесты цветовых пространств: lab/lch/oklab, color(), color-mix(), относительный синтаксис.

use super::*;

/// Сравнение идёт по ЗАЖАТЫМ величинам: цвет шире sRGB выходит за
/// пределы, и зажимает его разбор цвета, а не преобразование.
fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32), what: &str) {
    let a = (
        a.0.clamp(0.0, 1.0),
        a.1.clamp(0.0, 1.0),
        a.2.clamp(0.0, 1.0),
        a.3,
    );
    for (got, want) in [(a.0, b.0), (a.1, b.1), (a.2, b.2)] {
        assert!((got - want).abs() < 0.02, "{what}: {got} против {want}");
    }
}

/// Белый и красный обязаны совпасть во всех записях.
#[test]
fn known_colors_survive_the_conversion() {
    close(
        parse("lab(100% 0 0)").unwrap(),
        (1.0, 1.0, 1.0),
        "lab белый",
    );
    close(
        parse("oklch(0.628 0.2577 29.23)").unwrap(),
        (1.0, 0.0, 0.0),
        "oklch красный",
    );
    // Цвет шире охвата втягивается СЖАТИЕМ ЦВЕТНОСТИ (§13.1.5), а не
    // срезом: зелёный остаётся насыщенным зелёным, но уже не (0,1,0).
    let p3 = parse("color(display-p3 0 1 0)").unwrap();
    assert!(
        p3.1 > 0.9 && p3.0 < 0.3 && p3.2 < 0.5,
        "p3 зелёный после втягивания: {p3:?}"
    );
    close(
        parse("color(srgb 0.2 0.4 0.6)").unwrap(),
        (0.2, 0.4, 0.6),
        "srgb как есть",
    );
}

/// `hwb`: белизна с чернотой в сумме за единицу дают серый.
#[test]
fn hwb_mixes_white_and_black() {
    close(parse("hwb(0 100% 0%)").unwrap(), (1.0, 1.0, 1.0), "белый");
    close(parse("hwb(0 0% 100%)").unwrap(), (0.0, 0.0, 0.0), "чёрный");
    close(parse("hwb(0 50% 50%)").unwrap(), (0.5, 0.5, 0.5), "серый");
}

/// Доли смешивания приводятся к единице.
#[test]
fn color_mix_honours_shares() {
    close(
        parse("color-mix(in srgb, white 25%, black 75%)").unwrap(),
        (0.25, 0.25, 0.25),
        "четверть белого",
    );
}
