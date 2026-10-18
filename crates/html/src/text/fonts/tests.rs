//! Tests for fonts; split out to keep the owning module within 250 lines.

use super::loading::range_has_space;
use super::*;

#[test]
fn face_rule_gives_family_and_file() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let css = "@font-face { font-family: 'мой'; src: url('/f/a.woff2') format('woff2'), \
                   url('/f/a.woff') format('woff') } p { color: red }";
    let block = faces(css, vcx);
    assert_eq!(block.len(), 1);
    assert_eq!(
        declaration(&block[0], "font-family").as_deref(),
        Some("'мой'")
    );
    // Обе упаковки нам по силам, поэтому берётся первая же.
    assert_eq!(source(&block[0]).as_deref(), Some("/f/a.woff2"));
}

#[test]
fn only_font_face_blocks_are_taken() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let css = "p { font-family: 'нет' } @font-face { font-family: 'да'; src: url(a.ttf) }";
    let block = faces(css, vcx);
    assert_eq!(block.len(), 1);
    assert_eq!(
        declaration(&block[0], "font-family").as_deref(),
        Some("'да'")
    );
}

#[test]
fn space_decides_the_first_available_font() {
    // Дескриптора нет — покрыт весь набор знаков.
    assert!(range_has_space(None));
    // Одиночный знак и отрезок.
    assert!(range_has_space(Some("U+20")));
    assert!(!range_has_space(Some("U+0061")));
    assert!(range_has_space(Some("U+0-7F")));
    assert!(!range_has_space(Some("U+0021-00FF")));
    // Несколько кусков: хватает одного.
    assert!(range_has_space(Some("U+20,U+41-5A")));
    assert!(!range_has_space(Some("U+0061, U+0062")));
    // Маска.
    assert!(range_has_space(Some("U+00??")));
    assert!(!range_has_space(Some("U+04??")));
    // Негодная запись — дескриптор недействителен, умолчание покрывает всё.
    assert!(range_has_space(Some("мусор")));
}

#[test]
fn data_url_source_survives_its_separators() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // `;` и `,` внутри адреса `data:` не режут ни правило, ни список `src`.
    let css = "@font-face { font-family: 'd'; \
               src: url(data:font/woff2;charset=utf-8;base64,d09GMg==) format('woff2'); \
               font-weight: normal }";
    let block = faces(css, vcx);
    assert_eq!(
        source(&block[0]).as_deref(),
        Some("data:font/woff2;charset=utf-8;base64,d09GMg==")
    );
    assert_eq!(
        declaration(&block[0], "font-weight").as_deref(),
        Some("normal")
    );
}
