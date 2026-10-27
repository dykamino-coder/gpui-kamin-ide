//! Размер продолжения не наследует исходные пределы CSS.

use super::*;

#[test]
fn continuation_uses_content_edges_without_original_height_bounds() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut e = element(
        Computed {
            height: Some(Len::Px(80.0)),
            min_height: Some(Len::Px(150.0)),
            max_height: Some(Len::Px(180.0)),
            border_box: Some(true),
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(constrain(100.0, &e, 20.0, None, false, doc.values()), 150.0);
    e.marks.spanner_content_box = true;
    assert_eq!(constrain(100.0, &e, 20.0, None, false, doc.values()), 100.0);
    assert_eq!(
        border_size(80.0, &BoxDimensions::for_element(&e, &e.style.used()), 20.0),
        100.0
    );
    assert_eq!(e.style.used().min_height, Some(Len::Px(150.0)));
    assert_eq!(e.style.used().max_height, Some(Len::Px(180.0)));
    assert_eq!(e.style.used().border_box, Some(true));
}
