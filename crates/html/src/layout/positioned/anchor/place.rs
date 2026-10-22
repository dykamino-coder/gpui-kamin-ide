//! Единое размещение OOF по якорю и прозрачный адаптер готовой коробки.

use super::parse::{VIS_NO_OVERFLOW, VIS_VALID, VIS_VISIBLE};
use super::{AnchorPlan, PlacementPlans, geometry, reframe_if_stale, request_rebuild};
use crate::document::state::ValueCx;
use crate::hooks::{Hooks, IntoHooked};
use crate::style::computed::Computed;

use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

pub(crate) fn place_in_context(
    el: AnyElement,
    own: &Computed,
    inherited: &Computed,
    cx: &crate::layout::cx::LayoutCx,
) -> AnyElement {
    if let Some((key, probe)) = &cx.oof_trial
        && *key == own.anchor_key
        && let Some(mut plan) = AnchorPlan::of(own, inherited, cx.vcx())
    {
        plan.history = false;
        return probe.wrap(el, plan, own.position_visibility, own.anchor_key);
    }
    place(el, own, inherited, cx.vcx())
}

/// Обернуть готовую коробку заместителем; без `anchor()`-вставок, области,
/// `anchor-center`, списка вариантов и `position-visibility` — как есть.
///
/// Кандидаты §fallback строятся от БАЗОВЫХ стилей (`try_base` — стиль до
/// наложения выбранного варианта, `apply_chosen`): [0] — база, дальше по
/// одному на элемент списка: правило `@position-try` накладывается на копию
/// базы, `<position-area>` подменяет область, тактика преобразует свойства
/// до построения плана. Правило без определения — «does nothing»: варианта нет.
pub fn place(el: AnyElement, own: &Computed, inherited: &Computed, vcx: ValueCx<'_>) -> AnyElement {
    let base = own.try_base.as_deref().unwrap_or(own);
    let Some(first) = AnchorPlan::of(base, inherited, vcx) else {
        return el;
    };
    let mut plans = vec![first];
    let mut indices = vec![0];
    for (index, fb) in base.position_try_fallbacks.iter().enumerate() {
        let mut c = base.clone();
        if let Some(n) = &fb.name {
            let Some(decls) = vcx.doc.try_rule(n) else {
                continue;
            };
            super::resolve::apply_try_rule(&mut c, &decls, vcx);
        }
        if let Some(area) = fb.area {
            c.position_area = Some(area);
        }
        super::trials::tactics::apply(&mut c, inherited, fb.tactics, vcx);
        let Some(p) = AnchorPlan::of(&c, inherited, vcx) else {
            continue;
        };
        plans.push(p);
        indices.push(index + 1);
    }
    let plans = PlacementPlans {
        plans,
        indices,
        order: base.position_try_order,
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
    // IMCB нужен и клетке `position-area`, и паре вставок с `anchor()`
    // при `auto`-размере (`resolve::fill_anchor_pairs`).
    {
        vcx.doc
            .anchor_frame
            .area_now
            .borrow_mut()
            .insert(plans.key, p.imcb);
        vcx.doc
            .anchor_frame
            .cell_now
            .borrow_mut()
            .insert(plans.key, p.cell);
        let stale = vcx.doc.anchor_frame.area_last.borrow().get(&plans.key) != Some(&p.imcb)
            || vcx.doc.anchor_frame.cell_last.borrow().get(&plans.key) != Some(&p.cell);
        if stale && vcx.doc.anchor_frame.used_last.get() {
            reframe_if_stale(window, vcx);
        }
    }
    if plans.plans.len() > 1 {
        let prev = vcx
            .doc
            .anchor_frame
            .chosen
            .borrow_mut()
            .insert(plans.key, super::trials::Chosen::legacy(plans.indices[k]));
        // Объявления выбранного правила лягут в стиль на следующей
        // сборке (`apply_chosen`) — попросить её.
        if prev.map(|p| p.index) != Some(plans.indices[k]) {
            request_rebuild(window);
        }
    }
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
