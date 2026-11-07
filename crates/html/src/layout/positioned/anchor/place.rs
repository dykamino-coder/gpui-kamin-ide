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
    let Some(mut first) = AnchorPlan::of(own, inherited, cx.vcx(), cx.layout()) else {
        return el;
    };
    if let Some((key, inputs)) = cx.position.oof_sizes
        && key == own.anchor_key
    {
        first.refs_default |= inputs.refs_default;
    }
    if let Some((key, probe)) = &cx.position.oof_trial
        && *key == own.anchor_key
    {
        return probe.wrap(
            el,
            first,
            own.reset_positioned.position_visibility,
            own.anchor_key,
        );
    }
    let vcx = cx.vcx();
    let plans = PlacementPlans {
        plans: vec![first],
        indices: vec![0],
        order: 0,
        key: own.anchor_key,
        visibility: own.reset_positioned.position_visibility,
        values: vcx.keep(),
        layout: cx.layout().clone(),
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

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        let id = self.child.request_layout_async(window, cx, stack).await;
        (id, id)
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        place_one(
            &mut self.child,
            &self.plans,
            bounds,
            *state,
            window,
            cx,
            stack,
        )
        .await
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _request: &mut Self::Layout,
        hidden: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, _bounds, _request, hidden, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _request: &'a mut Self::Layout,
        hidden: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            if !*hidden {
                self.child.paint_async(window, cx, stack).await;
            }
        }
    }
}

/// Разместить одну уже разложенную коробку; слой и строчные входы используют
/// общую процедуру выбора варианта, публикации геометрии и видимости.
pub(crate) async fn place_one(
    child: &mut AnyElement,
    plans: &PlacementPlans,
    bounds: Bounds<Pixels>,
    layout: LayoutId,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> bool {
    let own = geometry::logical_own(layout, bounds, window);
    let (k, p) = plans.choose(own, window, plans.values.vcx());
    let plan = &plans.plans[k];
    plans.layout.anchors.present.borrow_mut().insert(plans.key);
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
    crate::layout::positioned::containing_block::metrics::record::settled_offset(
        plans.key,
        bounds.origin + gpui::point(px(p.dx), px(p.dy)),
        window,
    );
    window
        .with_exact_element_offset_async(gpui::point(px(p.dx), px(p.dy)), async |window| {
            child.prepaint_async(window, cx, stack).await
        })
        .await;
    hidden
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Computed, Position, ResetPositioned, StyleGroup};
    use gpui::{IntoElement, TestAppContext, canvas, point, size};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn nested_anchor_placements_use_a_bounded_prepaint_stack() {
        let mut app = TestAppContext::single();
        let mut addresses = Vec::new();
        for depth in [16, 64, 128] {
            let sample = Rc::new(Cell::new(0));
            app.add_empty_window()
                .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                    let sample = sample.clone();
                    let mut child = canvas(
                        move |_, _, _| {
                            let anchor = 0u8;
                            sample.set(std::hint::black_box(&anchor) as *const u8 as usize);
                        },
                        |_, (), _, _| {},
                    )
                    .into_any_element();
                    let cx = crate::layout::cx::LayoutCx::default();
                    let own = BoxStyle::new(Computed {
                        reset_positioned: StyleGroup::new(ResetPositioned {
                            position: Some(Position::Absolute),
                            position_visibility: VIS_NO_OVERFLOW,
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                    for _ in 0..depth {
                        let plan = AnchorPlan::of(&own, &own, cx.vcx(), cx.layout()).unwrap();
                        child = Placed {
                            child,
                            plans: PlacementPlans {
                                plans: vec![plan],
                                indices: vec![0],
                                order: 0,
                                key: 0,
                                visibility: VIS_NO_OVERFLOW,
                                values: cx.vcx().keep(),
                                layout: cx.layout().clone(),
                            },
                        }
                        .hooked();
                    }
                    child
                });
            assert_ne!(sample.get(), 0, "prepaint должен посетить лист");
            addresses.push(sample.get());
        }
        let spread = addresses.iter().max().unwrap() - addresses.iter().min().unwrap();
        assert!(
            spread < 32 * 1024,
            "anchor prepaint расходует стек с глубиной: {spread}"
        );
        app.quit();
    }
}
