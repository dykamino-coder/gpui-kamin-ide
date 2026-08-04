//! Кадр главного окна: `Render for RootView`.
//!
//! Сама сборка разложена по соседям — `state::frame` (пролог кадра),
//! `state::metrics` (размеры), `state::columns` (колонки и тело),
//! `state::handlers` (действия и мышь). Здесь остался только порядок
//! (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::state::consts::UI_FONT;
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Font, FontFeatures, FontStyle, FontWeight, IntoElement, Render, Window,
    div,
};
use std::sync::Arc;

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::web::drawn();
        let _draw_started = std::time::Instant::now();

        let _ctx_started = std::time::Instant::now();
        let crate::state::frame_ctx::FrameCtx {
            p,
            log_filter_focused,
            design_input_focused,
            viewport_w,
            viewport_h,
            sidebar_w,
            file_w,
            right_w,
            main_split,
            file_bottom_px,
            has_active,
            no_sessions,
        } = self.frame_ctx(window, cx);
        crate::web::ctx_took(_ctx_started.elapsed().as_micros() as u32);
        // Прыжок размера (разворот/восстановление): каскады раскладки опираются
        // на probe-замеры ПРОШЛОГО кадра — заказываем следующий кадр, пока
        // размер не устаканится. Сходится за 1-2 кадра, в покое не срабатывает.
        if self.last_frame_viewport != (viewport_w, viewport_h) {
            self.last_frame_viewport = (viewport_w, viewport_h);
            cx.notify();
        }
        // Welcome (~30 элементов) нужен только ветке «нет активной сессии»:
        // при живой сессии строить его и выбрасывать — работа в мусор.
        let welcome_el: AnyElement = if has_active {
            gpui::Empty.into_any_element()
        } else {
            self.welcome(window, cx, p)
        };
        let chat_content: AnyElement = self.chat_content(window, cx, p, has_active, welcome_el);

        // ── Колонка чата: chat + mainBottom drawer (по mainBottomVisible)
        let main_column: AnyElement =
            self.main_column(window, cx, p, no_sessions, main_split, chat_content);

        // ── Файловая колонка: [modeHeader Files|Web] + контент + drawer
        let file_column: AnyElement = self.file_column(window, cx, p, viewport_h, file_bottom_px);

        // ── Правая колонка: [card+rail] / handle(pr 48) / [card+rail]
        let right_column_el: AnyElement = self.right_column(window, cx, p);

        // Панельные элементы (сиблинги в body-flex) — вынесены, т.к. body
        // условно показывает либо колонки, либо welcome на всю область.
        let crate::state::columns::wraps::Wraps {
            main_wrap,
            main_file_handle,
            file_wrap,
            file_right_handle,
            right_wrap,
            main_column_present,
            file_fills,
            right_fills,
        } = self.column_wraps(
            cx,
            p,
            main_column,
            file_column,
            right_column_el,
            sidebar_w,
            file_w,
            right_w,
            no_sessions,
        );
        let root = div()
            .relative()
            .track_focus(&self.focus_handle)
            .key_context("Root")
            // `window.addEventListener("scroll", …, true)` оригинала
            // (`FileContextMenu.tsx:50,53`): ЛЮБОЕ прокручивание закрывает
            // файловое контекст-меню. `on_scroll_wheel` тут не годится — он
            // работает в bubble-фазе и только когда никакой вложенный
            // скроллер колесо не забрал; регистрируем оконный слушатель
            // капчур-фазы через канвас (ревью ц.23)
            // Ховер-пилюля группы иногда залипала: если `mouseleave` не
            // пришёл (строка перерисовалась, курсор ушёл рывком), она висела
            // вечно. Любое НАЖАТИЕ в окне её снимает — как в оригинале, где
            // поповер живёт только на `:hover` (баг найден юзером)
            .when(self.hover_pill.is_some(), |root| {
                let tx = self.tx.clone();
                root.child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |_, (), window, _| {
                            let tx = tx.clone();
                            window.on_mouse_event(move |_: &gpui::MouseDownEvent, phase, _, _| {
                                if phase == gpui::DispatchPhase::Capture {
                                    let _ = tx.try_send(ShellEvent::HoverPill(None));
                                }
                            });
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0(),
                )
            })
            // Меню веб-страницы: ЛЮБОЕ нажатие закрывает (клик по пункту
            // успевает отправить свою команду в bubble-фазе того же клика).
            .when(self.web_menu.is_some(), |root| {
                let tx = self.tx.clone();
                root.child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |_, (), window, _| {
                            let tx = tx.clone();
                            window.on_mouse_event(move |_: &gpui::MouseDownEvent, phase, _, _| {
                                if phase == gpui::DispatchPhase::Capture {
                                    let _ = tx.try_send(ShellEvent::WebMenu(None));
                                }
                            });
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0(),
                )
            })
            .when(self.file_menu.is_some(), |root| {
                let tx = self.tx.clone();
                root.child(
                    gpui::canvas(
                        |_, _, _| {},
                        // ТОЛЬКО из paint: `on_mouse_event` в prepaint паникует
                        // («this method can only be called during paint»)
                        move |_, (), window, _| {
                            let tx = tx.clone();
                            window.on_mouse_event(
                                move |_: &gpui::ScrollWheelEvent, phase, _, _| {
                                    if phase == gpui::DispatchPhase::Capture {
                                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                                    }
                                },
                            );
                        },
                    )
                    // Нулевой канвас gpui не красит — даём ему абсолютный бокс
                    // на всё окно, как `probe_area`
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0(),
                )
            })
            // Клик мимо веб-вью гасит фокус страницы: без этого каретка в
            // чате/консоли мигала и ловила ввод после ухода в наш интерфейс.
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    |_, (), window, _| {
                        window.on_mouse_event(move |ev: &gpui::MouseDownEvent, phase, _, _| {
                            if phase == gpui::DispatchPhase::Capture {
                                crate::web::blur_if_outside(
                                    f32::from(ev.position.x),
                                    f32::from(ev.position.y),
                                );
                            }
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0(),
            )
            // ↑/↓ в инпут-оверлеях перехватываем в ФАЗЕ CAPTURE: фокус
            // держит `Input` оверлея, и его собственная обработка стрелок
            // (движение каретки) съедает клавишу раньше корневых биндингов.
            .capture_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
                if !(this.sov.quickopen_open || this.sov.fif_open || this.sov.ws_open) {
                    return;
                }
                let delta = match ev.keystroke.key.as_str() {
                    "down" => 1,
                    "up" => -1,
                    _ => return,
                };
                cx.stop_propagation();
                this.move_overlay_active(delta);
                cx.notify();
            }))
            .map(|root| self.bind_actions(root, cx))
            .map(|root| self.bind_mouse(root, window, cx))
            .flex()
            .flex_col()
            .size_full()
            .font(Font {
                family: UI_FONT.into(),
                features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            })
            .text_color(rgba(p.text_primary))
            // Браузерный `line-height: normal` для Bricolage = 1.169 (замер в
            // оригинале: 13px → 15.2px строка). gpui по умолчанию 1.618 —
            // из-за этого КАЖДАЯ строка без фиксированной высоты выходила
            // на ~5px выше оригинала (нав Customize: 39 вместо 31.2).
            .line_height(gpui::relative(1.169))
            .map(|root| self.with_canvas_layers(root, p, cx))
            .map(|root| self.with_chrome(root, p, window, cx, viewport_w))
            .child(self.body_row(
                window,
                cx,
                p,
                main_wrap,
                main_file_handle,
                file_wrap,
                file_right_handle,
                right_wrap,
                main_column_present,
                file_fills,
                right_fills,
                sidebar_w,
                log_filter_focused,
                design_input_focused,
                no_sessions,
            ))
            .child(crate::ui::status_bar::status_bar(
                self.status_counts,
                self.update_available.clone(),
                self.update_progress,
                self.status_items.values().cloned().collect(),
                !self.ed.editor_tabs.is_empty(),
                self.ed
                    .editor_tabs
                    .get(self.ed.editor_active)
                    .map(|t| t.eol)
                    .filter(|_| !self.ed.editor_tabs.is_empty()),
                env!("CARGO_PKG_VERSION"),
                &self.tx,
                p,
            ))
            // Скрим-затемнение под инпутными оверлеями (палитра/QuickOpen/FiF/
            // symbols/модалка): сам оверлей рисуется в overlay-окне без альфы,
            // затемнить фон может только main (вебвью, увы, вне досягаемости)
            .map(|root| self.with_main_overlays(root, p))
            // Ф6: весь стек оверлеев (тултипы/меню/пикеры/модалки) — слоем
            // ЭТОГО окна, второго окна больше нет.
            .child(self.overlay_stack(window, cx));
        crate::web::draw_took(_draw_started.elapsed().as_micros() as u32);
        root
    }
}
