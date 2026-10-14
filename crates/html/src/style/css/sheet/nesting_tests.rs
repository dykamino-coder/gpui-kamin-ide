//! Тесты раскрытия вложенных правил (css-nesting-1).

use super::*;

#[test]
fn nested_rules_resolve_against_parent_list() {
    let rules = parse_stylesheet(
        ".a, .b { & + .c, &.d { color: green } } div.e { div& { color: red } }
         .f { > g { color: blue } color: lime }",
    );
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
