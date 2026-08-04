//! Попап страницы в offscreen-режиме: дропдаун `<select>`, автодополнение.
//!
//! CEF рисует такой попап ОТДЕЛЬНОЙ текстурой (`PET_POPUP`) и сообщает его
//! прямоугольник в координатах вью (`on_popup_size`) — собрать картинку из
//! двух слоёв обязан хост, иначе список просто не виден. Путь текстуры тот же,
//! что у основного кадра: открыть общий буфер В МОМЕНТ колбэка, скопировать в
//! свою под keyed mutex на потоке отрисовки (`shared_texture.rs`).

use super::gpu_texture::GpuTexture;
use super::open_shared::open_by_handle;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Прямоугольник попапа: x, y, ширина, высота в CSS px вью.
type PopupRect = (i32, i32, i32, i32);

static RECTS: LazyLock<Mutex<HashMap<String, PopupRect>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Последняя текстура попапа (открытый общий буфер Chromium).
static INCOMING: LazyLock<Mutex<HashMap<String, GpuTexture>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Своя копия + плитка атласа, по вью.
struct Slot {
    texture: GpuTexture,
    width: i32,
    height: i32,
    tile: gpui::ExternalTexture,
}

static SLOTS: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Ключ своей текстуры в `copy_frame`: попап живёт рядом с основным кадром.
fn copy_key(id: &str) -> String {
    format!("{id}::popup")
}

/// Запомнить прямоугольник попапа (зовётся с потока CEF).
pub(crate) fn set_rect(id: &str, x: i32, y: i32, w: i32, h: i32) {
    if let Ok(mut map) = RECTS.lock() {
        map.insert(id.to_string(), (x, y, w, h));
    }
}

/// Принять текстуру попапа (зовётся с потока CEF, буфер открываем СЕЙЧАС —
/// позже номер уже недействителен, как и у основного кадра).
pub(crate) fn put(id: &str, handle: isize, width: i32, height: i32) {
    if handle == 0 || width <= 0 || height <= 0 {
        return;
    }
    let device = super::shared_texture::device();
    if device == 0 {
        return;
    }
    let Some(shared) = open_by_handle(device, handle) else {
        return;
    };
    let desc = shared.desc();
    if desc.Width as i32 != width || desc.Height as i32 != height {
        return; // Chromium уже пересоздал буфер под другой размер.
    }
    if let Ok(mut map) = INCOMING.lock() {
        map.insert(id.to_string(), shared);
    }
}

/// Software-кадры попапа (RDP-режим): картинка вместо общей текстуры.
static SW: LazyLock<Mutex<HashMap<String, std::sync::Arc<gpui::RenderImage>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Принять software-кадр попапа (поток CEF).
pub(crate) fn put_sw(id: &str, buffer: &[u8], width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let (w, h) = (width as u32, height as u32);
    let expect = (w as usize) * (h as usize) * 4;
    if buffer.len() < expect {
        return;
    }
    let Some(img) = image::RgbaImage::from_raw(w, h, buffer[..expect].to_vec()) else {
        return;
    };
    let image = std::sync::Arc::new(gpui::RenderImage::new([image::Frame::new(img)]));
    if let Ok(mut map) = SW.lock()
        && let Some(old) = map.insert(id.to_string(), image)
    {
        super::frames::retire(old);
    }
}

/// Software-кадр попапа и его прямоугольник (поток отрисовки).
pub(crate) fn sw_frame(id: &str) -> Option<(std::sync::Arc<gpui::RenderImage>, PopupRect)> {
    let rect = RECTS.lock().ok()?.get(id).copied()?;
    let image = SW.lock().ok()?.get(id).cloned()?;
    Some((image, rect))
}

/// Спрятать попап вью (список закрылся).
pub(crate) fn hide(id: &str) {
    if let Ok(mut map) = RECTS.lock() {
        map.remove(id);
    }
    if let Ok(mut map) = INCOMING.lock() {
        map.remove(id);
    }
    if let Ok(mut map) = SW.lock()
        && let Some(old) = map.remove(id)
    {
        super::frames::retire(old);
    }
    if let Ok(mut map) = SLOTS.lock()
        && let Some(slot) = map.remove(id)
    {
        super::shared_texture::retire_tile(slot.tile);
    }
    super::copy_frame::forget_view(&copy_key(id));
}

/// Забыть попап при закрытии вью.
pub(crate) fn forget_view(id: &str) {
    hide(id);
}

/// Текстура попапа и его прямоугольник в CSS px вью. Зовётся с потока
/// отрисовки, поверх основного кадра.
pub(crate) fn texture_for(
    id: &str,
    window: &gpui::Window,
) -> Option<(gpui::ExternalTexture, PopupRect)> {
    let rect = RECTS.lock().ok()?.get(id).copied()?;
    let incoming = INCOMING.lock().ok()?.get(id).cloned()?;
    let device = window.d3d_device_raw()?;
    let context = window.d3d_context_raw()?;
    let own = super::copy_frame::copy_into_own(&copy_key(id), device, context, &incoming)?;
    let desc = own.desc();
    let (w, h) = (desc.Width as i32, desc.Height as i32);
    let known = SLOTS.lock().ok().and_then(|m| {
        m.get(id)
            .map(|s| (s.texture.clone(), s.width, s.height, s.tile.clone()))
    });
    let remember = |texture: GpuTexture, tile: &gpui::ExternalTexture| {
        if let Ok(mut map) = SLOTS.lock() {
            map.insert(
                id.to_string(),
                Slot {
                    texture,
                    width: w,
                    height: h,
                    tile: tile.clone(),
                },
            );
        }
    };
    let tile = match known {
        Some((texture, kw, kh, tile)) if texture.same_as(&own) && kw == w && kh == h => tile,
        Some((_, _, _, tile)) => {
            let next = window.update_external_texture(&tile, own.extra_ref(), w, h)?;
            remember(own, &next);
            next
        }
        None => {
            let tile = window.register_external_texture(own.extra_ref(), w, h)?;
            remember(own, &tile);
            tile
        }
    };
    Some((tile, rect))
}
