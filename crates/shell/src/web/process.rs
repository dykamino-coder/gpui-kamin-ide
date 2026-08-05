//! Жизненный цикл CEF: дочерние процессы, инициализация, насос сообщений.

use std::sync::atomic::{AtomicBool, Ordering};

use cef::rc::*;
use cef::*;

/// Инициализирован ли CEF в ЭТОМ процессе (браузерном).
static LIVE: AtomicBool = AtomicBool::new(false);

cef::wrap_app! {
    struct KaminApp;
    impl App {
        /// Все наши страницы — одного происхождения (`kamin.localhost`), но
        /// Chromium заводил по renderer-процессу на вью: четыре процесса по
        /// 6–140 МБ на одно и то же. Ограничиваем пул — страницы моста делят
        /// один renderer (браузер настоящих сайтов при этом получает свой).
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let browser_process = process_type.map(|t| t.to_string().is_empty()).unwrap_or(true);
            if !browser_process {
                return;
            }
            if let Some(cl) = command_line {
                cl.append_switch_with_value(
                    Some(&"renderer-process-limit".into()),
                    Some(&"2".into()),
                );
                // Эмуляция RDP/WARP для стендов: без GPU-композитинга
                // Chromium шлёт кадры только `on_paint` — так проверяется
                // software-путь отрисовки.
                if std::env::var_os("KAMIN_CEF_FORCE_SW").is_some() {
                    cl.append_switch(Some(&"disable-gpu-compositing".into()));
                }
            }
        }
    }
}

/// Первая строка `main`. В дочернем процессе делает его работу и НЕ
/// возвращается — вызывает `std::process::exit`, чтобы ниже по `main` не
/// поднялись probe, сайдкар и окно.
///
/// В браузерном процессе просто возвращает управление.
pub fn exit_if_child_process() {
    // Рукопожатие версий нужно ЛЮБОМУ процессу, и браузерному тоже: без него
    // libcef обрывает работу с «CefApp called with invalid version -1»
    // (поймано запуском приложения, а не чтением кода).
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    // Признак дочернего процесса — ключ `--type=` (renderer, gpu-process,
    // utility…). Решаем ПО НЕМУ, а не по коду возврата `execute_process`:
    // проверено запуском `kaminide-gpui.exe --type=renderer` — CEF вернул
    // «не мой процесс» (-1), и дочерний процесс пошёл дальше по `main`:
    // занял probe-порт, поднял второй `kamin-host` и загрузил шрифты.
    let child = std::env::args().any(|a| a.starts_with("--type="));
    if !child {
        return;
    }
    let args = cef::args::Args::new();
    let mut app = KaminApp::new();
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    // Выходим в любом случае: в дочернем процессе делать больше нечего.
    std::process::exit(code.max(0));
}

/// Поднять CEF в браузерном процессе. Зовётся ПОСЛЕ
/// [`exit_if_child_process`], один раз.
pub fn init() {
    if LIVE.load(Ordering::Relaxed) {
        return;
    }
    let args = cef::args::Args::new();
    let mut app = KaminApp::new();
    // Дети CEF: диспетчер задач группирует по ИМЕНИ файла, а строку
    // подписывает по FileDescription. Чтобы дети были И в группе приложения,
    // И различимы, помощник `kaminide-web.exe` (описание «KaminIDE Web»)
    // копируется в `web\kaminide-gpui.exe` — имя склеивает группу, описание
    // подписывает. libcef.dll из подпапки не видно — главный каталог добавлен
    // в PATH (дети наследуют окружение), а ресурсы CEF ищутся от libcef.
    // Нет помощника (нестандартная сборка) — дети копией главного exe.
    // RDP-сессия (приоритетный сценарий) и форс-энв → software-профиль
    // браузеров сразу, без 12с ожидания ватчдога.
    #[cfg(windows)]
    {
        let remote = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_REMOTESESSION,
            )
        } != 0;
        if remote || std::env::var_os("KAMIN_CEF_FORCE_SW").is_some() {
            super::set_sw_mode(true);
        }
    }
    // Тайминги фаз (#76, холодный старт): init блокирует показ окна —
    // цифры в лог, чтобы знать, ЧТО именно распараллеливать.
    let t_init = std::time::Instant::now();
    let subprocess = prepare_subprocess();
    let t_sub = t_init.elapsed().as_millis();
    let cache_dir = {
        let dir = crate::host::paths::data_dirs().1.join("cef");
        std::fs::create_dir_all(&dir).ok().map(|_| dir)
    };
    let settings = Settings {
        browser_subprocess_path: subprocess
            .map(|p| p.to_string_lossy().to_string().as_str().into())
            .unwrap_or_default(),
        windowless_rendering_enabled: 1,
        // CEF КРУТИТ СВОЙ ЦИКЛ НА СВОЁМ ПОТОКЕ. Это не тонкая настройка, а
        // единственный работающий вариант: с ручным насосом
        // (`external_message_pump` + `do_message_loop_work`) вызов на 56-й
        // прокрутке НЕ ВОЗВРАЩАЛСЯ — CEF входил в свой вложенный цикл и
        // забирал наш поток целиком (измерено: «вход в насос 56» без «выхода»).
        // Отсюда росло всё: наши задачи не крутились, перерисовка не
        // заказывалась, и кадр обновлялся только когда Windows сама доставляла
        // ввод — «анимация идёт, только когда двигаю мышью», «текстура
        // застыла на ресайзе».
        //
        // Взамен: все вызовы в CEF — только через `input::on_browser`
        // (`post_task` на его поток), а браузер создаём АСИНХРОННО
        // (`browser_host_create_browser`): синхронное создание разрешено лишь
        // с потока CEF, которого у нас теперь нет.
        multi_threaded_message_loop: 1,
        no_sandbox: 1,
        // Постоянный кэш: БЕЗ него профиль CEF жил in-memory — GPU-шейдеры
        // компилировались с нуля каждый запуск (долгий старт), а параллельный
        // хвост прошлого инстанса дрался за неявный каталог (нестабильность
        // «иногда вовсе не прогружается», жалоба юзера).
        root_cache_path: cache_dir
            .as_deref()
            .map(|p| p.to_string_lossy().to_string().as_str().into())
            .unwrap_or_default(),
        cache_path: cache_dir
            .as_deref()
            .map(|p| p.to_string_lossy().to_string().as_str().into())
            .unwrap_or_default(),
        // DevTools вебвью по CDP (правило тестируемости: e2e должен видеть
        // консоль/DOM чата). Только dev: в packaged-раскладке порт не открыт.
        remote_debugging_port: if cfg!(debug_assertions) || std::env::var("KAMIN_CEF_CDP").is_ok() {
            9224
        } else {
            0
        },
        ..Default::default()
    };
    let t_cef = std::time::Instant::now();
    let ok = initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    eprintln!(
        "[cef] init: helper {t_sub} мс, cef_initialize {} мс, всего {} мс",
        t_cef.elapsed().as_millis(),
        t_init.elapsed().as_millis()
    );

    // Наш HTML (чат Bridge, вью расширений) грузится не из сети: отдаём его
    // обработчиком схемы на хост `kamin.localhost` — так же, как это делал
    // перехват запросов у WebView2.
    super::scheme::register();
    if ok == 1 {
        LIVE.store(true, Ordering::Relaxed);
    } else {
        eprintln!("[cef] initialize вернула {ok} — веб-содержимое будет недоступно");
    }
}

/// Подготовить exe дочерних процессов: копия помощника под именем главного.
fn prepare_subprocess() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let helper = dir.join("kaminide-web.exe");
    if !helper.exists() {
        eprintln!("[cef] kaminide-web.exe не найден — дети будут копией главного exe");
        return None;
    }
    let sub_dir = dir.join("web");
    let target = sub_dir.join("kaminide-gpui.exe");
    let fresh = match (helper.metadata(), target.metadata()) {
        (Ok(src), Ok(dst)) => src.len() != dst.len() || src.modified().ok() > dst.modified().ok(),
        _ => true,
    };
    if fresh {
        if std::fs::create_dir_all(&sub_dir).is_err() {
            return None;
        }
        // Копия может быть занята живым ребёнком прошлого запуска — тогда
        // оставляем старую: она того же происхождения.
        let _ = std::fs::remove_file(&target);
        if std::fs::copy(&helper, &target).is_err() && !target.exists() {
            return None;
        }
    }
    // libcef.dll и его свита лежат рядом с ГЛАВНЫМ exe; дети из подпапки
    // найдут их через PATH (наследуется).
    let path = std::env::var("PATH").unwrap_or_default();
    let dir_s = dir.to_string_lossy();
    if !path.split(';').any(|p| p == dir_s) {
        unsafe { std::env::set_var("PATH", format!("{dir_s};{path}")) };
    }
    Some(target)
}

/// Живёт ли CEF в этом процессе (браузеры создаём только тогда).
pub(crate) fn is_live() -> bool {
    LIVE.load(Ordering::Relaxed)
}

/// Погасить CEF при выходе.
pub fn shutdown() {
    if LIVE.swap(false, Ordering::Relaxed) {
        cef::shutdown();
    }
}
