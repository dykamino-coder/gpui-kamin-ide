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

#[test]
fn group_frames_restore_layers_and_keep_rule_order() {
    let rules = parse_stylesheet(
        "@layer a { .a { color:red } @layer b { .b { color:green } }
         .c { color:blue } } .d { color:lime }",
    )
    .unwrap();
    let classes: Vec<_> = rules.iter().map(|r| r.sel.classes[0].as_str()).collect();
    assert_eq!(classes, ["a", "b", "c", "d"]);
    assert_eq!(rules[0].layer, [0, u32::MAX]);
    assert_eq!(rules[1].layer, [0, 0, u32::MAX]);
    assert_eq!(rules[2].layer, [0, u32::MAX]);
    assert_eq!(rules[3].layer, [u32::MAX]);
    assert_eq!(
        rules.iter().map(|r| r.order).collect::<Vec<_>>(),
        [0, 1, 1001, 1000]
    );
}

#[test]
fn media_without_nested_selectors_uses_environment_and_parent() {
    let css = ".a, .b { color:red; @MEDIA(min-width:800px) { color:green;
        @supports (display:grid) { background-color:blue } } color:lime }
        .c { @media print { color:blue } }";
    for width in [375., 800., 1440.] {
        let rules = parse_stylesheet_media(
            css,
            Media {
                width,
                ..Media::default()
            },
        )
        .unwrap();
        let colors: Vec<_> = rules.iter().filter_map(|r| r.decls.last("color")).collect();
        assert_eq!(
            colors,
            if width < 800. {
                vec!["red", "red", "lime", "lime"]
            } else {
                vec!["red", "red", "green", "green", "lime", "lime"]
            }
        );
        assert!(
            rules
                .iter()
                .all(|r| r.sel.classes == ["a"] || r.sel.classes == ["b"])
        );
        assert!(rules.windows(2).all(|r| r[0].order < r[1].order));
    }
}

#[test]
fn nested_layer_frames_restore_parent_layer_and_case() {
    let rules = parse_stylesheet(
        "@layer outer { .a { color:red;
        @layer Case { color:green; @layer deep { color:blue } color:lime }
        @layer case { color:orange } color:black } .b { color:white } }
        .c { color:purple }",
    )
    .unwrap();
    let layers: Vec<_> = rules.iter().map(|r| r.layer.clone()).collect();
    assert_eq!(
        layers,
        vec![
            vec![0, u32::MAX],
            vec![0, 0, u32::MAX],
            vec![0, 0, 0, u32::MAX],
            vec![0, 0, u32::MAX],
            vec![0, 1, u32::MAX],
            vec![0, u32::MAX],
            vec![0, u32::MAX],
            vec![u32::MAX]
        ]
    );
}

#[test]
fn unknown_nested_blocks_do_not_swallow_following_declarations() {
    let rules = parse_stylesheet(
        ".a { color:red; @mediafoo(all) { color:blue }
        @supportsfoo(color:green) { color:blue } @layerfoo { color:blue }
        color:green }",
    )
    .unwrap();
    assert_eq!(
        rules
            .iter()
            .map(|r| r.decls.last("color"))
            .collect::<Vec<_>>(),
        [Some("red"), Some("green")]
    );
}
