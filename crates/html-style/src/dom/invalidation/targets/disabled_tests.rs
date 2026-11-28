//! Disabledness depends on attributes of the control and its ancestors.
use super::*;

#[test]
fn board_appearance_disabled_attributes_reach_controls_below_the_changed_ancestor() {
    let (_, section, inside, outside) = nodes();
    let changed = Changed {
        elements: vec![1],
        classes: Vec::new(),
        attrs: vec!["disabled".into()],
    };
    for selector in [":disabled", ":enabled", ":not(:disabled)"] {
        let rules =
            crate::style::css::parse_stylesheet(&format!("{selector}{{color:red}}")).unwrap();
        let found = targets(&rules, &changed, std::slice::from_ref(&section));
        assert!(found.iter().any(|h| Rc::ptr_eq(h, &inside)), "{selector}");
        assert!(!found.iter().any(|h| Rc::ptr_eq(h, &outside)), "{selector}");
    }
}
