//! Grid track arithmetic preserves percentages and viewport dependencies.
use super::*;
use crate::values::DocumentValues;

#[test]
fn board_appearance_grid_tracks_keep_math_and_reject_invalid_components() {
    let doc = DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for value in [
        "min(10px, 20px)",
        "calc(25% + 2em)",
        "10vw",
        "minmax(min(1em, 20px), calc(50% - 10px))",
        "fit-content(calc(25% + 2em))",
        "repeat(2, minmax(0, calc(50% - 1em)))",
        "repeat(auto-fill, minmax(calc(25% - 10px), 1fr))",
    ] {
        assert!(parse_tracks(value, vcx).is_some(), "{value}");
    }
    for value in [
        "repeat(2, 10px nonsense)",
        "repeat(0, 1fr)",
        "minmax(1fr, 20px)",
        "minmax(0, 1fr, 20px)",
        "fit-content(auto)",
        "-1fr",
    ] {
        assert!(parse_tracks(value, vcx).is_none(), "{value}");
    }
    let cx = LenCx {
        font: Some(FontCx::new("", 20.0)),
        ..Default::default()
    };
    let value = Len::parse("calc(25% + 2em)", vcx).unwrap();
    assert_eq!(cx.math_terms(value, vcx), Some((0.25, 40.0)));
    assert_eq!(cx.px(value, vcx), None);
    assert_eq!(
        LenCx {
            basis: Some(400.0),
            ..cx
        }
        .px(value, vcx),
        Some(140.0)
    );
}

#[test]
fn board_appearance_auto_repeat_retains_math_on_both_axes() {
    let doc = DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for name in ["grid-template-columns", "grid-template-rows"] {
        let mut own = Computed::default();
        own.apply_one(
            name,
            "repeat(auto-fill, minmax(calc(25% - 10px), 1fr))",
            vcx,
        );
        let list = if name.ends_with("columns") {
            own.reset_grid.grid_tracks.as_ref()
        } else {
            own.reset_grid.grid_rows.as_ref()
        }
        .expect(name);
        assert!(
            matches!(list.as_slice(), [TrackSize::AutoRepeat { tracks, .. }]
            if matches!(tracks.as_slice(), [TrackSize::MinMax(Track::Font(_), Track::Fr(_))]))
        );
    }
}

#[test]
fn board_appearance_grid_viewport_dependencies_cover_all_track_lists() {
    let doc = DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    for name in [
        "grid-template-columns",
        "grid-template-rows",
        "grid-auto-columns",
        "grid-auto-rows",
    ] {
        let mut own = Computed::default();
        own.apply_one("font-size", "20px", vcx);
        own.apply_one(name, "calc(25% + 2em + 10vw) fit-content(10vh)", vcx);
        let style = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
        assert!(style.has_viewport_lengths(vcx), "{name}");
        let mut resolved = Computed::clone(&style);
        resolved.resolve_viewport((800.0, 600.0), vcx);
        assert!(!resolved.has_viewport_lengths(vcx), "{name}");
        let deferred = resolved
            .reset_grid
            .track_sizes()
            .find_map(|t| match t {
                TrackSize::Single(Track::Font(l)) => Some(*l),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            LenCx {
                font: Some(resolved.font_cx(vcx)),
                ..Default::default()
            }
            .math_terms(deferred, vcx),
            Some((0.25, 120.0)),
            "{name}"
        );
    }
}
