//! Секция Components дизайн-панели: кнопки, списки, меню, тосты, плейсхолдеры.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use gpui::Entity;
use gpui_component::input::InputState;

use crate::colors::rgba;
use crate::colors::tint;
use crate::host_link::ShellEvent;
use crate::ui::design::layout::{block, block_hint};
use crate::ui::design::samples_chrome::{
    sample_panel_icons, sample_section_header, sample_status_items,
};
use crate::ui::design::samples_input::{sample_input, sample_list_item};
use crate::ui::design::samples_nav::{sample_icon_column, sample_tab_strip, sample_tree};
use crate::ui::design_panel::MONO;
use crate::ui::design_samples::{self as ds, DesignState};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::{Color, Palette};
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn components_section(
    design: &DesignState,
    design_input: Option<&Entity<InputState>>,
    design_input_focused: bool,
    light: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // ── Components: кнопки — общий рецепт в design_samples::ds_btn
    // .chip / .chipMuted / .chipDanger: pad 1×8, r-xs, тон 14% + рамка 30%
    let chip = |label: &'static str, c: Color| {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(m::SPACE_2))
            .py(px(1.0))
            .rounded(px(m::RADIUS_XS))
            .text_size(px(m::FS_XS))
            .bg(tint(rgba(c), 0.14))
            .text_color(rgba(c))
            .border_1()
            .border_color(tint(rgba(c), 0.3))
            .child(label)
    };
    let inline_bits = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(m::SPACE_2))
        .child(chip("active", p.accent_green))
        // `.chipMuted` — 12% фон / 25% бордер (у остальных 14/30)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(m::SPACE_2))
                .py(px(1.0))
                .rounded(px(m::RADIUS_XS))
                .text_size(px(m::FS_XS))
                .bg(tint(rgba(p.text_muted), 0.12))
                .text_color(rgba(p.text_muted))
                .border_1()
                .border_color(tint(rgba(p.text_muted), 0.25))
                .child("idle"),
        )
        .child(chip("error", p.accent_red))
        .child(
            // .kbd
            div()
                .font_family(MONO)
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_secondary))
                .bg(tint(rgba(p.bg_overlay), 0.5))
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(m::RADIUS_XS))
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.7))
                .child("Ctrl+Shift+P"),
        )
        .child(
            // .codeInline
            div()
                .font_family(MONO)
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.accent_primary))
                .bg(tint(rgba(p.accent_primary), 0.10))
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(m::RADIUS_XS))
                .child("npm run check"),
        )
        .child(
            // .badge: min-w 18 h 18 r9 accent-red / bg-primary 600
            div()
                .min_w(px(18.0))
                .h(px(18.0))
                .px(px(6.0))
                .rounded(px(9.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(m::FS_XS))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .bg(rgba(p.accent_red))
                .text_color(rgba(p.bg_primary))
                .child("3"),
        );
    // `.compStack` — gap space-4 между блоками семплов.
    // Порядок блоков — ровно как в оригинале: ComponentSamples()
    // (Buttons…Tooltip) затем ExtraSamples() (Tabs…Placeholders).
    let components = div()
            .flex()
            .flex_col()
            .gap(px(m::SPACE_4))
            .child(block("Buttons", ds::sample_buttons(p), p))
            .child(block(
                "List item — active selection (sidebar pattern)",
                sample_list_item(light, p),
                p,
            ))
            .child(block("Input", sample_input(design_input, design_input_focused, p), p))
            .child(block(
                "Dropdown menu",
                ds::sample_dropdown(design.dropdown_open, &design.picked, light, tx, p),
                p,
            ))
            .child(block(
                "Tree (file-explorer pattern)",
                sample_tree(design, tx, p),
                p,
            ))
            .child(block(
                "Chips · Kbd · Code · Badge",
                inline_bits.into_any_element(),
                p,
            ))
            .child(block("In-app toasts", ds::sample_toast_triggers(tx, p), p))
            .child(block("Modals", ds::sample_modal_triggers(tx, p), p))
            .child(block_hint(
                "External toasts (out-of-app)",
                Some("Standalone BrowserWindows — auto-fire when KaminIDE is unfocused. Bottom timer bar shrinks over 8 s; hover pauses both bar and dismiss timer. Buttons below force one regardless of focus."),
                ds::sample_external_toast_triggers(tx, p),
                p,
            ))
            .child(block("Tooltip", ds::sample_tooltip(p), p))
            .child(block_hint(
                "Horizontal tab strip",
                Some("BottomTabBar / FileViewerTabs recipe — pill tabs, accent-tinted active state."),
                sample_tab_strip(design, tx, p),
                p,
            ))
            .child(block_hint(
                "Vertical icon column",
                Some("ActivityBar recipe — square icon tiles + picker dot at the end."),
                sample_icon_column(design, tx, p),
                p,
            ))
            .child(block_hint(
                "Checkbox dropdown",
                Some("LayoutToggles recipe — clicks toggle items WITHOUT closing the menu (only outside-click / Esc dismiss)."),
                ds::sample_checkbox_dropdown(design.checks, tx, p),
                p,
            ))
            .child(block_hint(
                "Context menu",
                Some("ActivityContextMenu recipe — right-click in the live UI; here a static preview of the same surface."),
                ds::sample_context_menu(p),
                p,
            ))
            .child(block_hint(
                "Section header",
                Some("Sidebar landmark — uppercase, muted, 0.08em letter-spacing."),
                sample_section_header(p),
                p,
            ))
            .child(block("Status-bar items", sample_status_items(p), p))
            .child(block_hint(
                "Panel icon family",
                Some("Same SVG family used by LayoutToggles + PanelPlaceholder — frame + highlighted slot."),
                sample_panel_icons(p),
                p,
            ))
            .child(block_hint(
                "Empty / active panel placeholders",
                Some("ActivityPlaceholder is shown once a tool is picked but its renderer isn't ready yet (Phase A)."),
                ds::sample_placeholders(p),
                p,
            ));
    components.into_any_element()
}
