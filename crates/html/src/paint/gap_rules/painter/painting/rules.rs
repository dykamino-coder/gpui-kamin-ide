//! Окраска одного сегмента линейки промежутка на сетке устройства.

use crate::paint::gap_rules::GapRuleSpec;
use crate::paint::gap_rules::gap_segments::segments;
use crate::paint::gap_rules::geometry::GapRun;
use crate::paint::gap_rules::{GapAxisRule, GapLayout, gap_fragment_runs};
use gpui::{Bounds, Pixels, TransformationMatrix, Window};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rule(
    window: &mut Window,
    gap_on_x: bool,
    run: &GapRun,
    rule: &GapAxisRule,
    main_like: bool,
    spec: &GapRuleSpec,
    bounds: Bounds<Pixels>,
    owner: &crate::paint::context::PaintCx,
) {
    let scope = crate::layout::fragment::gap_fragment::current(owner)
        .filter(|_| {
            !spec.vertical
                && spec.kind != GapLayout::Multicol
                && spec.lines_extent != 2
                && window.current_transformation() == TransformationMatrix::unit()
        })
        .map(|mut scope| {
            // Grid tracks stretch through the suppressed gutter's unused
            // tail; flex line decorations end with the line's item edges.
            if spec.kind != GapLayout::Grid {
                scope.end = scope.cut;
            }
            scope
        });
    let local;
    let run = if let Some(scope) = &scope {
        let Some(fragment) = gap_fragment_runs::local_run(run, gap_on_x, scope) else {
            return;
        };
        local = fragment;
        &local
    } else {
        run
    };
    let paint = |window: &mut Window| {
        paint_rule(window, gap_on_x, run, rule, main_like, spec, bounds);
    };
    if let Some(scope) = scope.filter(|s| window.content_mask() == s.mask) {
        // Insets may extend beyond a fragment's cap. The parent still clips
        // overflow, but the content slice must not cut decoration ink.
        window.with_content_mask_replaced(scope.parent, paint);
    } else {
        paint(window);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_rule(
    window: &mut Window,
    gap_on_x: bool,
    run: &GapRun,
    rule: &GapAxisRule,
    main_like: bool,
    spec: &GapRuleSpec,
    _bounds: Bounds<Pixels>,
) {
    if !rule.styles.at(run.index, run.count).unwrap_or(false) {
        return;
    }
    let w = rule.widths.at(run.index, run.count).unwrap_or(0.0);
    if w <= 0.0 {
        return;
    }
    let Some(colour) = rule.colors.at(run.index, run.count) else {
        return;
    };
    let kind = rule.kinds.at(run.index, run.count).unwrap_or(0);
    let c = (run.g0 + run.g1) / 2.0;
    // Отрезок вдоль строчной оси (горизонтальный в горизонтальном
    // письме) при `rtl` считает start/end от правого края.
    let flip = spec.rtl && !spec.vertical && !gap_on_x;
    for (s, e) in segments(run, rule, main_like, flip) {
        let rect = if gap_on_x {
            Bounds {
                origin: gpui::point(gpui::px(c - w / 2.0), gpui::px(s)),
                size: gpui::size(gpui::px(w), gpui::px(e - s)),
            }
        } else {
            Bounds {
                origin: gpui::point(gpui::px(s), gpui::px(c - w / 2.0)),
                size: gpui::size(gpui::px(e - s), gpui::px(w)),
            }
        };
        // Края — к точке устройства, как края коробок раскладки
        // (округление абсолютной координаты, `TaffyLayoutEngine::
        // layout_bounds`): иначе шейдер рисует долю точки, а эталон —
        // абсолютная коробка — ровную строку.
        let scale = window.scale_factor().max(0.01);
        let edge = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
        let (l, t) = (edge(rect.origin.x), edge(rect.origin.y));
        let (r, b) = (
            edge(rect.origin.x + rect.size.width),
            edge(rect.origin.y + rect.size.height),
        );
        let rect = Bounds {
            origin: gpui::point(l, t),
            size: gpui::size(r - l, b - t),
        };
        let third = (w / 3.0).round();
        if (kind == 3 || rule.double) && third >= 1.0 {
            // Two lines across the rule's width, each a third of it
            // (rounded like the `double` border in `render.rs`).
            let (a0, a1) = (c - w / 2.0, c + w / 2.0);
            for (p0, p1) in [(a0, a0 + third), (a1 - third, a1)] {
                let (q0, q1) = (edge(gpui::px(p0)), edge(gpui::px(p1)));
                let band = if gap_on_x {
                    Bounds {
                        origin: gpui::point(q0, t),
                        size: gpui::size(q1 - q0, b - t),
                    }
                } else {
                    Bounds {
                        origin: gpui::point(l, q0),
                        size: gpui::size(r - l, q1 - q0),
                    }
                };
                window.paint_quad(gpui::fill(band, colour.to_hsla()));
            }
            continue;
        }
        if kind == 1 || kind == 2 {
            // Blink paints the rule rect as one border side of the rule's
            // style (`GapDecorationsPainter::Paint` → `DrawBoxSide`), so the
            // dashes/dots follow the border pattern along the rule's length.
            let mut widths = gpui::Edges::<Pixels>::default();
            if gap_on_x {
                widths.left = rect.size.width;
            } else {
                widths.top = rect.size.height;
            }
            let style = if kind == 2 {
                gpui::BorderStyle::Dotted
            } else {
                gpui::BorderStyle::Dashed
            };
            window.paint_quad(gpui::quad(
                rect,
                gpui::Corners::default(),
                gpui::transparent_black(),
                widths,
                colour.to_hsla(),
                style,
            ));
            continue;
        }
        window.paint_quad(gpui::fill(rect, colour.to_hsla()));
    }
}
