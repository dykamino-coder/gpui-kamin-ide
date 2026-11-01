//! Подготовка копии перед рекурсивным prepaint: временные значения раскладки и
//! маршрутов освобождают кадр до входа в потомка. Сам маршрут остаётся один и
//! заимствуется при обходе, без перемещения крупного `Routes` по цепочке.

use super::copies::{layout_frag, prepaint_bands};
use crate::layout::fragment::{replay::Routes, static_coordinates::Places, types::Frag};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, Pixels, Point, Window, point, px};

pub(super) struct Prepared {
    pub(super) replay: Option<Routes>,
    pub(super) mapping: Option<(usize, Point<Pixels>, Places)>,
    pub(super) width: f32,
}

impl ColumnStack {
    #[inline(never)]
    pub(super) fn prepare_prepaint_frag(
        &mut self,
        f: &Frag,
        plan: &[Frag],
        (bounds, col_w, step): (Bounds<Pixels>, f32, f32),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Prepared> {
        let spill = Self::spill(bounds, window);
        // Ребёнок, чьи куски рисует запись главной копии, раскладывает только её.
        let replay = if self.replays(f.kid, plan) {
            self.replay_routes(
                f.kid,
                plan,
                bounds,
                (col_w, step, spill),
                window.scale_factor(),
            )
        } else {
            None
        };
        // У `clone` копия — САМ фрагмент своей высоты со своими рамками,
        // отбивкой и фоном (`render.rs::clone_fragment`): раскладывать её
        // на полную высоту коробки и поднимать на срез нельзя.
        // Корень clone в записи раскладывается непрерывной коробкой; украшения его
        // фрагментов — свои копии без содержимого на месте каждого куска.
        let clone = self.children[f.kid].clone_dec.is_some() && replay.is_none();
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
            prepaint_bands(r, f, point(x, y), (col_w, full_h), window, cx);
        }
        let decos = kid.clone_deco.as_mut().map(|d| &mut d.decos[..]);
        let deco = decos.and_then(|d| d.iter_mut().nth(f.copy));
        if let Some(deco) = deco.filter(|_| replay.is_some()) {
            let rtl = kid.par.rtl_root_in_ltr;
            layout_frag(deco, point(x, y), (col_w, f.h), rtl, window, cx);
            deco.prepaint(window, cx);
        }
        if replay.is_some() && f.copy != 0 {
            return None;
        }
        let el = crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)?;
        // Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её
        // часть делает маска в отрисовке. Иначе половина коробки просто
        // сжалась бы, а не продолжилась в следующей колонке.
        let at = point(x, if clone { y } else { y - px(f.from) });
        let laid = layout_frag(el, at, (col_w, full_h), kid.par.rtl_root_in_ltr, window, cx);
        let mapping = self
            .static_places
            .as_ref()
            .filter(|_| !clone)
            .map(|places| (f.kid, point(x, y - px(f.from)), places.clone()));
        Some(Prepared {
            replay,
            mapping,
            width: f32::from(laid.width),
        })
    }
}
