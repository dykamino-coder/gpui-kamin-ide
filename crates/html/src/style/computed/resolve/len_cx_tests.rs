//! Тесты канонического резолвера `LenCx`.

use super::*;
use crate::document::state::DocState;
use crate::style::css::Media;
use crate::style::values::value::{px_lossy_zero, px_missing_zero, px_strict};

/// Окно без оси письма и контейнера.
fn window(size: (f32, f32)) -> ViewportCx {
    ViewportCx {
        size,
        vertical: false,
        container: None,
    }
}

/// Только окно, горизонтальное письмо.
fn plain(viewport: Option<(f32, f32)>) -> LenCx<'static> {
    LenCx {
        viewport: viewport.map(window),
        ..LenCx::default()
    }
}

#[test]
fn viewport_units_need_a_viewport() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let cx = plain(Some((800.0, 600.0)));
    assert_eq!(cx.px(Len::Vw(0.5), vcx), Some(400.0));
    assert_eq!(cx.px(Len::Vh(0.5), vcx), Some(300.0));
    assert_eq!(plain(None).px(Len::Vw(0.5), vcx), None);
    // Шрифт и база долей в контексте не заданы — не решаются.
    assert_eq!(cx.px(Len::Em(1.0), vcx), None);
    assert_eq!(cx.px(Len::Pct(0.5), vcx), None);
}

#[test]
fn empty_context_matches_the_context_free_contracts() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let cx = LenCx::default();
    for l in [
        None,
        Some(Len::Auto),
        Some(Len::Px(3.0)),
        Some(Len::Pct(0.5)),
        Some(Len::Em(2.0)),
        Some(Len::Vw(0.1)),
    ] {
        assert_eq!(cx.px_strict(l, vcx), px_strict(l));
        assert_eq!(cx.px_missing_zero(l, vcx), px_missing_zero(l));
        assert_eq!(cx.px_lossy_zero(l, vcx), px_lossy_zero(l));
    }
}

#[test]
fn to_px_is_the_context_without_viewport() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let font = FontCx::new("", 20.0);
    let cx = LenCx {
        basis: Some(200.0),
        font: Some(font),
        viewport: None,
    };
    for l in [Len::Em(2.0), Len::Pct(0.25), Len::Lh(1.0), Len::Vh(1.0)] {
        assert_eq!(cx.px(l, vcx), l.to_px(Some(200.0), font, vcx));
    }
}

#[test]
fn fold_viewport_resolves_plain_and_calc_units() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let vp = window((800.0, 600.0));
    assert_eq!(Len::Vw(0.5).fold_viewport(vp, vcx), Len::Px(400.0));
    assert_eq!(Len::Vh(0.5).fold_viewport(vp, vcx), Len::Px(300.0));
    assert_eq!(Len::Pct(0.5).fold_viewport(vp, vcx), Len::Pct(0.5));
    let vmin = Len::parse("calc(10vmin + 5px)", vcx).unwrap();
    assert_eq!(vmin.fold_viewport(vp, vcx), Len::Px(65.0));
    let vertical = ViewportCx {
        vertical: true,
        ..vp
    };
    let vi = Len::parse("calc(10vi + 1px)", vcx).unwrap();
    assert_eq!(vi.fold_viewport(vertical, vcx), Len::Px(61.0));
}

#[test]
fn viewport_cx_drops_scrollbars_and_reads_the_container() {
    let c = Computed {
        viewport_unit_insets: Some([15.0, 0.0]),
        cq_box: (Some(100.0), None),
        ..Default::default()
    };
    let vp = c.viewport_cx((815.0, 600.0));
    assert_eq!(vp.size, (800.0, 600.0));
    assert!(!vp.vertical);
    assert_eq!(vp.container, Some((100.0, 600.0)));
}

#[test]
fn calc_viewport_terms_fold_with_a_viewport_only() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let l = Len::parse("calc(100vh - 10px)", vcx).unwrap();
    assert_eq!(plain(Some((800.0, 600.0))).px(l, vcx), Some(590.0));
    assert_eq!(plain(None).px(l, vcx), None);
    let vmin = Len::parse("10vmin", vcx).unwrap();
    assert_eq!(plain(Some((800.0, 600.0))).px(vmin, vcx), Some(60.0));
}

#[test]
fn box_viewport_reads_the_writing_axis() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let style = {
        let mut with_inherited = Computed {
            ..Default::default()
        };
        with_inherited.writing_mode =
            Some(crate::style::computed::writing_mode::WritingModeKw::VerticalRl);
        with_inherited
    };
    let vi = Len::parse("10vi", vcx).unwrap();
    assert_eq!(
        LenCx::box_viewport(Some((800.0, 600.0)), &style).px(vi, vcx),
        Some(60.0)
    );
}
