//! Фаза `paint` элемента `ClampCut`: забирает пробы кадра, считает чистый план среза
//! (`ClampGeometry`: строки и блочные коробки `ClampBox`, счётный `counted` и авто-режим
//! `automatic`, переполнение коробок заданной высоты `fit`) и применяет его к состоянию
//! документа (`ClampState`: точка, бюджет строк абзаца, первый абзац за точкой, видимая
//! протяжённость, высоты фрагментированных коробок), запрашивая новый кадр при изменении.
//! `boxes` — конец цепочки коробок, фрагментированных в точке: нижние рамка/паддинг,
//! `min-height` и схлопывание конечных полей (css-overflow-4 §5.3; Blink
//! `LineClampAncestorChain`).

mod automatic;
mod boxes;
mod counted;
mod fit;
mod geometry;
mod paragraph;
#[cfg(test)]
mod tests;

use super::ClampCut;
use super::{ClampEntry, clamp_cut, clamp_para};
pub(crate) use boxes::ClampBox;
pub(super) use boxes::{chain_end, chain_extra, chain_sizes};
use geometry::ClampGeometry;
use gpui::{App, Bounds, Pixels, Window};

impl ClampCut {
    // Keep the extracted GPUI Element paint signature intact.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_impl(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let vcx = self.values.vcx();
        let all_entries = std::mem::take(&mut *self.lines.borrow_mut());
        let para_rows = self.rows.take(self.key);
        let prev_para = clamp_para(self.key, vcx);
        let plan = ClampGeometry {
            limit: self.limit,
            max_h: self.max_h,
            trim_end: self.trim_end,
        }
        .compute(
            all_entries,
            &para_rows,
            f32::from(bounds.origin.y),
            prev_para,
        );
        let (para, next, extent) = (plan.para, plan.next, plan.extent);
        // The boxes fragmented at the point end there too.
        let mut boxes_moved = false;
        {
            let m = &vcx.doc.clamp.boxes;
            let mut m = m.borrow_mut();
            let mine = &plan.nodes;
            for n in mine {
                let now = plan.sizes.iter().find(|(k, _)| k == n).map(|(_, h)| *h);
                let before = m.get(n).copied();
                let differs = match (before, now) {
                    (Some(a), Some(b)) => (a - b).abs() > 0.5,
                    (a, b) => a.is_some() != b.is_some(),
                };
                if differs {
                    boxes_moved = true;
                    match now {
                        Some(h) => m.insert(*n, h),
                        None => m.remove(n),
                    };
                }
            }
        };
        if boxes_moved {
            window.request_animation_frame();
        }
        if prev_para != para {
            {
                let m = &vcx.doc.clamp.para;
                let mut m = m.borrow_mut();
                match para {
                    Some(v) => {
                        m.insert(self.key, v);
                    }
                    None => {
                        m.remove(&self.key);
                    }
                }
            };
            window.request_animation_frame();
        }
        if crate::text::clamp::line_state::clamp_next(self.key, vcx) != next {
            {
                let m = &vcx.doc.clamp.next;
                let mut m = m.borrow_mut();
                match next {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            };
            window.request_animation_frame();
        }
        let prev_extent = crate::text::clamp::clamp_extent(self.key, vcx);
        let moved = match (prev_extent, extent) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            (a, b) => a.is_some() != b.is_some(),
        };
        if moved {
            {
                let m = &vcx.doc.clamp.extent;
                let mut m = m.borrow_mut();
                match extent {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            };
            window.request_animation_frame();
        }
        // Infinity records a completed measurement with no cut; an absent
        // registry entry means no measurement has been applied yet.
        let v = plan.cut;
        let update = cut_update(clamp_cut(self.key, vcx), v);
        if update.write {
            vcx.doc.clamp.cuts.borrow_mut().insert(self.key, v);
        }
        if update.reframe {
            window.request_animation_frame();
        }
    }
}

/// Как применить новый срез `v` к записи реестра `prev`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CutUpdate {
    /// Записать `v`: реестр всегда хранит точное значение последнего замера,
    /// иначе потребители (`clamp_extent`, высота коробки) читают устаревшую
    /// геометрию.
    pub write: bool,
    /// Запросить ещё кадр. Допуск 4px (сдвиг полей, вызванный самим срезом)
    /// гасит лишние кадры, но не саму запись.
    pub reframe: bool,
}

pub(super) fn cut_update(prev: Option<f32>, v: f32) -> CutUpdate {
    let reframe = match prev {
        Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
        Some(a) => a.is_finite() != v.is_finite(),
        None => true,
    };
    CutUpdate {
        write: prev.is_none_or(|a| a.to_bits() != v.to_bits()),
        reframe,
    }
}
