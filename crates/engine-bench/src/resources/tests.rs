use super::*;
#[test]
fn linked_css_keeps_order_and_its_own_resource_base() {
    let dir = std::env::temp_dir().join(format!("kamin-bench-resources-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("css")).unwrap();
    std::fs::write(dir.join("css/asset.png"), b"fixture").unwrap();
    std::fs::write(dir.join("css/import.css"), "p { color: green }").unwrap();
    std::fs::write(
        dir.join("css/main.css"),
        "@import 'import.css'; p {background:url(asset.png)}",
    )
    .unwrap();
    let path = dir.join("input.html");
    std::fs::write(&path, "<!doctype html><meta charset=utf-8><link rel=stylesheet href=css/main.css><style>p{color:blue}</style><p>x").unwrap();
    let html = read(&path).unwrap();
    assert!(html.contains("color: green"));
    assert!(html.find("color: green") < html.find("color:blue"));
    assert!(
        html.contains(
            Url::from_file_path(dir.join("css/asset.png"))
                .unwrap()
                .as_str()
        )
    );
    for file in [
        "css/asset.png",
        "css/import.css",
        "css/main.css",
        "input.html",
    ] {
        std::fs::remove_file(dir.join(file)).unwrap();
    }
    std::fs::remove_dir(dir.join("css")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

/// Без токенов хоста быстрый путь отдаёт ровно то, что вернул бы полный разбор.
#[test]
fn source_without_host_tokens_matches_the_full_rewrite() {
    let base = Url::parse("file:///C:/bench/input.html").unwrap();
    for html in [
        "<!doctype html><style>p { color: red }</style><p>a < b (c) @media</p>",
        "<div class=row>Row 1 <b>alpha</b> beta</div><stylex>url</stylex>",
        "",
    ] {
        assert!(!host_tokens(html), "{html}");
        assert_eq!(rewrite(base.clone(), html.to_owned()).unwrap(), html);
    }
    for html in [
        "<LINK rel=stylesheet href=a.css>",
        "<Image src=a.png>",
        "<style>p { background: URL(a.png) }</style>",
        "<style>@IMPORT 'a.css';</style>",
        "<svg><style>&#117;rl(a.png)</style></svg>",
        "<object data=a></object>",
    ] {
        assert!(host_tokens(html), "{html}");
    }
}

/// UTF-8 вход без замен заимствуется из переиспользуемого буфера.
#[test]
fn utf8_source_is_borrowed_from_the_reused_buffer() {
    let path = std::env::temp_dir().join(format!("kamin-bench-borrow-{}.html", std::process::id()));
    std::fs::write(&path, "<style>p{color:blue}</style><p>x").unwrap();
    let mut source = Vec::with_capacity(4096);
    let capacity = source.capacity();
    let html = read_into(&path, &mut source).unwrap();
    assert!(matches!(
        html,
        Cow::Borrowed("<style>p{color:blue}</style><p>x")
    ));
    drop(html);
    assert_eq!(source.capacity(), capacity);
    std::fs::remove_file(path).unwrap();
}

fn recover_string_fixture(name: &str, external: bool, eof: bool) {
    let dir = std::env::temp_dir().join(format!(
        "kamin-bench-css-string-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // The literal URL forces the inline fixture through the host rewrite too.
    let css = if eof {
        "/* url(no) */ p { color: blue; content: \"unfinished"
    } else {
        "/* url(no) */ p { content: \"broken\n; color: blue } b { color: green }"
    };
    let html = if external {
        std::fs::write(dir.join("input.css"), css).unwrap();
        "<link rel=stylesheet href=input.css><p>fixture</p><b>after</b>".to_owned()
    } else {
        format!("<style>{css}</style><p>fixture</p><b>after</b>")
    };
    let path = dir.join("input.html");
    std::fs::write(&path, html).unwrap();
    let prepared = read(&path).expect("CSS syntax recovery must not abort page loading");
    assert!(
        prepared.contains(css),
        "the host must retain the CSS source"
    );
    kamin_html::dom::try_parse(&prepared, "").expect("the prepared document must parse");
    let rules = kamin_html::css::parse_stylesheet(css).unwrap();
    assert_eq!(rules[0].decls.last("color"), Some("blue"));
    if eof {
        assert_eq!(rules[0].decls.last("content"), Some("\"unfinished\""));
    } else {
        assert!(!rules[0].decls.contains("content"));
        assert_eq!(rules[1].decls.last("color"), Some("green"));
    }
    std::fs::remove_file(path).unwrap();
    if external {
        std::fs::remove_file(dir.join("input.css")).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn external_css_recovers_a_bad_string_in_the_middle() {
    recover_string_fixture("external-middle", true, false);
}

#[test]
fn external_css_keeps_a_string_at_eof() {
    recover_string_fixture("external-eof", true, true);
}

#[test]
fn inline_style_recovers_a_bad_string_in_the_middle() {
    recover_string_fixture("inline-middle", false, false);
}

#[test]
fn inline_style_keeps_a_string_at_eof() {
    recover_string_fixture("inline-eof", false, true);
}

#[test]
fn escaped_selector_quotes_do_not_hide_following_resources() {
    let dir = std::env::temp_dir().join(format!(
        "kamin-bench-css-escaped-selector-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("asset.png"), b"fixture").unwrap();
    let path = dir.join("input.html");
    let css = r#".escaped\'quote\' { color: blue } p { background: url(asset.png) }"#;
    std::fs::write(&path, format!("<style>{css}</style><p>fixture</p>")).unwrap();
    let prepared = read(&path).unwrap();
    assert!(prepared.contains(r#".escaped\'quote\'"#));
    let asset = Url::from_file_path(dir.join("asset.png")).unwrap();
    assert!(prepared.contains(&format!("url(\"{asset}\")")));
    for file in ["asset.png", "input.html"] {
        std::fs::remove_file(dir.join(file)).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

/// Unavailable subresources leave a usable document and preserve URL identity.
#[test]
fn missing_subresources_do_not_abort_the_document_or_drop_sibling_styles() {
    let base = Url::parse("file:///C:/missing-bench-fixture/input.html").unwrap();
    let source = "<link rel=stylesheet href=missing.css><link rel=stylesheet href=https://example.test/a.css><style>@import 'missing-import.css';p{color:green;background:url(missing.png)}</style><img src=missing-image.png><p>still visible</p>";
    let prepared = rewrite(base, source.into()).unwrap();
    assert!(prepared.contains("p{color:green"));
    assert!(prepared.contains("file:///C:/missing-bench-fixture/missing.png"));
    assert!(prepared.contains("src=\"missing-image.png\""));
    assert!(prepared.contains("still visible"));
    kamin_html::dom::try_parse(&prepared, "").unwrap();
}

/// A stylesheet, an import or an image the snapshot lacks is a failed fetch: the page still loads, the stylesheet
/// is not applied and the URLs are recorded for the report.
#[test]
fn unavailable_resources_are_failed_fetches_not_errors() {
    let dir = std::env::temp_dir().join(format!("kamin-bench-unavailable-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("input.html");
    std::fs::write(
        &path,
        "<!doctype html><link rel=stylesheet href=gone.css><link rel=stylesheet href=https://example.test/a.css>\
         <style>@import 'gone-import.css'; p{color:blue}</style><img src=gone.png><p>x",
    )
    .unwrap();
    let html = read(&path).unwrap();
    assert!(html.contains("p{color:blue}") && html.contains("gone.png"));
    let seen = super::stylesheet::unavailable();
    for name in ["gone.css", "gone-import.css", "gone.png"] {
        let url = Url::from_file_path(dir.join(name)).unwrap();
        assert!(seen.contains(&url.to_string()), "{name}");
    }
    assert!(seen.contains(&"https://example.test/a.css".to_string()));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
