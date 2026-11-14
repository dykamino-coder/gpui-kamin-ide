//! CSS-вход и RGB-последовательности измерений; replay отличим от реальной раскладки.

use super::*;

#[test]
fn relayout_input_alternates_without_leaving_the_supported_viewport_range() {
    assert_eq!(viewport((800, 600), true), (801, 600));
    assert_eq!(viewport((800, 600), false), (800, 600));
    assert_eq!(viewport((8192, 1), true), (8191, 1));
    assert_eq!(viewport((1, 8192), true), (2, 8192));
}

#[test]
fn counter_reset_cannot_masquerade_as_zero_work() {
    assert!(delta((5, 9), (1, 9)).is_err());
    assert!(delta((5, 9), (5, 0)).is_err());
    assert_eq!(delta((5, 9), (6, 9)).unwrap(), (1, 0));
    assert_eq!(delta((5, 9), (5, 10)).unwrap(), (0, 1));
}

#[test]
fn failure_report_keeps_first_refusal_and_pixel_artifact_paths() {
    let directory =
        std::env::temp_dir().join(format!("engine-bench-first-failure-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut pixels = Recorder::new(Some(&directory)).unwrap();
    for (index, rgb) in [0, 255].into_iter().enumerate() {
        pixels
            .observe(
                gpui::frame_capture::CapturedFrame {
                    width: 1,
                    height: 1,
                    bgra: vec![rgb, 0, 0, 255],
                    context: None,
                },
                index as u64,
                "cold",
                index,
            )
            .unwrap();
    }
    let report = pixels.report();
    let first: Result<()> = Err("first refusal".into());
    capture_refusal(Some(&directory), &report, &first);
    capture_refusal(Some(&directory), &Value::Null, &Err("retry refusal".into()));
    assert_eq!(
        first.as_ref().map_err(|error| error.to_string()),
        Err("first refusal".to_string())
    );
    let saved: Value =
        serde_json::from_slice(&std::fs::read(directory.join("failed-frame.json")).unwrap())
            .unwrap();
    assert_eq!(saved["error"], "first refusal");
    assert_eq!(saved["report"], report);
    for artifact in ["baseline", "changed", "absolute_rgb_diff"] {
        let path = std::path::Path::new(
            saved["report"]["frame_pixel_differences"][0]["artifacts"][artifact]
                .as_str()
                .unwrap(),
        );
        assert!(
            std::fs::read(path)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_file(directory.join("failed-frame.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
