//! Подсветка карты-панели под drop тула (`[data-activity-drop]`, `global.css`).
//!
//! Оригинал вешает утилити-атрибут на КАРТУ (сайдбар, main, main-bottom,
//! central-bottom, обе правые), потому что дроп-зона — вся панель, а не только
//! её полоска табов. Рисуем тем же слоем поверх содержимого: gpui не умеет
//! `outline-offset`, поэтому вместо обводки — абсолютный бокс с инсетом 2px.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_theme::Palette;

use crate::colors::{rgba, tint};

/// Слой подсветки: `over` — пунктир accent 60 % на подложке accent 10 %;
/// `blocked` (тул уже закреплён в этом слоте) — красное свечение без пунктира.
pub fn card_drop(over: bool, blocked: bool, p: &Palette) -> Option<AnyElement> {
    card_drop_r(over, blocked, 16.0, p)
}

/// То же с явным радиусом: у карт он 16 (glint), а у самого бара/рейла
/// (`<nav>` оригинала) скругления нет вовсе.
pub fn card_drop_r(over: bool, blocked: bool, radius: f32, p: &Palette) -> Option<AnyElement> {
    // Карты — glint-поверхности: заливки у них НЕ будет (см. модуль-док)
    card_drop_full(over, blocked, radius, radius > 0.0, p)
}

/// `fill_suppressed` — цель является glint-поверхностью, и её собственный
/// `background` перебивает заливку дроп-подсветки (порядок правил в
/// `global.css`): рисуем только обводку.
pub fn card_drop_full(
    over: bool,
    blocked: bool,
    radius: f32,
    fill_suppressed: bool,
    p: &Palette,
) -> Option<AnyElement> {
    if !over && !blocked {
        return None;
    }
    // Фон — на ВСЮ карту (`background-color` инсета не имеет), обводка —
    // отдельным слоем: у `over` с `outline-offset: -2px`, у `blocked` кольцо
    // `box-shadow: inset 0 0 0 2px` по самой кромке. Раньше вдвинут был и
    // фон, из-за чего у карты оставалась неподсвеченная рамка (ревью ц.13).
    // Радиус — 16, как у glint-поверхности карты.
    let inset = if blocked { 0.0 } else { 2.0 };
    let ring = div()
        .absolute()
        .top(px(inset))
        .left(px(inset))
        .right(px(inset))
        .bottom(px(inset))
        .rounded(px((radius - inset).max(0.0)));
    let (fill, ring) = if blocked {
        (
            tint(rgba(p.accent_red), 0.12),
            ring.border_2().border_color(tint(rgba(p.accent_red), 0.6)),
        )
    } else {
        (
            tint(rgba(p.accent_primary), 0.10),
            ring.border_1()
                .border_dashed()
                .border_color(tint(rgba(p.accent_primary), 0.6)),
        )
    };
    Some(
        div()
            .absolute()
            .inset_0()
            .rounded(px(radius))
            .when(!fill_suppressed, |d| d.bg(fill))
            .child(ring)
            .into_any_element(),
    )
}
