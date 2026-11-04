//! Главная копия записывается на общем стеке; допуск восстанавливается при отмене.

use super::*;
use crate::layout::cx::deferred::Deferred;
use crate::layout::fragment::effect_fragment::pieces::{self, Pieces};
use gpui::{App, PaintRecordError};

impl Routes {
    /// Запись краски главной копии `el` по сегментам кусков; `owner` — хозяин кусков
    /// художников фрагмента, `records` — нативные записи стопки для слотов.
    pub(crate) async fn record_async(
        &self,
        (el, owner, records): (
            &mut gpui::AnyElement,
            Option<&Deferred>,
            Option<crate::hooks::BareRecords>,
        ),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<Replayed, PaintRecordError> {
        let fx = (self.pieces.clone()).map(|p| Rc::new(Pieces::new(p, self.watched.clone())));
        let natives = records.as_ref().is_some_and(|r| !r.is_empty());
        self.admit(true);
        let admission = Admission(self);
        let recorded = match owner {
            Some(owner) => {
                pieces::with_async(owner, fx.clone(), async || {
                    self.record_scopes_async(records, window, async |w| {
                        self.paint_into_async(el, w, cx, stack).await
                    })
                    .await
                })
                .await
            }
            None => {
                self.record_scopes_async(records, window, async |w| {
                    self.paint_into_async(el, w, cx, stack).await
                })
                .await
            }
        };
        drop(admission);
        let ((painted, records), lines) = (recorded.0?, recorded.1);
        painted?;
        let masks = self.masks(natives, fx.iter().flat_map(|p| p.extra.take()), &lines);
        Ok((records, masks))
    }

    #[allow(clippy::type_complexity)]
    async fn paint_into_async(
        &self,
        el: &mut gpui::AnyElement,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<(Result<(), PaintRecordError>, PaintSegmentRecords), PaintRecordError> {
        let slices = self.slices.clone();
        window
            .record_paint_segments_async(
                self.extent,
                self.routes.clone(),
                cx,
                stack,
                async |window, cx, stack| {
                    window
                        .with_paint_segment_slices_async(slices, async |window| {
                            el.paint_async(window, cx, stack).await
                        })
                        .await
                },
            )
            .await
    }
}
