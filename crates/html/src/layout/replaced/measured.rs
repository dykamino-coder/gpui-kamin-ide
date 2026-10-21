//! Measure replaced content without fixing its automatic preferred dimensions.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, LayoutId, Pixels, Refineable, Style,
    Styled, Window, point, px, size,
};

pub(crate) type ControlMeasure = std::rc::Rc<
    dyn Fn(
        gpui::Size<Option<Pixels>>,
        gpui::Size<AvailableSpace>,
        &mut Window,
        &mut App,
    ) -> gpui::Size<Pixels>,
>;

/// Form controls have natural dimensions, but no natural aspect ratio (CSS Images 3 §2).
/// Keep that geometry available when intrinsic probes ignore author size constraints.
pub(crate) fn natural_control(painted: gpui::Div, natural: gpui::Size<Pixels>) -> AnyElement {
    natural_control_with(painted, natural, None)
}

pub(crate) fn natural_control_with(
    mut painted: gpui::Div,
    natural: gpui::Size<Pixels>,
    measure: Option<ControlMeasure>,
) -> AnyElement {
    let mut native = Style::default();
    native.refine(painted.style());
    let ratio = native.aspect_ratio;
    // Native decoration is clipped internally; the host keeps its authored overflow.
    // Putting hidden on the host would erase a flex item's automatic minimum.
    painted = painted.size_full().overflow_hidden();
    let inner = painted.style();
    inner.margin = Default::default();
    inner.min_size = Default::default();
    inner.max_size = Default::default();
    inner.calc_size = Some([None; 4]);
    inner.calc_size_max = Some([None; 2]);
    inner.calc_size_basis = Some([None; 6]);
    inner.sizing_keywords = None;
    inner.min_sizing_keywords = None;
    inner.max_sizing_keywords = None;
    inner.aspect_ratio = None;
    inner.content_box = Some(false);
    inner.flex_basis = None;
    inner.flex_grow = None;
    inner.flex_shrink = None;
    inner.position = None;
    inner.inset = Default::default();
    MeasuredReplaced {
        image: painted.into_any_element(),
        style: native,
        natural,
        ratio,
        measure,
    }
    .hooked()
}

pub(super) struct MeasuredReplaced {
    pub(super) image: AnyElement,
    pub(super) style: Style,
    pub(super) natural: gpui::Size<Pixels>,
    pub(super) ratio: Option<f32>,
    pub(super) measure: Option<ControlMeasure>,
}

impl Hooks for MeasuredReplaced {
    type Layout = (LayoutId, LayoutId);
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let picture = self.image.request_layout(window, cx);
        let natural = self.natural;
        let ratio = self.ratio;
        let measure = self.measure.clone();
        let layout = window.request_measured_layout(
            self.style.clone(),
            move |known, available, window, cx| {
                let natural = if known.width.is_some() && known.height.is_some() {
                    natural
                } else {
                    measure
                        .as_ref()
                        .map_or(natural, |f| f(known, available, window, cx))
                };
                match (known.width, known.height, ratio) {
                    (Some(w), Some(h), _) => size(w, h),
                    (Some(w), None, Some(r)) => size(w, px(f32::from(w) / r)),
                    (None, Some(h), Some(r)) => size(px(f32::from(h) * r), h),
                    (Some(w), None, None) => size(w, natural.height),
                    (None, Some(h), None) => size(natural.width, h),
                    (None, None, _) => natural,
                }
            },
        );
        (layout, (picture, layout))
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        // Reconstruct the decoration from CSS dimensions, before pixel snapping.
        // Using snapped bounds enlarges a 34px box to 34.4px at scale 1.25,
        // making its percentage-painted content overlap a dotted border.
        let dimensions = window.layout_size_unrounded(state.1);
        let origin = window.layout_origin_unrounded(state.1);
        window.compute_layout(
            state.0,
            size(
                AvailableSpace::Definite(dimensions.width),
                AvailableSpace::Definite(dimensions.height),
            ),
            cx,
        );
        window.set_layout_root_origin(state.0, origin);
        self.image.prepaint_at(point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.image.paint(window, cx);
    }
}
