//! Тесты `rule_shorthands`.

use super::{Inset, gap_rules, inset};

#[test]
fn gap_rule_list_projects_per_longhand() {
    assert_eq!(
        gap_rules("5px dashed red, repeat(2, blue 1px double)"),
        Some(vec![
            "5px, repeat(2, 1px)".to_string(),
            "dashed, repeat(2, double)".to_string(),
            "red, repeat(2, blue)".to_string(),
        ])
    );
    assert_eq!(
        gap_rules("dotted"),
        Some(vec![
            "medium".to_string(),
            "dotted".to_string(),
            "currentcolor".to_string()
        ])
    );
    assert_eq!(gap_rules("solid solid"), None);
}

#[test]
fn inset_copies_junction_and_end() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        inset(Inset::Full, "1px"),
        Some(s(&["1px", "1px", "1px", "1px"]))
    );
    assert_eq!(
        inset(Inset::Full, "1px 2px / 3px"),
        Some(s(&["1px", "2px", "3px", "3px"]))
    );
    assert_eq!(inset(Inset::Cap, "1px 2px"), Some(s(&["1px", "2px"])));
    assert_eq!(inset(Inset::Start, "1px 2px"), None);
}
