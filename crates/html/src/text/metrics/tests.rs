//! Tests for metrics; split out to keep the owning module within 250 lines.

use super::*;

#[test]
fn falls_back_to_half_the_font_size() {
    assert_eq!(ch_ex_px("нет такого шрифта", 20.0), (10.0, 10.0));
}

#[test]
fn probe_result_scales_with_the_font_size() {
    install_probe(|family, size| {
        // У Ahem все знаки в кегль, включая иероглиф.
        if family == "Ahem" {
            (size, size * 0.8, size, size)
        } else {
            (size * 0.5, size * 0.5, size * 1.2, size)
        }
    });
    assert_eq!(ch_ex_px("Ahem", 20.0), (20.0, 16.0));
    assert_eq!(ch_ex_px("Segoe UI", 20.0), (10.0, 10.0));
    // Щуп снимается: иначе он утечёт в соседние тесты того же потока.
    PROBE.with(|p| *p.borrow_mut() = None);
    CACHE.with(|c| c.borrow_mut().clear());
}
