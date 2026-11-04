//! Регрессии точного сравнения последовательностей RGB.

use super::*;

fn frame(bgra: Vec<u8>) -> CapturedFrame {
    CapturedFrame {
        width: 2,
        height: 2,
        bgra,
        context: None,
    }
}

#[test]
fn exact_rgb_comparison_ignores_alpha_and_bounds_all_changed_pixels() {
    let first = frame(vec![0; 16]);
    let mut changed = frame(vec![0; 16]);
    changed.bgra[3] = 255;
    assert_eq!(difference(&first, &changed), None);
    changed.bgra[4] = 1;
    changed.bgra[10] = 2;
    assert_eq!(
        difference(&first, &changed),
        Some(Difference {
            pixels: 2,
            bounds: [0, 0, 2, 2]
        })
    );
}

#[test]
fn changing_frames_are_recorded_without_rejection_or_replacing_baseline() {
    let first = frame(vec![0; 16]);
    let expected_hash = fingerprint(&first);
    let mut recorder = Recorder::new(None).expect("recorder");
    recorder.observe(first, 7, "cold", 1).expect("baseline");
    let mut changed = frame(vec![0; 16]);
    changed.bgra[0] = 1;
    recorder
        .observe(changed, 8, "cold", 2)
        .expect("changed frame accepted");
    recorder
        .observe(frame(vec![0; 16]), 9, "warm", 1)
        .expect("baseline restored");
    let report = recorder.report().unwrap();
    assert_eq!(report["first_frame_bgr_fnv1a64"], expected_hash);
    assert_eq!(report["frame_pixels_changed"], true);
    assert_eq!(
        report["frame_pixel_differences"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(report["frame_pixel_differences"][0]["frame_index"], 2);
    assert_eq!(report["frame_pixel_differences"][0]["sample_index"], 2);
}

#[cfg(feature = "allocation-audit")]
#[test]
fn repeated_rgb_differences_do_not_retain_per_frame_heap() {
    let mut recorder = Recorder::new(None).unwrap();
    recorder
        .observe(frame(vec![0; 16]), 1, "relayout", 1)
        .unwrap();
    recorder
        .observe(frame(vec![1; 16]), 2, "relayout", 2)
        .unwrap();
    let before = super::super::allocations::test_live::read();
    for index in 3..=1002 {
        recorder
            .observe(frame(vec![1; 16]), index as u64, "relayout", index)
            .unwrap();
    }
    let after = super::super::allocations::test_live::read();
    assert!(
        after <= before + 1024,
        "retained RGB difference heap grew by {} bytes",
        after - before
    );
    let report = recorder.report().unwrap();
    assert_eq!(
        report["frame_pixel_differences"].as_array().unwrap().len(),
        1001
    );
    assert_eq!(
        report["frame_pixel_differences"][1000]["sample_index"],
        1002
    );
}
