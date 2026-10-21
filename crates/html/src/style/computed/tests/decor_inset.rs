//! Тесты разбора и вычисленного значения `text-decoration-inset`
//! (CSS Text Decoration 4 §4.1): негодная запись роняет объявление целиком.

use super::*;

#[test]
fn inset_rejects_an_entire_invalid_declaration() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for invalid in [
        "1",
        "auto 2px",
        "2px 3px 4px",
        "stretch",
        "min-content",
        "normal",
    ] {
        let c = computed(
            &format!("text-decoration-inset: 7px -9px; text-decoration-inset: {invalid}"),
            vcx,
        );
        assert_eq!(
            c.td_inset,
            Some(Some([DecorLen::Px(7.0), DecorLen::Px(-9.0)])),
            "{invalid}"
        );
    }
    assert_eq!(
        computed("text-decoration-inset: 3px", vcx).td_inset,
        Some(Some([DecorLen::Px(3.0); 2]))
    );
    assert_eq!(
        computed("text-decoration-inset: auto", vcx).td_inset,
        Some(None)
    );
}

#[test]
fn inset_computes_font_lengths_without_resolving_percentages() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = computed(
        "font-size: 20px; line-height: 40px; text-decoration-inset: calc(10% + 1em) .5lh",
        vcx,
    );
    c.resolve_em(16.0, vcx);
    assert_eq!(
        c.td_inset,
        Some(Some([DecorLen::Mix(0.1, 20.0), DecorLen::Px(20.0)]))
    );
}

#[test]
fn inset_explicitly_inherits_the_computed_parent_value() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::style::cascade::inherit::inherit_unpainted;
    let parent = inherit_unpainted(
        &Computed::default(),
        &computed("font-size:40px;text-decoration-inset:1em", vcx),
        vcx,
    );
    let child = inherit_unpainted(
        &parent,
        &computed("font-size:20px;text-decoration-inset:inherit", vcx),
        vcx,
    );
    assert_eq!(child.td_inset, Some(Some([DecorLen::Px(40.0); 2])));
    let initial = inherit_unpainted(&parent, &computed("text-decoration-inset:unset", vcx), vcx);
    assert_eq!(initial.td_inset, None);
}
