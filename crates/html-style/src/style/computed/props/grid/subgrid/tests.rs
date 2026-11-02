//! Проверки поведения доменного модуля.
use super::{parse, valid};
use crate::style::computed::Computed;
use crate::style::css::{Media, parse_decls_in};
use crate::values::DocumentValues;

#[test]
fn subgrid_line_name_grammar() {
    let state = DocumentValues::detached(Media::default());
    let vcx = state.values();
    for value in [
        "subgrid",
        "subgrid [] [a b]",
        "subgrid[a]",
        "subgrid repeat(auto-fill, [])",
        "subgrid [a] repeat(+2, [b] [c]) [d]",
        "subgrid repeat(2, [a]) repeat(auto-fill, [b])",
        "SUBGRID REPEAT(AUTO-FILL, [])",
        "subgrid repeat(calc(1 + 1), [a])",
        "subgrid repeat(min(3, 2), [a])",
        "subgrid repeat(clamp(1, 2, 3), [a])",
        "subgrid repeat(calc(-1), [a])",
    ] {
        assert!(valid(value, vcx), "{value}");
    }
    for value in [
        "subgridx",
        "subgrid subgrid",
        "subgrid 1px",
        "subgrid [a] 1px",
        "subgrid repeat(auto-fill, 1px)",
        "subgrid repeat(auto-fill, line)",
        "subgrid repeat(auto-fit, [a])",
        "subgrid repeat(0, [a])",
        "subgrid repeat(2.0, [a])",
        "subgrid repeat(calc(2px), [a])",
        "subgrid repeat(calc(2) + calc(2), [a])",
        "subgrid repeat(2,)",
        "subgrid repeat(2, repeat(2, [a]))",
        "subgrid [1a]",
        "subgrid [auto]",
        "subgrid [a]\u{a0}[b]",
        "subgrid repeat(2,\u{a0}[b])",
        "subgrid [span]",
        "subgrid [inherit]",
        "subgrid [ReVeRt-RuLe]",
        "subgrid [[a]]",
        "subgrid [a",
        "subgrid repeat(auto-fill, [a]) repeat(auto-fill, [b])",
    ] {
        assert!(!valid(value, vcx), "{value}");
    }
}

#[test]
fn subgrid_names_use_the_validated_parse() {
    let state = DocumentValues::detached(Media::default());
    let names = parse(
        "SUBGRID [a] REPEAT(calc(1.5), [b]) REPEAT(AUTO-FILL, []) [c]",
        state.values(),
    )
    .unwrap();
    assert_eq!(names.before, vec![vec!["a"], vec!["b"], vec!["b"]]);
    assert_eq!(names.repeat, Some(vec![Vec::<String>::new()]));
    assert_eq!(names.after, vec![vec!["c"]]);
}

#[test]
fn invalid_subgrid_keeps_both_axes_and_names() {
    let state = DocumentValues::detached(Media::default());
    let mut style = Computed::default();
    let vcx = state.values();
    style.apply_decls_with_vars(
        &parse_decls_in("grid-template: subgrid [row] / subgrid [col]", vcx),
        &Default::default(),
        vcx,
    );
    for key in [
        "grid-template-columns",
        "grid-template-rows",
        "grid-template",
        "grid",
    ] {
        for value in [
            "subgrid 1px",
            "subgrid repeat(auto-fill, [bad] 10px)",
            "subgrid repeat(auto-fit, [bad])",
            "subgrid repeat(2, [bad] 10px)",
        ] {
            let value = if matches!(key, "grid" | "grid-template") {
                format!("20px / {value}")
            } else {
                value.to_string()
            };
            style.apply_decls_with_vars(
                &parse_decls_in(&format!("{key}: {value}"), vcx),
                &Default::default(),
                vcx,
            );
            assert!(
                style.reset_grid.subgrid_cols && style.reset_grid.subgrid_rows,
                "{key}: {value}"
            );
            assert_eq!(
                style
                    .reset_grid
                    .grid_col_line_names
                    .as_ref()
                    .unwrap()
                    .before,
                vec![vec!["col".to_string()]]
            );
            assert_eq!(
                style
                    .reset_grid
                    .grid_row_line_names
                    .as_ref()
                    .unwrap()
                    .before,
                vec![vec!["row".to_string()]]
            );
        }
    }
}
