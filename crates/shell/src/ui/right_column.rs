//! Правая колонка (RightPanel.tsx): карта + вертикальный activity-rail СПРАВА.
//! Сборка колонки (v-split со своей ручкой) — в root.rs; здесь — card_with_rail.
//! Rail = pinned-плитки activity-модели слота: press → ToolPress (dnd/клик),
//! RMB → меню Hide / Move to (единая механика со стрип-табами).

use crate::ui::rail::rail;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::activity::{PanelSlot, PanelState};
use crate::host_link::ShellEvent;

/// Гап-обёртка карты (4px → 8px между соседями, как .body gap);
/// min_w 0 — карта сжимается, rail стоит. Вертикального паддинга НЕТ:
/// у оригинала `.body { padding: 0 var(--space-1) }`, карты вплотную
/// к титлбару/статус-бару (замер: 42.4, а не 47.2).
fn gap_wrap(el: AnyElement) -> AnyElement {
    div()
        .size_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .overflow_hidden()
        // Слева полу-зазор колоночной сетки; СПРАВА 0 — карта вплотную
        // к рейлу (.cardWithBar без gap, ревью ц.1)
        .pl(px(4.0))
        .child(el)
        .into_any_element()
}

/// Строка правой колонки: [gap_wrap(card)][rail 48].
#[allow(clippy::too_many_arguments)]
pub fn card_with_rail(
    p: &Palette,
    card: AnyElement,
    slot: PanelSlot,
    state: &PanelState,
    // DnD рейла: индекс вставки и id перетаскиваемого тула
    drop_index: Option<usize>,
    dragging: Option<&str>,
    rail_bottom: bool,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    // rail_bottom=false → верхняя карта (pt 4, pb 0); true → нижняя (pt 0,
    // pb 4): смежные к ручке паддинги убраны — грип строго по центру зазора.
    div()
        .flex()
        .size_full()
        .min_w(px(0.))
        .child(gap_wrap(card))
        .child(rail(slot, state, drop_index, dragging, p, rail_bottom, tx))
        .into_any_element()
}
