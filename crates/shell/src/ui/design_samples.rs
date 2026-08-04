//! Живые семплы Design-панели, вынесенные из `design_panel.rs`
//! (оригинал так же разделён: `component-samples.tsx` +
//! `component-samples-extra.tsx`).
//!
//! Здесь блоки, которым нужны состояние или обработчики: дропдаун,
//! чекбокс-меню, триггеры тостов/модалок/внешних тостов, тултип,
//! контекст-меню, плейсхолдеры.

pub use crate::ui::ds::buttons::{DsBtn, ds_btn, sample_buttons};
pub use crate::ui::ds::dropdowns::{sample_checkbox_dropdown, sample_dropdown};
pub use crate::ui::ds::state::{DesignAction, DesignState};
pub use crate::ui::ds::triggers::{
    sample_external_toast_triggers, sample_modal_triggers, sample_toast_triggers,
};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::ui::icon::codicon;

// codicon-глифы семплов
pub(crate) const COLOR_MODE: &str = "\u{eac6}";
pub(crate) const CHEVRON_DOWN: &str = "\u{eab4}";
pub(crate) const CHEVRON_RIGHT: &str = "\u{eab6}";
pub(crate) const LIGHTBULB: &str = "\u{ea61}";
pub(crate) const DEVICE_DESKTOP: &str = "\u{ea7a}";
pub(crate) const CHECK: &str = "\u{eab2}";
pub(crate) const EYE_CLOSED: &str = "\u{eae7}";
pub(crate) const ARROW_RIGHT: &str = "\u{ea9c}";

impl Default for DesignState {
    fn default() -> Self {
        Self {
            dropdown_open: false,
            picked: "dark".into(),
            checks: [true, false, true],
            tree_expanded: ["src".to_string(), "src/host".to_string()]
                .into_iter()
                .collect(),
            tree_selected: "src/host/index.ts".into(),
            strip_tab: "terminal".into(),
            column_tile: "folders".into(),
        }
    }
}

/// `TooltipDemo` — одна `.btnGhost` с тултипом по ховеру.
pub fn sample_tooltip(p: &Palette) -> AnyElement {
    ds_btn(DsBtn::Ghost, "ds-tooltip-demo", "Hover me", p)
        .tooltip(crate::ui::tooltip::tooltip(
            "This is a tooltip — hover for the full text. data-tooltip is set on the element, document-level listener does the rest.",
        ))
        .into_any_element()
}

/// `ContextMenuRow` — статичное превью поверхности ActivityContextMenu:
/// min-w 180, bg-surface, divider-soft, r-md, без тени; Hide + Move to ▸.
pub fn sample_context_menu(p: &Palette) -> AnyElement {
    let item = |id: &'static str, glyph: &'static str, label: &'static str, chevron: bool| {
        let hover_bg = tint(rgba(p.text_primary), 0.1);
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .w_full()
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_primary))
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            // `.item` кегль кодикона не задаёт → база 16px
            .child(codicon(glyph, 16.0))
            .child(div().flex_1().child(label))
            .when(chevron, |d| {
                // `.chevron{font-size:12px}` стоит на самом `.codicon`
                // → (0,1,0) проигрывает вендорной базе (0,2,0): 16
                d.child(codicon(CHEVRON_RIGHT, 16.0).text_color(rgba(p.text_muted)))
            })
    };
    div()
        .min_w(px(180.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .child(item("ds-ctx-hide", EYE_CLOSED, "Hide", false))
        .child(item("ds-ctx-move", ARROW_RIGHT, "Move to", true))
        .into_any_element()
}

/// `PlaceholdersRow` — карточка-обёртка max-w 280 / min-h 160 / r-md /
/// bg-mantle вокруг `ActivityPlaceholder("terminal", "Terminal")`.
pub fn sample_placeholders(p: &Palette) -> AnyElement {
    div()
        .w_full()
        .max_w(px(280.0))
        .min_h(px(160.0))
        .flex()
        .flex_col()
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_mantle))
        .child(crate::ui::panel_placeholder::activity_placeholder(
            "terminal", "Terminal", p,
        ))
        .into_any_element()
}
