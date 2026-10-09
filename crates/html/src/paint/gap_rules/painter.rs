//! Элемент покраски правил промежутков.
// owner: A

use crate::paint::gap_rules::gap_segments::segments;
use crate::paint::gap_rules::geometry::{GAP_EPS, GapItem, GapRun, GridTracks, grid_runs, line_runs, uncollapsed, uniq_sorted};
use crate::paint::gap_rules::{GapAxisRule, GapItems, GapLayout, GapRuleSpec, gap_fragment_tail};
use gpui::{App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window, px};

/// Слой линеек промежутков. Забирает буфер проб в `paint` (к этому моменту
/// prepaint всех детей уже прошёл — так же работает `EdgePainter`), строит
/// геометрию промежутков по границам элементов и красит отрезки линеек
/// (css-gaps-1 §geometry, §break, §inset, §visibility-items, §lists).
pub struct GapRulePainter {
    pub(crate) items: GapItems,
    pub(crate) spec: GapRuleSpec,
}

impl GapRulePainter {
    pub fn new(items: GapItems, spec: GapRuleSpec) -> Self {
        GapRulePainter { items, spec }
    }
}

impl Element for GapRulePainter {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (Bounds<Pixels>, Option<GridTracks>);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        // Художник занимает паддинг-бокс контейнера: от него считается поле
        // содержимого (протяжённость главных промежутков строк и лент).
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> (Bounds<Pixels>, Option<GridTracks>) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        let tracks = (self.spec.kind == GapLayout::Grid || self.spec.lines_extent == 2)
            .then(|| window.parent_grid_tracks(*state))
            .flatten()
            .map(|(o, cols, rows)| {
                let (ox, oy) = (f32::from(o.x), f32::from(o.y));
                let gx = self.spec.gap_x.unwrap_or(0.0);
                let gy = self.spec.gap_y.unwrap_or(0.0);
                (
                    uncollapsed(cols.iter().map(|&(a, b)| (ox + a, ox + b)).collect(), gx),
                    uncollapsed(rows.iter().map(|&(a, b)| (oy + a, oy + b)).collect(), gy),
                )
            });
        (bounds, tracks)
    }

    fn paint(
        &mut self,
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
        let items = std::mem::take(&mut *self.items.borrow_mut());
        // A grid's gaps come from its track collection, not from its items
        // (css-gaps-1 §gap-grid; Blink `BuildGridTrackGapData`): an empty
        // grid or subgrid still has gaps to decorate
        // (`subgrid-gap-decorations-012/015/016/017`).
        if items.is_empty() && !(self.spec.kind == GapLayout::Grid && grid_tracks.is_some()) {
            return;
        }
        let spec = &self.spec;
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
                let spans = it
                    .iter()
                    .any(|i| starts.iter().any(|&s| s > i.a0 + GAP_EPS && s < i.a1 - GAP_EPS));
                // С дорожками раскладки ленты строятся по ним, и элемент во
                // несколько лент представим (запись в каждой ленте).
                if spans && grid_tracks.is_none() { GapLayout::Grid } else { spec.kind }
            }
            k => k,
        };
        match kind {
            GapLayout::Grid => {
                let ix: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, true)).collect();
                let iy: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, false)).collect();
                // Ось `a` прогона — та, ПОПЕРЁК которой лежит промежуток: у
                // линеек, стоящих в промежутках по x, дорожки `a` идут по x, а
                // поперечные `b` — по y; у линеек по y — наоборот.
                let (tx, ty) = (spec.tracks_x.as_deref(), spec.tracks_y.as_deref());
                // Дорожки раскладки: в вертикальном письме сетка уже
                // повёрнута в `apply.rs`, и колонки gpui — физические x.
                let abs_x = grid_tracks.as_ref().map(|(c, r)| (c.as_slice(), r.as_slice()));
                let abs_y = grid_tracks.as_ref().map(|(c, r)| (r.as_slice(), c.as_slice()));
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
                let (main, cross) = if stacked_vertically { (on_y, on_x) } else { (on_x, on_y) };
                let gap_b = if stacked_vertically { spec.gap_x } else { spec.gap_y };
                let lane_tracks = (spec.lines_extent == 2)
                    .then_some(grid_tracks.as_ref())
                    .flatten()
                    .map(|(c, r)| {
                        let t = if stacked_vertically { r.clone() } else { c.clone() };
                        (t, gap_b.unwrap_or(0.0))
                    })
                    .filter(|(t, _)| !t.is_empty());
                let gap_a = if stacked_vertically { spec.gap_y } else { spec.gap_x };
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
        let first_on_x = if spec.column_over_row { !col_on_x } else { col_on_x };
        let draw = |window: &mut Window, gap_on_x: bool, run: &GapRun, rule: &GapAxisRule, main_like: bool| {
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
                            Bounds { origin: gpui::point(q0, t), size: gpui::size(q1 - q0, b - t) }
                        } else {
                            Bounds { origin: gpui::point(l, q0), size: gpui::size(r - l, q1 - q0) }
                        };
                        window.paint_quad(gpui::fill(band, colour.to_hsla()));
                    }
                    continue;
                }
                window.paint_quad(gpui::fill(rect, colour.to_hsla()));
            }
        };
        for pass in [true, false] {
            for (on_x, run, rule, main_like) in &layers {
                if (*on_x == first_on_x) == pass {
                    draw(window, *on_x, run, rule, *main_like);
                }
            }
        }
    }
}

impl IntoElement for GapRulePainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
