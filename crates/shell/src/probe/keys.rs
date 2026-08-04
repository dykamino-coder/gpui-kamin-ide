//! Клавиши probe: коды VK и отправка нажатий.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::probe::input::sleep_ms;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CHAR};

/// Текст в сфокусированный инпут (WM_CHAR посимвольно).
/// gpui ставит input_handler только у АКТИВНОГО окна — шлём WM_ACTIVATE
/// (wparam=WA_ACTIVE) перед вводом, если окно в фоне.
/// Ввод в ЗАДАННОЕ окно. Инпуты оверлеев (палитра, quick-open, find-in-files,
/// prompt-модалка) живут в overlay-окне, и ввод в главное до них не доходил —
/// поэтому их поведение было непроверяемо кадром (ревью ц.28).
pub fn type_text_to(overlay: bool, text: &str) -> Result<(), String> {
    let hwnd = crate::probe::shot::find_window(overlay).ok_or("window not found")?;
    const WM_ACTIVATE: u32 = 0x0006;
    const WA_ACTIVE: usize = 1;
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_ACTIVATE, WPARAM(WA_ACTIVE), LPARAM(0));
    }
    sleep_ms(30);
    for ch in text.encode_utf16() {
        unsafe { PostMessageW(Some(hwnd), WM_CHAR, WPARAM(ch as usize), LPARAM(0)) }
            .map_err(|e| e.to_string())?;
        sleep_ms(10);
    }
    Ok(())
}
/// Виртуальная клавиша по имени (для стрелок/Enter/Escape/Tab — WM_CHAR их
/// не передаёт, а именно они двигают активную строку в оверлеях).
fn vk_of(name: &str) -> Option<usize> {
    Some(match name {
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "enter" => 0x0D,
        "escape" => 0x1B,
        "tab" => 0x09,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "backspace" => 0x08,
        "delete" => 0x2E,
        _ => return None,
    })
}
/// Нажатие клавиши. gpui на Windows читает клавиатуру через очередь
/// потока-владельца окна и синтетический `PostMessage(WM_KEYDOWN)` игнорирует
/// (проверено: ни Escape, ни стрелки не доходили), поэтому шлём настоящий
/// `SendInput`, предварительно подняв окно на передний план.
/// Клавиша в ЗАДАННОЕ окно. Esc/стрелки в оверлеях обрабатывает overlay-окно,
/// и `SetForegroundWindow` главного окна уводил фокус ввода мимо (ревью ц.28).
pub fn press_key_to(overlay: bool, name: &str, repeat: u32) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        SendInput, VIRTUAL_KEY,
    };
    let vk = vk_of(name).ok_or_else(|| format!("unknown key: {name}"))? as u16;
    let hwnd = crate::probe::shot::find_window(overlay).ok_or("window not found")?;
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
    }
    sleep_ms(60);
    // Стрелки/Home/End/PageUp/PageDown/Delete — расширенные клавиши
    let extended = matches!(vk, 0x25..=0x28 | 0x21..=0x24 | 0x2E);
    let mk = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                } | if extended {
                    KEYEVENTF_EXTENDEDKEY
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    for _ in 0..repeat.max(1) {
        let inputs = [mk(false), mk(true)];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != 2 {
            return Err(format!("SendInput sent {sent} of 2"));
        }
        sleep_ms(45);
    }
    Ok(())
}
