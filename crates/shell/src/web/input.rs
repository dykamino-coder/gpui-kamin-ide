//! Ввод в веб-вью: мышь, колесо, фокус (`plan/101-cef.md`, Ф3).
//!
//! Координаты приходят от gpui в ЛОГИЧЕСКИХ пикселях окна; вычитаем начало
//! элемента и отдаём CEF как есть — вью мы тоже завели в логическом размере,
//! физический растр Chromium делает сам.

use cef::{ImplBrowser, ImplBrowserHost, MouseButtonType, MouseEvent};

/// Модификаторы CEF (`cef_event_flags_t`), нужные для кликов и колеса.
const FLAG_SHIFT: u32 = 1 << 1;
const FLAG_CTRL: u32 = 1 << 2;
const FLAG_ALT: u32 = 1 << 3;
const FLAG_LEFT_DOWN: u32 = 1 << 4;
const FLAG_MIDDLE_DOWN: u32 = 1 << 5;
const FLAG_RIGHT_DOWN: u32 = 1 << 6;

/// Пересчёт модификаторов gpui → CEF.
pub(crate) fn modifiers(m: &gpui::Modifiers, held: Option<gpui::MouseButton>) -> u32 {
    let mut flags = 0;
    if m.shift {
        flags |= FLAG_SHIFT;
    }
    if m.control {
        flags |= FLAG_CTRL;
    }
    if m.alt {
        flags |= FLAG_ALT;
    }
    match held {
        Some(gpui::MouseButton::Left) => flags |= FLAG_LEFT_DOWN,
        Some(gpui::MouseButton::Middle) => flags |= FLAG_MIDDLE_DOWN,
        Some(gpui::MouseButton::Right) => flags |= FLAG_RIGHT_DOWN,
        _ => {}
    }
    flags
}

fn button(b: gpui::MouseButton) -> Option<MouseButtonType> {
    Some(match b {
        gpui::MouseButton::Left => {
            MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_LEFT)
        }
        gpui::MouseButton::Middle => {
            MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_MIDDLE)
        }
        gpui::MouseButton::Right => {
            MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_RIGHT)
        }
        _ => return None,
    })
}

/// Выполнить действие над браузером НА ЕГО ПОТОКЕ.
///
/// Методы `BrowserHost` живут на UI-потоке CEF. Вызов их прямо из обработчика
/// gpui (другой поток) валил приложение без паники при первом же клике —
/// поэтому работу отдаём задачей через `post_task`.
/// Работа, которую надо выполнить на потоке CEF.
type Job = Box<dyn FnOnce() + Send>;

pub(crate) fn on_browser(id: &str, f: impl FnOnce(&cef::BrowserHost) + Send + 'static) {
    let id = id.to_string();
    run_on_cef(move || {
        if let Some(browser) = super::browsers::handle(&id)
            && let Some(host) = browser.host()
        {
            f(&host);
        }
    });
}

/// Выполнить произвольную работу на UI-потоке CEF (методы Browser/BrowserHost
/// живут только там).
pub(crate) fn run_on_cef(f: impl FnOnce() + Send + 'static) {
    use cef::{ImplTask, Task, ThreadId, WrapTask, rc::*};

    let job = std::sync::Mutex::new(Some(Box::new(f) as Box<dyn FnOnce() + Send>));

    cef::wrap_task! {
        struct HostTask {
            job: std::sync::Arc<std::sync::Mutex<Option<Job>>>,
        }
        impl Task {
            fn execute(&self) {
                if let Ok(mut slot) = self.job.lock()
                    && let Some(job) = slot.take()
                {
                    job();
                }
            }
        }
    }

    let mut task = HostTask::new(std::sync::Arc::new(job));
    cef::post_task(
        ThreadId::from(cef::sys::cef_thread_id_t::TID_UI),
        Some(&mut task),
    );
}

/// Клик: `up = false` — нажатие, `true` — отпускание.
pub(crate) fn click(
    id: &str,
    x: f32,
    y: f32,
    b: gpui::MouseButton,
    up: bool,
    clicks: i32,
    mods: u32,
) {
    let Some(kind) = button(b) else { return };
    let event = MouseEvent {
        x: x.round() as i32,
        y: y.round() as i32,
        modifiers: mods,
    };
    let clicks = clicks.max(1);
    on_browser(id, move |host| {
        host.send_mouse_click_event(Some(&event), kind, up as i32, clicks);
    });
}

/// Движение мыши; `leave = true` — курсор ушёл с вью.
///
/// Частота ограничена: gpui шлёт движение сотнями в секунду, а каждое
/// отправление — задача в поток CEF. Без ограничения приложение залипало,
/// стоило навести курсор на страницу (поймано на живом прогоне).
pub(crate) fn mouse_move(id: &str, x: f32, y: f32, mods: u32, leave: bool) {
    use std::sync::atomic::{AtomicI64, Ordering};
    static LAST_AT: AtomicI64 = AtomicI64::new(0);
    static LAST_POS: AtomicI64 = AtomicI64::new(0);
    if !leave {
        let pos = ((x.round() as i64) << 32) | (y.round() as i64 & 0xffff_ffff);
        if LAST_POS.swap(pos, Ordering::Relaxed) == pos {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // Не чаще ~60 раз в секунду: странице этого хватает с запасом.
        if now - LAST_AT.load(Ordering::Relaxed) < 16 {
            return;
        }
        LAST_AT.store(now, Ordering::Relaxed);
    }
    let event = MouseEvent {
        x: x.round() as i32,
        y: y.round() as i32,
        modifiers: mods,
    };
    on_browser(id, move |host| {
        host.send_mouse_move_event(Some(&event), leave as i32);
    });
}

/// Колесо. Знак и величина — как у gpui, CEF ждёт «пиксели прокрутки».
pub(crate) fn wheel(id: &str, x: f32, y: f32, dx: f32, dy: f32, mods: u32) {
    let event = MouseEvent {
        x: x.round() as i32,
        y: y.round() as i32,
        modifiers: mods,
    };
    let (dx, dy) = (dx.round() as i32, dy.round() as i32);
    on_browser(id, move |host| {
        host.send_mouse_wheel_event(Some(&event), dx, dy);
    });
}

/// Клавиша в страницу.
///
/// Каждой клавише — ПОЛНАЯ пара RAWKEYDOWN/KEYUP с кодом Windows: страницы
/// слушают keydown (xterm.js в консоли Bridge живёт только им), а Ctrl+C/V/X/A
/// внутри Chromium срабатывают тоже по keydown с кодом буквы. Печатные символы
/// вдобавок получают CHAR — он рождает keypress/input, и раскладка (кириллица)
/// приходит символом как есть. С зажатым Ctrl/Alt CHAR не шлём: символа там
/// нет, есть сочетание.
pub(crate) fn key(id: &str, keystroke: &gpui::Keystroke, up: bool) {
    use cef::sys::cef_key_event_type_t as T;
    use cef::{KeyEvent, KeyEventType};

    let mods = modifiers(&keystroke.modifiers, None);
    let vk = windows_key_code(&keystroke.key)
        .or_else(|| keystroke.key_char.as_deref().and_then(char_vk))
        .or_else(|| char_vk(&keystroke.key));
    if let Some(vk) = vk {
        let event = KeyEvent {
            size: std::mem::size_of::<cef::sys::_cef_key_event_t>(),
            type_: KeyEventType::from(if up {
                T::KEYEVENT_KEYUP
            } else {
                T::KEYEVENT_RAWKEYDOWN
            }),
            modifiers: mods,
            windows_key_code: vk,
            native_key_code: 0,
            is_system_key: 0,
            character: 0,
            unmodified_character: 0,
            focus_on_editable_field: 0,
        };
        on_browser(id, move |host| host.send_key_event(Some(&event)));
    }
    // Печатный символ: только на нажатии и без Ctrl/Alt. Пробел — «именованная»
    // клавиша gpui (key="space", key_char=None): без явной ветки CHAR не
    // отправлялся вовсе, и страница не получала keypress/input — «спейс не
    // доходит» в чат (жалоба юзера, обе машины).
    let typed_char = keystroke
        .key_char
        .as_ref()
        .and_then(|c| c.chars().next())
        .or_else(|| (keystroke.key == "space").then_some(' '))
        // Enter/Tab — именованные клавиши без key_char, а textarea вставляет
        // \n и \t только на CHAR-событии: без него Shift+Enter не давал новую
        // строку, Tab не доходил (тот же класс, что «спейс не доходит»).
        // preventDefault на keydown в странице подавляет вставку от CHAR —
        // обычный Enter-отправка в чате не ломается.
        .or_else(|| (keystroke.key == "enter").then_some('\r'))
        .or_else(|| (keystroke.key == "tab").then_some('\t'));
    if !up
        && !keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && let Some(ch) = typed_char
        && (!ch.is_control() || ch == '\r' || ch == '\t')
    {
        let code = ch as u32 as u16;
        let event = KeyEvent {
            size: std::mem::size_of::<cef::sys::_cef_key_event_t>(),
            type_: KeyEventType::from(T::KEYEVENT_CHAR),
            modifiers: mods,
            windows_key_code: code as i32,
            native_key_code: 0,
            is_system_key: 0,
            character: code,
            unmodified_character: code,
            focus_on_editable_field: 0,
        };
        on_browser(id, move |host| host.send_key_event(Some(&event)));
    }
}

/// Код Windows ПЕЧАТНОЙ клавиши по её символу: латиница и цифры — напрямую,
/// остальное (кириллица, знаки) спрашиваем у раскладки `VkKeyScanW`.
fn char_vk(text: &str) -> Option<i32> {
    let ch = text.chars().next()?;
    if text.chars().count() != 1 || ch.is_control() {
        return None;
    }
    if ch.is_ascii_alphanumeric() {
        return Some(ch.to_ascii_uppercase() as i32);
    }
    if ch == ' ' {
        return Some(0x20);
    }
    #[cfg(windows)]
    {
        let mut buf = [0u16; 2];
        let code = ch.encode_utf16(&mut buf)[0];
        // SAFETY: чистый запрос к текущей раскладке, без побочных эффектов.
        let scan = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW(code) };
        if scan == -1 {
            return None;
        }
        Some((scan & 0xFF) as i32)
    }
    #[cfg(not(windows))]
    None
}

/// Коды Windows для клавиш, которые страница не получит символом.
fn windows_key_code(key: &str) -> Option<i32> {
    Some(match key {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" => 0x0D,
        "escape" => 0x1B,
        "space" => 0x20,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "end" => 0x23,
        "home" => 0x24,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "insert" => 0x2D,
        "delete" => 0x2E,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        _ => return None,
    })
}

/// Фокус клавиатуры у вью.
pub(crate) fn set_focus(id: &str, focus: bool) {
    on_browser(id, move |host| host.set_focus(focus as i32));
}

/// Вью, которому последним отдали фокус клавиатуры.
static FOCUSED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Отдать фокус вью, забрав его у прежнего: без явного `set_focus(false)`
/// каретка в прежнем вью продолжала мигать и ловить ввод.
pub(crate) fn focus_view(id: &str) {
    let prev = FOCUSED
        .lock()
        .map(|mut f| f.replace(id.to_string()))
        .unwrap_or(None);
    if let Some(prev) = prev
        && prev != id
    {
        set_focus(&prev, false);
    }
    set_focus(id, true);
}

/// Клик мимо всех вью: забрать фокус у последнего (клик в наш интерфейс).
pub(crate) fn blur_all() {
    let prev = FOCUSED.lock().map(|mut f| f.take()).unwrap_or(None);
    if let Some(prev) = prev {
        set_focus(&prev, false);
    }
}
