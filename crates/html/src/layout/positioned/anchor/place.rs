//! `AnchorPlace`: сдвиг абсолютной коробки к якорю на подготовке кадра.

use super::parse::{VIS_NO_OVERFLOW, VIS_VALID, VIS_VISIBLE};
use super::{
    AREA_LAST, AREA_NOW, AnchorPlace, AnchorPlan, CELL_LAST, CELL_NOW, CHOSEN, USED_LAST, geometry,
    reframe_if_stale, request_rebuild,
};
use crate::style::computed::Computed;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

/// Обернуть готовую коробку заместителем; без `anchor()`-вставок, области,
/// `anchor-center`, списка вариантов и `position-visibility` — как есть.
///
/// Кандидаты §fallback строятся от БАЗОВЫХ стилей (`try_base` — стиль до
/// наложения выбранного варианта, `apply_chosen`): [0] — база, дальше по
/// одному на элемент списка: правило `@position-try` накладывается на копию
/// базы, `<position-area>` подменяет область, тактика переворачивает
/// готовый план. Правило без определения — «does nothing»: варианта нет.
pub fn place(el: AnyElement, own: &Computed, inherited: &Computed) -> AnyElement {
    let base = own.try_base.as_deref().unwrap_or(own);
    let Some(first) = AnchorPlan::of(base, inherited) else {
        return el;
    };
    let mut plans = vec![first];
    for fb in &base.position_try_fallbacks {
        let mut c = base.clone();
        if let Some(n) = &fb.name {
            let Some(decls) = crate::style::css::try_rule(n) else {
                continue;
            };
            c.apply_decls(&decls);
        }
        if let Some(area) = fb.area {
            c.position_area = Some(area);
        }
        let Some(mut p) = AnchorPlan::of(&c, inherited) else {
            continue;
        };
        p.apply_tactics(fb.tactics);
        plans.push(p);
    }
    AnchorPlace {
        child: Some(el),
        plans,
        order: base.position_try_order,
        key: own.anchor_key,
        visibility: own.position_visibility,
    }
    .into_any_element()
}

impl Element for AnchorPlace {
    type RequestLayoutState = LayoutId;
    /// Спрятана ли коробка по `position-visibility`: `paint` пропускает
    /// всё поддерево (`force-hidden`; Blink — слой и потомки не рисуются).
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let id = self.child.as_mut().unwrap().request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let own = geometry::logical_own(*state, bounds, window);
        let (k, p) = self.choose(own, window);
        let plan = &self.plans[k];
        if plan.area.is_some() {
            AREA_NOW.with(|m| m.borrow_mut().insert(self.key, p.imcb));
            CELL_NOW.with(|m| m.borrow_mut().insert(self.key, p.cell));
            let stale = AREA_LAST.with(|m| m.borrow().get(&self.key) != Some(&p.imcb))
                || CELL_LAST.with(|m| m.borrow().get(&self.key) != Some(&p.cell));
            if stale && USED_LAST.with(|u| u.get()) {
                reframe_if_stale(window);
            }
        }
        if self.plans.len() > 1 {
            let prev = CHOSEN.with(|m| m.borrow_mut().insert(self.key, k));
            // Объявления выбранного правила лягут в стиль на следующей
            // сборке (`apply_chosen`) — попросить её.
            if prev != Some(k) {
                request_rebuild(window);
            }
        }
        let mut hidden = false;
        // §position-visibility. anchor-valid: якорь по умолчанию нужен, но не
        // разрешается. anchor-visible: якорь невидим либо целиком обрезан
        // промежуточной коробкой — его маска его не пересекает, а маска самой
        // коробки (обрезки содержащего блока и выше) пересекает. no-overflow:
        // выбранный вариант всё равно переполняет IMCB.
        if self.visibility & VIS_VALID != 0 && plan.refs_default && plan.rec(None).is_none() {
            hidden = true;
        }
        if self.visibility & VIS_VISIBLE != 0
            && let Some(r) = plan.rec(None)
        {
            let own_clip = window.content_mask().bounds;
            if r.hidden || (!r.rect.intersects(&r.clip) && r.rect.intersects(&own_clip)) {
                hidden = true;
            }
        }
        if self.visibility & VIS_NO_OVERFLOW != 0 && p.overflow {
            hidden = true;
        }
        let child = self.child.as_mut().unwrap();
        window.with_exact_element_offset(gpui::point(px(p.dx), px(p.dy)), |window| {
            child.prepaint(window, cx)
        });
        hidden
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut LayoutId,
        hidden: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*hidden {
            self.child.as_mut().unwrap().paint(window, cx);
        }
    }
}

impl IntoElement for AnchorPlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
