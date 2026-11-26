//! Native lifecycle regression test; requires the bounded desktop launcher.
use super::{Args, launch, text_rendering_mode};

#[test]
fn text_smoothing_defaults_to_grayscale_and_rejects_unknown_values() {
    use gpui::TextRenderingMode::{Grayscale, PlatformDefault};
    assert_eq!(text_rendering_mode(None).unwrap(), Grayscale);
    assert_eq!(text_rendering_mode(Some("grayscale")).unwrap(), Grayscale);
    assert_eq!(
        text_rendering_mode(Some("system")).unwrap(),
        PlatformDefault
    );
    assert!(text_rendering_mode(Some("lcd")).is_err());
}

#[test]
#[ignore = "requires an isolated native desktop and a bounded process launcher"]
fn cold_run_exits_after_report() {
    let directory = std::env::temp_dir().join(format!(
        "engine-bench-cold-lifecycle-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create lifecycle fixture directory");
    let fixture = directory.join("scrolling.html");
    let mut markup =
        String::from("<!doctype html><style>body{margin:0}p{margin:1em;padding:1px}</style>");
    for _ in 0..800 {
        markup.push_str("<p>A scrolling document with <em>inline text</em> and content.</p>");
    }
    std::fs::write(&fixture, markup).expect("write lifecycle fixture");
    // A private page can exercise this lifecycle without entering the repository.
    let path = std::env::var_os("ENGINE_BENCH_LIFECYCLE_PAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| fixture.clone());
    let args = Args::parse([
        path.to_string_lossy().into_owned(),
        "--mode".into(),
        "cold".into(),
        "--runs".into(),
        "1".into(),
        "--json".into(),
    ])
    .expect("parse lifecycle arguments")
    .expect("lifecycle arguments");
    let result = launch(args);
    std::fs::remove_file(fixture).expect("remove lifecycle fixture");
    std::fs::remove_dir(directory).expect("remove lifecycle fixture directory");
    result.expect("complete cold run and return from the native event loop");
}
