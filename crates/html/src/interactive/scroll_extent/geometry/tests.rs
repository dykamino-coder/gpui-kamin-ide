//! Overflow bounds regression checks, independent of painting and DOM selectors.
use super::*;

#[test]
fn clip_margin_exposes_only_the_intersection_of_descendant_overflow() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one("overflow", "clip", vcx);
    c.apply_one("overflow-clip-margin", "10px", vcx);
    let own = [0.0, 0.0, 100.0, 100.0];
    let child = [-50.0, -50.0, 150.0, 150.0];
    assert_eq!(
        Clip::new(&c).intersect(own, child),
        Some([-10.0, -10.0, 110.0, 110.0])
    );
    c.apply_one("overflow-clip-margin", "100px", vcx);
    assert_eq!(Clip::new(&c).intersect(own, child), Some(child));
}

#[test]
fn paint_containment_clips_overflow_but_hidden_ignores_clip_margin() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed {
        contain_paint: Some(true),
        clip_margin: Some(10.0),
        ..Default::default()
    };
    let own = [0.0, 0.0, 100.0, 100.0];
    let child = [-50.0, -50.0, 150.0, 150.0];
    assert_eq!(
        Clip::new(&c).intersect(own, child),
        Some([-10.0, -10.0, 110.0, 110.0])
    );
    c.contain_paint = None;
    c.apply_one("overflow", "hidden", vcx);
    assert_eq!(Clip::new(&c).intersect(own, child), Some(own));
}
