//! Предраскладка фрагментов колонок и линеек промежутков.

use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl ColumnStack {
    pub(super) fn prepaint_frags(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: &[Frag],
        step: f32,
    ) {
        use crate::layout::fragment::static_coordinates::{self, Place};
        let places = self.static_places.clone();
        if let Some(places) = &places {
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
        for f in plan {
            if self.native_kid(f.kid) {
                continue;
            }
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
                r.place_bands(f, |el, at| {
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
                });
            }
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            // Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её
            // часть делает маска в отрисовке. Иначе половина коробки просто
            // сжалась бы, а не продолжилась в следующей колонке.
            let at = point(x, if clone { y } else { y - px(f.from) });
            let avail = size(
                gpui::AvailableSpace::Definite(px(col_w)),
                gpui::AvailableSpace::Definite(px(full_h)),
            );
            let mut laid = el.layout_as_root_at(at, avail, window, cx);
            // Корень rtl taffy ставит у правого края колонки; содержащий
            // блок ltr — вернуть к левому (`Par::rtl_root_in_ltr`).
            if kid.par.rtl_root_in_ltr && f32::from(laid.width) < col_w {
                let back = px(col_w - f32::from(laid.width));
                laid = el.layout_as_root_at(point(at.x - back, at.y), avail, window, cx);
            }
            let mapping = places
                .as_ref()
                .filter(|_| !clone)
                .map(|places| (f.kid, point(x, y - px(f.from)), places.clone()));
            static_coordinates::with(&self.paint, mapping, || {
                el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
            });
            // Retain the actual root width for fragment decoration bounds.
            kid.laid_w.set(kid.laid_w.get().max(f32::from(laid.width)));
        }
    }
}
