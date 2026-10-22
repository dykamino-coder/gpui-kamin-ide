//! Проверка переполнения IMCB, включая отрицательный размер до коррекции.

#[test]
fn negative_imcb_overflows_even_with_a_negative_margin_box() {
    assert!(super::overflows(10., -1., 10., 9.));
    assert!(!super::overflows(10., 0., 10., 10.));
    assert!(super::overflows(10., 1., 10., 10.));
}
