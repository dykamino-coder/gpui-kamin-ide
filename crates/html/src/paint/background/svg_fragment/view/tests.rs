//! Regression checks for SVG initial views and Media Fragments error handling.

use super::super::rewrite_svg_fragment;

fn resolved(markup: &str, fragment: &str) -> String {
    String::from_utf8(rewrite_svg_fragment(markup.as_bytes().to_vec(), fragment)).unwrap()
}

fn root_attr(markup: &str, name: &str) -> Option<String> {
    roxmltree::Document::parse(markup)
        .unwrap()
        .root_element()
        .attribute(name)
        .map(str::to_owned)
}

#[test]
fn initial_view_overrides_only_explicit_parameters() {
    let svg = r#"<svg viewBox="0 0 200 100" preserveAspectRatio="xMinYMin" transform="translate(3)"><rect/></svg>"#;
    let actual = resolved(svg, "svgView(viewBox(10,20,30,40);transform(scale(2)))");
    assert_eq!(
        root_attr(&actual, "viewBox").as_deref(),
        Some("10,20,30,40")
    );
    assert_eq!(
        root_attr(&actual, "preserveAspectRatio").as_deref(),
        Some("xMinYMin")
    );
    assert_eq!(
        root_attr(&actual, "transform").as_deref(),
        Some("translate(3) scale(2)")
    );
}

#[test]
fn encoded_view_supports_spaces_and_static_time_segment() {
    let actual = resolved(
        "<svg/>",
        "svgView(viewBox(1%202%203%204)%20%3B%20preserveAspectRatio(none))&t=1,2",
    );
    assert_eq!(root_attr(&actual, "viewBox").as_deref(), Some("1 2 3 4"));
    assert_eq!(
        root_attr(&actual, "preserveAspectRatio").as_deref(),
        Some("none")
    );
}

#[test]
fn malformed_view_spec_keeps_the_original_document() {
    let svg = "<svg viewBox='0 0 200 100'/>";
    for fragment in [
        "svgView(viewBox(0,0,-1,2))",
        "svgView(viewBox(0,0,1,2);unknown(1))",
        "svgView(viewBox(0,0,1,2);viewBox(0,0,3,4))",
        "svgView(viewBox(0,0,1,2);)",
        "svgView(preserveAspectRatio(xMiddleYMiddle))",
        "svgView(viewBox(0,0,1,2);transform(unknown(1)))",
        "svgView(viewBox(0,0,1,2);transform(rotate(90,)))",
        "svgView(viewBox(0,0,1,2))&xywh=0,0,1,1",
    ] {
        assert_eq!(resolved(svg, fragment), svg, "{fragment}");
    }
}

#[test]
fn invalid_view_element_does_not_replace_a_valid_root_box() {
    let svg = "<svg viewBox='0 0 100 100'><view id='bad' viewBox='0 0 -1 20' preserveAspectRatio='none'/></svg>";
    let actual = resolved(svg, "bad");
    assert_eq!(
        root_attr(&actual, "viewBox").as_deref(),
        Some("0 0 100 100")
    );
    assert_eq!(
        root_attr(&actual, "preserveAspectRatio").as_deref(),
        Some("none")
    );
}

#[test]
fn spatial_percent_uses_original_size_and_default_object_size() {
    for (svg, expected) in [
        ("<svg width='200' height='100'/>", "100 0 100 100"),
        ("<svg/>", "150 0 150 150"),
        ("<svg viewBox='0 0 200 200'/>", "75 0 75 150"),
    ] {
        let actual = resolved(svg, "xywh=percent:50,0,50,100");
        assert_eq!(root_attr(&actual, "viewBox").as_deref(), Some(expected));
    }
}

#[test]
fn pixel_clips_are_bounded_only_by_resolved_natural_dimensions() {
    let actual = resolved(
        "<svg width='200' viewBox='0 0 200 200'/>",
        "xywh=100,100,200,200",
    );
    assert_eq!(
        root_attr(&actual, "viewBox").as_deref(),
        Some("100 100 100 100")
    );
    let actual = resolved("<svg viewBox='0 0 200 200'/>", "xywh=100,100,200,200");
    assert_eq!(
        root_attr(&actual, "viewBox").as_deref(),
        Some("100 100 200 200")
    );
}

#[test]
fn spatial_grammar_ignores_invalid_occurrences_and_uses_last_valid_one() {
    let svg = "<svg width='200' height='200'/>";
    for invalid in [
        "100.0,100,100,100",
        "-1,0,100,100",
        "0,0,0,100",
        "percent:90,0,20,10",
    ] {
        assert_eq!(resolved(svg, &format!("xywh={invalid}")), svg);
    }
    let actual = resolved(
        svg,
        "xywh=0,0,50,50&xywh=pixel:100,100,100,100&xywh=invalid",
    );
    assert_eq!(
        root_attr(&actual, "viewBox").as_deref(),
        Some("100 100 100 100")
    );
}
