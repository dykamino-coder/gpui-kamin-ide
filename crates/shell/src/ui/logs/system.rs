//! Панель System: события хоста и расширений.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::output_log::SysEntry;
use crate::ui::icon::codicon;
use crate::ui::logs::parts::matches;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, SharedString, div, px};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// System: newest-first диагностика + Clear.
pub fn system_panel(
    entries: &[SysEntry],
    filter: Option<(&Entity<InputState>, String)>,
    search_focused: bool,
    level: &'static str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let ftext = filter.as_ref().map(|(_, f)| f.clone()).unwrap_or_default();

    let toolbar = crate::ui::logs::system_toolbar::toolbar(filter, search_focused, level, tx, p);

    let mut col = div()
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.))
        .child(toolbar);

    let shown: Vec<&SysEntry> = entries
        .iter()
        .filter(|e| level == "all" || e.level == level)
        // `${e.source} ${e.message}` (`SystemLogPanel.tsx:23`) — УРОВНЯ в стоге
        // НЕТ: со словом «error» в поле оригинал ищет по тексту, а не по
        // уровню; запрос тримится (ревью ц.26)
        .filter(|e| matches(&format!("{} {}", e.source, e.message), ftext.trim()))
        .collect();

    if shown.is_empty() {
        // `.empty` — колонка по центру, gap 8, глиф 24 при opacity .5
        col = col.child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(m::SPACE_2))
                .p(px(m::SPACE_4))
                .text_color(rgba(p.text_muted))
                .child(crate::ui::icon::fa("\u{f01c}", 24.0).opacity(0.5))
                // Тексты дословно (`SystemLogPanel.tsx:57`) — были свои
                // короткие формулировки (ревью ц.16)
                // `entries.length === 0` — «нет логов вообще»; иначе это
                // отсев фильтром (`SystemLogPanel.tsx:57`, ревью ц.19)
                .child(if entries.is_empty() {
                    "No system logs yet — host, extension and renderer diagnostics land here."
                } else {
                    "No logs match the filter."
                }),
        );
        return col.into_any_element();
    }

    // `.list` — mono, fs-xs, свой скролл
    let mut body = div()
        .id("syslog-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scrollbar()
        .font_family(crate::ui::design_panel::MONO)
        .text_size(px(m::FS_XS));

    for (i, e) in shown.iter().rev().enumerate() {
        let (glyph, color) = match e.level {
            "error" => ("\u{ea87}", rgba(p.accent_red)),
            "warning" => ("\u{ea6c}", rgba(p.accent_yellow)),
            _ => ("\u{ea74}", rgba(p.accent_blue)),
        };
        // `.row` — 16px / max-content / 1fr / max-content, baseline, gap 8,
        // padding 3px 8, border-bottom divider-soft 50%, hover 5%
        let hover_bg = tint(rgba(p.text_primary), 0.05);
        body = body.child(
            div()
                .id(SharedString::from(format!("syslog-row-{i}")))
                .flex()
                .items_baseline()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_2))
                .py(px(3.0))
                .border_b_1()
                .border_color(tint(rgba(p.text_primary), 0.03))
                // `.row:hover { background: color-mix(text-primary 5%) }` —
                // ТОЛЬКО фон; цвет текста здесь не меняется (ревью ц.14)
                .hover(move |s| s.bg(hover_bg))
                .child(
                    div()
                        .w(px(16.0))
                        .flex_shrink_0()
                        .flex()
                        .justify_center()
                        // `.icon { align-self: center }` (`:81`): ряд стоит на
                        // baseline, иконка — по центру (ревью ц.26)
                        .map(|d| {
                            // В `Styled` хелпера нет — ставим поле напрямую
                            let mut d = d;
                            d.style().align_self = Some(gpui::AlignSelf::Center);
                            d
                        })
                        // `.icon { font-size: 13px }` (0,1,0) проигрывает базе
                        // `.codicon[class*=codicon-]` (0,2,0) → фактически 16
                        .child(codicon(glyph, 16.0).text_color(color)),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .text_color(rgba(p.text_muted))
                        .child(e.source.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(if e.level == "error" {
                            rgba(p.accent_red)
                        } else {
                            rgba(p.text_primary)
                        })
                        .child(e.message.clone()),
                )
                .child(
                    div()
                        .id(gpui::SharedString::from(format!("syslog-t-{i}")))
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .text_color(rgba(p.text_muted))
                        // `data-tooltip={absoluteTime(...)}` — абсолютное время
                        // подсказкой к относительному (`SystemLogPanel.tsx:66`)
                        .tooltip(crate::ui::tooltip::tooltip(
                            crate::ui::time_fmt::absolute_at(e.at),
                        ))
                        .child(crate::ui::time_fmt::relative_at(e.at)),
                ),
        );
    }
    col = col.child(body);
    col.into_any_element()
}
