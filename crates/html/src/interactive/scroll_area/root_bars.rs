//! The bars of a scrollport drawn apart from its content (`RootBars`).

use gpui::{Bounds, Pixels, Window};

/// The bars of a scrollport, drawn apart from its content. The thumbs follow
/// the scroll offset, so the root scrollport's are drawn by the document's
/// frame element after its retained record, every frame: a wheel step that
/// replays the record moved draws them at the new offset (as cc keeps
/// scrollbar layers out of the scrolled content's paint).
pub(crate) struct RootBars {
    pub(super) style: crate::style::computed::BoxStyle,
    pub(super) freeze: bool,
    pub(super) handle: gpui::ScrollHandle,
    pub(super) extent: crate::interactive::scroll_extent::Frame,
    pub(super) interaction: crate::interactive::scrollbars::interaction::Interaction,
    pub(super) bounds: Bounds<Pixels>,
}

impl RootBars {
    /// The hit areas of the bars.
    pub(crate) fn prepaint(
        &self,
        window: &mut Window,
    ) -> Vec<crate::interactive::scrollbars::interaction::HitBar> {
        crate::interactive::scrollbars::follow(
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            window,
        );
        crate::interactive::scrollbars::interaction::prepaint(
            self.freeze,
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            window,
        )
    }

    /// Their listeners and paint at the offset now.
    pub(crate) fn paint(
        &self,
        hitbars: &[crate::interactive::scrollbars::interaction::HitBar],
        window: &mut Window,
    ) {
        let alpha = self
            .interaction
            .paint(self.freeze, hitbars, &self.handle, window);
        crate::interactive::scrollbars::paint(
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            alpha,
            window,
        );
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
