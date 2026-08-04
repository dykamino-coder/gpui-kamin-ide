//! Logs-панель (LogsPanel 1:1, ядро): список каналов слева, буфер активного
//! справа + тулбар (Clear). (Фильтр-инпут и copy — следующая итерация.)
//! И System-панель: диагностика newest-first (level + source + message).

use crate::host::events::CzEvent;
pub use crate::ui::logs::system::system_panel;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, SharedString, div, px};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::output_log::OutputChannels;

/// Logs: [каналы | буфер].
pub fn logs_panel(
    output: &OutputChannels,
    filter: Option<(&Entity<InputState>, String)>,
    filter_focused: bool,
    // Скролл тела: он же источник флага «пользователь у дна»
    scroll: &gpui::ScrollHandle,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Пустая панель целиком (empty-state оригинала: inbox 32 + подсказка)
    if output.channels.is_empty() {
        return div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(m::SPACE_2))
            .p(px(m::SPACE_5))
            .text_color(rgba(p.text_muted))
            .child(
                // `.empty i` — FontAwesome fa-inbox 32, opacity .6 (не codicon)
                crate::ui::icon::fa("\u{f01c}", 32.0)
                    .w(px(32.0))
                    .h(px(32.0))
                    .opacity(0.6),
            )
            .child(
                // `.empty` кегля не задаёт — наследует базовые 16px документа;
                // `<code>` внутри — моно fs-xs
                div()
                    .flex()
                    .flex_wrap()
                    .items_baseline()
                    .justify_center()
                    .max_w(px(420.0))
                    .text_size(px(16.0))
                    .child("No output channels yet. Extensions register them via ")
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_size(px(m::FS_XS))
                            .child("vscode.window.createOutputChannel(name)"),
                    )
                    .child("."),
            )
            .into_any_element();
    }
    // Левая колонка: каналы (.item: 8×12, border-резерв, hover surface 50%,
    // active accent 14% + accent-текст + бордер 35%)
    // `.list { overflow: auto }` — длинный список каналов скроллится сам
    let mut list = div()
        .id("log-channels")
        .flex()
        .flex_col()
        .w(px(220.0))
        .flex_shrink_0()
        .min_h(px(0.))
        .overflow_y_scrollbar()
        .gap(px(2.0))
        .pr(px(m::SPACE_2));
    for c in &output.channels {
        let is_active = output.active.as_deref() == Some(c.key.as_str());
        let hover_bg = tint(rgba(p.bg_surface), 0.5);
        let tx = tx.clone();
        let key = c.key.clone();
        let name_color = if is_active {
            rgba(p.accent_primary)
        } else {
            rgba(p.text_secondary)
        };
        let group = SharedString::from(format!("och-group-{}", c.key));
        let mut row = div()
            .id(SharedString::from(format!("och-{}", c.key)))
            .group(group.clone())
            // `data-tooltip={`${extensionId} · ${name}`}` (`LogsPanel.tsx:91`)
            // — пункт молча выпал из «Осталось» после ц.14 (ревью ц.26)
            .tooltip(crate::ui::tooltip::tooltip(format!(
                "{} · {}",
                c.extension_id, c.name
            )))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .border_1()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::SelectOutputChannel(key.clone())));
            })
            .child(
                div()
                    .text_size(px(m::FS_SM))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(name_color)
                    // `.item:hover { color: text-primary }` — собственный
                    // цвет имени иначе не поднимается (ревью ц.16)
                    .group_hover(group.clone(), {
                        let tp = rgba(p.text_primary);
                        move |st| st.text_color(tp)
                    })
                    // `.item { align-items: flex-start }` (`:20`) + отсутствие
                    // `nowrap` у имени: длинное имя канала ПЕРЕНОСИТСЯ, а не
                    // режется многоточием (ревью ц.26)
                    .child(c.name.clone()),
            )
            .child(
                div()
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_muted))
                    .font_family("JetBrains Mono")
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(c.extension_id.clone()),
            );
        if is_active {
            row = row
                .bg(tint(rgba(p.accent_primary), 0.14))
                .border_color(tint(rgba(p.accent_primary), 0.35));
        } else {
            // `.item:hover { background: …; color: var(--text-primary) }`
            // (ревью ц.14: правка цикла 13 ушла в системный лог)
            row = row.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)));
        }
        list = list.child(row);
    }

    // Правая колонка: тулбар + буфер (хвост, моноширинный)
    let right =
        crate::ui::logs::channel::channel_body(output, filter, filter_focused, scroll, tx, p);
    div()
        .flex()
        .gap(px(m::SPACE_3))
        .size_full()
        .min_h(px(0.))
        .child(list)
        .child(right)
        .into_any_element()
}
