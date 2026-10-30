//! Тесты блока объявлений: порядок записи, повторы, раскрытие сокращений реестра.
use super::DeclBlock;

#[test]
fn keeps_write_order_and_repeats() {
    let mut d = DeclBlock::new();
    d.push("color", "red", false);
    d.push("width", "0", true);
    d.push("color", "green", false);
    assert_eq!(d.names(), ["color", "width"]);
    assert_eq!(d.last("color"), Some("green"));
    assert_eq!(d.descriptor("width"), None);
    assert_eq!(d.iter().map(|d| d.pos).collect::<Vec<_>>(), [0, 1, 2]);
}

#[test]
fn registry_shorthands_are_pushed_and_set_as_longhands() {
    let mut d = DeclBlock::new();
    d.push("color", "red", false);
    d.push("margin", "1px 2px", true);
    assert_eq!(
        d.names(),
        [
            "color",
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left"
        ]
    );
    assert!(d.iter().skip(1).all(|x| x.pos == 1 && x.important));
    d.set("margin-inline", "3px");
    d.set("margin", "4px");
    assert_eq!(d.last("margin-left"), Some("4px"));
    assert_eq!(d.len(), 7);
}

#[test]
fn set_and_merge_replace_same_names() {
    let mut d = DeclBlock::new();
    d.push("width", "1px", false);
    d.push("height", "2px", false);
    d.push("width", "3px", true);
    d.set("width", "4px");
    assert_eq!(d.len(), 2);
    assert_eq!(d.0[0].value, "4px");
    assert!(!d.0[0].important);
    let mut other = DeclBlock::new();
    other.push("height", "5px", false);
    d.merge(other);
    assert_eq!(d.names(), ["width", "height"]);
    assert_eq!(d.last("height"), Some("5px"));
}
