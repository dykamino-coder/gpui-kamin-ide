//! Блочная ось стопки страниц: разбиение вертикального корня на страницы вдоль x.
//!
//! css-page-3 §page-model: page area — фрагментаинер, и содержимое режется по блочной оси
//! режима письма корня (css-break-3 §2, css-writing-modes-4 §3.1). Арифметика стопки
//! (`ColumnStack::fill_at`, план) остаётся логической — «высота» есть блочный размер —
//! как в вертикальном пути многоколоночника (`column_stack/axis.rs`); этот модуль
//! переводит её в физические коробки через `axis_box` (Blink
//! `WritingModeConverter::ToPhysical`), а также продолжает видимое переполнение
//! коробки на следующие листы и считает протяжённость маски фрагмента.

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::Frag;
use crate::layout::fragment::types::{StackAxis, axis_box};
use gpui::{Bounds, Pixels, point, px};

impl PageStack {
    /// Paginate along the block axis `axis` of the root writing mode.
    pub fn with_axis(mut self, axis: StackAxis) -> Self {
        self.axis = axis;
        self
    }

    /// (inline, block) size of the page area of `g`.
    pub(crate) fn area_logical(&self, g: &PageGeom) -> (f32, f32) {
        if self.axis.is_vertical() {
            (g.area.1, g.area.0)
        } else {
            g.area
        }
    }

    /// Physical box (unscaled stack points, relative to the sheet origin) of
    /// the block range `[bo, bo + be)` across the whole inline size of the
    /// page area of `g`.
    pub(crate) fn area_slice(&self, g: &PageGeom, bo: f32, be: f32) -> (f32, f32, f32, f32) {
        let (ax, ay) = g.area_origin();
        let (inline, block) = self.area_logical(g);
        let b: Bounds<Pixels> =
            axis_box(self.axis, point(px(ax), px(ay)), block, 0.0, inline, bo, be);
        (
            f32::from(b.origin.x),
            f32::from(b.origin.y),
            f32::from(b.size.width),
            f32::from(b.size.height),
        )
    }

    /// Visible overflow past a box's last fragment continues on the next pages
    /// (css-overflow-3 §2, css-break-3 §2.1 parallel overflow): one more
    /// copy per page up to the box's reach (`PageKid::reach`), shifted like a
    /// continuous flow. Blink paints such overflow from the monolithic
    /// fragment on every page it reaches.
    pub(crate) fn spill_overflow(&self, plan: &mut Vec<Frag>, pages: usize, block: f32) {
        for (k, kid) in self.kids.iter().enumerate() {
            let Some(reach) = kid.reach else {
                continue;
            };
            let Some(last) = plan
                .iter()
                .filter(|f| f.kid == k)
                .max_by_key(|f| f.copy)
                .copied()
            else {
                continue;
            };
            // Kid coordinate at the top of the last fragment's page.
            let top = last.from - last.y;
            let mut copy = last.copy;
            let mut col = last.col + 1;
            while col < pages && copy < kid.frags.len() {
                let from = top + (col - last.col) as f32 * block;
                if from >= reach - 0.01 {
                    break;
                }
                copy += 1;
                plan.push(Frag {
                    kid: k,
                    copy,
                    col,
                    y: 0.0,
                    from,
                    h: block,
                    head: 0.0,
                    foot: 0.0,
                });
                col += 1;
            }
        }
    }

    /// Mask extent of fragment `f` along the block axis: a fragment that ends
    /// its box shows the box's overflow up to the page area's end.
    pub(crate) fn mask_extent(&self, g: &PageGeom, f: &Frag) -> f32 {
        let end = self.measured.get(&(f.kid, f.copy)).map_or_else(
            || self.kids.get(f.kid).map_or(f32::MAX, |k| k.end.get()),
            |m| m.kid.h,
        );
        if f.from + f.h >= end - 0.01 {
            (self.area_logical(g).1 - f.y).max(f.h)
        } else {
            f.h
        }
    }
}
