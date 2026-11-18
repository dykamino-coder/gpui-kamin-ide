//! Embeddable full-page document view. Its size is supplied by the embedding host.

use crate::{Document, DocumentBuilder, DocumentFailure, RenderOpts};
use gpui::{
    App, AppContext, Context, ElementPlace, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Size, Styled, Subscription, Window, div, px,
};

/// A persistent document with engine-owned root scrolling, independent of any shell.
///
/// Call `resize` with the embedded content-box size, in CSS pixels. The standalone
/// example uses window bounds; an embedding application supplies its own bounds.
pub struct DocumentView {
    document: Document,
    opts: RenderOpts,
    focus: FocusHandle,
    place: Option<ElementPlace>,
    _step: Subscription,
}

impl DocumentView {
    /// Build once, retaining source options, fonts and the document's resource budget.
    pub fn open(
        builder: &DocumentBuilder<'_>,
        viewport: Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Entity<Self>, DocumentFailure> {
        let document = builder.build_for_view(true)?;
        Ok(cx.new(|cx| Self::new(document, viewport, window, cx)))
    }

    fn new(
        document: Document,
        viewport: Size<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.weak_entity();
        let step = window.on_update_rendering(move |window, app| {
            let _ = view.update(app, |view, cx| {
                if let Some(place) = &view.place {
                    view.document
                        .update_rendering(&view.opts, place, window, cx, |_| {});
                }
            });
        });
        Self {
            document,
            place: None,
            opts: RenderOpts {
                text: gpui::TextStyle {
                    font_size: px(16.).into(),
                    font_family: "Times New Roman".into(),
                    ..Default::default()
                },
                viewport: (f32::from(viewport.width), f32::from(viewport.height)),
                normal_line_height: 1.31,
                cx: Default::default(),
                frag: Default::default(),
            },
            focus: cx.focus_handle(),
            _step: step,
        }
    }

    /// Resize layout without rebuilding the source document.
    /// Media-query invalidation will use Document::set_viewport when that API lands.
    pub fn resize(&mut self, viewport: Size<Pixels>, cx: &mut Context<Self>) {
        let viewport = (
            f32::from(viewport.width).max(0.),
            f32::from(viewport.height).max(0.),
        );
        if self.opts.viewport != viewport {
            self.opts.viewport = viewport;
            cx.notify();
        }
    }

    /// Access the same document used by input, retained drawing and diagnostics.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Apply document mutations, then notify this view to request a frame.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }
}

impl Focusable for DocumentView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DocumentView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.place = Some(window.element_place());
        let root = self.document.state_rc().root_scroll.clone();
        let focus = self.focus.clone();
        // An anonymous wrapper preserves the document step's element-id place.
        // Wheel input is handled by the document's existing scroll adapters.
        div()
            .w(px(self.opts.viewport.0))
            .h(px(self.opts.viewport.1))
            .bg(gpui::white())
            .track_focus(&self.focus)
            .on_key_down(move |event, window, cx| {
                if focus.is_focused(window) && root.as_ref().is_some_and(|root| root.key(event)) {
                    cx.stop_propagation();
                    window.refresh();
                }
            })
            .child(self.document.render_viewport(&self.opts))
    }
}

#[cfg(test)]
mod tests;
