//! Исходный paint-слот позиционированного элемента, построенного у содержащего блока.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::positioned::containing_block::DeferredPaint;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};

pub(crate) fn source(publication: DeferredPaint) -> AnyElement {
    DeferredSource(publication).hooked()
}

struct DeferredSource(DeferredPaint);

impl Hooks for DeferredSource {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.with_child(|child, mask| {
            // Исходная маска дополняет clip фрагмента, установленный paint.
            window.with_content_mask(Some(mask), |window| {
                gpui::paint_reopen(|| child.paint(window, cx));
            });
        });
    }
}
