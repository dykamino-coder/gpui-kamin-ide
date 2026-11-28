//! Scrolling is a pure translation: the frame drawn at a root offset is the
//! frame drawn at offset 0 moved by a whole number of device pixels, at every
//! device scale, whatever fraction of a device pixel the asked offset has.
//! A fractional move would land every box edge, ring and glyph between the
//! pixels each snaps to on its own, so the page would change shape from step
//! to step; the painted scroll translation is aligned to the device grid
//! instead (Gecko aligns the scroll position with device pixels, cc snaps the
//! scroll translation in screen space).

use super::*;

/// Circles with a 1 px ring (border-radius 50 %), a pixel-art SVG icon like
/// a classic radio button, and text, on rows of fractional heights.
fn page() -> String {
    let icon = "url(\"data:image/svg+xml;charset=utf-8,%3Csvg width='12' height='12' \
        xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M4 0h4v1H4zM0 4h1v4H0zM11 4h1v4h-1zM2 \
        1h2v1H2zM8 1h2v1H8zM10 2h1v2h-1zM1 2h1v2H1zM4 11h4v1H4zM2 10h2v1H2zM8 10h2v1H8zM10 \
        8h1v2h-1zM1 8h1v2H1z'/%3E%3C/svg%3E\")";
    let row: String = (0..6)
        .map(|i| format!("<span class=c></span><i class=icon></i>label {i} "))
        .collect();
    let rows: String = (0..120)
        .map(|i| format!("<div class=r>{row} row {i}</div>"))
        .collect();
    format!(
        "<style>body{{margin:0;font:13px sans-serif}}.r{{height:21.3px;padding:1.7px 3px}}\
         .c{{display:inline-block;width:12px;height:12px;border:1px solid #333;\
         border-radius:50%;margin:0 2.3px}}\
         .icon{{display:inline-block;width:12px;height:12px;background:{icon}}}</style>\
         <main>{rows}</main>"
    )
}

/// Every primitive whose ink lies within device rows `[top, bottom)` once
/// moved up by `dy`, left of `right`: kind, ink rect moved, atlas tile.
fn band(cx: &mut VisualTestContext, dy: f32, top: f32, bottom: f32, right: f32) -> Vec<String> {
    let mut out: Vec<String> = cx.update(|window, _| {
        window
            .painted_sequence()
            .into_iter()
            .filter_map(|(text, ink)| {
                let y = ink.origin.y.0 - dy;
                let inside = y >= top
                    && y + ink.size.height.0 <= bottom
                    && ink.origin.x.0 + ink.size.width.0 <= right
                    && ink.size.width.0 > 0.0
                    && ink.size.height.0 > 0.0;
                let kind = text.split(' ').next().unwrap_or_default().to_string();
                let tile = text
                    .find("tile_id: ")
                    .map(|at| text[at..].split(')').next().unwrap_or_default().to_string())
                    .unwrap_or_default();
                inside.then(|| {
                    format!(
                        "{kind} {:?} {:?} {}x{} {tile}",
                        ink.origin.x.0, y, ink.size.width.0, ink.size.height.0
                    )
                })
            })
            .collect()
    });
    out.sort();
    out
}

/// The root translation the frame paints, in device pixels, which must be
/// whole; the scroll position itself stays exact.
fn device_offset(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> f32 {
    let s = cx.update(|window, _| window.scale_factor());
    let d = f32::from(root(view, cx).painted_offset(s).y) * s;
    assert!(
        (d - d.round()).abs() < 1e-3,
        "at scale {s} the root scrolls by {d} device px, not a whole number"
    );
    d.round()
}

/// Fails with the primitives only one of the two frames holds.
fn same(moved: &[String], zero: &[String], name: &str) {
    let only = |a: &[String], b: &[String]| -> Vec<String> {
        a.iter()
            .filter(|x| !b.contains(x))
            .take(8)
            .cloned()
            .collect()
    };
    let (extra, missing) = (only(moved, zero), only(zero, moved));
    assert!(
        extra.is_empty() && missing.is_empty() && moved.len() == zero.len(),
        "{name}: not the frame at 0 moved ({} vs {}); only moved: {extra:?}; only at 0: {missing:?}",
        moved.len(),
        zero.len()
    );
}

/// Root offsets (CSS px, down positive) with fractional device parts at all
/// three scales; the last steps by a fraction of a CSS pixel.
const STEPS: [f32; 6] = [13.3, 53.0, 66.4, 120.7, 121.1, 200.45];

#[gpui::test]
fn the_frame_at_an_offset_is_the_frame_at_zero_moved_by_whole_device_pixels(
    cx: &mut TestAppContext,
) {
    let html = page();
    for scale in [1.0, 1.25, 1.5] {
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &html);
        vcx.simulate_scale_factor_change(scale);
        frame(vcx);
        frame(vcx);
        // The document view `open` makes: 400 x 300 CSS px.
        let (w, h) = (400.0 * scale, 300.0 * scale);
        // Content rows every step shows whole (below what the largest step
        // scrolls out, above the window's bottom edge), clear of the bar.
        let (top, bottom, right) = (6.0 + 201.0 * scale, h - 4.0, w - 30.0);
        let zero = band(vcx, 0.0, top, bottom, right);
        assert!(zero.len() > 20, "scale {scale}: the band holds the page");
        for y in STEPS {
            root(&view, vcx).set_offset(point(px(0.), px(-y)));
            frame(vcx);
            frame(vcx);
            let d = device_offset(&view, vcx);
            let moved = band(vcx, d, top, bottom, right);
            same(
                &moved,
                &zero,
                &format!("scale {scale}, offset {y}, moved by {d} device px"),
            );
        }
    }
}

/// A wheel step by a fraction of a device pixel still moves the frame by
/// whole device pixels, so it replays the frame moved instead of building.
#[gpui::test]
fn fractional_wheel_steps_replay_at_fractional_scales(cx: &mut TestAppContext) {
    let html = page();
    for scale in [1.25, 1.5] {
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &html);
        vcx.simulate_scale_factor_change(scale);
        frame(vcx);
        frame(vcx);
        // The document view `open` makes: 400 x 300 CSS px.
        let (w, h) = (400.0 * scale, 300.0 * scale);
        let (top, bottom, right) = (6.0 + 60.0 * scale, h - 4.0, w - 30.0);
        let zero = band(vcx, 0.0, top, bottom, right);
        for dy in [-13.3, -40.1, -0.7, 27.7] {
            let name = format!("scale {scale}, wheel by {dy}");
            let before = frames(&view, vcx);
            wheel_by(vcx, dy);
            frame(vcx);
            let after = frames(&view, vcx);
            let d = device_offset(&view, vcx);
            assert_eq!(after.0, before.0, "{name}: the step built the frame");
            assert!(after.1 > before.1, "{name}: the step did not replay");
            same(&band(vcx, d, top, bottom, right), &zero, &name);
        }
    }
}
