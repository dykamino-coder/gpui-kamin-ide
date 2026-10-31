//! Отрисовка содержимого среза под масками фрагмента и его эффектов.

use super::paint_piece::Piece;
use crate::layout::fragment::{
    fragment_mask, gap_fragment,
    types::{Frag, frag_copy},
};
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, Bounds, Window, point, px, size};

impl ColumnStack {
    pub(super) fn paint_slice(
        &mut self,
        f: &Frag,
        piece: &Piece,
        col_w: f32,
        records: Option<&crate::hooks::BareRecords>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Piece {
            at: gpui::Point { x, y },
            clip,
            continued,
            band,
            line_h,
            ..
        } = *piece;
        let kid = &mut self.children[f.kid];
        let Some(el) = frag_copy(&mut kid.el, &mut kid.frags, f.copy) else {
            return;
        };
        let parent = window.content_mask();
        if let Some(w) = &kid.watched {
            w.set_paint_outer(Some(parent));
        }
        // Продолжения корней рисуются под колонкой и собственным срезом копии.
        let area = band.map(|band| {
            let area = fragment_mask::snap(band.union(&clip), window.scale_factor());
            window.with_content_mask(Some(area), |window| window.content_mask())
        });
        let mask = fragment_mask::snap(clip, window.scale_factor());
        window.with_content_mask(Some(mask), |window| {
            let cut = f32::from(y) + f.h;
            let end = if continued && f.foot <= COORD_EPS {
                cut.max(f32::from(y) + line_h - f.y)
            } else {
                cut
            };
            let scope = Some(gap_fragment::Scope {
                parent,
                mask: window.content_mask(),
                start: f32::from(y),
                cut,
                end,
            });
            crate::layout::fragment::effect_fragment::with(
                &self.paint,
                Some(crate::layout::fragment::effect_fragment::Scope {
                    parent,
                    fragment: Bounds {
                        origin: point(x, y),
                        size: size(px(col_w), px(f.h)),
                    },
                    clip,
                }),
                || {
                    gap_fragment::with(&self.paint, scope, || {
                        let mut paint = |window: &mut Window| {
                            crate::hooks::with_copy_segment(
                                gpui::PaintSegmentId(f.col as u64),
                                records,
                                window,
                                |window| el.paint(window, cx),
                            )
                        };
                        match area {
                            Some(area) => crate::hooks::with_copy_area(area, window, paint),
                            None => paint(window),
                        }
                    })
                },
            );
        });
    }
}
