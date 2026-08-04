//! Скроллбар редактора.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::ui::minimap_geom::MIN_THUMB;
use crate::ui::minimap_geom::geom;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::input::InputState;
use kamin_theme::Palette;
use std::cell::Cell;
use std::rc::Rc;

/// Скроллбар редактора Zed-стиля: 12px, трек с бордером слева, thumb min 25px,
/// драг = text_unit_size (порт формул ScrollbarLayout). Ставится ПРАВЕЕ
/// минимапы (порядок Zed: текст → минимапа → скроллбар).
/// `markers` — диагностики активного файла (строка 0-based, severity как в
/// Problems): полосы 2px на треке, как markers Зеда.
pub fn scrollbar(input: &Entity<InputState>, markers: Vec<(u32, u8)>, p: &Palette) -> AnyElement {
    const SB_W: f32 = 12.0;
    let origin: Rc<Cell<(f32, f32, f32)>> = Rc::new(Cell::new((0.0, 0.0, 0.0)));

    let track_border = {
        let mut c = rgba(p.text_primary);
        c.a = 0.06;
        c
    };
    let thumb = {
        let mut c = rgba(p.bg_overlay);
        // Светлая тема: палка вдвое светлее (как у скроллбаров панелей)
        c.a = if kamin_theme::current_is_light() {
            0.28
        } else {
            0.55
        };
        c
    };
    let sev_color = {
        let (r, y, b, h) = (
            rgba(p.accent_red),
            rgba(p.accent_yellow),
            rgba(p.accent_blue),
            rgba(p.text_muted),
        );
        move |sev: u8| {
            let mut c = match sev {
                0 => r,
                1 => y,
                2 => b,
                _ => h,
            };
            c.a = 0.85;
            c
        }
    };

    let canvas = {
        let input = input.clone();
        let origin = origin.clone();
        gpui::canvas(
            move |bounds, _, _| {
                origin.set((
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.height),
                ));
            },
            move |bounds, _, window, cx| {
                let st = input.read(cx);
                let h = f32::from(bounds.size.height);
                let g = geom(st, h);
                if g.total <= g.vis_ed {
                    return;
                }
                let bx = f32::from(bounds.origin.x);
                let by = f32::from(bounds.origin.y);
                let bw = f32::from(bounds.size.width);
                // бордер трека слева (Zed BORDER_WIDTH 1px)
                window.paint_quad(gpui::fill(
                    gpui::Bounds::new(gpui::point(px(bx), px(by)), gpui::size(px(1.0), px(h))),
                    track_border,
                ));
                // Маркеры диагностик: позиция строки в долях буфера, полоса
                // 2px почти на всю ширину трека (Zed marker quads); ПОД
                // thumb'ом — он полупрозрачный и маркеры сквозь него видны.
                for (line, sev) in &markers {
                    let frac = (*line as f32 / g.total.max(1.0)).clamp(0.0, 1.0);
                    let my = by + frac * (h - 2.0).max(0.0);
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(
                            gpui::point(px(bx + 3.0), px(my)),
                            gpui::size(px(bw - 5.0), px(2.0)),
                        ),
                        sev_color(*sev),
                    ));
                }
                let th = ((g.vis_ed / g.total) * h).max(MIN_THUMB).min(h);
                // Редактор допускает overscroll (`BOTTOM_MARGIN_ROWS` в
                // gpui-component), поэтому scroll_row бывает БОЛЬШЕ max_scroll
                // и доля вылезала за 1 — палка уходила ниже трека.
                let frac = (g.scroll_row / g.max_scroll.max(1.0)).clamp(0.0, 1.0);
                let ty = by + frac * (h - th).max(0.0);
                // Thumb со скруглением сверху и снизу (половина ширины —
                // как у Zed/скроллбаров gpui-component: капсула)
                let tw = bw - 4.0;
                let mut q = gpui::fill(
                    gpui::Bounds::new(
                        gpui::point(px(bx + 2.0), px(ty)),
                        gpui::size(px(tw), px(th)),
                    ),
                    thumb,
                );
                q.corner_radii = gpui::Corners::all(px(tw / 2.0));
                window.paint_quad(q);
            },
        )
        .absolute()
        .size_full()
    };

    let on_jump = {
        let input = input.clone();
        let origin = origin.clone();
        move |wy: f32, cx: &mut gpui::App| {
            let (_, oy, h) = origin.get();
            input.update(cx, |st, cx| {
                let g = geom(st, h);
                if g.total <= g.vis_ed {
                    return;
                }
                let th = ((g.vis_ed / g.total) * h).max(MIN_THUMB);
                let frac = ((wy - oy - th / 2.0) / (h - th).max(1.0)).clamp(0.0, 1.0);
                let target = frac * g.max_scroll;
                st.scroll_handle
                    .set_offset(gpui::point(px(0.0), px(-(target * g.line_h))));
                cx.notify();
            });
        }
    };
    let on_jump_move = on_jump.clone();

    div()
        .id("editor-scrollbar")
        .w(px(SB_W))
        .flex_shrink_0()
        .h_full()
        .relative()
        .child(canvas)
        .on_mouse_down(gpui::MouseButton::Left, {
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                on_jump(f32::from(e.position.y), cx);
            }
        })
        .on_mouse_move({
            move |e: &gpui::MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(gpui::MouseButton::Left) {
                    on_jump_move(f32::from(e.position.y), cx);
                }
            }
        })
        .into_any_element()
}
