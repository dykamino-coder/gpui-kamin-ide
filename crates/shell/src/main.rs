#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "process entry point owns console diagnostics"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "process entry point reads OS and tool configuration from the environment"
)]
// GUI-subsystem в release: console-subsystem дарил packaged-приложению чёрное
// консольное окно. В dev (debug_assertions) консоль остаётся — туда идут логи
// сайдкара при запуске из терминала.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! KaminIDE GPUI shell — точка входа.
//!
//! Карта на один экран (подробности — в `ARCHITECTURE.md` в корне репозитория):
//!
//! * `host` — связь с процессом `kamin-host`: входящие события и запросы;
//! * `state` — состояние окна (`RootView`), обработчики событий и сборка кадра;
//! * `ui` — компоненты интерфейса, чистые функции без знания о состоянии;
//! * `overlay` — второе прозрачное окно поверх главного: меню, модалки, тосты;
//! * `web` — страницы на CEF: чат Bridge, браузер, вью расширений;
//! * `term` — терминал, `icon_theme` — иконки файлов, `probe` — отладочный канал.
//!
//! Событие идёт по кругу: клик в `ui` → канал → `state/events/dispatch.rs` →
//! обработчик правит `RootView` → следующий кадр собирается заново.
//!
//! mimalloc глобально (plan/10: уводит аллокации от DLP/AV heap-хуков).

mod actions;
mod activity;
mod assets;
mod colors;
#[cfg(windows)]
mod contrib_keys;
mod editor_lsp;
mod file_names;
mod fs_watch;
mod host;
mod host_link;
mod icon_light;
mod icon_raster;
mod icon_theme;
mod job;
mod layout_store;
mod legacy_bridge;
#[cfg(any(debug_assertions, test))]
mod native_acceptance;
mod os_clipboard;
mod output_log;
mod overlay;
mod probe;
mod root;
mod startup;
mod state;
mod term;
mod theme;
mod theme_sync;
mod toast;
mod ui;
mod updater;
mod web;
mod when;
mod win_integration;

use crate::host::events::EdEvent;
use gpui::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;
use kamin_metrics as m;

use root::RootView;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    // ПЕРВАЯ строка: дочерние процессы CEF — это копии нашего exe. Они обязаны
    // уйти отсюда до probe, сайдкара и окна (`web/process.rs`).
    web::exit_if_child_process();
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--native-acceptance-capabilities") {
        println!("debug-native-acceptance-v1");
        return;
    }
    // Общий Job: дети CEF в группе приложения и умирают вместе с ним.
    job::adopt_children();

    // Crash handler сохраняет модуль/адрес в cache/crash.log даже без консоли.
    kamin_crash::install("main", kamin_crash::AfterReport::SystemDialog);

    // DirectComposition сохраняет прозрачность при ресайзе. WM_SIZE +
    // resize_boost чинят разворот; GPUI_DISABLE_DIRECT_COMPOSITION=true → blit.
    // SAFETY: до Application::new, второго потока ещё нет.
    if std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none() {
        unsafe { std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "false") };
    }

    if startup::forward_launch() {
        return;
    }

    #[cfg(feature = "probe")]
    probe::host::start();

    // CEF поднимаем в браузерном процессе; кадры и элементы — следующие фазы.
    // ⚠ Отложить init за первый кадр НЕЛЬЗЯ: пробовано (#76, холодный старт) —
    // окно рисуется ЧЁРНЫМ (init участвует в постановке D3D/подложки до окна);
    // откатано сразу по скрину.
    web::init();

    // Подтяжка данных прод-KaminIDE (host-сессии + bridge-конфиг/чаты) — ДО
    // старта host: он читает сторы один раз при подъёме.
    host::migrate_prod::run();

    // Сайдкар kamin-host + WS: события в GPUI через smol-канал
    let _ = host_link::t0();
    let (tx, rx) = smol::channel::unbounded::<host_link::ShellEvent>();
    host_link::start(tx.clone());
    #[cfg(debug_assertions)]
    native_acceptance::startup();
    // Восстановить открытые файлы прошлой сессии (персист layout.json)
    for path in crate::layout_store::load_string_list("openFiles") {
        let _ = tx.try_send(host_link::ShellEvent::Ed(EdEvent::OpenFile(path)));
    }

    let app = startup::application();
    app.run(move |cx| {
        startup::configure(cx);

        // Насос CEF: очередь событий и перерисовка по приходу кадра.
        web::start_pump(cx);

        cx.spawn(async move |cx| {
            let bounds = cx.update(|cx| {
                Bounds::centered(
                    None,
                    size(px(m::WINDOW_DEFAULT_WIDTH), px(m::WINDOW_DEFAULT_HEIGHT)),
                    cx,
                )
            });
            // Рестор габаритов прошлого запуска (metrics.rs персистит на
            // устоявшийся ресайз): бут в ДРУГОМ размере прогонял ширины
            // колонок через фактор-масштаб и записывал урезанные — краш до
            // максимизации фиксировал их навсегда («лейаут сбросился»).
            let saved = crate::layout_store::load_raw_key("windowBounds");
            let window_bounds = saved
                .as_ref()
                .and_then(|v| {
                    let f = |k: &str| v.get(k)?.as_f64();
                    let (x, y, w, h) = (f("x")?, f("y")?, f("w")?, f("h")?);
                    if w < f64::from(m::WINDOW_MIN_WIDTH) || h < f64::from(m::WINDOW_MIN_HEIGHT) {
                        return None;
                    }
                    let b = Bounds {
                        origin: gpui::point(px(x as f32), px(y as f32)),
                        size: size(px(w as f32), px(h as f32)),
                    };
                    Some(
                        if v.get("maximized").and_then(|m| m.as_bool()) == Some(true) {
                            WindowBounds::Maximized(b)
                        } else {
                            WindowBounds::Windowed(b)
                        },
                    )
                })
                .unwrap_or(WindowBounds::Windowed(bounds));
            let options = WindowOptions {
                window_bounds: Some(window_bounds),
                focus: startup::focus(),
                // Нативный хедер убит: frameless, титлбар рисуем сами
                // (ui/titlebar). Заголовок всё равно задаём — им подписаны
                // строки диспетчера задач и Alt+Tab.
                titlebar: Some(TitlebarOptions {
                    title: Some("KaminIDE".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
                }),
                window_min_size: Some(size(px(m::WINDOW_MIN_WIDTH), px(m::WINDOW_MIN_HEIGHT))),
                ..Default::default()
            };
            let mut view_slot = None;
            let view_tx = tx.clone();
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| RootView::new(view_tx, cx));
                let fh = view.read(cx).focus_handle.clone();
                window.focus(&fh, cx);
                view_slot = Some(view.clone());
                // Заказы перерисовки на приход кадров CEF адресуем ЭТОМУ вью:
                // `cx.refresh()` глушил бы кэш панелей (`web/pump.rs`).
                web::set_repaint_target(view.downgrade());
                // Фон окна рисует корневой канвас `RootView`: фон `Root`
                // перекрывал бы его собственный слой (0.7.1: `Root` — `Styled`,
                // свой стиль ложится поверх фона темы).
                cx.new(|cx| {
                    gpui::Styled::bg(Root::new(view, window, cx), gpui::transparent_black())
                })
            })?;

            // Ф6: отдельного overlay-окна больше нет — весь стек оверлеев
            // рисует слой главного окна (`state/overlay_stack.rs`). HWND
            // главного всё же публикуем: его ждут подложка web и тосты.
            if view_slot.is_some() {
                smol::Timer::after(std::time::Duration::from_millis(300)).await;
                #[cfg(windows)]
                {
                    let main_hwnd = cx.update(|_cx| overlay::main_hwnd_isize());
                    overlay::set_main_hwnd(main_hwnd);
                }
            }

            // Насос событий host_link → RootView (foreground)
            if let Some(view) = view_slot {
                while let Ok(event) = rx.recv().await {
                    view.update(cx, |v, cx| {
                        // Имя — до `apply`, который забирает событие: в `[why]`
                        // видно, какое событие хоста заказало кадр окна.
                        let event_name = crate::web::diag_event_name::name(&event);
                        let needs_frame = v.apply(event, cx);
                        if needs_frame {
                            crate::web::diag_why::tick(event_name);
                        }
                        // Пропсы компонентов обновляем ЗДЕСЬ, а не в
                        // рендере: `notify` из фазы отрисовки не помечает
                        // окно грязным и нового кадра не заказывает
                        // (`vendor/gpui/src/window.rs:117-127`), то есть
                        // проверка изменений молча не работала.
                        v.sync_panels(cx);
                        // Холостые события (вывод терминала, доставка
                        // страницам) кадра не заказывают; остальные — через
                        // насос, в общем ритме кадров (`frame_rhythm`).
                        if needs_frame {
                            crate::web::repaint_requested();
                        }
                    });
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });

    // Окно закрыто — доносим отложенный layout-патч (дебаунс терял хвост
    // изменений: «лейаут открылся не таким, каким оставил») и гасим CEF:
    // иначе дочерние процессы переживут выход.
    crate::layout_store::flush_now();
    web::shutdown();
}
