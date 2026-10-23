//! Тесты сокращений раскладки, сетки и линеек: раскрытие при разборе в
//! длинные свойства реестра, негодное значение не даёт ничего, значение с
//! подстановкой ждёт подстановки.

use super::*;
use crate::style::css::parse_decls_in;

fn doc() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

/// Раскрытие `name: value` в пары (свойство, текст).
fn expand(name: &str, value: &str) -> Option<Vec<(String, String)>> {
    let doc = doc();
    let block = parse_decls_in(&format!("{name}: {value}"), doc.values());
    let out: Vec<_> = block
        .iter()
        .map(|d| (d.name.clone(), d.value.clone()))
        .collect();
    (!out.is_empty()).then_some(out)
}

fn texts(name: &str, value: &str) -> Option<Vec<String>> {
    expand(name, value).map(|v| v.into_iter().map(|(_, t)| t).collect())
}

#[test]
fn flex_follows_the_basic_values_and_blink_order() {
    assert_eq!(texts("flex", "1").unwrap(), ["1", "1", "0%"]);
    assert_eq!(texts("flex", "none").unwrap(), ["0", "0", "auto"]);
    assert_eq!(texts("flex", "2 3").unwrap(), ["2", "3", "0%"]);
    assert_eq!(texts("flex", "auto").unwrap(), ["1", "1", "auto"]);
    assert_eq!(texts("flex", "10px 2").unwrap(), ["2", "1", "10px"]);
    assert_eq!(texts("flex", "1 2 0").unwrap(), ["1", "2", "0px"]);
    assert_eq!(texts("flex", "content").unwrap(), ["1", "1", "content"]);
    for bad in [
        "1 10px 2",
        "1 2 3",
        "-1",
        "1 -2",
        "10px 20px",
        "auto 1 auto",
    ] {
        assert_eq!(texts("flex", bad), None, "{bad}");
    }
}

#[test]
fn flex_flow_gap_and_columns_fill_omitted_parts() {
    assert_eq!(texts("flex-flow", "wrap").unwrap(), ["row", "wrap"]);
    assert_eq!(
        texts("flex-flow", "column-reverse").unwrap(),
        ["column-reverse", "nowrap"]
    );
    assert_eq!(texts("flex-flow", "row column"), None);
    assert_eq!(texts("gap", "10px").unwrap(), ["10px", "10px"]);
    assert_eq!(texts("grid-gap", "1px 2%").unwrap(), ["1px", "2%"]);
    assert_eq!(texts("gap", "-1px"), None);
    assert_eq!(texts("gap", "auto"), None);
    assert_eq!(
        texts("gap", "calc(10% - 1rem / 2)").unwrap(),
        ["calc(10% - 1rem / 2)", "calc(10% - 1rem / 2)"]
    );
    assert_eq!(
        texts("columns", "2 10em").unwrap(),
        ["10em", "2", "auto", "auto"]
    );
    assert_eq!(
        texts("columns", "auto / 0").unwrap(),
        ["auto", "auto", "0", "auto"]
    );
    for bad in ["8 auto 6em", "0", "2 3", "10%", "2 /"] {
        assert_eq!(texts("columns", bad), None, "{bad}");
    }
}

#[test]
fn place_shorthands_split_multi_word_values_and_copy() {
    assert_eq!(
        texts("place-content", "safe center space-between").unwrap(),
        ["safe center", "space-between"]
    );
    assert_eq!(
        texts("place-content", "last baseline").unwrap(),
        ["last baseline", "start"]
    );
    assert_eq!(
        texts("place-items", "center").unwrap(),
        ["center", "center"]
    );
    assert_eq!(
        texts("place-items", "end legacy").unwrap(),
        ["end", "legacy"]
    );
    assert_eq!(texts("place-items", "auto"), None);
    assert_eq!(texts("place-self", "left"), None);
    assert_eq!(
        texts("place-self", "start right").unwrap(),
        ["start", "right"]
    );
    assert_eq!(texts("place-content", "left"), None);
}

#[test]
fn grid_shorthands_reset_their_longhands() {
    let names = |v: Vec<(String, String)>| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(
        names(expand("grid", "10px / 20px").unwrap()),
        [
            "grid-template-rows",
            "grid-template-columns",
            "grid-template-areas",
            "grid-auto-rows",
            "grid-auto-columns",
            "grid-auto-flow"
        ]
    );
    assert_eq!(
        texts("grid-template", "'a b' 'c d' / 1fr 2fr").unwrap(),
        ["auto auto", "1fr 2fr", "'a b' 'c d'"]
    );
    assert_eq!(texts("grid-template", "'a b' 'c' / 1fr"), None);
    assert_eq!(texts("grid-area", "a").unwrap(), ["a", "a", "a", "a"]);
    assert_eq!(texts("grid-row", "span 2").unwrap(), ["span 2", "auto"]);
    assert_eq!(texts("grid-column", "1 / span x / 3"), None);
}

#[test]
fn rule_shorthands_cover_both_axes() {
    assert_eq!(
        texts("rule", "2px solid red").unwrap(),
        ["2px", "solid", "red", "2px", "solid", "red"]
    );
    assert_eq!(
        texts("column-rule", "repeat(2, 1px dotted)").unwrap(),
        [
            "repeat(2, 1px)",
            "repeat(2, dotted)",
            "repeat(2, currentcolor)"
        ]
    );
    assert_eq!(texts("rule-break", "intersection").unwrap().len(), 2);
    assert_eq!(texts("rule-inset", "1px 2px / 3px").unwrap().len(), 8);
    assert_eq!(
        texts("column-rule-inset-start", "5px").unwrap(),
        ["5px", "5px"]
    );
    assert_eq!(texts("column-rule", "1px solid nonsense"), None);
}

#[test]
fn layout_shorthand_with_var_is_pending_on_every_longhand() {
    let block = expand("place-items", "var(--a) end").unwrap();
    assert_eq!(block.len(), 2);
    let doc = doc();
    let parsed = parse_decls_in("flex: var(--f)", doc.values());
    assert!(parsed.iter().all(|d| {
        d.specified == Specified::Pending(Some(ShorthandId::Flex)) && d.value == "var(--f)"
    }));
    assert_eq!(
        ShorthandId::Flex.component_of(LonghandId::FlexBasis, "2 30px", doc.values()),
        Some("30px".to_string())
    );
}

#[test]
fn webkit_and_grid_legacy_names_alias_the_standard_properties() {
    assert_eq!(legacy_name_alias("-webkit-flex"), Some("flex"));
    assert_eq!(legacy_name_alias("grid-row-gap"), Some("row-gap"));
    assert_eq!(
        expand("-webkit-justify-content", "center").unwrap(),
        [("justify-content".to_string(), "center".to_string())]
    );
}
