//! Проверки поведения доменного модуля.
//! Snap cascade regression checks, including invalid declarations and CSS-wide resets.
use crate::style::computed::BoxStyle;
use crate::style::{cascade::inherit::inherit, computed::Computed, values::value::Len};

#[test]
fn snap_is_local_and_explicit_inheritance_copies_only_requested_properties() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one("scroll-snap-type", "both mandatory", vcx);
    parent.apply_one("scroll-snap-align", "end center", vcx);
    parent.apply_one("scroll-padding", "10px 20px", vcx);
    parent.apply_one("scroll-margin", "5px", vcx);
    let mut own = Computed::default();
    assert_eq!(
        inherit(&BoxStyle::from(parent.clone()), &own, vcx)
            .reset_box_layout
            .snap
            .axis,
        0
    );
    own.apply_one("scroll-snap-type", "inherit", vcx);
    own.apply_one("scroll-padding-left", "inherit", vcx);
    let child = inherit(&BoxStyle::from(parent.clone()), &own, vcx);
    assert_eq!(child.reset_box_layout.snap.axis, 5);
    assert_eq!(child.reset_box_layout.snap.strictness, 2);
    assert_eq!(child.reset_box_layout.snap.align, [0; 2]);
    assert_eq!(
        child.reset_box_layout.snap.padding.left,
        Some(Len::Px(20.0))
    );
    assert_eq!(child.reset_box_layout.snap.padding.top, None);
    assert_eq!(child.reset_box_layout.snap.margin.top, None);
    own.apply_one("scroll-snap-type", "initial", vcx);
    assert_eq!(
        inherit(&BoxStyle::from(parent.clone()), &own, vcx)
            .reset_box_layout
            .snap
            .axis,
        0
    );
}

#[test]
fn snap_invalid_values_preserve_valid_declarations() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one("scroll-snap-align", "start end", vcx);
    c.apply_one("scroll-snap-align", "start invalid", vcx);
    assert_eq!(c.reset_box_layout.snap.align, [1, 2]);
    c.apply_one("scroll-padding", "10px", vcx);
    c.apply_one("scroll-padding", "-1px", vcx);
    assert_eq!(c.reset_box_layout.snap.padding.top, Some(Len::Px(10.0)));
    c.apply_one("scroll-margin", "-5px", vcx);
    c.apply_one("scroll-margin", "50%", vcx);
    assert_eq!(c.reset_box_layout.snap.margin.top, Some(Len::Px(-5.0)));
}
