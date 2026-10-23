//! Покраска слоя линеек: по устройству контейнера (`GapLayout`) пробеги строятся из коробок
//! элементов, дорожек сетки или плана многоколонника — пустая сетка и пустые ленты получают
//! линейки по дорожкам (css-gaps-1 §gap-grid); ленты с элементом в несколько лент без дорожек
//! считаются решёткой; слои красятся в порядке `rule-overlap` (§overlap).

mod grid;
mod rules;
pub(super) use rules::draw_rule;

use crate::layout::multicol::gap_rules::{GapAxisRule, GapLayout};
use crate::paint::gap_rules::geometry::{
    GAP_RULE_EPS, GapItem, GapRun, GridTracks, line_runs, uniq_sorted,
};
use crate::paint::gap_rules::painter::GapRulePainter;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    group: &mut GapRulePainter,
    _bounds: Bounds<Pixels>,
    _state: &mut LayoutId,
    prepaint: &mut (Bounds<Pixels>, Option<GridTracks>),
    window: &mut Window,
    _cx: &mut App,
) {
    let bounds = prepaint.0;
    let grid_tracks = prepaint.1.take();
    let buffer = std::mem::take(&mut *group.items.borrow_mut());
    let (items, margins): (Vec<_>, Vec<_>) = buffer.items.take().into_iter().unzip();
    // A grid's gaps come from its track collection, not from its items
    // (css-gaps-1 §gap-grid; Blink `BuildGridTrackGapData`): an empty
    // grid or subgrid still has gaps to decorate
    // (`subgrid-gap-decorations-012/015/016/017`).
    if items.is_empty()
        && buffer.multicol.is_none()
        && !(group.spec.kind == GapLayout::Grid && grid_tracks.is_some())
        // Grid lanes likewise: main gaps come from the grid-axis tracks
        // (Blink `GridLanesGapAccumulator::BuildMainGaps`), so empty lanes
        // still get grid-axis rules.
        && !(group.spec.lines_extent == 2 && grid_tracks.is_some())
    {
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
                    .any(|&s| s > i.a0 + GAP_RULE_EPS && s < i.a1 - GAP_RULE_EPS)
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
        GapLayout::Multicol => {
            if let Some(data) = buffer.multicol {
                let (columns, rows) = crate::paint::gap_rules::geometry::multicol_runs(&data, spec);
                for (runs, rule, on_x, main) in [
                    (columns, spec.col.as_ref(), !spec.vertical, false),
                    (rows, spec.row.as_ref(), spec.vertical, true),
                ] {
                    if let Some(rule) = rule {
                        layers.extend(runs.into_iter().map(|run| (on_x, run, rule, main)));
                    }
                }
            }
        }
        GapLayout::Grid => layers.extend(grid::layers(&items, spec, grid_tracks.as_ref())),
        GapLayout::Lines { stacked_vertically } => {
            // Ось укладки строк — `a`: главные промежутки лежат по ней.
            let it: Vec<GapItem> = items
                .iter()
                .enumerate()
                .map(|(index, b)| {
                    let mut item = GapItem::from_bounds(b, !stacked_vertically);
                    item.margin_end = margins
                        .get(index)
                        .map_or(0.0, |m| m[usize::from(!stacked_vertically)]);
                    item
                })
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
                (rev_main, rev_cross),
                extent,
                spec.lanes_content_aligned,
                lane_tracks,
                spec.flex_line_stretch.then(|| {
                    let [pt, pr, pb, pl] = spec.pad;
                    if stacked_vertically {
                        (
                            f32::from(bounds.top()) + pt,
                            f32::from(bounds.bottom()) - pb,
                        )
                    } else {
                        (
                            f32::from(bounds.left()) + pl,
                            f32::from(bounds.right()) - pr,
                        )
                    }
                }),
            );
            if let Some(r) = main {
                for run in mains {
                    // Lanes: the grid-axis gaps are grid tracks gaps, so
                    // overlap-join reaches the far edge of the stacking
                    // rule, as in grid (`*-inset-overlap-join`).
                    layers.push((!stacked_vertically, run, r, spec.lines_extent != 2));
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
            draw_rule(
                window,
                gap_on_x,
                run,
                rule,
                main_like,
                spec,
                bounds,
                &group.paint,
            );
        };
    for pass in [true, false] {
        for (on_x, run, rule, main_like) in &layers {
            if (*on_x == first_on_x) == pass {
                draw(window, *on_x, run, rule, *main_like);
            }
        }
    }
}
