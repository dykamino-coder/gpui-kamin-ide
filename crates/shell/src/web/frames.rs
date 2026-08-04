//! Кадры браузеров: последний кадр каждого вью и его размер.
//!
//! CEF зовёт `on_paint` со своего потока, кадр нужен потоку отрисовки —
//! поэтому общая карта под мьютексом. Держим ровно ОДИН последний кадр на вью:
//! промежуточные всё равно не увидит никто, а память тратить незачем.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use gpui::RenderImage;

/// Готовый к отрисовке кадр (BGRA — как раз то, что ждёт `RenderImage`).
pub(crate) struct Frame {
    pub(crate) image: Arc<RenderImage>,
}

static FRAMES: LazyLock<Mutex<HashMap<String, Frame>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Кадры, вытесненные новыми. Их текстуры живут в атласе gpui, а выбросить их
/// можно только с потока отрисовки — поэтому копим здесь и чистим на кадре.
/// Забыть кадр одного вью (закрытие браузера): картинка уходит в вытесненные,
/// её текстуру атлас отдаст на следующем кадре.
pub(crate) fn forget_view(id: &str) {
    if let Ok(mut map) = FRAMES.lock()
        && let Some(frame) = map.remove(id)
    {
        retire(frame.image);
    }
}

static RETIRED: LazyLock<Mutex<Vec<Arc<RenderImage>>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Отложить выброс текстуры до ближайшего кадра.
pub(crate) fn retire(image: Arc<RenderImage>) {
    if let Ok(mut q) = RETIRED.lock() {
        // Больше пары десятков за кадр не накопится, но страховка дешевле
        // разбирательства с распухшим атласом.
        if q.len() < 256 {
            q.push(image);
        }
    }
}

/// Кадры, отложенные на ОДИН такт: их текстуру мог держать кадр, который
/// прямо сейчас на экране. Выбрасывать сразу — значит иногда рвать картинку
/// (мигание при ресайзе, замечено на живом прогоне).
static COOLING: LazyLock<Mutex<Vec<Arc<RenderImage>>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Забрать кадры, отжившие СВОЙ ход: те, что были вытеснены не в этом такте,
/// а в прошлом. Зовётся с потока отрисовки.
pub(crate) fn take_retired() -> Vec<Arc<RenderImage>> {
    let fresh = RETIRED
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default();
    let Ok(mut cooling) = COOLING.lock() else {
        return Vec::new();
    };
    // Отдаём прошлую порцию, свежую кладём остывать на такт.
    std::mem::replace(&mut *cooling, fresh)
}

/// Последний software-кадр вью (поток отрисовки).
pub(crate) fn get(id: &str) -> Option<Arc<RenderImage>> {
    FRAMES.lock().ok()?.get(id).map(|f| f.image.clone())
}

/// Положить свежий кадр вью. Старый вытесняется и возвращается наружу, чтобы
/// вызывающий выбросил его текстуру из атласа gpui.
pub(crate) fn put(id: &str, buffer: &[u8], width: i32, height: i32) -> Option<Arc<RenderImage>> {
    let frame = image_from_bgra(buffer, width, height)?;
    let mut map = FRAMES.lock().ok()?;
    map.insert(id.to_string(), Frame { image: frame })
        .map(|old| old.image)
}

/// BGRA-буфер CEF → картинка gpui. `RenderImage` хранит именно BGRA, поэтому
/// перестановка каналов не нужна — только копия в свой буфер.
fn image_from_bgra(buffer: &[u8], width: i32, height: i32) -> Option<Arc<RenderImage>> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let (w, h) = (width as u32, height as u32);
    let expect = (w as usize) * (h as usize) * 4;
    if buffer.len() < expect {
        return None;
    }
    let img = image::RgbaImage::from_raw(w, h, buffer[..expect].to_vec())?;
    Some(Arc::new(RenderImage::new([image::Frame::new(img)])))
}
