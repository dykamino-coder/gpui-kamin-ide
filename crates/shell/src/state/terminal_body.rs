//! Тело тула «Терминал»: тулбар профилей, грид xterm-подобного вывода,
//! ресайз по фактическим размерам области и колесо прокрутки.
//!
//! Вынесено из `state/tools.rs` без изменения поведения.

use gpui::prelude::*;
use gpui::{AnyElement, Context, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;
use crate::root::{RootView, TERM_CELL_H, TERM_CELL_W};

impl RootView {
    /// Тело активного терминала слота (см. модульный комментарий).
    /// `None` — тула нет: ранний выход внутри тела так и остался ранним.
    pub(crate) fn terminal_tool_body(
        &mut self,
        p: &'static Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // Ресайз по фактическим размерам грид-области (bounds прошлого
        // кадра из probe_registry; шрифт моно 13px → ячейка 7.8×17)
        if let (Some(term), Some([_, _, w, h])) = (
            self.term.terminals.get_mut(self.term.term_active),
            crate::probe::registry::bounds_of("terminal"),
        ) {
            let cols = (w / TERM_CELL_W).floor() as u16;
            let rows = (h / TERM_CELL_H).floor() as u16;
            term.resize(cols, rows);
        }
        // Локальный PTY-терминал (создаётся лениво при первом показе)
        let Some(t) = self.term.terminals.get(self.term.term_active) else {
            // Карта и тулбар рисуются ВСЕГДА (`TerminalView.tsx:55-72`);
            // `.empty` — codicon-terminal 28 op .6 + подпись fs-sm —
            // лежит внутри `.body`, а не вместо всей панели
            let toolbar = crate::ui::term_toolbar::term_toolbar(
                &self.term.terminals,
                self.term.term_active,
                self.term.term_menu_open,
                self.term.term_default_shell.as_deref(),
                &self.term.term_tab_scroll,
                crate::probe::registry::bounds_of("terminal")
                    .map(|[_, _, w, _]| w)
                    .unwrap_or(280.0),
                self.main_viewport.0,
                self.main_viewport.1,
                p,
                &self.tx,
            );
            return Some(
                // `.root { margin: 0 6 6 }` — отступы даёт ОБЁРТКА
                // (padding), иначе `size_full` + margin выносит карту
                // за правый край панели
                div()
                    .size_full()
                    .min_h(px(0.))
                    .px(px(6.0))
                    .pb(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .bg(rgba(p.bg_mantle))
                            .rounded(px(m::RADIUS_MD))
                            .child(toolbar)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(m::SPACE_2))
                                    .bg(rgba(p.editor_bg))
                                    .rounded(px(m::RADIUS_MD))
                                    .text_color(rgba(p.text_muted))
                                    .child(crate::ui::icon::codicon("\u{ea85}", 28.0).opacity(0.6))
                                    .child(div().text_size(px(m::FS_SM)).child(
                                        "No terminal yet — pick a shell from the “+” menu.",
                                    )),
                            ),
                    )
                    .into_any_element(),
            );
        };
        let grid_el = crate::state::term_grid::term_grid(t, p, cx);
        let panel_w = crate::probe::registry::bounds_of("terminal")
            .map(|[_, _, w, _]| w)
            .unwrap_or(280.0);
        let toolbar = crate::ui::term_toolbar::term_toolbar(
            &self.term.terminals,
            self.term.term_active,
            self.term.term_menu_open,
            self.term.term_default_shell.as_deref(),
            &self.term.term_tab_scroll,
            panel_w,
            self.main_viewport.0,
            self.main_viewport.1,
            p,
            &self.tx,
        );
        Some(
            div()
                // `.root { margin: 0 6 6 }` — отступы даёт обёртка через
                // padding: `size_full` + margin выносил карту за правый
                // край панели (замер: +31.2 px)
                .size_full()
                .min_h(px(0.))
                .px(px(6.0))
                .pb(px(6.0))
                .child(
                    div()
                        .id("terminal-body")
                        .track_focus(&self.terminal_focus)
                        .key_context("Terminal")
                        .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
                            let Some(term) = this.term.terminals.get_mut(this.term.term_active)
                            else {
                                return;
                            };
                            // Ctrl+C с выделением — копия+сброс, НЕ SIGINT (xterm)
                            if ev.keystroke.modifiers.control
                                && ev.keystroke.key == "c"
                                && !ev.keystroke.modifiers.shift
                                && term.has_selection()
                            {
                                if let Some(text) = term.selection_text() {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                                }
                                term.selection_clear();
                                cx.stop_propagation();
                                cx.notify();
                                return;
                            }
                            // Ctrl+V — вставка клипборда (мультистрока: CRLF → CR)
                            if ev.keystroke.modifiers.control && ev.keystroke.key == "v" {
                                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text())
                                {
                                    term.write(text.replace("\r\n", "\r").as_bytes());
                                }
                                cx.stop_propagation();
                            } else if let Some(bytes) = crate::term::keystroke_bytes(&ev.keystroke)
                            {
                                term.write(&bytes);
                                cx.stop_propagation();
                            }
                            cx.notify();
                        }))
                        // Внутренний drag из дерева → путь в PTY
                        .on_drop(cx.listener(
                            |this, f: &crate::ui::file_list::DraggedFile, _, cx| {
                                if let Some(term) =
                                    this.term.terminals.get_mut(this.term.term_active)
                                {
                                    // Мультивыбор: все пути через пробел
                                    // (`dragPaths`, ревью ц.24)
                                    let line = f
                                        .paths
                                        .iter()
                                        .map(|p| {
                                            let s = p.replace('/', "\\");
                                            if s.contains(' ') {
                                                format!("\"{s}\"")
                                            } else {
                                                s
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    term.write(line.as_bytes());
                                    cx.notify();
                                }
                            },
                        ))
                        // Drop файла → путь в PTY (пробел в пути → кавычки)
                        .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
                            if let Some(term) = this.term.terminals.get_mut(this.term.term_active) {
                                let joined = paths
                                    .paths()
                                    .iter()
                                    .map(|p| {
                                        let s = p.display().to_string();
                                        if s.contains(' ') {
                                            format!("\"{s}\"")
                                        } else {
                                            s
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                term.write(joined.as_bytes());
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let fh = self.terminal_focus.clone();
                            move |_, window, _| window.focus(&fh)
                        })
                        .flex()
                        .flex_col()
                        .size_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        // .root оригинала: mantle-карта, margin 0 6 6, radius-md —
                        // отступы даёт обёртка ниже (padding), иначе карта выезжает
                        // за правый край панели
                        .bg(rgba(p.bg_mantle))
                        .rounded(px(m::RADIUS_MD))
                        .font_family("JetBrains Mono")
                        .text_size(px(13.0))
                        .line_height(px(TERM_CELL_H))
                        .text_color(rgba(p.editor_fg))
                        .child(toolbar)
                        .child(
                            // .body: editor-bg поверхность (общая с активным
                            // табом тулбара); .session инсеты 8/22/10/14
                            div()
                                .flex_1()
                                .min_h(px(0.))
                                .relative()
                                .bg(rgba(p.editor_bg))
                                .rounded(px(m::RADIUS_MD))
                                .overflow_hidden()
                                .pt(px(8.0))
                                .pr(px(22.0))
                                .pb(px(10.0))
                                .pl(px(14.0))
                                .flex()
                                .flex_col()
                                .child(grid_el),
                        ),
                )
                .into_any_element(),
        )
    }
}
