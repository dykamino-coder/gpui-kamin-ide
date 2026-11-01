//! Предраскладка фрагментов колонок и линеек промежутков.

use super::copies::prepaint_frag;
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl ColumnStack {
    /// Коробка `clone`, которую план не разрезал, остаётся целой (`slice` того же
    /// вида): её переполнение продолжается параллельным потоком, а не клонами
    /// коробки (css-break-3 §5.4 применяет декорации к фрагментам самой коробки).
    pub(super) fn settle_clone_kids(&mut self) {
        if !self.children.iter().any(|c| c.clone_dec.is_some()) {
            return;
        }
        let (_, _, plan, _) = self.balance(&self.kid_geoms());
        for (kid, c) in self.children.iter_mut().enumerate() {
            if c.clone_dec.is_some() && plan.iter().filter(|f| f.kid == kid).count() < 2 {
                c.clone_dec = None;
            }
        }
    }

    /// Копии `box-decoration-break: clone` — фрагменты итогового плана
    /// (`CloneGeom`): каждая получает свои `(from, h)` и края коробки.
    pub(super) fn apply_clone_frags(&mut self, plan: &[Frag]) {
        for (kid, c) in self.children.iter_mut().enumerate() {
            let (Some(clone), Some(copier)) = (c.clone_frags.clone(), c.copier.clone()) else {
                continue;
            };
            let mut geom: Vec<(f32, f32)> = Vec::new();
            for f in plan.iter().filter(|f| f.kid == kid) {
                if geom.len() <= f.copy {
                    geom.resize(f.copy + 1, (0.0, 0.0));
                }
                geom[f.copy] = (f.from, f.h);
            }
            // Видимый поток за нижним краем коробки (css-break-3 §3) продолжает
            // последний фрагмент: его край отсчитывается от начала содержимого,
            // как будто коробка кончалась ниже — с её нижней декорацией.
            let split = c.clone_dec.filter(|_| geom.len() > 1).map(|dec| {
                let end = if c.over > c.h + crate::tolerance::COORD_EPS {
                    c.over + dec.1
                } else {
                    c.h
                };
                (dec, geom, end)
            });
            let count = c.frags.len() + 1;
            if *clone.borrow() != split {
                *clone.borrow_mut() = split;
                c.el = copier(0, false);
                c.frags = (1..count).map(|part| copier(part, false)).collect();
                c.fragment_builders.clear();
                if let Some(d) = c.clone_deco.as_mut() {
                    d.decos.clear();
                }
            }
            // Украшения фрагментов корня, рисуемого записью, — по одной копии без содержимого.
            if let Some(d) = c.clone_deco.as_mut().filter(|d| d.decos.len() != count) {
                d.decos = (0..count).map(|part| (d.build)(part)).collect();
            }
        }
    }

    /// План колонок по итоговым мерам детей: линии, OOF, владельцы и клоны.
    #[inline(never)]
    pub(super) fn plan_columns(
        &mut self,
        bounds: Bounds<Pixels>,
        col_w: f32,
    ) -> (crate::layout::fragment::plan::Plan, Vec<(usize, f32)>) {
        self.col_w.set(col_w);
        self.settle_clone_kids();
        let heights = self.kid_geoms();
        let (_, lines, mut plan, spans) = self.balance(&heights);
        plan.check_copies(|kid| self.children[kid].frags.len() + 1);
        *self.lines_plan.borrow_mut() = lines;
        self.plan_oof(&plan, bounds);
        self.assign_flex_owners(&plan);
        self.assign_grid_items(&plan);
        self.apply_clone_frags(&plan);
        (plan, spans)
    }

    /// Спаннеры во всю ширину коробки, первой копией (запасных у них нет:
    /// между колонками они не режутся), и границы линеек промежутков.
    #[inline(never)]
    pub(super) fn prepaint_spanners(
        &mut self,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: crate::layout::fragment::plan::Plan,
        spans: Vec<(usize, f32)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for &(kid, sy) in &spans {
            let kid = &mut self.children[kid];
            let h = kid.h;
            kid.el.layout_as_root_at(
                point(bounds.origin.x, bounds.origin.y + px(sy)),
                size(
                    gpui::AvailableSpace::Definite(bounds.size.width),
                    gpui::AvailableSpace::Definite(px(h)),
                ),
                window,
                cx,
            );
            kid.el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        // Границы для линеек промежутков (css-gaps-1 §gap-multicol): коробки
        // ЗАНЯТЫХ колонок каждой линии (как Blink `AddCrossGap` на колонку
        // ряда; в третьем ряду `column-height-009` линейка одна) и спаннеры
        // во всю ширину — они обрывают линейки колонок.
        self.collect_gap_geometry(bounds, col_w, &plan, &spans);
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }

    /// Места статических координат кусков плана (`static_coordinates`).
    #[inline(never)]
    fn record_places(&self, bounds: Bounds<Pixels>, plan: &[Frag], step: f32) {
        use crate::layout::fragment::static_coordinates::Place;
        let Some(places) = &self.static_places else {
            return;
        };
        let mut entries = places.borrow_mut();
        entries.clear();
        entries.extend(plan.iter().map(|part| {
            let (column, row_y) = self.place(part.col);
            let child = &self.children[part.kid];
            Place {
                kid: part.kid,
                from: part.from,
                origin: point(
                    bounds.origin.x + px(column as f32 * step + child.rel.0 + child.par.dx),
                    bounds.origin.y + px(row_y + part.y + child.rel.1 - part.from),
                ),
            }
        }));
    }

    /// Копии кусков плана: каждая раскладывается и проходит prepaint на своём
    /// месте. Копия может нести вложенный многоколоночник, и его prepaint идёт
    /// внутри этого кадра — он держится малым.
    #[inline(never)]
    pub(super) fn prepaint_frags(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: &[Frag],
        step: f32,
    ) {
        self.record_places(bounds, plan, step);
        for f in plan {
            let Some(prepared) =
                self.prepare_prepaint_frag(f, plan, (bounds, col_w, step), window, cx)
            else {
                continue;
            };
            let kid = &mut self.children[f.kid];
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            let segment = gpui::PaintSegmentId(f.col as u64);
            let (paint, scopes) = (&self.paint, &self.oof.slot_scopes);
            let prepaint = |window: &mut Window, cx: &mut App| {
                prepaint_frag(el, paint, scopes, prepared.mapping, segment, window, cx)
            };
            match prepared.replay.as_ref() {
                Some(routes) => {
                    let _ = routes.prepaint(window, cx, prepaint);
                }
                None => prepaint(window, cx),
            }
            // Сохраняем реальную ширину корня для украшений фрагмента.
            kid.laid_w.set(kid.laid_w.get().max(prepared.width));
        }
    }
}
