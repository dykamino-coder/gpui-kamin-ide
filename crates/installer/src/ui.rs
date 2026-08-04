//! Брендированное окно инсталлера: тёмный фон + акцентный градиент-полоса
//! прогресса + Bricolage-заголовок. Обычный запуск и /update показывают его;
//! /S — полностью тихо (окно не создаётся).

use gpui::{
    App, AppContext as _, Application, Bounds, Context, Hsla, IntoElement, ParentElement, Render,
    Styled, Window, WindowBounds, WindowOptions, black, div, hsla, point, px, rgb, size,
};
use std::sync::atomic::{AtomicU8, AtomicBool, Ordering};

/// Прогресс 0..100 и флаг завершения — установка идёт в фоновом потоке,
/// окно только рисует их раз в кадр.
pub static PROGRESS: AtomicU8 = AtomicU8::new(0);
pub static DONE: AtomicBool = AtomicBool::new(false);
pub static FAILED: AtomicBool = AtomicBool::new(false);

pub fn set_progress(p: u8) {
    PROGRESS.store(p.min(100), Ordering::Relaxed);
}

struct Setup {
    version: String,
}

impl Render for Setup {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Перерисовка раз в ~30мс, пока не готово — тянет полосу.
        if !DONE.load(Ordering::Relaxed) {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(30))
                    .await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }

        let pct = PROGRESS.load(Ordering::Relaxed).min(100);
        let failed = FAILED.load(Ordering::Relaxed);
        let done = DONE.load(Ordering::Relaxed);

        // Бренд: фон #1e1f29, акцент — фиолетово-синий градиент, как у шелла.
        let bg = rgb(0x1e1f29);
        let track = hsla(0.0, 0.0, 1.0, 0.08);
        let accent_a: Hsla = hsla(258.0 / 360.0, 0.72, 0.62, 1.0); // #7c6cf0-ish
        let accent_b: Hsla = hsla(210.0 / 360.0, 0.85, 0.60, 1.0);

        let status = if failed {
            "Установка не завершена".to_string()
        } else if done {
            "Готово — запускаем KaminIDE…".to_string()
        } else {
            format!("Установка… {pct}%")
        };

        div()
            .flex()
            .flex_col()
            .justify_center()
            .size_full()
            .bg(bg)
            .px(px(48.0))
            .gap(px(20.0))
            .text_color(rgb(0xe1e4e8))
            // Заголовок
            .child(
                div()
                    .font_family("Bricolage Grotesque")
                    .text_size(px(30.0))
                    .child(format!("KaminIDE {}", self.version)),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(hsla(0.0, 0.0, 1.0, 0.6))
                    .child(status),
            )
            // Полоса: трек + заполнение градиентом.
            .child(
                div()
                    .w_full()
                    .h(px(10.0))
                    .rounded(px(5.0))
                    .bg(track)
                    .child(
                        div()
                            .h_full()
                            .rounded(px(5.0))
                            .w(gpui::relative(f32::from(pct) / 100.0))
                            .bg(gpui::linear_gradient(
                                90.0,
                                gpui::linear_color_stop(if failed { hsla(0.0, 0.7, 0.55, 1.0) } else { accent_a }, 0.0),
                                gpui::linear_color_stop(if failed { hsla(0.02, 0.7, 0.5, 1.0) } else { accent_b }, 1.0),
                            )),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(hsla(0.0, 0.0, 1.0, 0.35))
                    .child("dykamino.studio"),
            )
    }
}

/// Открыть окно и крутить цикл, пока установка не завершится. Возвращается
/// после закрытия окна (DONE выставляет фоновый поток → закрываем сами).
pub fn run_window(version: String) {
    let app = Application::new();
    app.run(move |cx: &mut App| {
        let win = size(px(460.0), px(240.0));
        // По центру основного дисплея.
        let bounds = cx
            .primary_display()
            .map(|d| {
                let db = d.bounds();
                Bounds {
                    origin: point(
                        db.origin.x + (db.size.width - win.width) / 2.0,
                        db.origin.y + (db.size.height - win.height) / 2.0,
                    ),
                    size: win,
                }
            })
            .unwrap_or(Bounds { origin: point(px(0.0), px(0.0)), size: win });
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_movable: true,
            is_resizable: false,     // фиксированный размер (запрос юзера)
            is_minimizable: false,
            window_min_size: Some(win),
            ..Default::default()
        };
        let _ = black();
        let handle = cx
            .open_window(opts, |_w, cx| cx.new(|_| Setup { version: version.clone() }))
            .expect("open setup window");

        // На передний план: gpui открывает окно без фокуса, и оно всплывало
        // ПОЗАДИ активного окна (Chrome) — «полосы не видно».
        let _ = handle.update(cx, |_, w, _| {
            w.activate_window();
        });

        // Фоновый сторож: закрываем окно после завершения, но НЕ раньше 1.8с
        // от старта — быстрая установка (прогретый кэш ~3с) иначе схлопывала
        // окно до того, как полосу успевали увидеть.
        let opened = std::time::Instant::now();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(80))
                    .await;
                let ended = DONE.load(Ordering::Relaxed) || FAILED.load(Ordering::Relaxed);
                if ended && opened.elapsed() >= std::time::Duration::from_millis(1800) {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(700))
                        .await;
                    let _ = cx.update(|cx| {
                        let _ = handle.update(cx, |_, w, _| w.remove_window());
                        cx.quit();
                    });
                    break;
                }
            }
        })
        .detach();
    });
}
