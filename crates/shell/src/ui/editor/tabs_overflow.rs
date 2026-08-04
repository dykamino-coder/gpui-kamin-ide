//! Кнопка «N ▾» и поповер скрытых табов редактора.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::ui::editor::tab_name::base_name;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn overflow(
    bar: gpui::Div,
    strip: gpui::Stateful<gpui::Div>,
    fit_all: bool,
    tabs: &[(String, bool, bool)],
    _widths: &[f32],
    active: usize,
    _available_w: f32,
    viewport_h: f32,
    overflow_open: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let mut bar = bar;
    // «N ▾» + deferred-поповер скрытых табов (паттерн overflow чипов)
    bar = bar.child(strip);
    if !fit_all {
        // `.overflowBtn:hover { background: var(--bg-surface-hover) }`
        let hover_bg = rgba(p.bg_surface_hover);
        let mut btn = div()
            .id("ftabs-overflow")
            .relative()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            // `.overflowBtn` — квадрат 24×24 с ОДНИМ chevron (счётчик был наш);
            // `.overflow { padding-right: space-1 }`
            .w(px(24.0))
            .h(px(24.0))
            // `.overflow { padding-right: space-1 }` — сама КНОПКА в потоке
            // стоит на 4 от правого края ряда
            .mr(px(m::SPACE_1))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_secondary))
            .tooltip(crate::ui::tooltip::tooltip("More open files"))
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, {
                let tx = tx.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::ToggleFileTabsOverflow));
                }
            })
            .child(crate::ui::icon::codicon(
                crate::ui::icon::CHEVRON_DOWN,
                16.0,
            ));
        if overflow_open {
            // .overflowMenu: min-w 200 / max-w 360, shadow 0 6 24 30%
            let mut menu = div()
                .id("file-tabs-overflow-menu")
                .occlude()
                .absolute()
                .top(px(26.0))
                // `.overflow { padding-right: space-1 }` — обёртка вокруг И
                // кнопки, И меню, поэтому `right: 0` меню отсчитывается от
                // края РЯДА: сдвигаем на те же 4 (ревью ц.20: меню было
                // на 4 px левее оригинала)
                .right(px(-m::SPACE_1))
                .min_w(px(200.0))
                .max_w(px(360.0))
                .max_h(px((0.6 * viewport_h).max(120.0)))
                .overflow_y_scroll()
                .rounded(px(m::RADIUS_MD))
                .bg(rgba(p.bg_surface))
                .border_1()
                .border_color(tint(rgba(p.text_primary), 0.06))
                // `.overflowMenu` НЕ берёт токен `--shadow-dropdown`, у него
                // своя тень `0 6px 24px rgb(0 0 0 / 30%)` (ревью ц.13)
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::Rgba {
                        r: 0.,
                        g: 0.,
                        b: 0.,
                        a: 0.3,
                    }
                    .into(),
                    offset: gpui::point(px(0.), px(6.)),
                    blur_radius: px(24.),
                    spread_radius: px(0.),
                }])
                .p(px(m::SPACE_1))
                .flex()
                .flex_col();
            // Меню перечисляет ВСЕ открытые файлы (`FileViewerTabs.tsx:214`),
            // а не только скрытые: у оригинала это «список открытых», а не
            // «остаток, не влезший в стрип» (ревью ц.11)
            for (i, (path, dirty, pinned)) in tabs.iter().enumerate() {
                let name = base_name(path);
                // .overflowItem: 5/8, gap 6, text-secondary, hover surface-hover
                let item_hover = rgba(p.bg_surface_hover);
                let tx = tx.clone();
                menu = menu.child(
                    div()
                        .id(SharedString::from(format!("ftov-{i}")))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(m::SPACE_2))
                        .py(px(5.0))
                        .rounded(px(m::RADIUS_SM))
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.text_secondary))
                        .cursor_pointer()
                        // `title={t.path}` — усечённое имя иначе не прочитать
                        .tooltip(crate::ui::tooltip::tooltip(path.clone()))
                        // `.overflowItemActive, .overflowItemActive:hover` —
                        // у активного пункта ховер НЕ перебивает accent-тинт;
                        // у обычного ховер красит и фон, И текст (ревью ц.13)
                        .when(i == active, |d| {
                            d.bg(tint(rgba(p.accent_primary), 0.16))
                                .text_color(rgba(p.text_primary))
                        })
                        .when(i != active, |d| {
                            d.hover(move |st| st.bg(item_hover).text_color(rgba(p.text_primary)))
                        })
                        // `.pinIcon { font-size: 11px; opacity: .7 }`
                        .when(*pinned, |el| {
                            el.child(crate::ui::icon::codicon("\u{eba0}", 16.0).opacity(0.7))
                        })
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::SelectEditorTab(i)));
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::ToggleFileTabsOverflow));
                        })
                        .child(
                            crate::icon_theme::file_img(&name)
                                .flex_shrink_0()
                                .w(px(14.0))
                                .h(px(14.0)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(name.clone()),
                        )
                        // `.dirty` — тот же ГЛИФ ●, что в табе (fs 10), а не
                        // нарисованный круг (ревью ц.13)
                        .when(*dirty, |el| {
                            el.child(
                                div()
                                    .text_size(px(10.0))
                                    .line_height(px(10.0))
                                    .text_color(rgba(p.accent_orange))
                                    .child("\u{25cf}"),
                            )
                        }),
                );
            }
            btn = btn.child(gpui::deferred(menu).with_priority(60));
        }
        bar = bar.child(btn);
    }
    bar.into_any_element()
}
