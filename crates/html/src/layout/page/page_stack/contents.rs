//! Краска содержимого листов: фрагменты, ICB-слои и fixed-слои по листам.

use super::PageStack;
use crate::layout::fragment::types::Frag;
use gpui::{App, Bounds, Pixels, Window};

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn paint_page_contents(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        pages: usize,
        k: f32,
        plan: Vec<Frag>,
        rect: impl Fn(f32, f32, f32, f32) -> Bounds<Pixels>,
        stack: &mut gpui::PaintStack,
    ) {
        for i in 0..pages {
            let g = self.geom(i);
            if self.slot(i).is_none() || g.turn.is_multiple_of(2) {
                continue;
            }
            let (sx, sy) = self.sheet_origin(i);
            // The sheet matrix (logical) in device pixels, as with_transformation.
            let sheet = self.sheet_matrix(bounds, i);
            let m = gpui::TransformationMatrix {
                rotation_scale: sheet.rotation_scale,
                translation: [sheet.translation[0] * k, sheet.translation[1] * k],
            };
            let records = self.copy_records();
            let plan_i: Vec<Frag> = plan.iter().copied().filter(|f| f.col == i).collect();
            window
                .with_transformation_masked_async(m, async |window| {
                    window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
                    let bx = sx + g.margin[3];
                    let by = sy + g.margin[0];
                    let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
                    let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
                    if let Some(c) = g.canvas {
                        let (cx0, cy0) = (bx.max(sx), by.max(sy));
                        let cw = ((bx + bw).min(sx + g.size.0) - cx0).max(0.0);
                        let ch = ((by + bh).min(sy + g.size.1) - cy0).max(0.0);
                        window.paint_quad(gpui::fill(rect(cx0, cy0, cw, ch), c));
                    }
                    let (bt, bc) = g.border;
                    if bt > 0.0 {
                        for r in [
                            (bx, by, bw, bt),
                            (bx, by + bh - bt, bw, bt),
                            (bx, by, bt, bh),
                            (bx + bw - bt, by, bt, bh),
                        ] {
                            window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), bc));
                        }
                    }
                    let (ax, ay) = g.area_origin();
                    let area = rect(sx + ax, sy + ay, g.area.0, g.area.1);
                    for f in plan_i {
                        let mask = gpui::ContentMask {
                            bounds: rect(sx + ax, sy + ay + f.y, g.area.0, f.h).intersect(&area),
                        };
                        let kid = &mut self.kids[f.kid];
                        kid.paint_sections(
                            &f,
                            |top, h| {
                                rect(sx + ax, sy + ay + f.y + top, g.area.0, h).intersect(&area)
                            },
                            window,
                            cx,
                            stack,
                        )
                        .await;
                        let Some(el) = crate::layout::fragment::types::frag_copy(
                            &mut kid.el,
                            &mut kid.frags,
                            f.copy,
                        ) else {
                            continue;
                        };
                        window
                            .with_content_mask_async(Some(mask), async |window| {
                                crate::hooks::with_copy_segment_async(
                                    gpui::PaintSegmentId(f.col as u64),
                                    records.as_ref(),
                                    window,
                                    async |window| el.paint_async(window, cx, stack).await,
                                )
                                .await
                            })
                            .await;
                    }
                    if let Some(layer) = self.icb.get_mut(i) {
                        for el in layer {
                            window
                                .with_content_mask_async(
                                    Some(gpui::ContentMask { bounds: area }),
                                    async |window| {
                                        crate::hooks::with_copy_segment_async(
                                            gpui::PaintSegmentId(i as u64),
                                            records.as_ref(),
                                            window,
                                            async |window| el.paint_async(window, cx, stack).await,
                                        )
                                        .await
                                    },
                                )
                                .await;
                        }
                    }
                    if let Some(layer) = self.fixed.get_mut(i) {
                        for el in layer {
                            window
                                .with_content_mask_async(
                                    Some(gpui::ContentMask { bounds: area }),
                                    async |window| {
                                        crate::hooks::with_copy_segment_async(
                                            gpui::PaintSegmentId(i as u64),
                                            records.as_ref(),
                                            window,
                                            async |window| el.paint_async(window, cx, stack).await,
                                        )
                                        .await
                                    },
                                )
                                .await;
                        }
                    }
                    let clip = self.presented_area(bounds, i);
                    let _ = self.paint_native(bounds, i, clip, window);
                    let sheet = rect(sx, sy, g.size.0, g.size.1);
                    for (p, el) in &mut self.margin_els {
                        if *p == i {
                            window
                                .with_content_mask_async(
                                    Some(gpui::ContentMask { bounds: sheet }),
                                    async |window| el.paint_async(window, cx, stack).await,
                                )
                                .await;
                        }
                    }
                })
                .await;
        }
    }
}

/// Paint a sheet's copy under its page-area mask and the stack scale; its slots
/// replay the sheet's own continuation records.
pub(super) async fn paint_copy(
    (records, origin, scale): (
        Option<&crate::hooks::BareRecords>,
        gpui::Point<gpui::Pixels>,
        f32,
    ),
    page: usize,
    mask: gpui::ContentMask<gpui::Pixels>,
    el: &mut gpui::AnyElement,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    stack: &mut gpui::PaintStack,
) {
    window
        .with_content_mask_async(Some(mask), async |window| {
            window
                .with_mask_scale_async(origin, scale, async |window| {
                    crate::hooks::with_copy_segment_async(
                        gpui::PaintSegmentId(page as u64),
                        records,
                        window,
                        async |window| el.paint_async(window, cx, stack).await,
                    )
                    .await
                })
                .await
        })
        .await;
}
