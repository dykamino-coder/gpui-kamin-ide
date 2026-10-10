//! Прогон reftest-ов WPT: тест и эталон рисуются НАШИМ движком, снимки
//! сравниваются между собой.
//!
//!     cargo run --example wptrun -- <список.txt> [ширина] [высота]
//!
//! Формат списка: по строке на пару, `тест|эталон`. Эталон в reftest написан
//! примитивной вёрсткой, дающей заведомо тот же результат, поэтому
//! эталонного снимка из браузера не нужно: если наша раскладка верна, оба
//! файла дают одинаковую картинку.
//!
//! Всё в ОДНОМ процессе и одном окне: запуск окна стоит секунды, а пар —
//! тысячи. Документ подменяется в той же сущности, кадр снимается прямо
//! отсюда (`PrintWindow`), и следующая пара идёт без перезапуска.

// The runner is configured through WPT_* environment variables (see clippy.toml).
#![allow(clippy::disallowed_methods)]

#[path = "wptrun/capture.rs"]
mod capture;
#[path = "wptrun/capture_dump.rs"]
mod capture_dump;
#[path = "wptrun/capture_name.rs"]
mod capture_name;
#[path = "wptrun/config.rs"]
mod config;
#[path = "wptrun/frame_metrics.rs"]
mod frame_metrics;
#[path = "wptrun/html_attrs.rs"]
mod html_attrs;
#[path = "wptrun/links.rs"]
mod links;
#[path = "wptrun/page.rs"]
mod page;
#[path = "wptrun/page_box.rs"]
mod page_box;
#[path = "wptrun/page_box_values.rs"]
mod page_box_values;
#[path = "wptrun/pair.rs"]
mod pair;
#[path = "wptrun/pixel_compare.rs"]
mod pixel_compare;
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
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write("target/wpt-panic.txt", format!("{info}"));
        eprintln!("{info}");
    }));
    let args: Vec<String> = std::env::args().collect();
    let list = args.get(1).cloned().unwrap_or_default();
    let w: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800.);
    let h: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(600.);
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

    gpui_platform::application().run(move |cx| {
        // Серые (не ClearType) глифы: как до gpui-pre 0.3.8, точный RGB стенда.
        cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
        if let Some(elapsed) = animation_elapsed {
            eprintln!("WPT animation elapsed override: {} ms", elapsed.as_millis());
            cx.set_global(gpui::AnimationElapsedTime(elapsed));
        }
        // Ahem — служебный шрифт набора: все его буквы одинаковые чёрные
        // квадраты в кегль. На нём построены сотни тестов: фигуры сходятся
        // ровно потому, что метрики предсказуемы. Без него текст набирается
        // системным шрифтом, и «квадраты» разъезжаются.
        for name in ["Ahem.ttf", "AHEM____.TTF"] {
            let path = std::path::Path::new("vendor/wpt-parsing/fonts").join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                let _ = cx.text_system().add_fonts(vec![bytes.into()]);
                break;
            }
        }
        // Щуп метрик ставится ПОСЛЕ регистрации: до неё `Ahem` ещё не найден,
        // и `ch` мерился бы по шрифту-подмене.
        kamin_html::metrics::use_text_system(cx.text_system().clone());
        // Свои шрифты страницы (`@font-face`): файл отдаётся системе, а её имя
        // семейства запоминается под тем, которым шрифт зовут в разметке.
        let fonts = cx.text_system().clone();
        kamin_html::fonts::install_loader(move |bytes| {
            // Имя — из name-таблицы файла: разность общего списка имён
            // пуста, когда система уже знает такое семейство, и алиас
            // терялся (FontWithFancyFeatures — весь кластер OpenType-фич).
            let named = kamin_html::fonts::sfnt_family(&bytes);
            let before: std::collections::HashSet<String> =
                fonts.all_font_names().into_iter().collect();
            fonts.add_fonts(vec![bytes.into()]).ok()?;
            named.or_else(|| {
                fonts
                    .all_font_names()
                    .into_iter()
                    .find(|name| !before.contains(name))
            })
        });
        let empty = Rc::new(Document::new("", BROWSER_CSS));
        if capture::offscreen() {
            gpui::frame_capture::enable();
        }
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
                    ..Default::default()
                },
                |_, cx| -> Entity<Page> {
                    cx.new(|_| Page {
                        doc: empty,
                        select: None,
                    })
                },
            )
            .unwrap();
        // Фокус НЕ забираем: стенд идёт десятками минут, и всё это время его
        // окно висело поверх чужой работы — в паузах между страницами белое.
        // Снимок делает `PrintWindow` с флагом 3, ему передний план не нужен:
        // достаточно, чтобы окно не было СВЁРНУТО.

        let cfg = run::RunConfig {
            pairs,
            exact,
            dumping,
            dump_all,
        };
        cx.spawn(async move |cx| run::run(cx, window.into(), cfg).await)
            .detach();
    });
}
