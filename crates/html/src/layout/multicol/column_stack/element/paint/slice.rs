//! Область среза колонки удерживается до завершения дочерней краски.

use super::*;
use crate::layout::cx::deferred::Deferred;

pub(super) struct Slice {
    pub(super) fragment: Frag,
    pub(super) at: gpui::Point<Pixels>,
    pub(super) width: f32,
    pub(super) line_h: f32,
    pub(super) continued: bool,
    pub(super) parent: gpui::ContentMask<Pixels>,
    pub(super) area: Option<gpui::ContentMask<Pixels>>,
    pub(super) mask: gpui::ContentMask<Pixels>,
    pub(super) clip: Bounds<Pixels>,
}

impl Slice {
    pub(super) async fn paint(
        self,
        el: &mut gpui::AnyElement,
        owner: &Deferred,
        records: Option<&crate::hooks::BareRecords>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        window
            .with_content_mask_async(Some(self.mask), async |window| {
                let cut = f32::from(self.at.y) + self.fragment.h;
                let end = if self.continued && self.fragment.foot <= COORD_EPS {
                    cut.max(f32::from(self.at.y) + self.line_h - self.fragment.y)
                } else {
                    cut
                };
                let scope = Some(gap_fragment::Scope {
                    parent: self.parent,
                    mask: window.content_mask(),
                    start: f32::from(self.at.y),
                    cut,
                    end,
                });
                crate::layout::fragment::effect_fragment::with_async(
                    owner,
                    Some(crate::layout::fragment::effect_fragment::Scope {
                        parent: self.parent,
                        fragment: Bounds {
                            origin: self.at,
                            size: size(px(self.width), px(self.fragment.h)),
                        },
                        clip: self.clip,
                    }),
                    async || {
                        gap_fragment::with_async(owner, scope, async || {
                            let mut paint = async |window: &mut Window| {
                                crate::hooks::with_copy_segment_async(
                                    gpui::PaintSegmentId(self.fragment.col as u64),
                                    records,
                                    window,
                                    async |window| el.paint_async(window, cx, stack).await,
                                )
                                .await
                            };
                            match self.area {
                                Some(area) => {
                                    crate::hooks::with_copy_area_async(area, window, paint).await
                                }
                                None => paint(window).await,
                            }
                        })
                        .await
                    },
                )
                .await;
            })
            .await;
    }
}
