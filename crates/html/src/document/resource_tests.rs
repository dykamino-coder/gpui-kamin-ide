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
    assert_eq!(
        fonts::alias("BaseTest", doc.state.values()).as_deref(),
        Some("BaseFont49")
    );
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

#[test]
fn resources_come_from_stylesheets_instead_of_markup_substrings() {
    fonts::install_loader(|_| Some("ParsedResourceFont".to_string()));
    let face = "@font-face {font-family: RealResource; src: url(data:font/ttf;base64,AAEAAHJlc291cmNlLVMxNA==)}";
    let fake = face.replace("RealResource", "FakeResource");
    let html = format!(
        "<!-- {fake} --><script>{fake}</script><p>{fake}</p>\
        <template><style>{fake}</style></template><style>{face}</style>"
    );
    let theme = face.replace("RealResource", "ThemeResource");
    let doc = Document::new(&html, &theme);
    assert_eq!(
        fonts::alias("RealResource", doc.state.values()).as_deref(),
        Some("ParsedResourceFont")
    );
    assert_eq!(
        fonts::alias("ThemeResource", doc.state.values()).as_deref(),
        Some("ParsedResourceFont")
    );
    assert_eq!(fonts::alias("FakeResource", doc.state.values()), None);
}

#[test]
fn interleaved_documents_keep_their_font_face_descriptors() {
    fonts::install_loader(|bytes| Some(format!("Face{}", bytes.last().unwrap())));
    let page = |data: &str, adjust: u32, range: &str| {
        format!(
            "<style>@font-face {{font-family: Shared; src: url(data:font/ttf;base64,{data}); \
         size-adjust: {adjust}%; unicode-range: {range}}}</style><p>text</p>"
        )
    };
    let first = Document::new(&page("AAEAADE=", 150, "U+41"), "");
    let second = Document::new(&page("AAEAADI=", 75, "U+20-7F"), "");
    let a = first.state.values();
    let b = second.state.values();
    assert_eq!(fonts::alias("Shared", a).as_deref(), Some("Face49"));
    assert_eq!(fonts::alias("Shared", b).as_deref(), Some("Face50"));
    assert_eq!(fonts::size_adjust("Shared", 0, a), 1.5);
    assert_eq!(fonts::size_adjust("Shared", 0, b), 0.75);
    assert!(!fonts::covers_space("Shared", a));
    assert!(fonts::covers_space("Shared", b));
    fonts::load_faces("", b);
    assert_eq!(fonts::alias("Shared", b), None);
    assert_eq!(fonts::alias("Shared", a).as_deref(), Some("Face49"));
    assert_eq!(fonts::size_adjust("Shared", 0, a), 1.5);
}
