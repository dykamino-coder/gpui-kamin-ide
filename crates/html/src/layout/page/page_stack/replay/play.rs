//! Игра записи главной копии на листах: кусок — под матрицей своего листа и маской копии,
//! сегменты `LATE` — после потока всех кусков; маска куска в координатах окна.

use super::{Played, present};
use crate::layout::fragment::replay::{self, LATE};
use crate::layout::fragment::types::Frag;
use crate::layout::page::page_stack::PageStack;
use gpui::{Bounds, ContentMask, PaintSegmentRecords, Pixels, Window};
use std::collections::HashMap;

impl PageStack {
    /// Маска куска `f` в координатах окна: полоса его фрагмента в page area листа (`page` —
    /// маска page area), по строчной оси — весь лист, снятая на пиксели устройства.
    pub(in crate::layout::page::page_stack) fn piece_mask(
        &self,
        bounds: Bounds<Pixels>,
        f: &Frag,
        page: ContentMask<Pixels>,
        scale: f32,
    ) -> ContentMask<Pixels> {
        let s = self.grid.get().1;
        let (sx, sy) = self.sheet_origin(f.col);
        let g = self.geom(f.col);
        let (ax, ay) = g.area_origin();
        // The fragment that ends its box shows the box's block-end
        // overflow up to the page area's end (overflow is visible,
        // css-overflow-3 §2; `block-001-wm-vrl`: a negative block-end
        // margin shrinks the margin box under the border box).
        // The fragment that starts its box likewise shows the box's
        // block-start ink overflow up to the page area's start (a glyph
        // rising above its line box; `page-size-012-print`).
        let (top, ext) =
            crate::layout::page::page_stack::axis::mask_span(f, self.mask_extent(&g, f));
        let (x, y, w, h) = if self.axis.is_vertical() {
            self.area_slice(&g, top, ext)
        } else {
            (ax, ay + top, g.area.0, ext)
        };
        let mask = ContentMask {
            bounds: Bounds {
                origin: gpui::point(
                    bounds.origin.x + gpui::px((sx + x) * s),
                    bounds.origin.y + gpui::px((sy + y) * s),
                ),
                size: gpui::size(gpui::px(w * s), gpui::px(h * s)),
            }
            .intersect(&page.bounds),
        };
        // По оси строки — весь лист (`PageStack::sheet_wide`).
        let mask = self.sheet_wide(bounds, f.col, mask);
        // Clip and painted box edges share the device-pixel grid.
        crate::layout::fragment::fragment_mask::snap(mask.bounds, scale)
    }

    /// Кусок `f` из записи под маской копии `mask` и продлениями записи `extra`.
    pub(in crate::layout::page::page_stack) fn play_piece(
        &self,
        bounds: Bounds<Pixels>,
        f: &Frag,
        mask: ContentMask<Pixels>,
        (records, extra): (&PaintSegmentRecords, &HashMap<usize, Bounds<Pixels>>),
        window: &mut Window,
    ) {
        let s = self.grid.get().1;
        let mut clip = mask.bounds;
        if let Some(e) = extra.get(&f.copy) {
            clip = clip.union(&present(bounds, s, *e));
        }
        self.play(bounds, f.col, (records, f.copy), clip, window);
    }

    /// Сегменты `LATE` записей — после потока всех кусков, каждый на листе своего куска.
    pub(in crate::layout::page::page_stack) fn play_late(
        &self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        replayed: &Played,
        window: &mut Window,
    ) {
        let s = self.grid.get().1;
        let mut kids: Vec<_> = replayed.iter().collect();
        kids.sort_by_key(|(kid, _)| **kid);
        for (kid, played) in kids {
            let Some((records, masks)) = played else {
                continue;
            };
            for (segment, b) in replay::late_segments(masks) {
                let copy = segment & !(LATE as usize);
                let Some(f) = plan.iter().find(|f| f.kid == *kid && f.copy == copy) else {
                    continue;
                };
                // Под маской куска, как отложенные задания копии.
                let page = self.area_mask(bounds, f.col);
                let piece = self.piece_mask(bounds, f, page, window.scale_factor());
                let clip = present(bounds, s, b).intersect(&piece.bounds);
                self.play(bounds, f.col, (records, segment), clip, window);
            }
        }
    }

    fn play(
        &self,
        bounds: Bounds<Pixels>,
        sheet: usize,
        (records, segment): (&PaintSegmentRecords, usize),
        clip: Bounds<Pixels>,
        window: &mut Window,
    ) {
        let matrix = self.sheet_matrix(bounds, sheet);
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            let _ = window.with_paint_presentation(matrix, |window| {
                replay::paint_segment(records, segment, window)
            });
        });
    }
}
