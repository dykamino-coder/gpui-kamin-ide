//! Window inputs the recorded document frame depends on: bounds, content mask,
//! inherited text style, offset, scale, rem, the epoch of inputs that changed
//! element state and of focus moves, window activity and appearance, the
//! animation snapshot time, reduce-motion. An input that changed nothing (a
//! pointer move over content) keeps the frame (`gpui::Window::interaction_epoch`).

use gpui::{App, Bounds, ContentMask, Pixels, Point, TextStyle, Window, WindowAppearance};

/// Входы окна, от которых зависит записанный кадр.
#[derive(Clone)]
pub(super) struct WindowKey {
    bounds: Bounds<Pixels>,
    content_mask: ContentMask<Pixels>,
    text: TextStyle,
    offset: Point<Pixels>,
    scale: f32,
    rem: Pixels,
    interaction: u64,
    active: bool,
    appearance: WindowAppearance,
    elapsed: Option<std::time::Duration>,
    reduce_motion: bool,
    animation: (Option<std::time::Duration>, u64),
    /// Redraws hover styles asked for (`gpui::Window::hover_epoch`): not an input of a
    /// frame whose scene layer repaints its hover boxes (`Builder::repaint_boxes`), only
    /// of one without a layer (`WindowKey::same_hovers`).
    hovers: usize,
}

/// Equal inputs of the frame, apart from the hover redraws (`hovers`).
impl PartialEq for WindowKey {
    fn eq(&self, other: &Self) -> bool {
        self.bounds == other.bounds
            && self.content_mask == other.content_mask
            && self.text == other.text
            && self.offset == other.offset
            && self.scale == other.scale
            && self.rem == other.rem
            && self.interaction == other.interaction
            && self.active == other.active
            && self.appearance == other.appearance
            && self.elapsed == other.elapsed
            && self.reduce_motion == other.reduce_motion
            && self.animation == other.animation
    }
}

impl WindowKey {
    /// Before host layout, the last painted placement is only a replay candidate.
    /// Prepaint captures the actual placement and validates it again before reuse.
    pub(super) fn at_recorded_place(mut self, recorded: &Self) -> Self {
        self.bounds.origin = recorded.bounds.origin;
        self.content_mask = recorded.content_mask;
        self.text = recorded.text.clone();
        self.offset = recorded.offset;
        self
    }

    /// No hover style asked for a redraw between the two frames.
    pub(super) fn same_hovers(&self, other: &Self) -> bool {
        self.hovers == other.hovers
    }

    /// Name changed inputs without formatting their potentially large values.
    pub(super) fn changed(&self, other: &Self) -> Vec<&'static str> {
        [
            (self.bounds != other.bounds, "bounds"),
            (self.content_mask != other.content_mask, "content_mask"),
            (self.text != other.text, "text"),
            (self.offset != other.offset, "offset"),
            (self.scale != other.scale, "scale"),
            (self.rem != other.rem, "rem"),
            (self.interaction != other.interaction, "interaction"),
            (self.active != other.active, "active"),
            (self.appearance != other.appearance, "appearance"),
            (self.elapsed != other.elapsed, "elapsed"),
            (self.reduce_motion != other.reduce_motion, "reduce_motion"),
            (self.animation != other.animation, "animation"),
        ]
        .into_iter()
        .filter_map(|(changed, name)| changed.then_some(name))
        .collect()
    }

    /// Discount inputs that only moved the root scroller
    /// (`RootScroll::key`); the frame replays them through its scroll nodes.
    pub(super) fn capture(
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &App,
        state: &super::DocState,
    ) -> Self {
        let scroll_inputs = state
            .root_scroll
            .as_ref()
            .map_or(0, |root| root.scroll_inputs.get());
        state.animation.sample(window, cx);
        Self {
            bounds,
            content_mask: window.content_mask(),
            text: window.text_style(),
            offset: window.element_offset(),
            scale: window.scale_factor(),
            rem: window.rem_size(),
            interaction: window.interaction_epoch().wrapping_sub(scroll_inputs),
            active: window.is_window_active(),
            appearance: window.appearance(),
            // Snapshot time is an input only after CSS sampled an effect.
            // Advancing a scroll clock alone must keep the recorded document.
            elapsed: cx
                .try_global::<gpui::AnimationElapsedTime>()
                .filter(|_| state.animation.frame().is_some_and(|f| f.time_dependent))
                .map(|t| t.0),
            reduce_motion: cx.reduce_motion(),
            animation: state.animation.render_key(),
            hovers: window.hover_epoch(),
        }
    }
}
