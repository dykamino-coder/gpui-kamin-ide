//! Правая колонка: карты тулов и их слоты.
//!
//! Блок `render` вынесен как есть (`plan/100-refactor-250.md`).

use crate::probe::registry::probe_area;
use crate::root::DragKind;
use crate::state::model::RootView;
use crate::ui::splitter::h_handle;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn right_column(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
    ) -> AnyElement {
        {
            let top_body = self.tool_body(crate::activity::PanelSlot::RightTop, p, cx);
            // Гейт rightPanelBottomVisible: низ скрыт → верхняя карта на всю
            // высоту, сплиттер и нижняя карта не рендерятся (как оригинал)
            let right_bottom_on = self.layout.right_panel_bottom_visible;
            let split = self.right_split;
            // `.splitHandle { height: 10px }` — ручка не сжимается и съедает
            // высоту у обеих карт
            const SPLIT_HANDLE_H: f32 = 10.0;
            // Высота колонки ИЗ ВЬЮПОРТА, не из probe: замер прошлого кадра
            // на прыжке размера (restore из maximize) давал верхней карте
            // высоту больше колонки, и нижняя (flex_basis 0 → нулевой вес
            // сжатия) обнулялась — «панель пропала» (аудит ресайза, ф.1/2).
            // Колонка всегда во всю высоту body — формула точна на ЭТОМ кадре.
            let col_h = {
                let vp = window.viewport_size();
                (f32::from(vp.height) - m::TITLEBAR_HEIGHT - m::STATUS_BAR_HEIGHT).max(0.0)
            };
            let mut right_col = div()
                    .relative()
                    // Досье 56 — КОЛОНКА; `right-top` это её верхняя карта
                    // (досье 58), общий регион давал им один кроп (ревью ц.26)
                    .child(probe_area("right-panel-column"))
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_w(px(0.))
                    // `.column { min-height: 0 }` (`RightPanel.module.css:10`)
                    .min_h(px(0.))
                    .child(
                        div()
                            .map(|d| {
                                if right_bottom_on {
                                    // Оригинал даёт ОБЕИМ картам процентный базис,
                                    // а ручка `flex-shrink: 0; height: 10px`
                                    // (`:128-131`) вычитается из остатка, поэтому
                                    // фактическая высота верха = `split·(H − 10)`.
                                    // Мы брали `split·H` и при H 1000, split .55
                                    // давали 550 вместо 544.5 (ревью ц.26)
                                    d.h(px((split * (col_h - SPLIT_HANDLE_H)).max(0.0)))
                                } else {
                                    d.flex_1().min_h(px(0.))
                                }
                            })
                            .w_full()
                            .child(crate::ui::right_column::card_with_rail(
                                p,
                                {
                                    // Слот rightTop — унифицированное тело
                                    // (activity-модель), не хардкод дерева
                                    let _slot = crate::activity::PanelSlot::RightTop;
                                    crate::ui::glint::glint_surface_wv_holed(
                                    p,
                                    div()
                                        .id("right-top")
                                        .relative()
                                        .size_full()
                                        .child(probe_area("right-top"))
                                        .children(self.card_drop_hint_top(
                                            crate::activity::PanelSlot::RightTop,
                                            p,
                                        ))
                                        // Правые карты БЕЗ таб-стрипа (как
                                        // оригинал): тулы и «…» — в РЕЙЛЕ,
                                        // карта = чистое тело активного тула
                                        .child(match top_body {
                                            Some(el) => el,
                                            None => crate::ui::panel_placeholder::panel_placeholder_ex(
                                                "Right",
                                                "Open new tool or drag-n-drop tool from other panels",
                                                crate::ui::panel_placeholder::SlotIcon::RightTop,
                                                Some(crate::ui::slot_panel::open_tool_btn(
                                                    crate::activity::PanelSlot::RightTop,
                                                    // `PanelPlaceholder` всегда
                                                    // `popDirection="up"`
                                                    true,
                                                    &self.tx,
                                                    p,
                                                )),
                                                p,
                                            ),
                                        })
                                        .into_any_element(),
                                )
                                .into_any_element()
                                },
                                crate::activity::PanelSlot::RightTop,
                                self.activity.state(crate::activity::PanelSlot::RightTop),
                                self.tool_drag_over_index(crate::activity::PanelSlot::RightTop),
                                self.tool_dragging_in(crate::activity::PanelSlot::RightTop),
                                false,
                                &self.tx,
                            )),
                    );
            if right_bottom_on {
                right_col = right_col
                    .child(h_handle(
                        "right-split-handle",
                        p,
                        self.handle_show("right-split-handle", DragKind::RightSplit),
                        self.drag.is_some(),
                        // pr = рейл 48; pl = 4 — компенсация того, что обёртка
                        // колонки шире карты на 4 (`right_w + 4`), иначе грип
                        // уезжает на 2px левее центра карты (ревью ц.14)
                        m::ACTIVITY_BAR_WIDTH,
                        4.0,
                        cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                            let vw = f32::from(window.viewport_size().width);
                            this.begin_drag(DragKind::RightSplit, f32::from(e.position.y), vw);
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hovered_handle = if *hovered {
                                Some("right-split-handle")
                            } else {
                                None
                            };
                            cx.notify();
                        }),
                    ))
                    .child(div().flex_1().min_h(px(0.)).min_w(px(0.)).child(
                        crate::ui::right_column::card_with_rail(
                            p,
                            {
                                // Слот rightBottom (plan-вебвью — фаза расширений)
                                let slot = crate::activity::PanelSlot::RightBottom;
                                let body = self.tool_body(slot, p, cx);
                                crate::ui::glint::glint_surface_wv_holed(
                                    p,
                                    div()
                                        .id("right-bottom")
                                        .relative()
                                        .size_full()
                                        .child(probe_area("right-bottom"))
                                        .children(self.card_drop_hint_top(crate::activity::PanelSlot::RightBottom, p))
                                        .child(match body {
                                            Some(el) => el,
                                            None => {
                                                crate::ui::panel_placeholder::panel_placeholder_ex(
                                                    "Right Bottom",
                                                    "Open new tool or drag-n-drop tool from other panels",
                                                    crate::ui::panel_placeholder::SlotIcon::RightBottom,
                                                    Some(crate::ui::slot_panel::open_tool_btn(
                                                        crate::activity::PanelSlot::RightBottom,
                                                        true,
                                                        &self.tx,
                                                        p,
                                                    )),
                                                    p,
                                                )
                                            }
                                        })
                                        .into_any_element(),
                                )
                                .into_any_element()
                            },
                            crate::activity::PanelSlot::RightBottom,
                            self.activity.state(crate::activity::PanelSlot::RightBottom),
                            self.tool_drag_over_index(crate::activity::PanelSlot::RightBottom),
                            self.tool_dragging_in(crate::activity::PanelSlot::RightBottom),
                            true,
                            &self.tx,
                        ),
                    ));
            }
            right_col.into_any_element()
        }
    }
}
