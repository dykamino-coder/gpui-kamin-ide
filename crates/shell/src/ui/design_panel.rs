//! Design-панель (DesignPanel + design-sections 1:1): design-system справочник —
//! цвета (4 группы, 26 токенов), типографика (font-сэмплы + 5-шаговая шкала),
//! спейсинг (7 рядов), радиусы (4 бокса 80×80), тени (9 токенов из словаря),
//! компоненты (кнопки 4 вида, chips/kbd/code/badge). Read-only; значения из
//! активной палитры на рендере. Grid-раскладки CSS → flex-wrap с фикс-шириной
//! ячеек (в gpui нет grid — deviation).

use crate::ui::design::layout::section;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::host_link::ShellEvent;
use crate::ui::design_samples::DesignState;
use smol::channel::Sender;

pub(crate) const MONO: &str = "JetBrains Mono";

/// Тело Design-панели.
pub fn design_panel(
    design: &DesignState,
    design_input: Option<&gpui::Entity<gpui_component::input::InputState>>,
    design_input_focused: bool,
    light: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let (colors, typo, spacing, radius, shadows) =
        crate::ui::design::tokens_page::token_sections(p);
    let components = crate::ui::design::components::components_section(
        design,
        design_input,
        design_input_focused,
        light,
        tx,
        p,
    );

    div()
        .id("design-panel")
        .relative()
        .child(crate::probe::registry::probe_area("design-panel"))
        .flex()
        .flex_col()
        // `.root { gap: space-6; padding-bottom: space-6 }` и НИКАКОГО своего
        // скроллера — страница скроллится телом Customize (ревью ц.23).
        // `size_full` тут НЕЛЬЗЯ: внутри скролл-контейнера он прибивает высоту
        // к вьюпорту, и содержимое ниже становится недостижимым
        .w_full()
        .gap(px(m::SPACE_6))
        .pb(px(m::SPACE_6))
        .child(section(
            "Colors",
            "Theme tokens — resolve from the active dark/light palette.",
            colors.into_any_element(),
            p,
        ))
        .child(section(
            "Typography",
            "Font families + the 5-step size scale.",
            typo.into_any_element(),
            p,
        ))
        .child(section(
            "Spacing",
            "space-1..7 — every gap/padding in the codebase resolves to one of these.",
            spacing.into_any_element(),
            p,
        ))
        .child(section(
            "Radius",
            "4-step concentric scale anchored at 16px outer.",
            radius.into_any_element(),
            p,
        ))
        .child(section(
            "Shadows",
            "Elevation tokens. Lower index = more grounded.",
            shadows.into_any_element(),
            p,
        ))
        .child(section(
            "Components",
            "Live samples — values track the palette above.",
            components.into_any_element(),
            p,
        ))
        .into_any_element()
}
