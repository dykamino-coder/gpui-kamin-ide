//! `impl Element` для `ColumnStack`.
// owner: A

use crate::flow::*;

impl Element for ColumnStack {
    type RequestLayoutState = LayoutId;
    type PrepaintState = Bounds<Pixels>;

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
        self.request_column_layout(window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        if self.axis.is_vertical() {
            self.prepaint_axis(bounds, window, cx);
            return bounds;
        }
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        let heights: Vec<Kid> = self
            .children
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
            .collect();
        let (_, lines, plan, spans) = self.balance(&heights);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        let step = col_w + self.gap;
        for f in &plan {
            // У `clone` копия — САМ фрагмент своей высоты со своими рамками,
            // отбивкой и фоном (`render.rs::clone_fragment`): раскладывать её
            // на полную высоту коробки и поднимать на срез нельзя.
            let clone = self.children[f.kid].clone_dec.is_some();
            let full_h = if clone { f.h } else { self.children[f.kid].h };
            // Колонка в своём ряду: `x` по номеру в ряду, `y` от верха ряда.
            let (c, ry) = self.place(f.col);
            // Сдвиг фрагмента (css-break-3 §5.5) — здесь, а не внутри копии:
            // `layout_as_root` края её КОРНЯ не читает (проба `pm1`).
            let rel = self.children[f.kid].rel;
            let x = bounds.origin.x + px(c as f32 * step + rel.0);
            let x = x + px(self.children[f.kid].par.dx);
            let y = bounds.origin.y + px(ry + f.y + rel.1);
            let kid = &mut self.children[f.kid];
            // Полосы повтора таблицы — своими копиями: шапка встаёт над
            // содержимым продолжения (`f.y − f.head`), подвал — сразу под ним.
            // Копия та же полная таблица, поднятая так, что её полоса
            // совпадает с местом во фрагменте; видимой полосу делает маска.
            if let Some(r) = kid.repeat.as_mut() {
                let mut band = |el: Option<&mut AnyElement>, at: f32| {
                    if let Some(el) = el {
                        el.layout_as_root_at(
                            point(x, y + px(at)),
                            size(
                                gpui::AvailableSpace::Definite(px(col_w)),
                                gpui::AvailableSpace::Definite(px(full_h)),
                            ),
                            window,
                            cx,
                        );
                        el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
                    }
                };
                if let (Some((src, _)), true) = (r.head, f.head > 0.01 && f.copy > 0) {
                    band(r.head_els.get_mut(f.copy - 1), -f.head - src);
                }
                if let (Some((src, bh)), true) = (r.foot, f.foot > 0.01) {
                    band(r.foot_els.get_mut(f.copy), f.h + f.foot - bh - src);
                }
            }
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её
            // часть делает маска в отрисовке. Иначе половина коробки просто
            // сжалась бы, а не продолжилась в следующей колонке.
            let laid = el.layout_as_root_at(
                point(x, if clone { y } else { y - px(f.from) }),
                size(
                    gpui::AvailableSpace::Definite(px(col_w)),
                    gpui::AvailableSpace::Definite(px(full_h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
            // Retain the actual root width for fragment decoration bounds.
            kid.laid_w.set(kid.laid_w.get().max(f32::from(laid.width)));
        }
        // Спаннер — во всю ширину коробки, первой копией (запасных у него
        // нет: между колонками он не режется).
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
                    items.push(Bounds {
                        origin: point(bounds.origin.x + px(c as f32 * step), bounds.origin.y + px(ly)),
                        size: size(px(col_w), px(lh)),
                    });
                }
            }
            for &(kid, sy) in &spans {
                items.push(Bounds {
                    origin: point(bounds.origin.x, bounds.origin.y + px(sy)),
                    size: size(bounds.size.width, px(self.children[kid].h)),
                });
            }
        }
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
        bounds
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        prepaint: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.axis.is_vertical() {
            self.paint_axis(bounds, window, cx);
            return;
        }
        let bounds = *prepaint;
        // Линейки — по центрам промежутков, высотой в колонку, в каждой линии.
        // Это простая `column-rule` без рядов; при рядах `render.rs` отдаёт
        // линейки (в том числе `row-rule`, css-multicol-2 §rg) художнику
        // `GapRulePainter` по буферу `gap_items`, и `rule` здесь `None`.
        if let Some((rw, color)) = self.rule {
            let col_w = self.col_w.get();
            let rows = self.lines_plan.borrow().clone();
            // css-multicol-1 §4 (`column-rule`): «Column rules are only drawn
            // between two columns that both have content». Занятость — по
            // плану укладки: наибольшая колонка линии с НЕнулевым куском
            // (щуп статической позиции абсолюта — кусок нулевой высоты).
            // `grid-container-fragmentation-007/008`: 3 колонки из 5, лишние
            // линейки в 3-м и 4-м промежутках — 0.83 %. Без рядов (и при
            // `nowrap`) все колонки — одна линия, переполняющие тоже в ней.
            let wrap = matches!(self.rows, Some(r) if r.wrap);
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
            let last = rows.len().saturating_sub(1);
            for (l, &(ry, rh)) in rows.iter().enumerate() {
                let rh = match self.rule_to {
                    Some(to) if l == last && self.rows.is_none_or(|r| r.cap) => rh.max(to - ry),
                    _ => rh,
                };
                for i in 1..used.get(l).copied().unwrap_or(0).min(self.count) {
                    let cx_ = i as f32 * (col_w + self.gap) - self.gap * 0.5;
                    window.paint_quad(gpui::fill(
                        fragment_mask::snap(
                            Bounds {
                                origin: point(
                                    bounds.origin.x + px(cx_ - rw * 0.5),
                                    bounds.origin.y + px(ry),
                                ),
                                size: size(px(rw), px(rh)),
                            },
                            window.scale_factor(),
                        ).bounds,
                        color,
                    ));
                }
            }
        }
        let plan = self.plan.borrow().clone();
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        // Маска нужна ТОЛЬКО разрезанному ребёнку. У целого она обрезала бы
        // его собственное переполнение, которого коробка не прячет: отсюда
        // уходили в красное `overflow-clip-004`, `overflow-unsplittable-*`,
        // `overflowing-block-003` и родня.
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        let plan_all = plan.clone();
        // CSS 2.1 Appendix E: блочные потомки потока (шаг 4) раньше
        // позиционированных (шаг 8) — фрагменты позиционированных детей
        // красятся вторым проходом, в порядке разметки.
        let plan: Vec<Frag> = plan
            .iter()
            .filter(|f| !self.children[f.kid].positioned)
            .chain(plan.iter().filter(|f| self.children[f.kid].positioned))
            .copied()
            .collect();
        for f in plan {
            let (c, ry) = self.place(f.col);
            // Срез едет вместе со сдвинутым фрагментом (css-break-3 §5.5):
            // маска, оставленная на месте колонки, съедала его целиком
            // (`out-of-flow-in-multicolumn-042/045`, проба `pm3`).
            let rel = self.children[f.kid].rel;
            let x = bounds.origin.x + px(c as f32 * step + rel.0);
            let x = x + px(self.children[f.kid].par.dx);
            let y = bounds.origin.y + px(ry + f.y + rel.1);
            // Маска — устройство `slice`. Фрагмент `clone` самодостаточен:
            // содержимое режет его внутренняя обёртка (`clone_fragment`), а
            // тень/контур обязаны выходить за колонку (`clone-009`).
            let split = parts[f.kid] > 1 && self.children[f.kid].clone_dec.is_none();
            // Срез `slice` (css-break-3 §4) — поперёк БЛОЧНОЙ оси. Вбок колонка
            // переполнение не режет: css-multicol-1 §8.1 «content that extends
            // outside column boxes visibly overflows and is not clipped to the column
            // box» (Blink режет только `overflow` самой коробки). Маска шириной в
            // колонку прятала жёлтую полосу 180px поверх линеек (`column-rule-002`),
            // правую четверть ребёнка 100px в колонке 75
            // (`relative-child-overflowing-column-gap`) и всё содержимое при стопке
            // шириной 0 (`relative-child-overflowing-container`, колонка 1px). Вылет —
            // на ширину окна (у стопки нулевой ширины своей ширины нет); дальше режет
            // маска предка. Заменяет P10 `scout-grid-frag-2026-09-30.md` §5.10.
            // Кроме ребёнка с вложенным многоколоночником (`StackChild::nested_cols`):
            // его ширина за колонкой — артефакт плоской копии, а не переполнение
            // (`scout-mcnested-2026-09b.md` §5.3: красный потомок `margin-left:100%`
            // в `multicol-fill-balance-nested-000` прячет только маска колонки).
            let win_w = window.viewport_size().width;
            let spill = if self.children[f.kid].nested_cols {
                px(0.)
            } else if win_w > bounds.size.width {
                win_w
            } else {
                bounds.size.width
            };
            let mask = fragment_mask::snap(Bounds {
                    origin: point(x - spill, y),
                    size: size(px(col_w) + spill + spill, px(f.h)),
                }, window.scale_factor());
            let kid = &mut self.children[f.kid];
            // Полосы повтора таблицы — каждая своей маской по своей полосе.
            if let Some(r) = kid.repeat.as_mut() {
                let mask_scale = window.scale_factor();
                let band_mask = |top: f32, h: f32| fragment_mask::snap(Bounds {
                        origin: point(x - spill, y + px(top)),
                        size: size(px(col_w) + spill + spill, px(h)),
                    }, mask_scale);
                if f.head > 0.01
                    && f.copy > 0
                    && let Some(el) = r.head_els.get_mut(f.copy - 1)
                {
                    window.with_content_mask(Some(band_mask(-f.head, f.head)), |window| {
                        el.paint(window, cx)
                    });
                }
                if f.foot > 0.01
                    && let Some((_, bh)) = r.foot
                    && let Some(el) = r.foot_els.get_mut(f.copy)
                {
                    window.with_content_mask(Some(band_mask(f.h + f.foot - bh, bh)), |window| {
                        el.paint(window, cx)
                    });
                }
            }
            // Хвост непоследнего фрагмента таблицы — фоном таблицы (`slack`).
            if let Some(bg) = kid.slack
                && plan_all.iter().any(|g| g.kid == f.kid && g.copy == f.copy + 1)
            {
                let line = if matches!(self.rows, Some(r) if r.wrap) { f.col / self.count } else { 0 };
                let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
                let band = kid.repeat.as_ref().and_then(|r| r.foot).map_or(0.0, |b| b.1);
                let tail = if f.foot > 0.01 { f.foot - band } else { line_h - f.y - f.h };
                // Не шире колонки: разложенная ширина копии несёт дробный
                // остаток раскладки, и хвост залезал на край соседней колонки
                // (`multi-line-row-flex-fragmentation-090`: пиксель красного у
                // левого края третьей колонки).
                let w = kid.laid_w.get().min(col_w);
                if tail > 0.01 && w > 0.01 {
                    // По пикселям устройства, как маска фрагмента: дробная
                    // ширина копии (`33.333px`) иначе оставляла полупрозрачный
                    // край рядом с соседом (`multi-line-row-flex-
                    // fragmentation-090`).
                    let sf = window.scale_factor();
                    window.paint_quad(gpui::fill(
                        fragment_mask::snap(
                            Bounds { origin: point(x, y + px(f.h)), size: size(px(w), px(tail)) },
                            sf,
                        )
                        .bounds,
                        bg,
                    ));
                }
            }
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Маска и режет: копия нарисована во всю свою высоту, видна
            // только полоса своей колонки (css-break-3 §4, вид `slice`).
            if split {
                let line = if matches!(self.rows, Some(r) if r.wrap) { f.col / self.count } else { 0 };
                let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
                let continued = plan_all.iter().any(|g| g.kid == f.kid && g.copy == f.copy + 1);
                let parent = window.content_mask();
                let root = Bounds {
                    origin: point(x, y - px(f.from)),
                    size: size(px(kid.laid_w.get()), px(kid.h)),
                };
                window.with_content_mask(Some(mask), |window| {
                    let scope = (continued && f.foot <= 0.01 && line_h - f.y - f.h > 0.01)
                        .then(|| gap_fragment::Scope {
                            root,
                            parent,
                            mask: window.content_mask(),
                            cut: f32::from(y) + f.h,
                            end: f32::from(y) + line_h - f.y,
                        });
                    gap_fragment::with(scope, || el.paint(window, cx));
                });
            } else {
                el.paint(window, cx);
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
    }
}
