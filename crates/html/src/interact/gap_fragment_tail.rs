//! Continue column gap decorations to the edge of a fragment's suppressed gutter.

use super::{GapAxisRule, GapRun, segments};
use gpui::{Bounds, ContentMask, Hsla, Pixels, TransformationMatrix, Window, point, px, size};

pub(crate) fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    run: &GapRun,
    rule: &GapAxisRule,
    color: Hsla,
) {
    let Some(scope) = crate::flow::gap_fragment::current() else {
        return;
    };
    if window.current_transformation() != TransformationMatrix::unit()
        || window.content_mask() != scope.mask
        || (f32::from(bounds.origin.x - scope.root.origin.x)).abs() > 0.01
        || (f32::from(bounds.origin.y - scope.root.origin.y)).abs() > 0.01
        || (f32::from(bounds.size.width - scope.root.size.width)).abs() > 0.01
        || (f32::from(bounds.size.height - scope.root.size.height)).abs() > 0.01
    {
        return;
    }
    let Some(crossing) = run
        .crossings
        .iter()
        .find(|c| (c.lo - scope.cut).abs() < 0.01 && scope.end <= c.hi + 0.01)
    else {
        return;
    };
    // CSS Gaps 1 §fragmentation suppresses the crossing row gutter, but
    // perpendicular column rules still reach the fragmentainer edge.
    // The logical cut excludes the gutter from content. Extend decoration
    // painting only; fixed tracks and spanning item geometry do not grow.
    let mut tail = run.clone();
    tail.crossings.retain(|c| c.lo != crossing.lo);
    tail.r1 = scope.end;
    tail.end_edge = None;
    for hidden in &mut tail.hidden {
        if (hidden.1 - crossing.lo).abs() < 0.01 {
            hidden.1 = scope.end;
        }
    }
    let width = rule.widths.at(run.index, run.count).unwrap_or(0.0);
    let x = (run.g0 + run.g1 - width) / 2.0;
    let scale = window.scale_factor();
    let edge = |v: f32| px((v * scale).round() / scale);
    let mask = ContentMask {
        bounds: Bounds {
            origin: point(scope.mask.bounds.left(), edge(scope.cut)),
            size: size(
                scope.mask.bounds.size.width,
                edge(scope.end) - edge(scope.cut),
            ),
        }
        .intersect(&scope.parent.bounds),
    };
    window.with_content_mask_replaced(mask, |window| {
        for (start, end) in segments(&tail, rule, false, false) {
            let start = start.max(scope.cut);
            let end = end.min(scope.end);
            if end <= start {
                continue;
            }
            window.paint_quad(gpui::fill(
                Bounds {
                    origin: point(edge(x), edge(start)),
                    size: size(edge(x + width) - edge(x), edge(end) - edge(start)),
                },
                color,
            ));
        }
    });
}
