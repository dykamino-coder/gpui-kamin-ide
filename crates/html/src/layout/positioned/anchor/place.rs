//! Единое размещение OOF по якорю и прозрачный адаптер готовой коробки.

use super::parse::{VIS_NO_OVERFLOW, VIS_VALID, VIS_VISIBLE};
use super::{AnchorPlan, PlacementPlans, geometry};
use crate::hooks::{Hooks, IntoHooked};
use crate::style::computed::BoxStyle;

use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

pub(crate) fn place_in_context(
    el: AnyElement,
    own: &BoxStyle,
    inherited: &BoxStyle,
    cx: &crate::layout::cx::LayoutCx,
) -> AnyElement {
    let Some(mut first) = AnchorPlan::of(own, inherited, cx.vcx()) else {
        return el;
    };
    if let Some((key, inputs)) = cx.oof_sizes
        && key == own.anchor_key
    {
        first.refs_default |= inputs.refs_default;
    }
    if let Some((key, probe)) = &cx.oof_trial
        && *key == own.anchor_key
    {
        return probe.wrap(el, first, own.position_visibility, own.anchor_key);
    }
    let vcx = cx.vcx();
    let plans = PlacementPlans {
        plans: vec![first],
        indices: vec![0],
        order: 0,
        key: own.anchor_key,
        visibility: own.position_visibility,
        values: vcx.keep(),
    };
    Placed { child: el, plans }.hooked()
}

struct Placed {
    child: AnyElement,
    plans: PlacementPlans,
}

impl Hooks for Placed {
    type Layout = LayoutId;
    /// Спрятана ли коробка по `position-visibility`: `paint` пропускает
    /// всё поддерево (`force-hidden`; Blink — слой и потомки не рисуются).
    type Prepaint = bool;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let id = self.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        place_one(&mut self.child, &self.plans, bounds, *state, window, cx)
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _request: &mut Self::Layout,
        hidden: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*hidden {
            self.child.paint(window, cx);
        }
    }
}

/// Разместить одну уже разложенную коробку; слой и строчные входы используют
/// общую процедуру выбора варианта, публикации геометрии и видимости.
pub(crate) fn place_one(
    child: &mut AnyElement,
    plans: &PlacementPlans,
    bounds: Bounds<Pixels>,
    layout: LayoutId,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let vcx = plans.values.vcx();
    let own = geometry::logical_own(layout, bounds, window);
    let (k, p) = plans.choose(own, window, plans.values.vcx());
    let plan = &plans.plans[k];
    vcx.doc.anchor_frame.present.borrow_mut().insert(plans.key);
    let mut hidden = false;
    // §position-visibility. anchor-valid: якорь по умолчанию нужен, но не
    // разрешается. anchor-visible: якорь невидим либо целиком обрезан
    // промежуточной коробкой — его маска его не пересекает, а маска самой
    // коробки (обрезки содержащего блока и выше) пересекает. no-overflow:
    // выбранный вариант всё равно переполняет IMCB.
    if plans.visibility & VIS_VALID != 0 && plan.refs_default && plan.rec(None).is_none() {
        hidden = true;
    }
    if plans.visibility & VIS_VISIBLE != 0
        && let Some(r) = plan.rec(None)
    {
        let own_clip = window.content_mask().bounds;
        if r.hidden || (!r.rect.intersects(&r.clip) && r.rect.intersects(&own_clip)) {
            hidden = true;
        }
    }
    if plans.visibility & VIS_NO_OVERFLOW != 0 && p.overflow {
        hidden = true;
    }
    window.with_exact_element_offset(gpui::point(px(p.dx), px(p.dy)), |window| {
        child.prepaint(window, cx)
    });
    hidden
}
