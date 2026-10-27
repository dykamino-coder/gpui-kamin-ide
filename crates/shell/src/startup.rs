//! Настройка приложения шелла: instrumentation, шрифты, клавиши и начальная тема.

use crate::{actions, assets, theme_sync};

pub fn application() -> gpui::Application {
    gpui_platform::application()
        .with_instrumentation(instrumentation())
        .with_assets(assets::Assets)
}

// Шелл не зависит от HTML-движка: окружение читает собственный host запуска.
#[allow(
    clippy::disallowed_methods,
    reason = "shell launch reads process configuration"
)]
fn instrumentation() -> gpui::InstrumentationConfig {
    let debug_device = gpui::DebugDevice::Default;
    #[cfg(debug_assertions)]
    let debug_device = match std::env::var("KAMIN_DEBUG_D3D_DEVICE").as_deref() {
        Ok("warp") => gpui::DebugDevice::Warp,
        Ok("d3d11on12") => gpui::DebugDevice::D3D11On12,
        _ => debug_device,
    };
    gpui::InstrumentationConfig {
        prepaint: std::env::var("KAMIN_PREPAINT_PROF").as_deref() == Ok("1"),
        gpu_timing: std::env::var("KAMIN_DIAG_GPU_TIMING").as_deref() == Ok("1"),
        scale_factor: std::env::var("KAMIN_FORCE_SCALE")
            .ok()
            .and_then(|v| v.parse().ok()),
        debug_device,
    }
}

#[allow(clippy::print_stdout, reason = "startup reports loaded icon fonts")]
pub fn configure(cx: &mut gpui::App) {
    // Серые глифы, как до gpui-pre 0.3.8 (ClearType меняет вид всего текста
    // и не пишет альфу — в прозрачных слоях текст бы пропадал).
    cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
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
            std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/codicon.ttf").as_slice()),
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
}

/// Передать запуск существующему окну либо подготовить новое.
pub fn forward_launch() -> bool {
    // Single-instance: живой инстанс (probe-порт отвечает) получает нашу
    // папку из argv («Open with KaminIDE») или просто фокус — а мы выходим.
    // СТРОГО до probe::host::start(), иначе начнём отбирать порт у живого.
    let launch_folder = crate::win_integration::launch_folder();
    if crate::win_integration::forward_to_running_instance(launch_folder.as_deref()) {
        return true;
    }
    crate::win_integration::set_launch_folder(launch_folder);
    // Self-heal контекстного меню Explorer: фоном, reg.exe небыстрый.
    std::thread::spawn(crate::win_integration::register_context_menu);
    false
}

/// Отладочный запуск на рабочем столе владельца может не перехватывать фокус.
pub fn focus() -> bool {
    #[cfg(debug_assertions)]
    {
        !crate::native_acceptance::config::Config::from_env().no_activate
    }
    #[cfg(not(debug_assertions))]
    {
        true
    }
}
