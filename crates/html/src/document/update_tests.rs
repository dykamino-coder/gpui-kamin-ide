//! Тесты: обновление документа проходит тот же жизненный цикл ресурсов и кэша, что и первая
//! сборка.

use super::Document;
use crate::text::fonts;

#[test]
fn update_reloads_resources_and_matches_a_fresh_document() {
    let fonts = crate::text::metrics::FontContext::default()
        .with_loader(|bytes| Some(format!("UpdateFont{}", bytes.last().unwrap())));
    let old = "<style>@font-face {font-family: OldUpdate;\
        src: url(data:font/ttf;base64,AAEAADE=)}</style><p>old</p>";
    let new = "<style>@font-face {font-family: NewUpdate;\
        src: url(data:font/ttf;base64,AAEAADI=)}</style><p>new</p>";
    let mut updated = Document::builder(old, "")
        .fonts(&fonts)
        .base_uri("file:///tests/page.html")
        .build();
    assert_eq!(
        fonts::alias("OldUpdate", updated.state.values()).as_deref(),
        Some("UpdateFont49")
    );
    let old_salt = updated.salt();
    assert!(updated.update(new, "p {color: red}"));
    assert_ne!(old_salt, updated.salt());
    assert_eq!(fonts::alias("OldUpdate", updated.state.values()), None);
    assert_eq!(
        fonts::alias("NewUpdate", updated.state.values()).as_deref(),
        Some("UpdateFont50")
    );
    let fresh = Document::builder(new, "p {color: red}")
        .fonts(&fonts)
        .base_uri("file:///tests/page.html")
        .build();
    assert_eq!(updated.salt(), fresh.salt());
    assert_eq!(updated.node_count(), fresh.node_count());
    assert_eq!(
        format!("{:?}", updated.nodes()),
        format!("{:?}", fresh.nodes())
    );
    assert_eq!(
        format!("{:?}", updated.root_style()),
        format!("{:?}", fresh.root_style())
    );
    let nodes = updated.nodes().as_ptr();
    assert!(!updated.update(new, "p {color: red}"));
    assert_eq!(nodes, updated.nodes().as_ptr());
}

#[test]
fn update_preserves_xml_parsing() {
    let mut doc = Document::builder("<svg xmlns='http://www.w3.org/2000/svg'><g/></svg>", "")
        .path(std::path::Path::new("update.svg"))
        .build();
    let new = "<svg xmlns='http://www.w3.org/2000/svg'><metadata/><g/></svg>";
    assert!(doc.update(new, ""));
    let fresh = Document::builder(new, "")
        .path(std::path::Path::new("update.svg"))
        .build();
    assert_eq!(format!("{:?}", doc.nodes()), format!("{:?}", fresh.nodes()));
}
