//! Проверки контракта родительского модуля; вынесены для ограничения размера файлов.

use super::*;
use crate::style::computed::Tiling;

#[test]
fn tiling_starts_before_the_box_and_covers_it() {
    // Смещение 30 при плитке 20: первая копия обязана начаться левее нуля,
    // иначе между краем коробки и первой плиткой остаётся дыра.
    let xs = tiling(Tiling::Repeat, 30.0, 20.0, 100.0);
    let first = xs[0];
    assert!(first <= 0.0, "первая плитка начинается не правее коробки");
    assert!(
        first + xs.len() as f64 * 20.0 >= 100.0,
        "плитки обязаны закрыть коробку целиком"
    );
}

#[test]
fn without_repeat_there_is_exactly_one_copy() {
    assert_eq!(tiling(Tiling::None, 12.0, 20.0, 100.0), vec![12.0]);
}

/// `space` раздаёт остаток РАВНЫМИ зазорами, а крайние плитки прижимает к
/// краям (css-backgrounds-3 §3.4).
#[test]
fn space_pins_the_edges_and_shares_the_rest() {
    let xs = tiling(Tiling::Space, 0.0, 32.0, 106.0);
    assert_eq!(xs.len(), 3, "целых плиток влезает три");
    assert_eq!(xs[0], 0.0);
    assert!((xs[2] + 32.0 - 106.0).abs() < 0.01, "последняя у края");
}

/// `round` подгоняет САМУ плитку под целое их число.
#[test]
fn round_fits_a_whole_number_of_tiles() {
    assert_eq!(rounded(Tiling::Round, 30.0, 100.0), 100.0 / 3.0);
    assert_eq!(rounded(Tiling::Repeat, 30.0, 100.0), 30.0);
}

/// Умолчальный размер: доля своей стороной не является, и рисунок
/// занимает место под фон целиком (css-images-3 §5.3).
#[test]
fn default_size_falls_back_to_the_area() {
    let none = Intrinsic::default();
    assert_eq!(default_size(none, (256.0, 768.0)), (256.0, 768.0));
    let ratio = Intrinsic {
        ratio: Some(2.0),
        ..Default::default()
    };
    assert_eq!(default_size(ratio, (200.0, 400.0)), (200.0, 100.0));
    let sides = Intrinsic {
        w: Some(60.0),
        h: Some(30.0),
        ratio: Some(2.0),
    };
    assert_eq!(default_size(sides, (200.0, 400.0)), (60.0, 30.0));
}

/// Своя величина рисунка: доля стороной не считается, `viewBox` даёт
/// только соотношение.
#[test]
fn svg_percent_side_is_not_intrinsic() {
    let i = svg_size("<svg xmlns=\"…\" height=\"50%\"></svg>");
    assert_eq!(i, Intrinsic::default());
    let i = svg_size("<svg viewBox=\"0 0 2560 208\"></svg>");
    assert_eq!(i.w, None);
    assert_eq!(i.ratio, Some(2560.0 / 208.0));
}

#[test]
fn base64_reads_a_known_payload() {
    assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
}
