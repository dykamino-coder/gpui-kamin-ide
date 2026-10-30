//! Тесты `grid_shorthands`.

use super::{grid, grid_template, lines};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn grid_line_copies_custom_idents_only() {
    assert_eq!(lines("a", 2), Some(s(&["a", "a"])));
    assert_eq!(lines("2", 2), Some(s(&["2", "auto"])));
    assert_eq!(lines("span a", 2), Some(s(&["span a", "auto"])));
    assert_eq!(lines("a", 4), Some(s(&["a", "a", "a", "a"])));
    assert_eq!(lines("a / b", 4), Some(s(&["a", "b", "a", "b"])));
    assert_eq!(lines("1 / b / 3", 4), Some(s(&["1", "b", "3", "b"])));
    assert_eq!(lines("1 / 2 / 3 / 4 / 5", 4), None);
    assert_eq!(lines("/ 2", 2), None);
}

#[test]
fn grid_template_areas_form_builds_rows_and_areas() {
    assert_eq!(
        grid_template("[a] \"x y\" 40px [b] [c] 'z z' [d] / 1fr 2fr"),
        Some(s(&["[a] 40px [b c] auto [d]", "1fr 2fr", "\"x y\" 'z z'"]))
    );
    assert_eq!(
        grid_template("10px / 20px"),
        Some(s(&["10px", "20px", "none"]))
    );
    assert_eq!(grid_template("10px"), None);
    assert_eq!(grid_template("'a' / repeat(auto-fill, 10px)"), None);
}

#[test]
fn grid_auto_flow_forms_reset_the_other_axis() {
    assert_eq!(
        grid("auto-flow dense 40px / 1fr"),
        Some(s(&["none", "1fr", "none", "40px", "auto", "row dense"]))
    );
    assert_eq!(
        grid("100px / auto-flow"),
        Some(s(&["100px", "none", "none", "auto", "auto", "column"]))
    );
    assert_eq!(
        grid("none"),
        Some(s(&["none", "none", "none", "auto", "auto", "row"]))
    );
    assert_eq!(grid("auto-flow / auto-flow"), None);
}
