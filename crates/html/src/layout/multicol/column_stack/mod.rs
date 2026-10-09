//! Элемент стопки колонок `ColumnStack`.
// owner: A

use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::types::{
    Frag, Intrinsic, Kid, RepeatGeom, Rows, StackAxis, StackChild, axis_box,
};
use gpui::{App, Bounds, IntoElement, Pixels, Window, point, px, size};

pub mod avoid;
pub mod balance;
pub mod element;
pub mod fill;

pub struct ColumnStack {
    pub(crate) children: Vec<StackChild>,
    pub(crate) count: usize,
    pub(crate) gap: f32,
    /// Ось прогрессии колонок (`StackAxis`); при вертикальном письме —
    /// вертикальная. Ставится `with_axis`.
    pub(crate) axis: StackAxis,
    /// Смещение начала рядов: вложенный многоколоночник начат на `row_phase`
    /// ниже верха внешней колонки, и его ПЕРВЫЙ ряд — остаток `h − row_phase`
    /// (Blink `column_layout_algorithm.cc:818-828` `available_outer_space =
    /// FragmentainerSpaceLeftForChildren() − line_offset`). Ставится
    /// `with_row_phase`; ноль — ряды от верха.
    pub(crate) row_phase: f32,
    /// `column-fill: auto` + заданная высота: заполнение без баланса.
    pub(crate) fixed_height: Option<f32>,
    /// Линейка между колонками: ширина и цвет.
    pub(crate) rule: Option<(f32, gpui::Hsla)>,
    pub(crate) rule_to: Option<f32>,
    /// Ряды колонок; `None` — одна линия, как в css-multicol-1.
    pub(crate) rows: Option<Rows>,
    /// Сколько копий у ребёнка (= сколько колонок он может занять).
    pub(crate) copies: usize,
    /// Внутренние размеры контейнера, если его ширину решает СОДЕРЖИМОЕ.
    /// `None` — ширину даёт родитель, и мерить детей незачем: лишний проход
    /// раскладки стоит дороже, чем всё остальное в этом элементе.
    pub(crate) intrinsic: Option<Intrinsic>,
    /// Буфер границ колонок и спаннеров для `GapRulePainter` (css-gaps-1
    /// §gap-multicol): колонки линии — элементы строки, спаннер — элемент во
    /// всю ширину. Заполняется в `prepaint`, художник забирает в `paint`.
    pub(crate) gap_items: Option<crate::paint::gap_rules::GapItems>,
    pub(crate) plan: std::cell::RefCell<Vec<Frag>>,
    pub(crate) col_w: std::cell::Cell<f32>,
    /// Линии колонок после укладки: `(y, высота)` каждой — линейкам и
    /// смещениям. Колонка `col` стоит в линии `col / count`. Без спаннеров
    /// линия = ряд; спаннер режет ряд на линии (Blink `LayoutLine`).
    pub(crate) lines_plan: std::cell::RefCell<Vec<(f32, f32)>>,
    /// Спаннеры после укладки: `(ребёнок, y)`.
    pub(crate) spans_plan: std::cell::RefCell<Vec<(usize, f32)>>,
}

impl ColumnStack {
    pub fn new(
        children: Vec<StackChild>,
        count: usize,
        gap: f32,
        fixed_height: Option<f32>,
        rule: Option<(f32, gpui::Hsla)>,
        rows: Option<Rows>,
        gap_items: Option<crate::paint::gap_rules::GapItems>,
        intrinsic: Option<Intrinsic>,
    ) -> Self {
        // Без рядов копий ровно столько, сколько колонок (как прежде); с
        // рядами — сколько построил `render.rs`: ребёнок может занять
        // больше колонок, чем `column-count`.
        let copies = match rows {
            Some(_) => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1),
            // Переполняющие колонки (css-multicol-1 §8.2) при `column-fill: auto`
            // с заданной высотой: копий столько, сколько построил `render.rs`
            // (лишние он строит только ребёнку с абсолютным потомком). Без
            // такого ребёнка у всех детей ровно `count` копий, и значение
            // тождественно прежнему. Балансировку это не трогает: без
            // `fixed_height` ветка ниже, как прежде.
            None if fixed_height.is_some() => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1)
                .max(count.max(1)),
            None => count.max(1),
        };
        ColumnStack {
            children,
            count: count.max(1),
            gap,
            fixed_height,
            rule,
            rule_to: None,
            rows,
            axis: StackAxis::Horizontal,
            row_phase: 0.0,
            copies,
            gap_items,
            intrinsic,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Блочный размер СОДЕРЖИМОГО многоколоночника рядами `rows` (та же
    /// укладка, что у `request_layout`): мера вложенного многоколоночника для
    /// внешней стопки (`render::nested_rows_shape`).
    pub(crate) fn measure_rows(
        kids: &[Kid],
        count: usize,
        gap: f32,
        fixed: Option<f32>,
        rows: Rows,
    ) -> f32 {
        let mut probe =
            ColumnStack::new(Vec::new(), count, gap, fixed, None, Some(rows), None, None);
        probe.row_phase = 0.0;
        // Копий — как у `render.rs` для стопки с рядами: сколько колонок
        // ребёнок может занять, с запасом на поля и срезы.
        let per = rows.h.unwrap_or(f32::MAX).max(1.0);
        let span = kids
            .iter()
            .map(|k| (k.h / per).ceil() as usize)
            .max()
            .unwrap_or(0);
        probe.copies = (span + 2).max(count).min(48);
        // Блочный размер — низ ПОСЛЕДНЕЙ линии, а не полный ряд: последний
        // фрагмент сбалансирован и короче ряда (Blink `LayoutRow`:
        // `intrinsic_block_size_` растёт на высоту строки колонок;
        // `multicol-breaking-006`: ряд 100 + хвост 80 + рамка 20).
        let (h, lines, _, _) = probe.balance(kids);
        lines
            .iter()
            .map(|l| l.0 + l.1)
            .fold(0.0f32, f32::max)
            .min(h)
            .max(0.0)
    }

    /// Смещение начала рядов (`row_phase`).
    /// Блочный размер содержимого коробки заданной высоты: линейки
    /// единственной линии колонок тянутся до него (Blink `PaintColumnRules`:
    /// «Paint column rules as tall as the entire multicol container, but only
    /// when at the last row»).
    pub fn with_rule_stretch(mut self, h: Option<f32>) -> Self {
        self.rule_to = h;
        self
    }

    pub fn with_row_phase(mut self, phase: f32) -> Self {
        self.row_phase = phase.max(0.0);
        self
    }

    /// Ось прогрессии колонок (см. `StackAxis`).
    pub fn with_axis(mut self, axis: StackAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Геометрия детей для укладки (то же, что строят `request_layout` и
    /// `prepaint` горизонтальной стопки).
    pub(crate) fn kid_geoms(&self) -> Vec<Kid> {
        self.children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
                repeat: c.repeat.as_ref().map_or(RepeatGeom::default(), |r| r.geom),
                par: c.par,
            })
            .collect()
    }

    /// Строчный и блочный размеры коробки стопки по её оси.
    pub(crate) fn axis_sizes(&self, bounds: Bounds<Pixels>) -> (f32, f32) {
        if self.axis.is_vertical() {
            (f32::from(bounds.size.height), f32::from(bounds.size.width))
        } else {
            (f32::from(bounds.size.width), f32::from(bounds.size.height))
        }
    }

    /// Раскладка стопки в ВЕРТИКАЛЬНОМ письме: та же укладка (`balance`),
    /// физика — через `axis_box`. Горизонтальная стопка идёт прежним
    /// `prepaint` байт в байт. Повтор шапок таблицы, строки flex (`Par`),
    /// `clone` и хвост `slack` сюда не приходят: `render.rs` их в вертикали
    /// не взводит.
    pub(crate) fn prepaint_axis(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let axis = self.axis;
        let (inline_avail, block_avail) = self.axis_sizes(bounds);
        let col_w =
            ((inline_avail - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        let heights = self.kid_geoms();
        let (_, lines, plan, spans) = self.balance(&heights);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        let step = col_w + self.gap;
        let rl = axis == StackAxis::VerticalRl;
        for f in &plan {
            let full_h = self.children[f.kid].h;
            let (c, ry) = self.place(f.col);
            let rel = self.children[f.kid].rel;
            // Копия раскладывается ЦЕЛИКОМ по блочной оси и сдвигается на
            // срез: видимой её часть делает маска (css-break-3 §4, `slice`).
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step,
                col_w,
                ry + f.y - f.from,
                full_h,
            );
            let kid = &mut self.children[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Строчный размер блока `auto` — во всю колонку (CSS 2.1 §10.3.3
            // в логических осях; css-writing-modes-4 §7.3): у вертикальной
            // копии это ВЫСОТА. Корень `layout_as_root` с гибким рядом
            // (так блок вертикального письма строится в `element()`) высоту
            // по доступному месту не тянет — тянет поперечная ось обёртки
            // (`align-items: stretch`). У `vertical-rl` начало блочной оси —
            // ПРАВЫЙ край обёртки (`flex_row_reverse`).
            {
                use gpui::{ParentElement as _, Styled as _};
                let inner = std::mem::replace(el, gpui::Empty.into_any_element());
                let w = gpui::div().flex().w(b.size.width).h(b.size.height);
                let w = if rl {
                    w.flex_row_reverse()
                } else {
                    w.flex_row()
                };
                *el = w.child(inner).into_any_element();
            }
            // Копия раскладывается ОТ своего абсолютного начала, как у
            // горизонтальной стопки (`layout_as_root_at`): края её коробок
            // округляются в той же сетке устройства, что и маска фрагмента
            // (`fragment_mask::snap`). Прежде округление шло от нуля, а сдвиг
            // на дробное начало колонки добавлялся потом — на стыке колонок
            // оставалась строка точек фона (`transform-001`, `overflow-clip-002`).
            el.layout_as_root_at(
                point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                size(
                    gpui::AvailableSpace::Definite(b.size.width),
                    gpui::AvailableSpace::Definite(b.size.height),
                ),
                window,
                cx,
            );
            el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        // Спаннер — во всю СТРОЧНУЮ сторону коробки.
        for &(kid, sy) in &spans {
            let h = self.children[kid].h;
            let b = axis_box(axis, bounds.origin, block_avail, 0.0, inline_avail, sy, h);
            let kid = &mut self.children[kid];
            kid.el.layout_as_root_at(
                b.origin,
                size(
                    gpui::AvailableSpace::Definite(b.size.width),
                    gpui::AvailableSpace::Definite(b.size.height),
                ),
                window,
                cx,
            );
            kid.el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        if let Some(items) = &self.gap_items {
            let lines = self.lines_plan.borrow();
            let mut used = vec![0usize; lines.len()];
            for f in &plan {
                if let Some(u) = used.get_mut(f.col / self.count) {
                    *u = (*u).max(f.col % self.count + 1);
                }
            }
            let mut items = items.borrow_mut();
            for (l, &(ly, lh)) in lines.iter().enumerate() {
                for c in 0..used[l].max(1) {
                    items.push(axis_box(
                        axis,
                        bounds.origin,
                        block_avail,
                        c as f32 * step,
                        col_w,
                        ly,
                        lh,
                    ));
                }
            }
            for &(kid, sy) in &spans {
                items.push(axis_box(
                    axis,
                    bounds.origin,
                    block_avail,
                    0.0,
                    inline_avail,
                    sy,
                    self.children[kid].h,
                ));
            }
        }
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }

    /// Отрисовка вертикальной стопки (пара к `prepaint_axis`).
    pub(crate) fn paint_axis(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let axis = self.axis;
        let (_, block_avail) = self.axis_sizes(bounds);
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        let wrap = matches!(self.rows, Some(r) if r.wrap);
        if let Some((rw, color)) = self.rule {
            let rows = self.lines_plan.borrow().clone();
            // css-multicol-1 §4: линейка только между ЗАНЯТЫМИ колонками.
            let used: Vec<usize> = {
                let plan = self.plan.borrow();
                (0..rows.len())
                    .map(|l| {
                        plan.iter()
                            .filter(|f| {
                                f.h > 0.01 && (if wrap { f.col / self.count } else { 0 }) == l
                            })
                            .map(|f| if wrap { f.col % self.count } else { f.col } + 1)
                            .max()
                            .unwrap_or(0)
                    })
                    .collect()
            };
            for (l, &(ry, rh)) in rows.iter().enumerate() {
                for i in 1..used.get(l).copied().unwrap_or(0).min(self.count) {
                    // Толщина линейки идёт по СТРОЧНОЙ оси (она стоит в
                    // промежутке между колонками), длина — по блочной.
                    let io = i as f32 * step - self.gap * 0.5 - rw * 0.5;
                    window.paint_quad(gpui::fill(
                        axis_box(axis, bounds.origin, block_avail, io, rw, ry, rh),
                        color,
                    ));
                }
            }
        }
        let plan = self.plan.borrow().clone();
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        for f in plan {
            let (c, ry) = self.place(f.col);
            let rel = self.children[f.kid].rel;
            let split = parts[f.kid] > 1;
            // Срез поперёк БЛОЧНОЙ оси; вдоль строчной колонка переполнение
            // не режет (css-multicol-1 §8.1 «visibly overflows and is not
            // clipped to the column box») — вылет на высоту окна, как у
            // горизонтальной стопки на его ширину. Кроме вложенного
            // многоколоночника (`nested_cols`).
            let win_h = f32::from(window.viewport_size().height);
            let spill = if self.children[f.kid].nested_cols {
                0.0
            } else {
                win_h.max(f32::from(bounds.size.height))
            };
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step - spill,
                col_w + spill + spill,
                ry + f.y,
                f.h,
            );
            let mask = fragment_mask::snap(
                Bounds {
                    origin: point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                    size: b.size,
                },
                window.scale_factor(),
            );
            let kid = &mut self.children[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            if split {
                window.with_content_mask(Some(mask), |window| el.paint(window, cx));
            } else {
                el.paint(window, cx);
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
    }

    /// Где стоит колонка `col`: номер в линии и смещение линии. Без рядов и
    /// при `nowrap` колонки идут вбок сплошь (переполняющие — за край).
    pub(crate) fn place(&self, col: usize) -> (usize, f32) {
        match self.rows {
            Some(r) if r.wrap => {
                let lines = self.lines_plan.borrow();
                (
                    col % self.count,
                    lines.get(col / self.count).map_or(0.0, |l| l.0),
                )
            }
            _ => (col, 0.0),
        }
    }
}

impl IntoElement for ColumnStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
