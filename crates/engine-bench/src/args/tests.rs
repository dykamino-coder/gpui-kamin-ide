use super::Args;

#[test]
fn markup_configuration_is_owned_by_each_parse_session() {
    let page = std::env::temp_dir().join("engine-bench-session-config-test.html");
    std::fs::write(&page, "<p>x</p>").unwrap();
    let parse = |extra: &[&str]| {
        Args::parse(
            std::iter::once(page.to_str().unwrap())
                .chain(extra.iter().copied())
                .map(str::to_owned),
        )
    };
    let first = parse(&["--edit-markup", "first.html"]).unwrap().unwrap();
    let second = parse(&["--edit-markup", "second.html"]).unwrap().unwrap();
    assert_eq!(
        first.edit_markup.as_deref(),
        Some(std::path::Path::new("first.html"))
    );
    assert_eq!(
        second.edit_markup.as_deref(),
        Some(std::path::Path::new("second.html"))
    );
    assert!(parse(&[]).unwrap().unwrap().edit_markup.is_none());
    assert!(parse(&["--edit-markup", "a.html", "--edit-markup", "b.html"]).is_err());
    std::fs::remove_file(page).unwrap();
}

#[test]
fn rejects_missing_and_empty_gpu_before_opening_the_input() {
    for options in [
        vec!["missing.html", "--gpu"],
        vec!["missing.html", "--gpu", " "],
    ] {
        let error = Args::parse(options.into_iter().map(str::to_owned))
            .err()
            .unwrap();
        assert!(error.to_string().contains("value") || error.to_string().contains("empty"));
    }
}

#[test]
fn memory_audit_requires_five_warm_frames_and_a_bounded_hold() {
    for options in [
        vec!["--runs", "4", "--memory-settle-ms", "2000"],
        vec!["--mode", "cold", "--memory-settle-ms", "2000"],
        vec!["--mode", "relayout", "--memory-settle-ms", "2000"],
        vec!["--memory-settle-ms", "0"],
        vec!["--memory-settle-ms", "60001"],
    ] {
        let error = Args::parse(
            std::iter::once("missing.html")
                .chain(options)
                .map(str::to_owned),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("memory audit"));
    }
}

/// mimalloc returns pages after about one second; shorter holds measure frame time.
#[test]
fn memory_audit_window_is_at_least_the_two_second_steady_window() {
    let page = std::env::temp_dir().join("engine-bench-settle-window-test.html");
    std::fs::write(&page, "<p>x</p>").unwrap();
    let parse = |ms: &str| {
        Args::parse(
            [page.to_str().unwrap(), "--memory-settle-ms", ms]
                .into_iter()
                .map(str::to_owned),
        )
    };
    for short in ["1", "500", "1999"] {
        let error = parse(short).err().unwrap().to_string();
        assert!(error.contains("memory audit hold must be 2000"), "{error}");
    }
    let args = parse("2000").unwrap().unwrap();
    assert_eq!(args.memory_settle_ms, super::MEMORY_STEADY_WINDOW_MS);
    std::fs::remove_file(&page).unwrap();
}

#[test]
fn surface_defaults_to_offscreen_and_rejects_unknown_values() {
    let page = std::env::temp_dir().join("engine-bench-surface-test.html");
    std::fs::write(&page, "<p>x</p>").unwrap();
    let parse = |extra: &[&str]| {
        Args::parse(
            std::iter::once(page.to_str().unwrap())
                .chain(extra.iter().copied())
                .map(str::to_owned),
        )
    };
    assert_eq!(
        parse(&[]).unwrap().unwrap().surface,
        super::Surface::Offscreen
    );
    let window = parse(&["--surface", "window"]).unwrap().unwrap();
    assert_eq!(window.surface, super::Surface::Window);
    assert!(parse(&["--surface", "hidden"]).is_err());
    std::fs::remove_file(&page).unwrap();
}
