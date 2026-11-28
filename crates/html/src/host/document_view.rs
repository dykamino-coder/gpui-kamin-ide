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
    #[cfg(feature = "why-profiler")]
    journal: Option<crate::why::WindowJournal>,
}

impl DocumentView {
    /// Build once, retaining source options, fonts and the document's resource budget.
    /// The initial `viewport` is also the size of the screen medium of the build.
    pub fn open(
        builder: &mut DocumentBuilder<'_>,
        viewport: Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Entity<Self>, DocumentFailure> {
        let document = builder.viewport(css_size(viewport)).build_for_view(true)?;
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
                #[cfg(feature = "why-profiler")]
                let mut sample = view.journal.as_ref().map(|j| j.begin(&view.document));
                if let Some(place) = &view.place
                    && window.view_needs_rendering(cx.entity_id(), cx)
                {
                    view.document
                        .update_rendering(&view.opts, place, window, cx, |phase| {
                            #[cfg(feature = "why-profiler")]
                            if let Some(sample) = &mut sample {
                                sample.phase(phase);
                            }
                            #[cfg(not(feature = "why-profiler"))]
                            let _ = phase;
                        });
                    #[cfg(feature = "why-profiler")]
                    if let Some(sample) = &mut sample {
                        sample.prepared();
                    }
                }
                #[cfg(feature = "why-profiler")]
                if let Some(sample) = sample {
                    let weak = cx.weak_entity();
                    window.defer(cx, move |_, cx| {
                        let _ = weak.update(cx, |view, _| {
                            if let Some(journal) = &view.journal {
                                journal.drawn(sample, &view.document);
                            }
                        });
                    });
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
                viewport: css_size(viewport),
                normal_line_height: 1.31,
                cx: Default::default(),
                frag: Default::default(),
            },
            focus: cx.focus_handle(),
            _step: step,
            #[cfg(feature = "why-profiler")]
            journal: None,
        }
    }

    /// Follow a new embedded size: the screen medium takes it as an environment
    /// change of the document (`Document::set_viewport`), which restyles only
    /// when a media query flips; otherwise layout alone follows the new size.
    pub fn resize(
        &mut self,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> Result<(), DocumentFailure> {
        let viewport = css_size(viewport);
        if self.opts.viewport == viewport {
            return Ok(());
        }
        self.opts.viewport = viewport;
        cx.notify();
        self.document.set_viewport(viewport).map(drop)
    }

    /// Access the same document used by input, retained drawing and diagnostics.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Apply document mutations, then notify this view to request a frame.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// Attach the shared DevTools frame journal. Without a recorder there
    /// are no diagnostic clocks, checkpoints or memory walks in this view.
    #[cfg(feature = "why-profiler")]
    pub fn set_journal(&mut self, journal: crate::why::WindowJournal, window: &Window, cx: &App) {
        journal.sample_memory("open", &self.document, window, cx);
        self.journal = Some(journal);
    }
}

fn css_size(viewport: Size<Pixels>) -> (f32, f32) {
    (
        f32::from(viewport.width).max(0.),
        f32::from(viewport.height).max(0.),
    )
}

impl Focusable for DocumentView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DocumentView {
    fn cached_style(&self) -> Option<gpui::StyleRefinement> {
        Some(gpui::StyleRefinement {
            size: size_style(self.opts.viewport),
            ..Default::default()
        })
    }

    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.place = Some(window.element_place());
        let root = self.document.state_rc().root_scroll.clone();
        let focus = self.focus.clone();
        let released = root.clone();
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
            .on_key_up(move |event, _, _| {
                if let Some(root) = &released {
                    root.released(event);
                }
            })
            .child(self.document.render_viewport(&self.opts))
    }
}

fn size_style((width, height): (f32, f32)) -> gpui::SizeRefinement<gpui::Length> {
    gpui::SizeRefinement {
        width: Some(px(width).into()),
        height: Some(px(height).into()),
    }
}

#[cfg(test)]
mod bar_wheel_tests;
#[cfg(test)]
mod embedding_tests;
#[cfg(test)]
mod fixed_hoisted_tests;
#[cfg(test)]
mod fixed_tests;
#[cfg(test)]
mod host_frame_tests;
#[cfg(test)]
#[cfg(feature = "why-profiler")]
mod journal_tests;
#[cfg(test)]
mod nested_scroll_tests;
#[cfg(test)]
mod paint_compare;
#[cfg(test)]
mod resize_tests;
#[cfg(test)]
mod scroll_record_tests;
#[cfg(test)]
mod scroll_replay_tests;
#[cfg(test)]
mod tests;
