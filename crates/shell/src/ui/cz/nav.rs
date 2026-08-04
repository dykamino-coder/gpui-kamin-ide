//! Навигация Customize: список панелей и иконки.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::codicon_map::codicon_by_name;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Пункты навигации (CustomizeMode.tsx PANELS).
pub const PANELS: [(&str, &str, &str); 5] = [
    ("settings", "settings-gear", "Settings"),
    ("design", "symbol-color", "Design"),
    ("extensions", "extensions", "Extensions"),
    ("logs", "output", "Logs"),
    ("system", "pulse", "System"),
];
/// `NavIcon` оригинала: путь/URL → картинка 16×16, иначе codicon 14.
pub fn nav_icon(icon: &str) -> AnyElement {
    let is_img = icon.starts_with("data:")
        || icon.starts_with("http:")
        || icon.starts_with("https:")
        || icon.starts_with("file:")
        || icon.starts_with('/');
    if is_img {
        return gpui::img(icon.to_string())
            .w(px(16.0))
            .h(px(16.0))
            .flex_shrink_0()
            .into_any_element();
    }
    // Неизвестное имя: оригинал ставит класс `codicon-<name>`, глифа нет —
    // рисуем пустой бокс той же ширины, а не подменяем шестерёнкой
    match codicon_by_name(icon) {
        Some(g) => codicon(g, 14.0).flex_shrink_0().into_any_element(),
        None => div().w(px(14.0)).flex_shrink_0().into_any_element(),
    }
}
/// Сайдбар-навигация Customize.
pub fn customize_nav(
    active: &str,
    contrib: &[crate::host_link::CzContainer],
    active_contrib: Option<&str>,
    collapsed: &std::collections::HashSet<String>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Оригинал CustomizeMode.module.css: `.root` = padding space-3 0 + gap
    // space-2; `.header` = padding 8px 12px; `.list` = padding 0 space-2, gap 2
    let col = div()
        .relative()
        .child(crate::probe::registry::probe_area("customize-nav"))
        .flex()
        .flex_col()
        .size_full()
        .gap(px(m::SPACE_2))
        .py(px(m::SPACE_3))
        .child(
            div()
                .flex()
                .items_center()
                .px(px(12.0))
                .py(px(m::SPACE_2))
                .text_size(px(m::FS_XS))
                .letter_spacing(px(m::FS_XS * 0.08))
                .font(crate::ui::typo::ss01(gpui::FontWeight::MEDIUM))
                .text_color(rgba(p.text_muted))
                .child("CUSTOMIZE"),
        );
    let mut list = div().flex().flex_col().gap(px(2.0)).px(px(m::SPACE_2));
    for (id, icon, label) in PANELS {
        let is_active = active == id;
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        let tx = tx.clone();
        let mut row = div()
            .id(SharedString::from(format!("cz-{id}")))
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_MD))
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            // `.active, .active:hover` держат accent-тинт: ховер вешаем ТОЛЬКО
            // на неактивную строку, иначе под курсором тинт пропадал
            // (ревью ц.13)
            .when(!is_active, |d| {
                d.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::SetCustomizePanel(id)));
            })
            // `.item :global(.codicon)` задаёт только размер 14 — цвет
            // наследуется от строки (ховер/актив красят и иконку)
            .child(codicon(codicon_by_name(icon).unwrap_or("\u{eb51}"), 14.0))
            .child(label);
        if is_active {
            row = row
                .bg(tint(rgba(p.accent_primary), 0.16))
                .text_color(rgba(p.text_primary));
        }
        list = list.child(row);
    }
    // Contributed-страницы (viewsContainers location=customize) — как
    // CustomizeMode оригинала: РАСКРЫВАЕМЫЙ узел на КАЖДЫЙ контейнер
    // (chevron + иконка и титул контейнера из реестра), дети с отступом
    for c in contrib {
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        let tx_grp = tx.clone();
        // Оригинал CustomizeMode.tsx: родитель подсвечен, когда открыта его
        // дочерняя страница (childActive)
        let is_collapsed = collapsed.contains(&c.id);
        let grp_id = c.id.clone();
        let child_active = active == "contrib"
            && active_contrib.is_some_and(|a| c.views.iter().any(|v| v.id == a));
        // Клик по родителю: тоггл + если ни одна его страница не открыта —
        // открыть первую (`views[0]`).
        let open_first = if child_active {
            None
        } else {
            c.views.first().map(|v| v.id.clone())
        };
        let mut group_row = div()
            .id(SharedString::from(format!("cz-contrib-{}", c.id)))
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_MD))
            // `.item { color: var(--text-secondary) }` — не primary
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            .when(!child_active, |d| {
                d.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx_grp.try_send(ShellEvent::Cz(CzEvent::ToggleCustomizeContribGroup(
                    grp_id.clone(),
                )));
                if let Some(id) = open_first.clone() {
                    let _ = tx_grp.try_send(ShellEvent::Cz(CzEvent::SetCustomizeContribPage(id)));
                }
            })
            .child(
                // `.item :global(.codicon){font-size:14px!important}` перебивает
                // `.chevron{12px!important}` по специфичности (ревью ц.7).
                // Поворота на 90° в gpui нет — подменяем глиф.
                codicon(if is_collapsed { "\u{eab6}" } else { "\u{eab4}" }, 14.0)
                    .flex_shrink_0()
                    .text_color(rgba(p.text_muted)),
            )
            .child(nav_icon(&c.icon))
            .child(SharedString::from(c.title.clone()));
        if child_active {
            group_row = group_row
                .bg(tint(rgba(p.accent_primary), 0.16))
                .text_color(rgba(p.text_primary));
        }
        list = list.child(group_row);
        if is_collapsed {
            continue;
        }
        for v in &c.views {
            let is_active = active == "contrib" && active_contrib == Some(v.id.as_str());
            let hover_bg = tint(rgba(p.bg_surface), 0.5);
            let tx = tx.clone();
            let ev_id = v.id.clone();
            let mut row = div()
                .id(SharedString::from(format!("czc-{}", v.id)))
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                // `.child` = padding-left calc(space-3 + 18px)
                .pl(px(m::SPACE_3 + 18.0))
                .pr(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_MD))
                .text_color(rgba(p.text_secondary))
                .cursor_pointer()
                .when(!is_active, |d| {
                    d.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::SetCustomizeContribPage(
                        ev_id.clone(),
                    )));
                })
                .child(nav_icon(&v.icon))
                .child(SharedString::from(v.name.clone()));
            if is_active {
                row = row
                    .bg(tint(rgba(p.accent_primary), 0.16))
                    .text_color(rgba(p.text_primary));
            }
            list = list.child(row);
        }
    }
    col.child(list).into_any_element()
}
