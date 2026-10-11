//! PageStack как элемент gpui: раскладка листов (prepaint) и их краска (paint).

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window,
    point, px, size,
};

impl Element for PageStack {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
    ) -> (LayoutId, ()) {
        // Стопка занимает РОДИТЕЛЯ целиком (окно стенда): число страниц
        // становится известно только после меры детей в `prepaint`, а размер
        // листов подгоняется масштабом, не размером стопки. Измеряемая
        // раскладка здесь не годится: блочный родитель не отдаёт ей
        // определённой высоты, и стопка получала 800x0 — масштаб схлопывался
        // в точку, кадр выходил пустым (первый заход, 06.09).
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
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
        let (kids, limit) = self.page_kids(window, cx, aw);
        // Листы бывают РАЗНЫЕ (`@page :first { size }`, `@page <имя>`), и
        // высота page area решает разрезы, а имя листа — от первой коробки на
        // нём, то есть от тех же разрезов. Укладка повторяется, пока имена
        // листов не перестанут меняться (у одинаковых листов — один проход,
        // байт-в-байт прежний).
        let kid_names: Vec<String> = self.kids.iter().map(|k| k.page.clone()).collect();
        let names_of = |plan: &[Frag], pages: usize| -> Vec<String> {
            let mut out: Vec<Option<String>> = vec![None; pages];
            for f in plan {
                if let Some(slot) = out.get_mut(f.col)
                    && slot.is_none()
                {
                    *slot = Some(kid_names[f.kid].clone());
                }
            }
            let mut last = first_name.clone();
            out.into_iter()
                .map(|n| {
                    if let Some(n) = n {
                        last = n;
                    }
                    last.clone()
                })
                .collect()
        };
        let mut geoms: Vec<PageGeom> = vec![g];
        let mut names: Vec<String> = vec![first_name.clone()];
        let mut result = None;
        for _ in 0..4 {
            let tail = names.last().cloned().unwrap_or_default();
            let at = |c: usize| -> f32 {
                geoms
                    .get(c)
                    .copied()
                    .unwrap_or_else(|| (self.geom_for)(c, &tail))
                    .area
                    .1
                    .max(1.0)
            };
            let (pages, _, plan) = ColumnStack::fill_at(&kids, &at, limit, true);
            let new_names = names_of(&plan, pages);
            let stable = new_names == names;
            names = new_names;
            geoms = names
                .iter()
                .enumerate()
                .map(|(i, n)| (self.geom_for)(i, n))
                .collect();
            result = Some((pages, plan));
            if stable {
                break;
            }
        }
        let (pages, plan) = result.unwrap_or_default();
        // Абсолюты корня добавляют листы, пока не кончится их досягаемость
        // (Blink: «If overflowed by monolithic overflow, we need more pages»,
        // box_fragment_builder.h). Потолок — число копий слоя.
        let icb_pages = if self.icb.is_empty() {
            0
        } else {
            ((self.icb_reach / ah).ceil().max(0.0) as usize).min(self.icb.len())
        };
        let pages = pages.max(icb_pages).max(1);
        // Листы сверх плана (досягаемость абсолютов) — с именем последнего.
        let tail = names.last().cloned().unwrap_or_default();
        while geoms.len() < pages {
            let i = geoms.len();
            geoms.push((self.geom_for)(i, &tail));
        }
        geoms.truncate(pages);
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
        self.prepaint_pages(bounds, window, cx, g, aw, ah, kids, &geoms, &plan, pages);
        // Марджин-боксы — после листов: `counter(pages)` знает их число.
        self.margin_els.clear();
        self.prepaint_margins(bounds, window, cx, &geoms, names, pages, tail);
        *self.plan.borrow_mut() = plan;
        *self.geoms.borrow_mut() = geoms;
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let s = self.grid.get().1;
        let pages = self.pages.get();
        // Масштаб вокруг левого верха стопки; матрица — в точках устройства,
        // как у `interact::Transformed` (interact.rs:1152-1188).
        let k = window.scale_factor();
        let dev = |v: f32| px(v).scale(k);
        let origin = point(
            dev(f32::from(bounds.origin.x)),
            dev(f32::from(bounds.origin.y)),
        );
        let back = point(
            dev(-f32::from(bounds.origin.x)),
            dev(-f32::from(bounds.origin.y)),
        );
        let matrix = gpui::TransformationMatrix::unit()
            .translate(origin)
            .compose(gpui::TransformationMatrix {
                rotation_scale: [[s, 0.0], [0.0, s]],
                translation: [0.0, 0.0],
            })
            .translate(back);
        let plan = self.plan.borrow().clone();
        let masks: Vec<gpui::ContentMask<Pixels>> =
            (0..pages).map(|i| self.area_mask(bounds, i)).collect();
        let rect = |x: f32, y: f32, w: f32, h: f32| Bounds {
            origin: point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
            size: size(px(w), px(h)),
        };
        window.with_transformation(matrix, |window| {
            self.paint_sheets(bounds, cx, s, pages, &plan, masks, rect, window);
        });
        // Повёрнутые листы (`page-orientation`): тот же порядок слоёв, но под
        // СВОЕЙ матрицей — поворот на четверть оборота вокруг листа, затем
        // масштаб стопки. Лист разложен в ячейке как неповёрнутый (левый верх
        // `O`); маски детей и фрагментов заданы в его непреобразованных
        // точках и едут вместе с ним (`with_transformation_masked`: поворот на
        // четверть оборота оси сохраняет).
        self.paint_page_contents(bounds, window, cx, s, pages, k, plan, rect);
    }
}
