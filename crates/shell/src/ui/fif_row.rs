//! Строка результата текстового поиска.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Хит текстового поиска (одна строка совпадения).
#[derive(Clone)]
pub struct TextHit {
    pub rel: String,
    pub abs: String,
    pub line: u32,
    pub match_start: usize,
    pub match_end: usize,
    pub snippet: String,
}
/// Безопасно нарезать сниппет на (до, совпадение, после) по байт-оффсетам.
fn split3(s: &str, a: usize, b: usize) -> (String, String, String) {
    let a = a.min(s.len());
    let b = b.min(s.len());
    if a > b || !s.is_char_boundary(a) || !s.is_char_boundary(b) {
        return (s.to_string(), String::new(), String::new());
    }
    (s[..a].to_string(), s[a..b].to_string(), s[b..].to_string())
}
pub(crate) fn hit_row(
    row_index: usize,
    h: &TextHit,
    first: bool,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let base = if first {
        tint(rgba(p.accent_primary), 0.14)
    } else {
        gpui::transparent_black().into()
    };
    // Ховер = активная строка (оригинал двигает active по `onMouseEnter`)
    let (before, mtch, after) = split3(&h.snippet, h.match_start, h.match_end);
    let tx = tx.clone();
    let abs = h.abs.clone();
    let line = h.line;
    // Строка — `<li>` БЕЗ `tabIndex` (`FindInFiles.tsx:114`): не таб-стоп и
    // кольца `:focus-visible` не получает. Кольцо тут было бы нашей выдумкой
    div()
        .id(SharedString::from(format!("fif-{}-{}", h.abs, h.line)))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .px(px(14.0))
        .py(px(6.0))
        .rounded(px(m::RADIUS_XS))
        .bg(base)
        .cursor_pointer()
        .on_mouse_move({
            let tx = tx.clone();
            move |_, _, _| {
                // `onMouseEnter` оригинала: подсвечена ровно ОДНА строка —
                // наведение ПЕРЕНОСИТ активную, а не красит вторую поверх
                let _ = tx.try_send(ShellEvent::OverlayRowHover("fif", row_index));
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::OpenFileAt(abs.clone(), line)));
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
            let _ = tx.try_send(ShellEvent::CloseFindInFiles);
        })
        // header: rel + :line
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(4.0))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(
                    div()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(h.rel.clone()),
                )
                .child(div().flex_shrink_0().child(format!(":{}", h.line))),
        )
        // snippet (mono) с подсветкой совпадения
        .child(
            div()
                .flex()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family("JetBrains Mono")
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_secondary))
                // `.itemSnippet` (`FindInFiles.module.css:78-85`) — ОДНА
                // строка `nowrap/overflow/ellipsis` с тремя инлайн-детьми.
                // `flex_shrink_0` у префикса и совпадения выталкивал `<mark>`
                // за `overflow_hidden`: у оригинала эллипсис режет ХВОСТ, и
                // совпадение остаётся видно (ревью ц.26). `items_baseline` —
                // фон `<mark>` по line-box, а не по высоте ряда
                .items_baseline()
                .child(
                    div()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(before),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .rounded(px(2.0))
                        .bg(tint(rgba(p.accent_orange), 0.35))
                        .text_color(rgba(p.text_primary))
                        .child(mtch),
                )
                .child(
                    div()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(after),
                ),
        )
        .into_any_element()
}
