//! PageStack как элемент gpui: раскладка листов (prepaint) и их краска (paint).

use super::PageStack;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window, point, px, size};

impl Hooks for PageStack {
    type Layout = ();
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        // Стопка занимает РОДИТЕЛЯ целиком (окно стенда): число страниц
        // становится известно только после меры детей в `prepaint`, а размер
        // листов подгоняется масштабом, не размером стопки. Измеряемая
        // раскладка здесь не годится: блочный родитель не отдаёт ей
        // определённой высоты, и стопка получала 800x0 — масштаб схлопывался
        // в точку, кадр выходил пустым.
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        // Первый лист — по имени первой коробки (css-page-3 §using-named-pages,
        // п. 3: «the first page … is given the start page value of the root»):
        // его page area — начальный содержащий блок и ширина меры.
        let first_name = self
            .kids
            .first()
            .map(|k| k.page.clone())
            .unwrap_or_default();
        let g = (self.geom_for)(0, &first_name);
        let (aw, ah) = (g.area.0.max(1.0), g.area.1.max(1.0));
        // 1. Мера: ширина — page area, высота — по содержимому. Поля детей
        //    — из меры (`PageKid::mt/mb`), у измеренных без меры они внутри обёртки.
        let area = self.area_logical(&g);
        self.complete.set(false);
        let Ok((kids, mut geoms, names, pages, mut plan)) = self
            .fragment_plan_async(window, cx, g, area, stack)
            .await
            .inspect_err(|error| {
                self.plan.borrow_mut().failure = match error {
                    super::planning::Failure::Continuation(error) => Some(*error),
                    super::planning::Failure::Settlement => None,
                };
            })
        else {
            self.plan.borrow_mut().clear();
            self.plan.borrow_mut().incomplete();
            self.geoms.borrow_mut().clear();
            self.pages.set(0);
            self.margin_els.clear();
            self.canvas_els.clear();
            return;
        };
        // Monolithic overflow reserves pages past the box (`PageKid::reach`).
        let block = self.area_logical(&g).1.max(1.0);
        let reach_pages = plan
            .iter()
            .filter(|f| f.copy == 0)
            .filter_map(|f| {
                let r = self.kids[f.kid].reach?;
                Some(f.col + ((f.y + r) / block - 0.01).ceil().max(1.0) as usize)
            })
            .max()
            .unwrap_or(0)
            .min(self.kids.first().map_or(1, |k| k.frags.len() + 1));
        let pages = pages.max(reach_pages).max(1);
        self.spill_overflow(&mut plan, pages, block);
        // Листы сверх плана (досягаемость абсолютов) — с именем последнего.
        let tail = names.last().cloned().unwrap_or_default();
        while geoms.len() < pages {
            let i = geoms.len();
            geoms.push((self.geom_for)(i, &tail));
        }
        geoms.truncate(pages);
        self.plan_oof_async(
            super::oof::Normal {
                plan: &plan,
                geoms: &geoms,
                names: &names,
                count: pages,
            },
            window,
            cx,
            stack,
        )
        .await;
        // Independent OOF continuations need their own sheets (css-break-3
        // parallel flows): the stream plan, not an estimate, owns the count.
        let oof_pages = self.oof.plan.borrow().iter().map(|f| f.col + 1).max();
        let pages = pages.max(oof_pages.unwrap_or(0));
        while geoms.len() < pages {
            let i = geoms.len();
            geoms.push((self.geom_for)(i, &tail));
        }
        // 2. Сетка листов и масштаб до вмещения в коробку стопки.
        let (ww, wh) = (
            f32::from(bounds.size.width).max(1.0),
            f32::from(bounds.size.height).max(1.0),
        );
        // Ячейка сетки — наибольший лист (у одинаковых — сам лист).
        let (pw, ph) = geoms
            .iter()
            .enumerate()
            .filter(|(i, _)| self.slot(*i).is_some())
            .fold((1.0f32, 1.0f32), |(w, h), (_, g)| {
                (w.max(g.shown().0), h.max(g.shown().1))
            });
        self.cell.set((pw, ph));
        let shown = (0..pages)
            .filter(|&i| self.slot(i).is_some())
            .count()
            .max(1);
        let mut best = (1usize, 0.0f32);
        for per_row in 1..=shown {
            let rows = shown.div_ceil(per_row);
            let s = (ww / (per_row as f32 * pw))
                .min(wh / (rows as f32 * ph))
                .min(1.0);
            if s > best.1 {
                best = (per_row, s);
            }
        }
        self.grid.set(best);
        self.pages.set(pages);
        // 3. Копии раскладываются ЦЕЛИКОМ и поднимаются на срез — ровно как
        //    в `ColumnStack::prepaint`; видимую часть делает маска. Ширина —
        //    page area СВОЕГО листа (`page-size-004`: `width: 50%` на листе
        //    100px — 50, на листах 320px — 160).
        plan.check_copies(|kid| self.kids[kid].frags.len() + 1);
        // Pieces replayed from a main copy place it through the final sheet geometry.
        *self.geoms.borrow_mut() = geoms.clone();
        stack
            .run(|stack| {
                self.prepaint_pages(
                    bounds, window, cx, g, aw, ah, kids, &geoms, &plan, pages, stack,
                )
            })
            .await;
        stack
            .run(|stack| self.prepaint_canvas(bounds, window, cx, &geoms, stack))
            .await;
        *self.geoms.borrow_mut() = geoms.clone();
        if stack
            .run(|stack| self.prepaint_native(bounds, &plan, window, cx, stack))
            .await
            .is_err()
        {
            let mut oof = self.oof.plan.borrow_mut();
            oof.failure = Some(crate::layout::fragment::token::Error::InvalidGeometry);
            oof.incomplete();
        }
        // Марджин-боксы — после листов: `counter(pages)` знает их число.
        self.margin_els.clear();
        stack
            .run(|stack| {
                self.prepaint_margins(bounds, window, cx, &geoms, names, pages, tail, stack)
            })
            .await;
        *self.plan.borrow_mut() = plan;
        *self.geoms.borrow_mut() = geoms;
        self.complete.set(true);
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, state, prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        bounds: Bounds<Pixels>,
        _state: &'a mut Self::Layout,
        _prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        self.paint_sheets_async(bounds, window, cx, stack)
    }
}
mod paint;
