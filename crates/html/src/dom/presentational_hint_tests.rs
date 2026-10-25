//! Тесты представленческих подсказок HTML §15: `hr size` с `color`/`noshade` и
//! без (§15.3.11), разбор неотрицательного целого и устаревшего цвета
//! (§2.3.6: пустая строка и `transparent` не цвет), регистр `ol type`
//! (§15.3.7), нулевая ширина таблицы игнорируется (§15.3.8).

use super::presentational_hr;
use super::presentational_legacy;

fn attrs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// §15.3.11: `size` becomes a height without colour/noshade, a border width with them.
#[test]
fn hr_size_depends_on_color_and_noshade() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let plain = presentational_hr::hints(&attrs(&[("size", "50")]), vcx);
    assert!(plain.contains("height: 48px"), "{plain}");
    let shaded = presentational_hr::hints(&attrs(&[("size", "50"), ("noshade", "")]), vcx);
    assert!(shaded.contains("border-width: 25px") && shaded.contains("border-style: solid"));
    assert_eq!(presentational_hr::non_negative_integer("-0foo"), Some(0));
    assert_eq!(presentational_hr::non_negative_integer("-1"), None);
    assert_eq!(presentational_hr::non_negative_integer(" +7%"), Some(7));
    let one = presentational_hr::hints(&attrs(&[("size", "1")]), vcx);
    assert!(one.contains("border-bottom-width: 0"), "{one}");
}

/// §2.3.6: the empty string and `transparent` are not legacy colours.
#[test]
fn legacy_colors_reject_transparent() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(presentational_hr::legacy_color("", vcx), None);
    assert_eq!(presentational_hr::legacy_color("Transparent", vcx), None);
    assert_eq!(
        presentational_hr::legacy_color("0000ff", vcx).as_deref(),
        Some("#0000ff")
    );
    assert_eq!(
        presentational_hr::legacy_color("green", vcx).as_deref(),
        Some("green")
    );
}

/// §15.3.7: `ol type` letters are case-sensitive; §15.3.8: table width ignores zero.
#[test]
fn list_types_and_table_width() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let lower = presentational_legacy::hints("ol", &attrs(&[("type", "a")]), &[], vcx);
    let upper = presentational_legacy::hints("ol", &attrs(&[("type", "A")]), &[], vcx);
    assert!(lower.contains("lower-alpha") && upper.contains("upper-alpha"));
    let zero = presentational_legacy::hints("table", &attrs(&[("width", "0")]), &[], vcx);
    assert!(!zero.contains("width:"), "{zero}");
}

/// HTML Rendering §15.3.4: br[clear] — подсказка ниже авторских правил.
/// https://html.spec.whatwg.org/multipage/rendering.html#phrasing-content-3
#[test]
fn br_clear_is_a_presentational_hint() {
    fn clear(nodes: &[super::Node]) -> Option<i8> {
        for node in nodes {
            if let super::Node::Element(element) = node {
                if element.tag == "br" {
                    return element.style.cascaded().clear;
                }
                if let Some(value) = clear(&element.children) {
                    return Some(value);
                }
            }
        }
        None
    }
    for (value, expected) in [
        ("left", Some(-1)),
        ("RIGHT", Some(1)),
        ("aLl", Some(0)),
        ("both", Some(0)),
        (" all ", None),
        ("none", None),
    ] {
        let nodes = super::parse(&format!("<br clear='{value}'>"), "");
        assert_eq!(clear(&nodes), expected, "{value}");
    }
    let nodes = super::parse("<style>br { clear: right; }</style><br clear=all>", "");
    assert_eq!(clear(&nodes), Some(1));
}
