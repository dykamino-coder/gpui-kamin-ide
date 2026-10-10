//! Окраска одного сегмента линейки промежутка на сетке устройства.

use crate::paint::gap_rules::GapRuleSpec;
use crate::paint::gap_rules::gap_segments::segments;
use crate::paint::gap_rules::geometry::GapRun;
use crate::paint::gap_rules::{GapAxisRule, GapLayout, gap_fragment_tail};
use gpui::{Bounds, Pixels, Window};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rule(
    window: &mut Window,
    gap_on_x: bool,
    run: &GapRun,
    rule: &GapAxisRule,
    main_like: bool,
    spec: &GapRuleSpec,
    bounds: Bounds<Pixels>,
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
    let c = (run.g0 + run.g1) / 2.0;
    if spec.kind == GapLayout::Grid && !spec.vertical && gap_on_x {
        gap_fragment_tail::paint(window, bounds, run, rule, colour.to_hsla());
    }
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
        if rule.double && third >= 1.0 {
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
        window.paint_quad(gpui::fill(rect, colour.to_hsla()));
    }
}
