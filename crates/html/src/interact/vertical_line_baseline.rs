//! A sideways line inherits the actual baseline of its formatted descendants.
//! Font metrics remain the fallback when the subtree has no baseline set.
use gpui::{AnyElement, App, AvailableSpace, Font, Pixels, Size, Style, Window, size};

fn project(height: Pixels, baseline: Pixels, ccw: bool, lr: bool) -> (f32, bool) {
    if ccw {
        (f32::from(baseline), false)
    } else if lr {
        (f32::from(height - baseline), false)
    } else {
        (f32::from(baseline), true)
    }
}

pub(crate) fn apply(
    style: &mut Style,
    child: &mut AnyElement,
    natural: Size<Pixels>,
    ccw: bool,
    lr: bool,
    first_line: Option<&(Font, Pixels, Option<Pixels>, bool)>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((font, font_size, line_height, central)) = first_line else {
        return;
    };
    if !central {
        let mut snapshot = child.snapshot_layout_measurement(window, cx);
        let measured = snapshot.measure_physical_baselines(
            size(
                AvailableSpace::Definite(natural.width),
                AvailableSpace::MaxContent,
            ),
            window,
            cx,
        );
        if let Some(baseline) = measured.first_y {
            style.baseline_x_hint = Some(project(measured.size.height, baseline, ccw, lr));
            return;
        }
    }
    let ts = window.text_system();
    let id = ts.resolve_font(font);
    let ascent = ts.ascent(id, *font_size);
    let descent = ts.descent(id, *font_size).abs();
    let height = line_height.unwrap_or(ascent + descent);
    let baseline = (height - ascent - descent) / 2.0 + ascent;
    style.baseline_x_hint = Some(if *central {
        (f32::from(height / 2.0), !ccw && !lr)
    } else {
        project(height, baseline, ccw, lr)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn descendant_baseline_includes_box_edges_in_each_vertical_flow() {
        // A 43px line containing a descendant baseline 31px from the top.
        assert_eq!(project(px(43.), px(31.), false, true), (12., false));
        assert_eq!(project(px(43.), px(31.), false, false), (31., true));
        assert_eq!(project(px(43.), px(31.), true, true), (31., false));
    }
}
