//! Find in Files (Ctrl+Shift+F, FindInFiles 1:1): подстрочный поиск по индексу
//! (kamin:index:findInFiles). Бокс w720 max-h 76vh, инпут (border-b) + status
//! («Searching…»/«Type at least 2 chars»/«N hits») + список: заголовок (rel +
//! :line мут) + сниппет (моно, совпадение подсвечено accent-orange 35%).

pub use crate::ui::fif_row::TextHit;

use crate::host::events::EdEvent;
use crate::ui::fif_row::hit_row;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

const BOX_W: f32 = 720.0;
pub(crate) const MAX_ROWS: usize = 200;

/// Рендер Find in Files. `input` — Entity<InputState> (в root.rs).
#[allow(clippy::too_many_arguments)]
pub fn find_in_files(
    results: &[TextHit],
    active: usize,
    query_len: usize,
    busy: bool,
    input: &Entity<InputState>,
    viewport_w: f32,
    viewport_h: f32,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // `--overlay-soft`: тем-зависимый, единый источник
    let scrim = crate::ui::scrim::soft_literal();
    let tx_close = tx.clone();
    let tx_key = tx.clone();
    let active_hit = results.get(active).map(|h| (h.abs.clone(), h.line));

    let status = if busy {
        "Searching…".to_string()
    } else if query_len < 2 {
        "Type at least 2 chars".to_string()
    } else {
        format!("{} hits", results.len())
    };

    let mut list = div()
        .flex()
        .flex_col()
        .pb(px(m::SPACE_2))
        .overflow_y_scrollbar();
    for (i, h) in results.iter().take(MAX_ROWS).enumerate() {
        list = list.child(hit_row(i, h, i == active, tx, p));
    }

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .justify_center()
        .items_start()
        // 10vh реального вьюпорта БЕЗ клампа 600 (ревью ц.1)
        .pt(px(0.10 * viewport_h))
        .bg(scrim)
        .child(crate::overlay::input_area())
        // Escape ловим в ФАЗЕ CAPTURE: фокус держит `Input` оверлея, он
        // обрабатывает клавишу первым, и до bubble-обработчика скрима
        // Escape не доходил — оверлей не закрывался (баг найден юзером)
        .capture_key_down({
            let tx = tx.clone();
            move |ev: &gpui::KeyDownEvent, _, cx| {
                if ev.keystroke.key.as_str() == "escape" {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::CloseFindInFiles);
                }
            }
        })
        .on_key_down(
            move |ev: &gpui::KeyDownEvent, _, _| match ev.keystroke.key.as_str() {
                "escape" => {
                    let _ = tx_key.try_send(ShellEvent::CloseFindInFiles);
                }
                "enter" => {
                    if let Some((abs, line)) = &active_hit {
                        let _ = tx_key
                            .try_send(ShellEvent::Ed(EdEvent::OpenFileAt(abs.clone(), *line)));
                        let _ = tx_key.try_send(ShellEvent::Ed(EdEvent::SetFileMode("files")));
                        let _ = tx_key.try_send(ShellEvent::CloseFindInFiles);
                    }
                }
                _ => {}
            },
        )
        // Клик мимо коробки: ловим на СВОЁМ абсолютном слое, а не на скриме.
        // `input_area()` — канвас `absolute size_full` НАД скримом, и
        // mouse-down скрима до него не доходил (баг найден юзером)
        .child(
            div()
                .absolute()
                .inset_0()
                // Закрываем на ОТПУСКАНИИ и только если обе точки вне коробки
                // — то же правило `target === currentTarget`, что у модалки.
                // Коробка гасит своё mouse-down, поэтому «нажал внутри,
                // отпустил снаружи» сюда не доходит вовсе
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    move |ev: &gpui::MouseUpEvent, _, _| {
                        let inside = crate::probe::registry::bounds_of("fif-box").is_some_and(
                            |[x, y, bw, bh]| {
                                let (ux, uy) = (f32::from(ev.position.x), f32::from(ev.position.y));
                                ux >= x && ux <= x + bw && uy >= y && uy <= y + bh
                            },
                        );
                        if !inside {
                            let _ = tx_close.try_send(ShellEvent::CloseFindInFiles);
                        }
                    },
                ),
        )
        .child(
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Измеряемый бокс: без него закрытие по клику мимо
                // невозможно проверить кадром (приём с модалки, ц.27)
                .child(crate::probe::registry::probe_area("fif-box"))
                .w(px(BOX_W))
                // `min(640, 100vw − 32)` — пола у оригинала нет
                .max_w(px(viewport_w - 32.0))
                .max_h(px(0.76 * viewport_h)) // 76vh без клампа (ревью ц.1)
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(m::RADIUS_MD))
                .relative()
                .child(crate::probe::registry::probe_area("ov-fif"))
                .bg(rgba(p.bg_mantle))
                .child(crate::overlay::hit_area())
                .border_1()
                .border_color(tint(rgba(p.bg_surface), 0.6))
                .shadow(crate::ui::shadows::dropdown())
                .child(
                    div()
                        .w_full()
                        .px(px(14.0))
                        .h(px(40.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(tint(rgba(p.bg_surface), 0.5))
                        .child(
                            Input::new(input)
                                .appearance(false)
                                // `--fs-md` 13 и НУЛЕВОЙ собственный бокс: свои
                                // `px 8 / py 2 / h 24` Input ставит до
                                // `refine_style`, отступы даёт ряд (ревью ц.20)
                                .with_size(gpui_component::Size::Size(px(m::FS_MD / 0.875)))
                                .px_0()
                                .py_0()
                                .h_full(),
                        ),
                )
                .child(
                    div()
                        .px(px(14.0))
                        .py(px(6.0))
                        .text_size(px(m::FS_XS))
                        .text_color(rgba(p.text_muted))
                        .child(status),
                )
                .child(list),
        )
        .into_any_element()
}
