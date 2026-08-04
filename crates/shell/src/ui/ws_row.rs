//! Строка результата поиска символов.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Хит символа воркспейса.
#[derive(Clone)]
pub struct SymbolHit {
    pub name: String,
    pub kind: u32,
    pub container: Option<String>,
    pub uri: String,
    /// 1-based строка символа (range.startLine+1); None → просто открыть файл.
    pub line: Option<u32>,
}
fn basename(p: &str) -> String {
    let p = p.replace('\\', "/");
    p.rsplit('/').next().unwrap_or(&p).to_string()
}
/// SymbolKind (LSP) → codicon-глиф (WorkspaceSymbols.tsx:14-19 1:1).
fn kind_glyph(kind: u32) -> &'static str {
    match kind {
        1 | 2 => "\u{ea8b}",      // namespace/module
        4 => "\u{eb5b}",          // class
        5 | 8 | 11 => "\u{ea8c}", // method/ctor/function
        6 => "\u{eb65}",          // property
        7 => "\u{eb5f}",          // field
        9 => "\u{ea95}",          // enum
        10 => "\u{eb61}",         // interface
        12 => "\u{ea88}",         // variable
        13 => "\u{eb5d}",         // constant
        22 => "\u{ea91}",         // struct
        23 => "\u{ea86}",         // event
        _ => "\u{eb63}",          // misc
    }
}
pub(crate) fn symbol_row(
    row_index: usize,
    h: &SymbolHit,
    first: bool,
    light: bool,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Тот же `QuickOpen.module.css`: в светлой теме активная строка залита
    // `[data-theme=light] .itemActive` красит и текст: имя — accent-action-fg,
    // путь — 80% от него (`QuickOpen.module.css:66-68,80-82`)
    let (name_color, path_color) = if first && light {
        (
            rgba(p.accent_action_fg),
            tint(rgba(p.accent_action_fg), 0.8),
        )
    } else {
        (rgba(p.text_primary), rgba(p.text_muted))
    };
    let base = if first && light {
        rgba(p.accent_primary)
    } else if first {
        tint(rgba(p.accent_primary), 0.14)
    } else {
        gpui::transparent_black().into()
    };
    let tx = tx.clone();
    let uri = h.uri.clone();
    let line = h.line;
    let path_label = match &h.container {
        Some(c) if !c.is_empty() => format!("{c} · {}", basename(&h.uri)),
        _ => basename(&h.uri),
    };
    // `<li>` без `tabIndex` (`WorkspaceSymbols.tsx:94`) — не таб-стоп
    div()
        .id(SharedString::from(format!("ws-{}-{}", h.uri, h.name)))
        .flex()
        .items_baseline()
        .gap(px(m::SPACE_2))
        .px(px(14.0))
        .py(px(6.0))
        .bg(base)
        .cursor_pointer()
        .on_mouse_move({
            let tx = tx.clone();
            move |_, _, _| {
                // `onMouseEnter` оригинала: подсвечена ровно ОДНА строка —
                // наведение ПЕРЕНОСИТ активную, а не красит вторую поверх
                let _ = tx.try_send(ShellEvent::OverlayRowHover("ws", row_index));
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            // reveal: с диапазоном → прыжок к строке символа
            let _ = tx.try_send(match line {
                Some(l) => ShellEvent::Ed(EdEvent::OpenFileAt(uri.clone(), l)),
                None => ShellEvent::Ed(EdEvent::OpenFile(uri.clone())),
            });
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
            let _ = tx.try_send(ShellEvent::CloseWorkspaceSymbols);
        })
        .child(
            // Цвет наследуется от строки: у активной в СВЕТЛОЙ теме это
            // accent-action-fg (`[data-theme=light] .itemActive`), иначе
            // text-primary (ревью ц.1/23). Класса-кегля у
            // `<span class="codicon codicon-symbol-…">` нет вовсе →
            // вендорная база 16 (ревью ц.14)
            codicon(kind_glyph(h.kind), 16.0)
                .flex_shrink_0()
                .text_color(name_color),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(m::FS_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(name_color)
                .child(h.name.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(m::FS_XS))
                .text_color(path_color)
                .text_right()
                .child(path_label),
        )
        .into_any_element()
}
