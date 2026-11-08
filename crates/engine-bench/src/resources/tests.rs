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
