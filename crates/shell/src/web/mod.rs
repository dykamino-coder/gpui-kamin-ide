//! Веб-содержимое на CEF: браузер рисует кадр, кадр рисуем мы (`plan/101-cef.md`).
//!
//! Здесь только жизненный цикл процесса и библиотеки. Сам элемент интерфейса
//! и приём кадров — в соседних модулях, по мере готовности фаз.
//!
//! **Главное про запуск.** CEF на Windows запускает свои дочерние процессы
//! (renderer, gpu, network) КОПИЕЙ нашего же exe с другими ключами командной
//! строки. Поэтому [`exit_if_child_process`] обязана стоять первой строкой
//! `main`: до probe, до сайдкара, до окна. Иначе каждый дочерний процесс
//! попытается открыть тот же TCP-порт, поднять `kamin-host` и нарисовать окно.

mod browsers;
mod context_menu;
mod copy_frame;
mod cursors;
mod d3d_log;
mod diag;
mod element;
mod frames;
mod gpu_mode;
mod gpu_texture;
mod input;
mod open_shared;
mod outbox;
mod popup;
mod process;
mod pump;
mod render_handler;
mod scheme;
mod shared_texture;
mod visibility;

pub use diag::{ctx_took, draw_took, drawn, rows_built};
pub use element::{ensure_focus_handles, web_view};
pub use process::{exit_if_child_process, init, shutdown};
pub use visibility::{mark_visible, mark_visible_union};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

/// Пришёл новый кадр — окну пора перерисоваться.
static DIRTY: AtomicBool = AtomicBool::new(false);

/// Зовётся из хендлера кадров (чужой поток).
///
/// Тут же просим окно перерисоваться. Без этого цепочка рвётся: кадр пришёл,
/// но пока пользователь не шевелит мышью, окно не перерисовывается, нового
/// запроса кадра не уходит — и страница навсегда остаётся прежнего размера
/// (поймано на живом прогоне: «не догнал кадр, ресайз так и остался»).
pub(crate) fn repaint_requested() {
    DIRTY.store(true, Ordering::Relaxed);
    // Будим цикл gpui: без этого окно не перерисовывается, пока не придёт
    // ввод — кадр есть, а на экране старый, «зависает, пока не кликну»
    // (поймано на живом прогоне). Канал будит ждущую задачу, а её пробуждение
    // само поднимает цикл сообщений окна.
    if let Some(tx) = WAKE.get() {
        let _ = tx.try_send(());
    }
}

/// Сигнал «пришёл кадр» для задачи перерисовки.
static WAKE: std::sync::OnceLock<smol::channel::Sender<()>> = std::sync::OnceLock::new();

/// Отдать каналу пробуждения отправителя (зовётся один раз при старте).
pub use pump::{set_repaint_target, start_pump};

/// Стендовая клавиша в вью (probe `webkey`): нажатие и отпускание разом.
pub(crate) fn probe_key(id: &str, key: &str, ch: Option<&str>, ctrl: bool, shift: bool, alt: bool) {
    let keystroke = gpui::Keystroke {
        modifiers: gpui::Modifiers {
            control: ctrl,
            alt,
            shift,
            platform: false,
            function: false,
        },
        key: key.to_string(),
        key_char: ch.map(|s| s.to_string()),
    };
    input::key(id, &keystroke, false);
    input::key(id, &keystroke, true);
}

/// Стендовый фокус клавиатуры в вью (probe `webfocus`).
pub(crate) fn probe_focus(id: &str) {
    input::focus_view(id);
}

/// SOFTWARE-режим CEF (RDP/WARP/нет GPU): браузеры создаются БЕЗ общих
/// текстур — с `shared_texture_enabled=1` без GPU-композитинга CEF не шлёт
/// кадры ВООБЩЕ (ни accelerated, ни on_paint; поймано стендом). Включается:
/// детектом RDP-сессии на старте, форсом `KAMIN_CEF_FORCE_SW`, либо
/// ватчдогом после первого пересоздания «вью без кадров».
static SW_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn sw_mode() -> bool {
    SW_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn set_sw_mode(on: bool) {
    if on && !SW_MODE.swap(on, std::sync::atomic::Ordering::Relaxed) {
        println!("[cef] software-режим отрисовки (RDP/нет GPU-композитинга)");
    }
}

/// Открыто ли сейчас контекст-меню страницы (флаг для Esc в обёртке вью).
static MENU_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_menu_open(open: bool) {
    MENU_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn menu_open() -> bool {
    MENU_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Исполнить команду контекст-меню страницы (клик по пункту в слое).
pub(crate) fn execute_menu_command(id: &str, cmd: i32) {
    context_menu::execute(id, cmd);
}

/// Клик мимо всех веб-вью: забрать у страницы фокус клавиатуры, иначе каретка
/// в ней продолжает мигать и ловить ввод, когда пользователь ушёл в наш UI.
pub(crate) fn blur_if_outside(x: f32, y: f32) {
    if !element::hit_any_view(x, y) {
        input::blur_all();
    }
}

pub fn set_wake(tx: smol::channel::Sender<()>) {
    let _ = WAKE.set(tx);
}

// Просить перерисовку через `InvalidateRect` НЕЛЬЗЯ: gpui рисует через
// DirectComposition и не подтверждает область перерисовки, поэтому Windows
// шлёт `WM_PAINT` без конца — процесс съедал целое ядро на простое (замер:
// 5.03 с CPU за 5 с, без CEF — 0). Перерисовку заказывает задача-насос
// в `main.rs` через `cx.refresh()`.

/// Забрать и сбросить признак «есть новый кадр» (поток UI).
pub fn take_repaint_request() -> bool {
    DIRTY.swap(false, Ordering::Relaxed)
}

/// Когда вью пропало из раскладки (для отсрочки выгрузки).
static HIDDEN_AT: LazyLock<Mutex<std::collections::HashMap<String, std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Сколько скрытое вью живёт до выгрузки. Достаточно, чтобы щёлканье по тулам
/// туда-обратно не убивало renderer, и мало, чтобы память возвращалась.
const HIDDEN_TTL: std::time::Duration = std::time::Duration::from_secs(20);

/// Выгрузить браузеры вью, скрытых дольше [`HIDDEN_TTL`]: их renderer-процессы
/// держат десятки МБ на состояние, которое всё равно пересобирается при
/// возврате (страница поднимается из HTML-стора и сама дозапрашивает данные —
/// тот же путь, что при первом открытии). Зовётся раз в секунду из насоса.
/// Ватчдог «вью без кадров»: браузер заказан, вью ВИДИМ, а ни одного кадра
/// так и нет — закрыть; следующая отрисовка поднимет его заново
/// (`ensure_html_view`/`open`). Лечит редкие простои CEF на старте
/// («иногда вовсе не прогружается», жалоба юзера). Две попытки на вью —
/// дальше причина точно не в браузере, не циклим.
pub(crate) fn respawn_stalled() {
    use std::collections::HashMap;
    static RESPAWNS: LazyLock<Mutex<HashMap<String, u32>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let visible = visibility::visible_set();
    for id in browsers::ids() {
        if !visible.contains(&id) {
            continue;
        }
        let stalled = shared_texture::get(&id).is_none()
            && frames::get(&id).is_none() // software-кадры (RDP) = вью живой
            && browsers::opened_ago(&id).is_some_and(|d| d.as_secs() >= 12);
        if !stalled {
            continue;
        }
        let attempts = RESPAWNS
            .lock()
            .map(|mut m| {
                let e = m.entry(id.clone()).or_insert(0);
                *e += 1;
                *e
            })
            .unwrap_or(99);
        if attempts > 2 {
            continue;
        }
        println!("[cef] вью {id} без кадров 12с — пересоздаю (попытка {attempts})");
        // Кадры не пришли ни разу — вероятно, GPU-композитинга нет (RDP):
        // пересоздаём уже в software-профиле.
        set_sw_mode(true);
        browsers::close(&id);
        repaint_requested();
    }
}

/// Чат никогда не выгружаем: его холодный подъём — самый дорогой (реплей
/// всей переписки) и самый заметный («чат перезагрузился»). Память чата
/// оправдана всегда — это главная поверхность приложения.
const NEVER_REAP: &[&str] = &["claudeBridgeChat"];

pub(crate) fn reap_hidden() {
    let visible = visibility::visible_set();
    let now = std::time::Instant::now();
    for id in browsers::ids() {
        if NEVER_REAP.contains(&id.as_str()) {
            continue;
        }
        if visible.contains(&id) {
            if let Ok(mut m) = HIDDEN_AT.lock() {
                m.remove(&id);
            }
            continue;
        }
        let expired = {
            let Ok(mut m) = HIDDEN_AT.lock() else {
                continue;
            };
            now.duration_since(*m.entry(id.clone()).or_insert(now)) >= HIDDEN_TTL
        };
        if expired {
            println!(
                "[cef] вью {id} скрыт дольше {}s — выгружаю renderer",
                HIDDEN_TTL.as_secs()
            );
            if let Ok(mut m) = HIDDEN_AT.lock() {
                m.remove(&id);
            }
            // Renderer уходит — exthost обязан узнать, иначе копит посты и HTML
            // для несуществующего iframe. Скрытым вью его уже уведомили
            // (`visibility::sleep_hidden`), но выгрузка — отдельное событие:
            // вью могло пропасть из раскладки минуя обычный путь.
            visibility::notify_view_state(&id, false);
            browsers::close(&id);
            shared_texture::forget_view(&id);
            copy_frame::forget_view(&id);
            frames::forget_view(&id);
            popup::forget_view(&id);
            cursors::forget_view(&id);
        }
    }
}

/// Адрес для обкатки Ф2: страница видна целиком и хорошо заметна на глаз.
pub const DEMO_URL: &str = "https://example.com";

/// Выбросить текстуры кадров, вытесненных новыми. Зовётся с потока UI между
/// кадрами — внутри отрисовки атлас трогать нельзя.
pub fn flush_retired(cx: &mut gpui::App) {
    for old in frames::take_retired() {
        cx.drop_image(old, None);
    }
}

/// Открыть вью с адресом. Идемпотентно: второй раз ничего не делает.
/// Живы ли браузеры CEF. Флага больше нет: CEF — единственный путь.
pub fn enabled() -> bool {
    process::is_live()
}

/// Поднять вью моста и отдать ему свежий HTML.
///
/// Зовётся из отрисовки: браузер заводится один раз, а HTML переливается,
/// когда стор моста его обновил — так несколько вью одной сессии показывают
/// одно и то же, а переход на другую сессию просто меняет содержимое.
pub fn ensure_html_view(view_id: &str) {
    let id: &'static str = crate::ui::webview_body::static_view_id(view_id);
    // Грузим НЕ `data:`-ссылкой: страницы моста бывают в мегабайты, а ссылка
    // раздувает их и ломает относительные пути. HTML отдаёт наш обработчик
    // схемы по адресу `kamin.localhost/<id>` (`web/scheme.rs`).
    let url = crate::ui::chat_webview::view_url(id);
    if !browsers::exists(id) {
        // Поднимаем СРАЗУ в последнем известном размере вью и с текущим
        // scale: жёсткие 800×600@1.0 гарантировали, что первые кадры придут
        // чужого размера — и `element` рисовал их растянутыми (`big_mismatch`).
        // Это же било по возврату вью после `reap_hidden` (RDP: «недорастянутая
        // текстура»). Нет прошлого размера (самый первый показ) — прежний
        // дефолт, его тут же поправит первый prepaint.
        let (w, h) = element::last_size(id).unwrap_or((800.0, 600.0));
        let scale = browsers::scale_of(id);
        let scale = if scale > 0.1 { scale } else { 1.0 };
        open(id, &url, w.round() as i32, h.round() as i32, scale);
        // Вью поднялось (первый показ или возврат после reap) — сообщаем
        // exthost, иначе он считает его скрытым и не шлёт посты.
        visibility::notify_view_state(id, true);
        return;
    }
    // Стор обновился — перечитываем ту же страницу: обработчик схемы отдаст
    // свежий HTML. Так несколько вью одной сессии показывают одно и то же, а
    // переход на другую сессию просто меняет содержимое.
    if html_changed(id) {
        browsers::reload(id);
    }
}

/// Менялась ли страница вью с прошлого раза. Сравниваем НОМЕР ВЕРСИИ из стора:
/// сравнение содержимого заставляло каждый кадр копировать все страницы моста.
fn html_changed(id: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    static SEEN: LazyLock<Mutex<HashMap<String, u64>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let Some(rev) = crate::ui::chat_webview::html_rev(id) else {
        return false;
    };
    let Ok(mut seen) = SEEN.lock() else {
        return false;
    };
    if seen.get(id) == Some(&rev) {
        return false;
    }
    seen.insert(id.to_string(), rev);
    true
}

/// Показать в вью нашу страницу (HTML моста).
/// Отдать странице пачку сообщений расширения.
///
/// В скрипт кладём только НОМЕР пачки: тело страница забирает запросом
/// (`web/outbox.rs` объясняет, почему не одним куском).
pub fn deliver(view_id: &str, json: String) {
    let id = crate::ui::webview_body::static_view_id(view_id);
    if !browsers::exists(id) {
        return;
    }
    outbox::push(id, json);
    // Страница заберёт ВСЁ накопленное для себя: пачки идут быстрее, чем она
    // успевает приходить, и нумеровать их незачем.
    //
    // КОАЛЕСИНГ: раньше `execute_script` шёл на КАЖДУЮ пачку — на реплей-
    // шторме это сотни eval'ов в секунду, каждый провоцирует у страницы
    // перевёрстку и новый кадр (на RDP это прямой источник фризов). Ставим
    // флаг «есть непрочитанное» и дёргаем pull ОДИН раз за тик насоса.
    if let Ok(mut set) = PULL_PENDING.lock() {
        set.insert(id.to_string());
    }
}

/// Вью с непрочитанным outbox — pull для них шлётся раз за тик (`pump`).
static PULL_PENDING: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Разослать отложенные `__kaminPull()` — один вызов на вью за тик.
pub(crate) fn flush_pending_pulls() {
    let ids: Vec<String> = match PULL_PENDING.lock() {
        Ok(mut set) => set.drain().collect(),
        Err(_) => return,
    };
    for id in ids {
        if browsers::exists(&id) {
            browsers::execute_script(&id, "window.__kaminPull()");
        }
    }
}

/// Шаг назад по истории вью.
pub fn go_back(view_id: &str) {
    with_view(view_id, browsers::go_back);
}

/// Шаг вперёд по истории вью.
pub fn go_forward(view_id: &str) {
    with_view(view_id, browsers::go_forward);
}

/// Перечитать страницу вью.
pub fn reload(view_id: &str) {
    with_view(view_id, browsers::reload);
}

/// Открыть инструменты разработчика для вью.
pub fn open_devtools(view_id: &str) {
    with_view(view_id, browsers::open_devtools);
}

/// Выполнить действие над вью, если он уже поднят.
fn with_view(view_id: &str, action: fn(&str)) {
    let id = crate::ui::webview_body::static_view_id(view_id);
    if browsers::exists(id) {
        action(id);
    }
}

/// Открыть в существующем вью другой адрес. Пусто, если вью ещё нет.
pub fn navigate(view_id: &str, url: &str) {
    let id = crate::ui::webview_body::static_view_id(view_id);
    if browsers::exists(id) {
        browsers::navigate(id, url);
    }
}

/// Выполнить JS в странице вью. Пусто, если вью нет — так расширение может
/// слать сообщения, не зная, поднят ли уже браузер.
pub fn execute_script(view_id: &str, js: &str) {
    let id = crate::ui::webview_body::static_view_id(view_id);
    if browsers::exists(id) {
        browsers::execute_script(id, js);
    }
}

/// Все живые вью (для рассылок вроде смены темы `webview_theme::push_live`).
pub fn all_view_ids() -> Vec<String> {
    browsers::ids()
}

pub fn open(id: &str, url: &str, width: i32, height: i32, scale: f32) {
    browsers::open(id, url, width, height, scale);
}
