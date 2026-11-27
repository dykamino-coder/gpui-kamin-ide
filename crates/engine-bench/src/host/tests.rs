//! Native lifecycle regression test; requires the bounded desktop launcher.
use super::{Args, launch};

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

#[test]
#[ignore = "requires an isolated native desktop and a bounded process launcher"]
fn scaled_root_scrollport_opens_in_finite_layout_passes() {
    let directory =
        std::env::temp_dir().join(format!("engine-bench-root-layout-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let fixture = directory.join("grid.html");
    std::fs::write(&fixture, "<!doctype html><style>body{margin:0}.grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:9px}.scroll{height:65px;overflow:auto}.wide{width:130%;height:200px}</style><div class=grid><div class=scroll><div class=wide>wide</div></div><div class=scroll><div class=wide>nested</div></div><div style='height:2000px'>tall</div></div>").unwrap();
    let path = std::env::var_os("ENGINE_BENCH_LIFECYCLE_PAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| fixture.clone());
    let why = directory.join("why");
    let args = Args::parse([
        path.to_string_lossy().into_owned(),
        "--mode".into(),
        "cold".into(),
        "--runs".into(),
        "1".into(),
        "--viewport".into(),
        "1446x804".into(),
        "--scale-factor".into(),
        "1.25".into(),
        "--root-scroller".into(),
        "--gpu".into(),
        "software".into(),
        "--why".into(),
        why.to_string_lossy().into_owned(),
    ])
    .unwrap()
    .unwrap();
    launch(args).expect("present a scaled root scrollport and exit");
    let journal: serde_json::Value =
        serde_json::from_slice(&std::fs::read(why.join("why.json")).unwrap()).unwrap();
    let cold = journal["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["step"] == "cold")
        .unwrap();
    assert!(cold["error"].is_null(), "{cold}");
    let passes = cold["root_layout_passes"].as_f64().unwrap();
    // This static fixture needs at most the initial layout and the three
    // scrollbar refinements; repeated creation of input owners must fail.
    assert!((1.0..=4.0).contains(&passes), "{cold}");
    assert_eq!(passes, cold["layout_retries"].as_f64().unwrap() + 1.0);
    let document_passes = cold["timings_ms"]["frame.document_layout_pass"]["calls"]
        .as_u64()
        .unwrap();
    assert!((1..=3 * passes as u64).contains(&document_passes), "{cold}");
    std::fs::remove_file(fixture).unwrap();
    for name in ["why.json", "why.txt", "progress.json"] {
        std::fs::remove_file(why.join(name)).unwrap();
    }
    std::fs::remove_dir(why).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
