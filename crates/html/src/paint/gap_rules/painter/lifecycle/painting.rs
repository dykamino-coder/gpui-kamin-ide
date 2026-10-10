//! Расчёт и окраска сегментов gap rules.

mod rules;
pub(super) use rules::draw_rule;

use crate::paint::gap_rules::geometry::{
    GAP_EPS, GapItem, GapRun, GridTracks, grid_runs, line_runs, uniq_sorted,
};
use crate::paint::gap_rules::painter::GapRulePainter;
use crate::paint::gap_rules::{GapAxisRule, GapLayout};
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window};

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    group: &mut GapRulePainter,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    _bounds: Bounds<Pixels>,
    _state: &mut LayoutId,
    prepaint: &mut (Bounds<Pixels>, Option<GridTracks>),
    window: &mut Window,
    _cx: &mut App,
) {
    let bounds = prepaint.0;
    let grid_tracks = prepaint.1.take();
    let items = std::mem::take(&mut *group.items.borrow_mut());
    // A grid's gaps come from its track collection, not from its items
    // (css-gaps-1 §gap-grid; Blink `BuildGridTrackGapData`): an empty
    // grid or subgrid still has gaps to decorate
    // (`subgrid-gap-decorations-012/015/016/017`).
    if items.is_empty() && !(group.spec.kind == GapLayout::Grid && grid_tracks.is_some()) {
        return;
    }
    let spec = &group.spec;
    // Физические семейства: промежутки, лежащие по x (линейки
    // вертикальные), и по y. `column-rule` — колонки; в вертикальном
    // письме колонки идут по y.
    let (on_x, on_y) = if spec.vertical {
        (spec.row.as_ref(), spec.col.as_ref())
    } else {
        (spec.col.as_ref(), spec.row.as_ref())
    };
    // (промежуток по x?, линейка, правило, главный промежуток строк?)
    let mut layers: Vec<(bool, GapRun, &GapAxisRule, bool)> = vec![];
    // Ленты с элементом во несколько лент: такой элемент выравнивает
    // ленты по укладке (css-grid-3 §grid-lanes-placement), и картина
    // промежутков — решётка; модель строк его не представляет.
    let kind = match spec.kind {
        GapLayout::Lines { stacked_vertically } if spec.lines_extent == 2 => {
            let it: Vec<GapItem> = items
                .iter()
                .map(|b| GapItem::from_bounds(b, !stacked_vertically))
                .collect();
            let starts = uniq_sorted(it.iter().map(|i| i.a0).collect());
            let spans = it.iter().any(|i| {
                starts
                    .iter()
                    .any(|&s| s > i.a0 + GAP_EPS && s < i.a1 - GAP_EPS)
            });
            // С дорожками раскладки ленты строятся по ним, и элемент во
            // несколько лент представим (запись в каждой ленте).
            if spans && grid_tracks.is_none() {
                GapLayout::Grid
            } else {
                spec.kind
            }
        }
        k => k,
    };
    match kind {
        GapLayout::Grid => {
            let ix: Vec<GapItem> = items
                .iter()
                .map(|b| GapItem::from_bounds(b, true))
                .collect();
            let iy: Vec<GapItem> = items
                .iter()
                .map(|b| GapItem::from_bounds(b, false))
                .collect();
            // Ось `a` прогона — та, ПОПЕРЁК которой лежит промежуток: у
            // линеек, стоящих в промежутках по x, дорожки `a` идут по x, а
            // поперечные `b` — по y; у линеек по y — наоборот.
            let (tx, ty) = (spec.tracks_x.as_deref(), spec.tracks_y.as_deref());
            // Дорожки раскладки: в вертикальном письме сетка уже
            // повёрнута в `apply.rs`, и колонки gpui — физические x.
            let abs_x = grid_tracks
                .as_ref()
                .map(|(c, r)| (c.as_slice(), r.as_slice()));
            let abs_y = grid_tracks
                .as_ref()
                .map(|(c, r)| (r.as_slice(), c.as_slice()));
            if let Some(r) = on_x {
                for mut run in grid_runs(&ix, spec.gap_x, spec.gap_y, r, on_y, tx, ty, abs_x) {
                    if spec.rev_x {
                        run.index = run.count - 1 - run.index;
                    }
                    layers.push((true, run, r, false));
                }
            }
            if let Some(r) = on_y {
                for mut run in grid_runs(&iy, spec.gap_y, spec.gap_x, r, on_x, ty, tx, abs_y) {
                    if spec.rev_y {
                        run.index = run.count - 1 - run.index;
                    }
                    layers.push((false, run, r, false));
                }
            }
        }
        GapLayout::Lines { stacked_vertically } => {
            // Ось укладки строк — `a`: главные промежутки лежат по ней.
            let it: Vec<GapItem> = items
                .iter()
                .map(|b| GapItem::from_bounds(b, !stacked_vertically))
                .collect();
            let (main, cross) = if stacked_vertically {
                (on_y, on_x)
            } else {
                (on_x, on_y)
            };
            let gap_b = if stacked_vertically {
                spec.gap_x
            } else {
                spec.gap_y
            };
            let lane_tracks = (spec.lines_extent == 2)
                .then_some(grid_tracks.as_ref())
                .flatten()
                .map(|(c, r)| {
                    let t = if stacked_vertically {
                        r.clone()
                    } else {
                        c.clone()
                    };
                    (t, gap_b.unwrap_or(0.0))
                })
                .filter(|(t, _)| !t.is_empty());
            let gap_a = if stacked_vertically {
                spec.gap_y
            } else {
                spec.gap_x
            };
            // Главные промежутки лежат по оси укладки строк, поперечные —
            // по оси элементов строки; каждая нумеруется от своего
            // логического начала.
            let (rev_main, rev_cross) = if stacked_vertically {
                (spec.rev_y, spec.rev_x)
            } else {
                (spec.rev_x, spec.rev_y)
            };
            let extent = (spec.lines_extent != 0).then(|| {
                let [pt, pr, pb, pl] = spec.pad;
                let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                let (x1, y1) = (
                    x0 + f32::from(bounds.size.width),
                    y0 + f32::from(bounds.size.height),
                );
                if stacked_vertically {
                    (spec.lines_extent, x0 + pl, x1 - pr)
                } else {
                    (spec.lines_extent, y0 + pt, y1 - pb)
                }
            });
            let (mains, crosses) = line_runs(
                &it,
                gap_a,
                main,
                cross,
                rev_cross,
                extent,
                spec.lanes_content_aligned,
                lane_tracks,
            );
            if let Some(r) = main {
                for mut run in mains {
                    if rev_main {
                        run.index = run.count - 1 - run.index;
                    }
                    layers.push((!stacked_vertically, run, r, true));
                }
            }
            if let Some(r) = cross {
                for run in crosses {
                    layers.push((stacked_vertically, run, r, false));
                }
            }
        }
    }
    // §overlap: по умолчанию ряды поверх колонок — колонки красятся первыми.
    let col_on_x = !spec.vertical;
    let first_on_x = if spec.column_over_row {
        !col_on_x
    } else {
        col_on_x
    };
    let draw =
        |window: &mut Window, gap_on_x: bool, run: &GapRun, rule: &GapAxisRule, main_like: bool| {
            draw_rule(window, gap_on_x, run, rule, main_like, spec, bounds);
        };
    for pass in [true, false] {
        for (on_x, run, rule, main_like) in &layers {
            if (*on_x == first_on_x) == pass {
                draw(window, *on_x, run, rule, *main_like);
            }
        }
    }
}
