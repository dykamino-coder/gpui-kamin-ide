//! Сетка терминала: строки с раскраской, курсор, выделение.
//!
//! Блок перенесён из `terminal_body` как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::root::{RootView, TERM_CELL_H, TERM_CELL_W};
use gpui::prelude::*;
use gpui::{Context, div, px};
use kamin_theme::Palette;

/// Возвращает готовую сетку символов для тела терминала.
pub(crate) fn term_grid(
    t: &crate::term::TermSession,
    p: &Palette,
    cx: &mut Context<RootView>,
) -> gpui::Stateful<gpui::Div> {
    let (rows_runs, cursor) = t.screen_styled();
    let mut grid_el = div()
        .id("terminal-grid")
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_hidden()
        .child(crate::probe::registry::probe_area("terminal"))
        .on_scroll_wheel(cx.listener(|this, ev: &gpui::ScrollWheelEvent, _, cx| {
            if let Some(term) = this.term.terminals.get_mut(this.term.term_active) {
                let lines = match ev.delta {
                    gpui::ScrollDelta::Lines(d) => d.y * 3.0,
                    gpui::ScrollDelta::Pixels(d) => f32::from(d.y) / TERM_CELL_H,
                };
                term.scroll(lines.round() as i32);
                cx.notify();
            }
        }))
        // Выделение мышью (1=точка, 2=слово, 3=строка) — как xterm
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, ev: &gpui::MouseDownEvent, _, cx| {
                let Some([bx, by, _, _]) = crate::probe::registry::bounds_of("terminal") else {
                    return;
                };
                let col = ((f32::from(ev.position.x) - bx) / TERM_CELL_W)
                    .floor()
                    .max(0.0) as usize;
                let row = ((f32::from(ev.position.y) - by) / TERM_CELL_H)
                    .floor()
                    .max(0.0) as usize;
                if let Some(term) = this.term.terminals.get_mut(this.term.term_active) {
                    term.selection_start(col, row, ev.click_count);
                    this.term.term_selecting = true;
                    cx.notify();
                }
            }),
        )
        .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, _, cx| {
            if !this.term.term_selecting || ev.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            let Some([bx, by, _, _]) = crate::probe::registry::bounds_of("terminal") else {
                return;
            };
            let col = ((f32::from(ev.position.x) - bx) / TERM_CELL_W)
                .floor()
                .max(0.0) as usize;
            let row = ((f32::from(ev.position.y) - by) / TERM_CELL_H)
                .floor()
                .max(0.0) as usize;
            if let Some(term) = this.term.terminals.get_mut(this.term.term_active) {
                term.selection_update(col, row);
                cx.notify();
            }
        }))
        .on_mouse_up(
            gpui::MouseButton::Left,
            cx.listener(|this, _: &gpui::MouseUpEvent, _, _| {
                this.term.term_selecting = false;
            }),
        );
    // Символ под курсором — рисуем поверх блока цветом фона
    let cursor_ch: Option<(usize, usize, String)> = cursor.map(|(ccol, crow)| {
        let ch = rows_runs
            .get(crow)
            .map(|runs| {
                runs.iter()
                    .flat_map(|(t, _, _)| t.chars())
                    .collect::<Vec<_>>()
            })
            .and_then(|chars| chars.get(ccol).copied())
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".into());
        (ccol, crow, ch)
    });
    let sel_bg = {
        let mut c = rgba(p.accent_primary);
        c.a = 0.3;
        c
    };
    for runs in rows_runs {
        let mut row_el = div().flex().whitespace_nowrap().h(px(TERM_CELL_H));
        for (text, fg, selected) in runs {
            let mut cell = div();
            if selected {
                cell = cell.bg(sel_bg);
            }
            row_el = row_el.child(match fg {
                Some(c) => cell.text_color(gpui::rgba(c)).child(text),
                None => cell.child(text),
            });
        }
        grid_el = grid_el.child(row_el);
    }
    // Block-курсор absolute: моно-сетка даёт точную позицию;
    // спрятан пока вьюпорт в скроллбэке
    if let Some((ccol, crow, ch)) = cursor_ch {
        // `cursorBlink: true` (`TerminalSession.tsx:75`): xterm
        // мигает периодом 1.2 с — половину периода курсор виден,
        // половину нет. У нас он был статичным (ревью ц.19)
        use gpui::AnimationExt as _;
        let cursor = div()
            .absolute()
            .left(px(ccol as f32 * TERM_CELL_W))
            .top(px(crow as f32 * TERM_CELL_H))
            .w(px(TERM_CELL_W))
            .h(px(TERM_CELL_H))
            .bg(rgba(p.editor_cursor))
            .text_color(rgba(p.editor_bg))
            .child(ch);
        // Под RDP вечное мигание = постоянные кадры в сеть — курсор статичен.
        grid_el = grid_el.child(if crate::win_integration::reduce_motion() {
            cursor.into_any_element()
        } else {
            cursor
                .with_animation(
                    "term-cursor-blink",
                    gpui::Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                    |d, delta| if delta < 0.5 { d } else { d.invisible() },
                )
                .into_any_element()
        });
    }
    grid_el
}
