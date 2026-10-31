//! Прогон reftest-ов WPT: тест и эталон рисуются НАШИМ движком, снимки
//! сравниваются между собой.
//! `WPT_ANIMATION_FRAMES=0,125,500,1000` compares animated pairs at every listed
//! millisecond time; without it the ordinary single-capture protocol is unchanged.
//!
//!     cargo run --example wptrun --features wpt-harness -- <список.txt> [ширина] [высота]
//!
//! Формат списка: по строке на пару, `тест|эталон`. Эталон в reftest написан
//! примитивной вёрсткой, дающей заведомо тот же результат, поэтому
//! эталонного снимка из браузера не нужно: если наша раскладка верна, оба
//! файла дают одинаковую картинку.
//!
//! Всё в ОДНОМ процессе и одном окне: запуск окна стоит секунды, а пар —
//! тысячи. Документ подменяется в той же сущности, кадр снимается прямо
//! из GPU readback до Present, и следующая пара идёт без перезапуска.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "process entry point owns console diagnostics"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "process entry point reads OS and tool configuration from the environment"
)]

#[path = "wptrun/animation_frames.rs"]
mod animation_frames;
#[path = "wptrun/audit.rs"]
mod audit;
#[path = "wptrun/capture.rs"]
mod capture;
#[path = "wptrun/capture_dump.rs"]
mod capture_dump;
#[path = "wptrun/capture_name.rs"]
mod capture_name;
#[path = "wptrun/config.rs"]
mod config;
#[path = "wptrun/css_scan.rs"]
mod css_scan;
#[path = "wptrun/fonts.rs"]
mod fonts;
#[path = "wptrun/frame_metrics.rs"]
mod frame_metrics;
#[path = "wptrun/gpu.rs"]
mod gpu;
#[path = "wptrun/html_attrs.rs"]
mod html_attrs;
#[path = "wptrun/links.rs"]
mod links;
#[path = "wptrun/page.rs"]
mod page;
#[path = "wptrun/page_border.rs"]
mod page_border;
#[path = "wptrun/page_box.rs"]
mod page_box;
#[path = "wptrun/page_box_values.rs"]
mod page_box_values;
#[path = "wptrun/paged.rs"]
mod paged;
#[path = "wptrun/pair.rs"]
mod pair;
#[path = "wptrun/phase_timing.rs"]
mod phase_timing;
#[path = "wptrun/pixel_compare.rs"]
mod pixel_compare;
#[path = "wptrun/prepare.rs"]
mod prepare;
#[path = "wptrun/ready.rs"]
mod ready;
#[path = "wptrun/reference_result.rs"]
mod reference_result;
#[path = "wptrun/reftest_meta.rs"]
mod reftest_meta;
#[path = "wptrun/retry.rs"]
mod retry;
#[path = "wptrun/run.rs"]
mod run;
#[path = "wptrun/show.rs"]
mod show;
#[path = "wptrun/style_imports.rs"]
mod style_imports;
#[path = "wptrun/stylesheet.rs"]
mod stylesheet;

use gpui::{
    AppContext as _, Bounds, Entity, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use kamin_html::{BROWSER_CSS, Document};
use page::Page;
use std::rc::Rc;

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): подставлять `@import` содержимым файла
// внутри `<style>`, чтобы набор WOFF2 подключал свои шрифты. Срез 2012 пар
// (WOFF2 + шрифты + текст): 1480 -> 1446, +0/-34. Все 34 потери — семьи
// `*-invalid-*`, `*-bad-*`, `blocks-*`, `tabledata-*`: они проверяют, что
// НЕГОДНЫЙ шрифт НЕ применяется, и с подстановкой мы начали его принимать
// (0.41 -> 4.29). Правильный порядок обратный: сперва научить `fonts.rs`
// отвергать негодный woff2, и только потом подставлять `@import`.

fn main() {
    // Паника В КАДРЕ не роняет процесс: оконный вызов Windows её глотает, и
    // на экране молча остаётся предыдущая страница — стенд считает её пустой.
    // След в файле — единственный способ увидеть такую панику.
    // Стенд запускают пачками и без человека у экрана: любое модальное окно
    // gpui («Failed to launch») повесило бы процесс намертво.
    unsafe { std::env::set_var("GPUI_NO_ERROR_DIALOG", "1") };
    let mut instrumentation = kamin_html::host::instrumentation::from_env();
    // WPT по умолчанию использует DPR 1; явно заданное окружение сохраняет приоритет.
    if std::env::var_os("KAMIN_FORCE_SCALE").is_none() {
        instrumentation.scale_factor = Some(1.);
    }
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write("target/wpt-panic.txt", format!("{info}"));
        eprintln!("{info}");
    }));
    let mut args: Vec<String> = std::env::args().collect();
    let host = gpu::Host::new(&mut args).unwrap_or_else(|e| audit::fatal(e));
    let list = args.get(1).cloned().unwrap_or_default();
    let w: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800.);
    let h: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(600.);
    let audit = audit::Audit::new(&host, &list, w, h);
    let (capture, sink) = capture::setup();
    let sink = Some(audit.sink(sink));
    // `WPT_DUMP=1` — снимки разошедшихся пар, `WPT_DUMP=all` — всех подряд.
    // Второе нужно ручному разбору: там проверяется и то, что стенд счёл
    // сошедшимся (см. `scripts/wpt_review.py`).
    let exact = pixel_compare::exact_mode();
    let dump_mode = std::env::var("WPT_DUMP").unwrap_or_default();
    let dumping = !dump_mode.is_empty();
    let dump_all = dump_mode == "all";
    let pairs = config::pairs(&list);
    let animation_elapsed = config::animation_elapsed();
    if pairs.is_empty() {
        eprintln!("пустой список пар: {list}");
        std::process::exit(1);
    }

    host.application()
        .with_instrumentation(instrumentation)
        .run(move |cx| {
            // Серые (не ClearType) глифы: как до gpui-pre 0.3.8, точный RGB стенда.
            cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
            if let Some(elapsed) = animation_elapsed {
                eprintln!("WPT animation elapsed override: {} ms", elapsed.as_millis());
                cx.set_global(gpui::AnimationElapsedTime(elapsed));
            }
            fonts::register(cx);
            // Щуп метрик ставится ПОСЛЕ регистрации: до неё `Ahem` ещё не найден,
            // и `ch` мерился бы по шрифту-подмене.
            let fonts = kamin_html::metrics::FontContext::new(cx.text_system().clone());
            let empty = Rc::new(
                Document::builder("", BROWSER_CSS)
                    .fonts(&fonts)
                    .build()
                    .expect("static empty runner document"),
            );
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            // An off-screen origin here made GPUI fall back to a
                            // display-sized window (1920x1080 shots): create on screen,
                            // `capture::park_offscreen` moves it away before the first frame.
                            origin: point(px(40.), px(40.)),
                            size: size(px(w), px(h)),
                        })),

                        titlebar: Some(TitlebarOptions {
                            appears_transparent: true,
                            ..Default::default()
                        }),
                        window_decorations: Some(WindowDecorations::Client),
                        window_background: WindowBackgroundAppearance::Opaque,
                        is_resizable: false,
                        is_minimizable: false,
                        focus: false,
                        ..Default::default()
                    },
                    |window, cx| -> Entity<Page> {
                        if let Some(sink) = sink {
                            window
                                .set_frame_capture(Some(sink))
                                .expect("Windows GPU readback");
                        }
                        cx.new(|_| Page {
                            doc: empty,
                            select: None,
                            #[cfg(feature = "wpt-harness")]
                            probes: Default::default(),
                        })
                    },
                )
                .unwrap();
            // Фокус НЕ забираем: стенд идёт десятками минут, и всё это время его
            // окно висело поверх чужой работы — в паузах между страницами белое.
            // GPU readback снимает кадр до Present и не требует переднего плана.

            let cfg = run::RunConfig {
                capture,
                pairs,
                exact,
                dumping,
                dump_all,
                audit,
            };
            cx.spawn(async move |cx| run::run(cx, window.into(), cfg).await)
                .detach();
        });
}
