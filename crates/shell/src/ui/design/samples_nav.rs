//! Семплы: стрип табов, колонка иконок, дерево.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
pub(crate) use crate::ui::design::samples_tree::sample_tree;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Горизонтальный tab-стрип — рецепт `BottomTabBar`: `.strip` gap space-1 +
/// padding 4/8, `.tab` h24 px10 gap6 r-sm fs11/500 text-secondary,
/// active = accent 16% + text-primary.
pub(crate) fn sample_tab_strip(
    st: &crate::ui::design_samples::DesignState,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Токены тулов, а не готовые глифы: `ToolIcon` отдаёт Phosphor-SVG для
    // `terminal`/`warning` и кодикон для `output` (ревью ц.15)
    // `useState("terminal")` оригинала (`component-samples-extra.tsx:45`) —
    // активный таб живёт в состоянии и переключается кликом (ревью ц.23)
    let tabs: [(&'static str, &'static str); 3] = [
        ("terminal", "Terminal"),
        ("warning", "Problems"),
        ("output", "Output"),
    ];
    // `.strip` — внешняя полоса, `.tabs` — ВНУТРЕННИЙ слой со скроллом
    // (`BottomTabBar.module.css:14-21`): flex 1, min-width 0, overflow-x auto
    let mut tabs_layer = div()
        .id("smp-tabs")
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .flex_1()
        .min_w(px(0.))
        .overflow_x_scroll()
        .relative()
        // Досье 146 — СЕМПЛ стрипа, а не живой стрип панели (досье 42)
        .child(crate::probe::registry::probe_area("sample-tab-strip"));
    for (icon, label) in tabs {
        let active = st.strip_tab == icon;
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        // `TAB_ICON_SIZE_PX = 13` (`BottomTabBar.tsx:24`)
        let glyph_color = if active {
            rgba(p.text_primary)
        } else {
            rgba(p.text_secondary)
        };
        let tab_group = SharedString::from(format!("smp-tabg-{label}"));
        let mut t = div()
            .id(SharedString::from(format!("smp-tab-{label}")))
            .group(tab_group.clone())
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(24.0))
            .px(px(10.0))
            .rounded(px(m::RADIUS_SM))
            // `.tab { white-space: nowrap }` (`BottomTabBar.module.css:37`)
            .whitespace_nowrap()
            .text_size(px(11.0))
            .letter_spacing(px(11.0 * 0.02))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, {
                let tx = tx.clone();
                let id = icon.to_string();
                move |_, _, _| {
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::DesignSample(
                        crate::ui::design_samples::DesignAction::PickStripTab(id.clone()),
                    )));
                }
            })
            // `.tab:hover { color: text-primary }` через currentColor красит
            // и глиф — прибитый аргументом цвет ховер не видел (ревью ц.25)
            .child(crate::ui::activity_bar::tool_glyph_group_hover(
                icon,
                13.0,
                13.0,
                glyph_color,
                tab_group.clone(),
                rgba(p.text_primary),
            ))
            // `.tabLabel { overflow: hidden; text-overflow: ellipsis }`
            // (`BottomTabBar.module.css:56-60`) — ц.23 объявила это закрытым,
            // но код с эллипсисом лежал в ДРУГОМ семпле (ревью ц.25)
            .child(
                div()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(label),
            );
        if active {
            t = t
                .bg(tint(rgba(p.accent_primary), 0.16))
                .text_color(rgba(p.text_primary));
        } else {
            // `.tab:hover` — bg-surface 50 % + text-primary; ховера у семпла
            // не было вовсе (ревью ц.13)
            t = t.hover(move |st| st.bg(hover_bg).text_color(rgba(p.text_primary)));
        }
        // Табы оригинала — `<button aria-pressed>`, значит таб-стопы с
        // `button:focus-visible` (`theme/global.css:38-43`), ревью ц.26
        tabs_layer = tabs_layer.child(crate::ui::focus_ring::focusable(
            t,
            &format!("smp-tab:{label}"),
            m::RADIUS_SM,
            rgba(p.accent_primary),
        ));
    }
    // `.strip`: flex, items-center, gap space-1, flex-shrink 0, padding 4/8,
    // radius-sm — `gap` и `flex-shrink` стояли только на внутреннем `.tabs`
    // (ревью ц.35)
    div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .flex_shrink_0()
        .w_full()
        .max_w(px(360.0))
        .px(px(m::SPACE_2))
        .py(px(4.0))
        .rounded(px(m::RADIUS_SM))
        .child(tabs_layer)
        .into_any_element()
}
/// Вертикальная колонка иконок — рецепт `ActivityBar`: бар 48 + py space-3,
/// `.list` gap 2, плитка 32×32 r-sm, active accent 16% + text-primary,
/// «…»-пикер в конце через gap space-2.
pub(crate) fn sample_icon_column(
    st: &crate::ui::design_samples::DesignState,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // `ToolIcon` по умолчанию 18; токены из `component-samples-extra.tsx:75-78`
    // (были подменены кодиконами folder/file/search, ревью ц.15)
    // `useState("projects")` оригинала (`component-samples-extra.tsx:74`)
    let tiles: [(&'static str, &'static str); 3] = [
        ("folders", "Projects"),
        ("tree-view", "Folder tree"),
        ("search", "Search"),
    ];
    let mut list = div().flex().flex_col().items_center().gap(px(2.0));
    for (i, (icon, tip)) in tiles.into_iter().enumerate() {
        let active = st.column_tile == icon;
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        let glyph_color = if active {
            rgba(p.text_primary)
        } else {
            rgba(p.text_muted)
        };
        let mut b = div()
            .id(SharedString::from(format!("smp-tile-{i}")))
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(m::RADIUS_SM))
            .text_color(rgba(p.text_muted))
            .tooltip(crate::ui::tooltip::tooltip(tip))
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, {
                let tx = tx.clone();
                let id = icon.to_string();
                move |_, _, _| {
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::DesignSample(
                        crate::ui::design_samples::DesignAction::PickColumnTile(id.clone()),
                    )));
                }
            })
            .child(crate::ui::activity_bar::tool_glyph_split(
                icon,
                18.0,
                18.0,
                glyph_color,
            ));
        if active {
            b = b
                .bg(tint(rgba(p.accent_primary), 0.16))
                .text_color(rgba(p.text_primary));
        } else {
            // `.btn:hover` — bg-surface 50 % + text-primary (ревью ц.13)
            b = b.hover(move |st| st.bg(hover_bg).text_color(rgba(p.text_primary)));
        }
        list = list.child(b);
    }
    div()
        .w(px(m::ACTIVITY_BAR_WIDTH))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(m::SPACE_2))
        .py(px(m::SPACE_3))
        .child(list)
        .child({
            let hover_bg = tint(rgba(p.bg_surface), 0.5);
            div()
                .id("smp-picker")
                .w(px(32.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(m::RADIUS_SM))
                .text_color(rgba(p.text_muted))
                // `.picker:hover` — тот же рецепт, что у плиток
                .hover(move |st| st.bg(hover_bg).text_color(rgba(p.text_primary)))
                .tooltip(crate::ui::tooltip::tooltip("Add or remove items"))
                .child(crate::ui::icon::codicon("\u{ea7c}", 18.0))
        })
        .into_any_element()
}
