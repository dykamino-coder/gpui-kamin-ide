//! Prepaint: рамки строчных коробок-якорей и строчных содержащих блоков в реестр якорей.

use crate::text::inline::InlineCb;
use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Window, point, px};

impl Paragraph {
    /// Записать строчную коробку куска-носителя `b` (место куска `at`) в
    /// реестр якорей: как содержащий блок — край отбивки
    /// (`inline_cb_rect`, CSS 2.1 §10.1 п.4), как именованный якорь —
    /// объемлющий border box фрагментов (`inline_box_rect`).
    pub(super) fn register_inline_box(
        &self,
        segs: &[Seg],
        at: usize,
        b: InlineCb,
        info: &crate::text::inline::inline_box::InlineBoxInfo,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) {
        let Some(values) = &self.anchor_values else {
            return;
        };
        let vcx = values.vcx();
        let s = (at as isize + b.start).max(0) as usize;
        let e = (at as isize + b.end).max(0) as usize;
        let shift = |r: Bounds<Pixels>| Bounds {
            origin: point(r.origin.x + px(b.shift.0), r.origin.y + px(b.shift.1)),
            size: r.size,
        };
        if info.cb
            && let Some(r) = self.inline_cb_rect(segs, s, e, info.pad, bounds)
        {
            crate::layout::positioned::anchor::register_inline_cb(
                info.node,
                info.own_cb,
                shift(r),
                vcx,
            );
        }
        if !info.names.is_empty() {
            let edges: [f32; 4] = std::array::from_fn(|k| info.pad[k] + info.border[k]);
            if let Some(r) = self.inline_box_rect(segs, s, e, edges, bounds) {
                crate::layout::positioned::anchor::register_inline_anchor(
                    &info.names,
                    crate::layout::positioned::anchor::Entry {
                        id: info.node,
                        cb: info.own_cb,
                        seq: info.seq,
                        hidden: info.hidden,
                    },
                    shift(r),
                    window,
                    vcx,
                );
            }
        }
    }
}
