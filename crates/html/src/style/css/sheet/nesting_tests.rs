//! Тесты раскрытия вложенных правил (css-nesting-1).

use super::*;

#[test]
fn nested_rules_resolve_against_parent_list() {
    let rules = parse_stylesheet(
        ".a, .b { & + .c, &.d { color: green } } div.e { div& { color: red } }
         .f { > g { color: blue } color: lime }",
    )
    .unwrap();
    let pseudos: Vec<Option<String>> = rules.iter().map(|r| r.sel.pseudo.clone()).collect();
    assert_eq!(rules.len(), 5, "{pseudos:?}");
    assert_eq!(rules[1].sel.classes, vec!["d".to_string()]);
    assert_eq!(rules[1].sel.pseudo.as_deref(), Some("is(.a, .b)"));
    assert_eq!(rules[2].sel.tag.as_deref(), Some("div"));
    assert_eq!(rules[2].sel.pseudo.as_deref(), Some("is(div.e)"));
    assert_eq!(rules[3].sel.tag.as_deref(), Some("g"));
    assert_eq!(rules[4].sel.classes, vec!["f".to_string()]);
    assert!(rules[4].order > rules[3].order);
}

#[test]
fn conditional_declarations_keep_parent_and_source_order() {
    let rules = parse_stylesheet(
        ".a { color: red; @supports (color: green) { color: green } color: blue; }
         .b { color: green; @supports (unknown: invalid) { color: red } }",
    )
    .unwrap();
    let colors: Vec<_> = rules.iter().map(|r| r.decls.last("color")).collect();
    assert_eq!(
        colors,
        [Some("red"), Some("green"), Some("blue"), Some("green")]
    );
    assert!(rules[..3].iter().all(|r| r.sel.classes == ["a"]));
    assert!(rules.windows(2).all(|r| r[0].order < r[1].order));
}
