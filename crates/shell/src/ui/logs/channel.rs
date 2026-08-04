//! Правая колонка панели Logs: содержимое выбранного канала.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::output_log::OutputChannels;
use crate::ui::logs::parts::{filter_input, matches, tool_btn};
use gpui::prelude::*;
use gpui::{Entity, SharedString, div, px};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn channel_body(
    output: &OutputChannels,
    filter: Option<(&Entity<InputState>, String)>,
    filter_focused: bool,
    scroll: &gpui::ScrollHandle,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Div {
    let active = output.active_channel();
    let mut right = div().flex().flex_col().flex_1().min_w(px(0.)).min_h(px(0.));
    if let Some(c) = active {
        let tx_clear = tx.clone();
        let key = c.key.clone();
        let has_buffer = !c.buffer.is_empty();
        let copy_text = c.buffer.clone();
        right = right.child(
            // .toolbar: search flex-1 + Copy + Clear (26×26)
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .flex_shrink_0()
                .pb(px(m::SPACE_2))
                .when_some(filter.as_ref(), |d, (inp, _)| {
                    d.child(filter_input(inp, filter_focused, p))
                })
                .child(tool_btn(
                    "log-copy",
                    "\u{ebcc}", // codicon-copy
                    "Copy entire buffer",
                    has_buffer,
                    p,
                    move |cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_text.clone()));
                    },
                ))
                .child(tool_btn(
                    "log-clear",
                    "\u{eabf}", // codicon-clear-all
                    "Clear channel",
                    has_buffer,
                    p,
                    move |_| {
                        let _ = tx_clear
                            .try_send(ShellEvent::Cz(CzEvent::ClearOutputChannel(key.clone())));
                    },
                )),
        );
        // .body: bg-base + border bg-surface, mono fs-xs, pre-wrap
        // (хвост 400 строк — рендер безопасен)
        let mut body = div()
            .id("log-buffer")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scrollbar_with(scroll)
            .p(px(m::SPACE_3))
            .rounded(px(m::RADIUS_SM))
            .bg(rgba(p.bg_base))
            .border_1()
            .border_color(rgba(p.bg_surface))
            .font_family("JetBrains Mono")
            .text_size(px(m::FS_XS))
            .line_height(px(m::FS_XS * 1.3))
            .text_color(rgba(p.text_primary));
        let ftext = filter.as_ref().map(|(_, f)| f.as_str()).unwrap_or("");
        let lines: Vec<&str> = c.buffer.lines().filter(|l| matches(l, ftext)).collect();
        let start = lines.len().saturating_sub(400);
        // Пустой буфер оригинал НЕ подписывает — `<pre>` просто пуст
        // (`LogsPanel.tsx:96`); подставной текст был отсебятиной (ревью ц.19)
        for line in &lines[start..] {
            // `white-space: pre-wrap; word-break: break-word`
            // (`LogsPanel.module.css:109-110`): длинная строка ПЕРЕНОСИТСЯ, а
            // не уезжает в горизонтальный скролл. По умолчанию gpui не
            // переносит — включаем явно (ревью ц.26)
            body = body.child(
                div()
                    .w_full()
                    .child(SharedString::from((*line).to_string())),
            );
        }
        // `stickToBottom` (`LogsPanel.tsx:51-70`): пока пользователь у дна
        // (зазор ≤ 6 px), прирост буфера тянет вьюпорт вниз
        const SCROLL_BOTTOM_SLACK_PX: f32 = 6.0;
        let off = f32::from(scroll.offset().y);
        let max = f32::from(scroll.max_offset().height);
        if max - (-off) <= SCROLL_BOTTOM_SLACK_PX {
            scroll.scroll_to_bottom();
        }
        right = right.child(body);
    } else {
        // Оригинал (`LogsPanel.tsx:98-134`) при отсутствии активного канала
        // рисует ТОТ ЖЕ тулбар с disabled-кнопками и ПУСТОЙ `<pre>`, а не
        // подпись по центру — «Select a channel» было отсебятиной
        // (ревью ц.26)
        right = right.child(
            div()
                .flex_1()
                .min_h(px(0.))
                .p(px(m::SPACE_3))
                .rounded(px(m::RADIUS_SM))
                .bg(rgba(p.bg_base))
                .border_1()
                .border_color(rgba(p.bg_surface))
                .font_family(crate::ui::design_panel::MONO)
                .text_size(px(m::FS_XS))
                .line_height(px(m::FS_XS * 1.3)),
        );
    }

    right
}
