//! Document resource loading uses the first effective base before style computation.

use super::Document;
use crate::text::fonts;

#[test]
fn font_loading_resolves_the_first_html_base_before_parsing() {
    let dir = std::env::temp_dir().join(format!("kamin-document-base-{}", std::process::id()));
    let assets = dir.join("шрифты assets");
    std::fs::create_dir_all(&assets).unwrap();
    let font = assets.join("base.ttf");
    std::fs::write(&font, b"\0\x01\0\0\x31").unwrap();
    fonts::install_loader(|bytes| Some(format!("BaseFont{}", bytes.last().unwrap())));
    let source = url::Url::from_file_path(dir.join("page.html")).unwrap();
    let html = "<base href='шрифты assets/'><base href='ignored/'>\
        <style>@font-face {font-family: BaseTest; src: url(base.ttf)}</style>\
        <p style='font-family: BaseTest'>text</p>";
    let doc = Document::new_with_base(html, "", source.as_str());
    assert_eq!(fonts::alias("BaseTest").as_deref(), Some("BaseFont49"));
    assert!(doc.node_count() > 0);
    std::fs::remove_file(font).unwrap();
    std::fs::remove_dir(assets).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn absolute_html_base_works_without_a_source_url() {
    let base = super::base_url::document_base(
        "<base href='file:///tests/assets/'><base href='file:///ignored/'>",
        None,
    );
    assert_eq!(
        base.as_ref().map(url::Url::as_str),
        Some("file:///tests/assets/")
    );
}
