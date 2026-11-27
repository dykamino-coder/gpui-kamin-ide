//! Tests of deferred math functions: tree parsing, folding by bases, rejections.

use super::super::{Len, Sum};

fn viewport(w: f32) -> impl FnMut(&mut Sum) -> bool {
    move |s| s.fold_viewport((w, 600.0), false, None)
}

#[test]
fn clamp_over_root_and_viewport_units_folds_at_each_width() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let tree = Len::parse("clamp(2rem, 1rem + 5vw, 4rem)", vcx).unwrap();
    assert!(matches!(tree, Len::Math(_)));
    assert_eq!(tree.resolve_math(vcx, viewport(375.0)), Len::Px(34.75));
    assert_eq!(tree.resolve_math(vcx, viewport(800.0)), Len::Px(56.0));
    assert_eq!(tree.resolve_math(vcx, viewport(1440.0)), Len::Px(64.0));
    // Same declaration, same record: restyles do not grow the arena.
    assert_eq!(Len::parse("clamp(2rem, 1rem + 5vw, 4rem)", vcx), Some(tree));
}

#[test]
fn bases_arrive_in_separate_steps() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let tree = Len::parse("calc(1px + 2 * min(3em, 8vw))", vcx).unwrap();
    let font = tree.resolve_math(vcx, |s| s.fold_font(20.0, || [0.0; 4]));
    assert!(matches!(font, Len::Math(_)));
    assert_ne!(font, tree);
    // A step with nothing to substitute keeps the handle.
    assert_eq!(font.resolve_math(vcx, |s| s.fold_line(10.0)), font);
    assert_eq!(font.resolve_math(vcx, viewport(500.0)), Len::Px(81.0));
    assert_eq!(font.resolve_math(vcx, viewport(1000.0)), Len::Px(121.0));
    assert_eq!(
        font.math_sum(vcx, viewport(1000.0)).map(|s| s.px),
        Some(121.0)
    );
}

#[test]
fn deferred_trees_check_types_and_reject_percentages() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for raw in [
        "min(1px, 2)",
        "clamp(1px, 2vw)",
        "max(1em, 1deg)",
        "min(1em * 1vw, 2px)",
        // A percentage basis is known only to layout.
        "min(300px, 50%)",
    ] {
        assert_eq!(Len::parse(raw, vcx), None, "{raw}");
    }
    // Homogeneous comparisons still fold at parse time.
    assert_eq!(Len::parse("max(1em, 2em, 3em)", vcx), Some(Len::Em(3.0)));
    assert_eq!(
        Len::parse_mixed("calc(min(1em, 2vw) + round(up, max(3vh, 5px), 2px))", vcx)
            .map(|l| matches!(l, Len::Math(_))),
        Some(true)
    );
}

#[test]
fn line_and_glyph_terms_resolve_with_their_bases() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let tree = Len::parse("max(2lh, 1ch + 10px)", vcx).unwrap();
    let glyphs = tree.resolve_math(vcx, |s| s.fold_font(16.0, || [11.0, 8.0, 7.0, 16.0]));
    assert!(tree.math_any(vcx, |s| s.lh != 0.0));
    assert_eq!(
        glyphs.resolve_math(vcx, |s| s.fold_line(5.0)),
        Len::Px(18.0)
    );
    assert_eq!(
        glyphs.resolve_math(vcx, |s| s.fold_line(20.0)),
        Len::Px(40.0)
    );
}
