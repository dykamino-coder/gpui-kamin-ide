//! Проба «accelerated-кадр копируется, но пустой».
//!
//! INC-2026-0073: кадры чата приходили общей текстурой, копия рисовалась
//! (счётчик отрисовок > 0, отказов открытия нет), а на экране был чёрный
//! прямоугольник. Ни один счётчик пути кадра этого не видит: всё «успешно».
//! Поэтому раз в [`EVERY`] читаем назад несколько точек СВОЕЙ копии кадра
//! (то, что реально рисуется) и считаем подряд идущие пустые пробы.
//!
//! Пусто — все точки с нулевым RGB. Наши браузеры создаются с непрозрачным
//! фоном (`background_color`), а страницы моста красят фон темой, поэтому
//! живой кадр так не выглядит. Чисто чёрная страница теоретически даст
//! ложное срабатывание; цена — переход в software (медленнее, но рисует),
//! поэтому порог — [`STREAK`] проб подряд, то есть несколько секунд.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Как часто пробовать кадр одного вью: чтение назад ждёт GPU.
const EVERY: Duration = Duration::from_secs(2);

/// Сколько пустых проб подряд считается сломанным путём.
const STREAK: u32 = 3;

#[derive(Default, Clone, Copy)]
struct State {
    last: Option<Instant>,
    streak: u32,
}

static STATES: LazyLock<Mutex<HashMap<String, State>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Пора ли пробовать вью (не чаще [`EVERY`]).
fn due(id: &str, now: Instant) -> bool {
    STATES.lock().is_ok_and(|mut m| {
        let s = m.entry(id.to_string()).or_default();
        let due = s.last.is_none_or(|t| now.duration_since(t) >= EVERY);
        if due {
            s.last = Some(now);
        }
        due
    })
}

/// Учесть результат пробы; вернуть длину серии пустых.
fn note(id: &str, blank: bool) -> u32 {
    STATES.lock().map_or(0, |mut m| {
        let s = m.entry(id.to_string()).or_default();
        s.streak = streak_after(s.streak, blank);
        s.streak
    })
}

/// Серия после очередной пробы: пустая продлевает, живая обнуляет.
fn streak_after(streak: u32, blank: bool) -> u32 {
    if blank { streak.saturating_add(1) } else { 0 }
}

/// Пусты ли все точки (BGRA/RGBA по 4 байта; альфа не важна).
fn all_blank(pixels: &[[u8; 4]]) -> bool {
    !pixels.is_empty() && pixels.iter().all(|p| p[..3] == [0, 0, 0])
}

/// Точки пробы внутри региона страницы: центр и четыре точки на четвертях.
fn sample_points(content: (i32, i32, i32, i32)) -> Vec<(u32, u32)> {
    let (x, y, w, h) = content;
    if w <= 0 || h <= 0 || x < 0 || y < 0 {
        return Vec::new();
    }
    [(1, 1), (1, 3), (3, 1), (3, 3), (2, 2)]
        .iter()
        .map(|(fx, fy)| ((x + w * fx / 4) as u32, (y + h * fy / 4) as u32))
        .collect()
}

/// Accelerated-кадры вью подряд пустые — путь сломан (`respawn::tick`).
pub(crate) fn is_blank(id: &str) -> bool {
    STATES
        .lock()
        .is_ok_and(|m| m.get(id).is_some_and(|s| s.streak >= STREAK))
}

/// Забыть вью (закрытие браузера).
pub(crate) fn forget_view(id: &str) {
    if let Ok(mut m) = STATES.lock() {
        m.remove(id);
    }
}

/// Проверить свою копию кадра (поток отрисовки, после `copy_into_own`).
#[cfg(windows)]
pub(crate) fn check(
    id: &str,
    device_raw: *mut std::ffi::c_void,
    context_raw: *mut std::ffi::c_void,
    own: &super::gpu_texture::GpuTexture,
    content: (i32, i32, i32, i32),
) {
    if !due(id, Instant::now()) {
        return;
    }
    let desc = own.desc();
    let points: Vec<_> = sample_points(content)
        .into_iter()
        .filter(|(x, y)| *x < desc.Width && *y < desc.Height)
        .collect();
    let Some(pixels) = read_points(device_raw, context_raw, own, &points) else {
        return;
    };
    if cfg!(debug_assertions) && std::env::var_os("KAMIN_DEBUG_FORCE_BLANK").is_some() {
        super::diag_sink::emit_line(format!("[cef] вью {id}: проба кадра {pixels:?}"));
    }
    let blank = all_blank(&pixels) || forced_blank(id);
    let streak = note(id, blank);
    if streak == STREAK {
        super::diag_sink::emit_line(format!(
            "[cef] вью {id}: accelerated-кадр пуст {STREAK} проб подряд"
        ));
    }
}

/// Стенд (только debug): `KAMIN_DEBUG_FORCE_BLANK=<id>` считает настоящую
/// пробу вью пустой — так проверяется путь «кадры есть, но пустые» на машине,
/// где GPU-кадры рисуются.
fn forced_blank(id: &str) -> bool {
    cfg!(debug_assertions)
        && std::env::var("KAMIN_DEBUG_FORCE_BLANK").is_ok_and(|v| v.split(',').any(|s| s == id))
}

/// Скопировать точки в маленькую staging-текстуру и прочитать их.
#[cfg(windows)]
fn read_points(
    device_raw: *mut std::ffi::c_void,
    context_raw: *mut std::ffi::c_void,
    own: &super::gpu_texture::GpuTexture,
    points: &[(u32, u32)],
) -> Option<Vec<[u8; 4]>> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext,
        ID3D11Texture2D,
    };
    use windows::core::Interface;

    if points.is_empty() || device_raw.is_null() || context_raw.is_null() {
        return None;
    }
    let src = own.desc();
    unsafe {
        // Borrowed: владеющие ссылки держит caller до конца отрисовки.
        let device = ID3D11Device::from_raw_borrowed(&device_raw)?;
        let context = ID3D11DeviceContext::from_raw_borrowed(&context_raw)?;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: points.len() as u32,
            Height: 1,
            MipLevels: 1,
            ArraySize: 1,
            Format: src.Format,
            SampleDesc: src.SampleDesc,
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .ok()?;
        let staging = staging?;
        let src_res: windows::Win32::Graphics::Direct3D11::ID3D11Resource =
            own.raw().cast().ok()?;
        for (i, (x, y)) in points.iter().enumerate() {
            let pick = D3D11_BOX {
                left: *x,
                top: *y,
                front: 0,
                right: x + 1,
                bottom: y + 1,
                back: 1,
            };
            context.CopySubresourceRegion(
                &staging,
                0,
                i as u32,
                0,
                0,
                &src_res,
                0,
                Some(&pick as *const _),
            );
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .ok()?;
        let bytes = std::slice::from_raw_parts(mapped.pData as *const u8, points.len() * 4);
        let pixels = bytes
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        context.Unmap(&staging, 0);
        Some(pixels)
    }
}

#[cfg(not(windows))]
pub(crate) fn check(
    _id: &str,
    _device_raw: *mut std::ffi::c_void,
    _context_raw: *mut std::ffi::c_void,
    _own: &super::gpu_texture::GpuTexture,
    _content: (i32, i32, i32, i32),
) {
}

#[cfg(test)]
#[path = "blank_probe_tests.rs"]
mod tests;
