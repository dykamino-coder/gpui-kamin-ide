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
mod crash_report;
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
mod os_clipboard;
mod output_log;
mod overlay;
mod probe;
mod root;
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
use gpui::{
    AppContext as _, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_component::Root;
use kamin_metrics as m;

use root::RootView;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    // ПЕРВАЯ строка: дочерние процессы CEF — это копии нашего exe. Они обязаны
    // уйти отсюда до probe, сайдкара и окна (`web/process.rs`).
    web::exit_if_child_process();
    // Общий Job: дети CEF в группе приложения и умирают вместе с ним.
    job::adopt_children();

    // Падение в чужом коде (D3D11, Chromium) уносит процесс молча — ставим
    // перехватчик, который назовёт модуль и адрес.
    crash_report::install();

    // DirectComposition ВКЛЮЧЁН: непокрытая при ресайзе область окна
    // ПРОЗРАЧНА (просвечивает то, что за окном) — а не чёрные полосы blit
    // (жалоба юзера). Историческая болезнь dcomp «разворот со старым
    // контентом» вылечена синхронным кадром в WM_SIZE + resize_boost
    // (vendored events.rs): стенд 6/6 разворотов и внешние ресайзы чисты.
    // Аварийный выключатель: GPUI_DISABLE_DIRECT_COMPOSITION=true → blit
    // (там дополнительно выключено стирание фона — см. патчи
    // hbrBackground/WM_ERASEBKGND).
    // SAFETY: до Application::new, второго потока ещё нет.
    if std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none() {
        unsafe { std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "false") };
    }

    // Single-instance: живой инстанс (probe-порт отвечает) получает нашу
    // папку из argv («Open with KaminIDE») или просто фокус — а мы выходим.
    // СТРОГО до probe::host::start(), иначе начнём отбирать порт у живого.
    let launch_folder = win_integration::launch_folder();
    if win_integration::forward_to_running_instance(launch_folder.as_deref()) {
        return;
    }
    win_integration::set_launch_folder(launch_folder);
    // Self-heal контекстного меню Explorer: фоном, reg.exe небыстрый.
    std::thread::spawn(win_integration::register_context_menu);

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
    // Восстановить открытые файлы прошлой сессии (персист layout.json)
    for path in crate::layout_store::load_string_list("openFiles") {
        let _ = tx.try_send(host_link::ShellEvent::Ed(EdEvent::OpenFile(path)));
    }

    let app = Application::new().with_assets(assets::Assets);
    app.run(move |cx| {
        // Шрифты вложены в бинарь (plan/22: Bricolage задаёт всю метрику).
        // Сабсеты ТЕ ЖЕ, что шипит kamin-ide; name-таблица починена
        // (апстрим: family «96pt ExtraBold», STAT-синтез — см. память).
        cx.text_system()
            .add_fonts(vec![
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/bricolage-latin.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/bricolage-latin-ext.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/JetBrainsMono-Variable.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/JetBrainsMono-Italic-Variable.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/codicon.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/fa-solid-900.ttf").as_slice(),
                ),
            ])
            .expect("embed fonts");
        for name in cx.text_system().all_font_names() {
            let l = name.to_lowercase();
            if l.contains("awesome") || l.contains("codicon") {
                println!("font family loaded: {name}");
            }
        }

        gpui_component::init(cx);
        // Клавиши шелла (роутятся через key_context "Root").
        // «!webview»: при фокусе В СТРАНИЦЕ (CEF) биндинг молчит, и клавиша
        // уходит листенеру обёртки → в Chromium. gpui матчит биндинги РАНЬШЕ
        // key-листенеров, поэтому Enter/Escape/Ctrl+S иначе срабатывали как
        // действия шелла и до чата не доходили. `Not` в предикате проверяет
        // ВЕСЬ стек контекстов (`keymap/context.rs:284`) — глубина работает.
        // Палитра — глобальная, работает и из страницы.
        cx.bind_keys([
            gpui::KeyBinding::new("ctrl-shift-p", actions::TogglePalette, Some("Root")),
            gpui::KeyBinding::new("ctrl-p", actions::ToggleQuickOpen, Some("Root && !webview")),
            gpui::KeyBinding::new(
                "ctrl-shift-f",
                actions::ToggleFindInFiles,
                Some("Root && !webview"),
            ),
            gpui::KeyBinding::new(
                "ctrl-t",
                actions::ToggleWorkspaceSymbols,
                Some("Root && !webview"),
            ),
            gpui::KeyBinding::new("escape", actions::CloseOverlay, Some("Root && !webview")),
            // Enter обрабатывается ЗДЕСЬ, в главном окне: обработчики
            // оверлеев висят на элементах overlay-слоя и не получают
            // клавиатуру — фокус инпута живёт в main (ревью ц.7).
            gpui::KeyBinding::new("enter", actions::PressEnter, Some("Root && !webview")),
            gpui::KeyBinding::new("ctrl-s", actions::SaveFile, Some("Root && !webview")),
            gpui::KeyBinding::new(
                "ctrl-b",
                actions::ToggleSidebarAction,
                Some("Root && !webview"),
            ),
            gpui::KeyBinding::new("ctrl-z", actions::UndoFileOp, Some("Root && !webview")),
            gpui::KeyBinding::new("f2", actions::RenameActiveSession, Some("Root && !webview")),
        ]);
        // Тема gpui-component: полный сет + оверрайды палитры KaminIDE
        // (общая точка с Appearance-переключателем — theme_sync).
        // Сначала КЭШ contributed-темы (мгновенно, до загрузки расширения-
        // поставщика — бут раньше шёл на дефолтной теме до прихода реестра);
        // без кэша — builtin по персисту themeChoice.
        if !theme_sync::apply_cached_contributed(cx) {
            let choice = crate::layout_store::load_raw_key("themeChoice")
                .and_then(|v| v.as_str().map(str::to_string));
            let kind = if choice.as_deref() == Some("light") {
                kamin_theme::ThemeKind::Light
            } else {
                kamin_theme::ThemeKind::Dark
            };
            theme_sync::apply(kind, cx);
        }

        // Насос CEF: очередь событий и перерисовка по приходу кадра.
        web::start_pump(cx);

        cx.spawn(async move |cx| {
            let bounds = cx.update(|cx| {
                Bounds::centered(
                    None,
                    size(px(m::WINDOW_DEFAULT_WIDTH), px(m::WINDOW_DEFAULT_HEIGHT)),
                    cx,
                )
            })?;
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
                    Some(if v.get("maximized").and_then(|m| m.as_bool()) == Some(true) {
                        WindowBounds::Maximized(b)
                    } else {
                        WindowBounds::Windowed(b)
                    })
                })
                .unwrap_or(WindowBounds::Windowed(bounds));
            let options = WindowOptions {
                window_bounds: Some(window_bounds),
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
                window.focus(&fh);
                view_slot = Some(view.clone());
                // Заказы перерисовки на приход кадров CEF адресуем ЭТОМУ вью:
                // `cx.refresh()` глушил бы кэш панелей (`web/pump.rs`).
                web::set_repaint_target(view.downgrade());
                cx.new(|cx| {
                    let mut root = Root::new(view, window, cx);
                    // Фон окна рисует корневой канвас `RootView`: фон `Root`
                    // перекрывал бы его собственный слой.
                    root.transparent = true;
                    root
                })
            })?;

            // Ф6: отдельного overlay-окна больше нет — весь стек оверлеев
            // рисует слой главного окна (`state/overlay_stack.rs`). HWND
            // главного всё же публикуем: его ждут подложка web и тосты.
            if view_slot.is_some() {
                smol::Timer::after(std::time::Duration::from_millis(300)).await;
                #[cfg(windows)]
                {
                    let main_hwnd = cx.update(|_cx| overlay::main_hwnd_isize())?;
                    overlay::set_main_hwnd(main_hwnd);
                }
            }

            // Насос событий host_link → RootView (foreground)
            if let Some(view) = view_slot {
                while let Ok(event) = rx.recv().await {
                    if view
                        .update(cx, |v, cx| {
                            let needs_frame = v.apply(event, cx);
                            // Пропсы компонентов обновляем ЗДЕСЬ, а не в
                            // рендере: `notify` из фазы отрисовки не помечает
                            // окно грязным и нового кадра не заказывает
                            // (`vendor/gpui/src/window.rs:117-127`), то есть
                            // проверка изменений молча не работала.
                            v.sync_panels(cx);
                            // Холостые события (вывод терминала, доставка
                            // страницам) кадра не заказывают: раньше каждое
                            // будило полную пересборку RootView.
                            if needs_frame {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break; // окно закрыто
                    }
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
