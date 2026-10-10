//! Предраскладка фрагментов колонок и линеек промежутков.

use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{AnyElement, App, Bounds, Pixels, Window, point, px, size};

impl ColumnStack {
    pub(super) fn paint_column_rules(&mut self, window: &mut Window, bounds: Bounds<Pixels>) {
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
                        )
                        .bounds,
                        color,
                    ));
                }
            }
        }
    }

    pub(super) fn prepaint_gap_items(
        &mut self,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: &Vec<Frag>,
        spans: &Vec<(usize, f32)>,
        step: f32,
    ) {
        if let Some(items) = &self.gap_items {
            let lines = self.lines_plan.borrow();
            let mut used = vec![0usize; lines.len()];
            for f in plan {
                if let Some(u) = used.get_mut(f.col / self.count) {
                    *u = (*u).max(f.col % self.count + 1);
                }
            }
            let mut items = items.borrow_mut();
            for (l, &(ly, lh)) in lines.iter().enumerate() {
                for c in 0..used[l].max(1) {
                    items.push(Bounds {
                        origin: point(
                            bounds.origin.x + px(c as f32 * step),
                            bounds.origin.y + px(ly),
                        ),
                        size: size(px(col_w), px(lh)),
                    });
                }
            }
            for &(kid, sy) in spans {
                items.push(Bounds {
                    origin: point(bounds.origin.x, bounds.origin.y + px(sy)),
                    size: size(bounds.size.width, px(self.children[kid].h)),
                });
            }
        }
    }

    pub(super) fn prepaint_frags(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: &Vec<Frag>,
        step: f32,
    ) {
        for f in plan {
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
    }
}
