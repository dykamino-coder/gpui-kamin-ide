//! Sticky-scroll редактора (VS Code editor.stickyScroll): заголовки
//! объемлющих блоков липнут к верху вьюпорта. Модель — INDENTATION
//! (официальный фолбэк VS Code, работает для любого языка): предки первой
//! видимой строки = цепочка вышестоящих строк со строго меньшим отступом.
//! Скролл-оффсет — из vendored-патча gpui-component (pub scroll_handle).

use crate::host::events::EdEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;

/// Кап липких строк (editor.stickyScroll.maxLineCount default = 5).
pub const MAX_STICKY: usize = 5;
/// Высота строки редактора gpui-component: LINE_HEIGHT = 1.25rem = 20px.
pub const EDITOR_LINE_H: f32 = 20.0;

/// Отступ строки в «колонках» (таб = 4); None для пустых/пробельных строк.
fn indent_of(line: &str) -> Option<usize> {
    if line.trim().is_empty() {
        return None;
    }
    let mut n = 0;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    Some(n)
}

/// Индексы (0-based) липких строк для first_visible (indentation-модель):
/// идём вверх, собирая строки со строго убывающим отступом — «заголовки»
/// блоков, внутри которых находится первая видимая строка.
pub fn compute(lines: &[&str], first_visible: usize) -> Vec<usize> {
    if first_visible == 0 || first_visible >= lines.len() {
        return Vec::new();
    }
    // Отступ якоря: первая непустая строка от first_visible вниз
    let mut anchor_indent = None;
    for l in lines.iter().skip(first_visible) {
        if let Some(ind) = indent_of(l) {
            anchor_indent = Some(ind);
            break;
        }
    }
    let Some(mut cur) = anchor_indent else {
        return Vec::new();
    };
    let mut out: Vec<usize> = Vec::new();
    for i in (0..first_visible).rev() {
        let Some(ind) = indent_of(lines[i]) else {
            continue;
        };
        if ind < cur {
            out.push(i);
            cur = ind;
            if cur == 0 || out.len() == MAX_STICKY {
                break;
            }
        }
    }
    out.reverse();
    out.truncate(MAX_STICKY);
    out
}

/// Рендер стопки: absolute top-0, фон редактора, нижняя граница; клик по
/// строке → GotoLine (как в VS Code — прыжок к заголовку).
pub fn sticky_overlay(
    rows: &[(usize, String)],
    path: &str,
    p: &Palette,
    tx: &Sender<ShellEvent>,
) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    let mut col = div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .flex()
        .flex_col()
        .bg(
            if false
            /* KAMIN_VWV_PAINTDBG законстанчен OFF (диагностика plan/97) */
            {
                // ДИАГ: маджента = sticky_scroll
                gpui::Rgba {
                    r: 1.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                }
            } else {
                rgba(p.editor_bg)
            },
        )
        .border_b_1()
        .border_color({
            let mut c = rgba(p.text_primary);
            c.a = 0.08;
            c
        })
        .shadow(vec![gpui::BoxShadow {
            color: gpui::Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.25,
            }
            .into(),
            offset: gpui::point(px(0.), px(2.)),
            blur_radius: px(6.),
            spread_radius: px(0.),
        }]);
    for (idx, text) in rows {
        let line_1based = (idx + 1) as u32;
        let tx = tx.clone();
        let path = path.to_string();
        let hover_bg = {
            let mut c = rgba(p.text_primary);
            c.a = 0.06;
            c
        };
        col = col.child(
            div()
                .id(SharedString::from(format!("sticky-{idx}")))
                .flex()
                .items_center()
                .h(px(EDITOR_LINE_H))
                .overflow_hidden()
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ =
                        tx.try_send(ShellEvent::Ed(EdEvent::GotoLine(path.clone(), line_1based)));
                })
                .child(
                    // Гуттер под номер строки — ширина как у редактора
                    div()
                        .flex_shrink_0()
                        .w(px(48.0))
                        .pr(px(m::SPACE_2))
                        .text_size(px(m::FS_XS))
                        .text_color(rgba(p.text_disabled))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .child(SharedString::from(line_1based.to_string())),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_color(rgba(p.text_secondary))
                        .child(SharedString::from(text.clone())),
                ),
        );
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::compute;

    const SRC: &str = "fn outer() {\n    let a = 1;\n    if a > 0 {\n        for i in 0..3 {\n            println!(\"{i}\");\n            let b = i;\n            let c = b;\n            let d = c;\n        }\n    }\n}\n";

    #[test]
    fn ancestors_by_indent() {
        let lines: Vec<&str> = SRC.lines().collect();
        // Вьюпорт начинается с "let b = i;" (idx 5, indent 12):
        // предки = for (idx 3), if (idx 2), fn (idx 0)
        let s = compute(&lines, 5);
        assert_eq!(s, vec![0, 2, 3]);
    }

    #[test]
    fn top_of_file_no_sticky() {
        let lines: Vec<&str> = SRC.lines().collect();
        assert!(compute(&lines, 0).is_empty());
    }

    #[test]
    fn near_header_single_ancestor() {
        let lines: Vec<&str> = SRC.lines().collect();
        // first_visible=1 ("let a"): единственный предок fn(0)
        assert_eq!(compute(&lines, 1), vec![0]);
    }

    #[test]
    fn blank_lines_skipped() {
        let src = "fn f() {\n\n    x;\n\n    y;\n}\n";
        let lines: Vec<&str> = src.lines().collect();
        let s = compute(&lines, 4); // "    y;"
        assert_eq!(s, vec![0]);
    }
}
