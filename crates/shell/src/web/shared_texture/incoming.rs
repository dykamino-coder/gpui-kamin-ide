//! Приём кадров Chromium с его потока: последний кадр вью и то, что в кадрах
//! менялось с прошлой отрисовки окна.
//!
//! Перерисованное Chromium (`dirty_rects` у `on_accelerated_paint`) окно
//! получает через `Window::damage_external_texture` и рисует заново только
//! это, а не всё окно. Так же поступает композитор Chromium с вложенной
//! поверхностью: её повреждение переносится в кадр родителя
//! (`SurfaceAggregator`). Кадров между двумя отрисовками окна бывает
//! несколько, показан будет последний — поэтому их прямоугольники копятся.

use super::super::gpu_texture::GpuTexture;
use super::super::open_shared::open_by_handle;
use super::device;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Прямоугольник в px буфера: x, y, w, h.
pub(crate) type Rect = (i32, i32, i32, i32);

/// Последний кадр вью: открытая общая текстура Chromium и её размер.
#[derive(Clone)]
pub(crate) struct Incoming {
    pub(crate) shared: GpuTexture,
    /// Регион СТРАНИЦЫ внутри буфера (`content_rect`: x, y, w, h) — только
    /// его и рисуем: остальное прозрачно и читалось бы чёрным. Размер самого
    /// буфера не храним: он берётся у своей текстуры, иначе плитка атласа
    /// могла заявить больше, чем в текстуре есть.
    pub(crate) content: Rect,
    /// Number of the frame, counted per view by `store`.
    pub(crate) frame: u64,
}

/// The latest frame of each view and the damage of the frames that are not
/// copied into the own texture yet, together under one lock: a copy takes
/// exactly the damage of the frames it may hold.
#[derive(Default)]
struct Views {
    latest: HashMap<String, Incoming>,
    damage: HashMap<String, Rect>,
}

static VIEWS: LazyLock<Mutex<Views>> = LazyLock::new(Default::default);

/// Принять кадр (зовётся с потока CEF).
///
/// Номер буфера ОТКРЫВАЕМ ЗДЕСЬ: у CEF он живёт только на время вызова, и
/// попытка открыть его позже, с потока отрисовки, даёт `E_INVALIDARG` — панель
/// тогда остаётся пустой (наступал на это дважды). Копировать с этого потока
/// нельзя: контекст D3D11 не потокобезопасен, копия делается в отрисовке.
/// `dirty` — перерисованное Chromium, `None` — неизвестно (весь буфер).
pub(crate) fn put(
    id: &str,
    handle: isize,
    width: i32,
    height: i32,
    content: Rect,
    dirty: Option<Rect>,
) {
    if handle == 0 || width <= 0 || height <= 0 {
        return;
    }
    let Some(device) = device() else { return };
    let Some(shared) = open_by_handle(device.raw() as isize, handle) else {
        return;
    };
    // Размер спрашиваем У САМОЙ текстуры: номера буферов Chromium
    // переиспользует, и кэш открытых мог отдать буфер ПРОШЛОГО размера —
    // копирование между разными размерами слой проверки D3D11 считает ошибкой.
    let desc = shared.desc();
    if desc.Width as i32 != width || desc.Height as i32 != height {
        // Открыли не тот буфер — Chromium уже пересоздал его под другой размер.
        super::super::diag::mismatch();
        return;
    }
    let dirty = dirty.unwrap_or((0, 0, width, height));
    let _ = super::with_current_device(&device, || store(id, shared, content, dirty));
}

/// Запомнить последний кадр вью и сложить его повреждение с непоказанными.
fn store(id: &str, shared: GpuTexture, content: Rect, dirty: Rect) {
    super::super::copy_frame::frame_arrived(id);
    if let Ok(mut views) = VIEWS.lock() {
        let frame = views.latest.get(id).map_or(1, |last| last.frame + 1);
        views.latest.insert(
            id.to_string(),
            Incoming {
                shared,
                content,
                frame,
            },
        );
        let next = views
            .damage
            .get(id)
            .map_or(dirty, |prev| union(*prev, dirty));
        views.damage.insert(id.to_string(), next);
    }
}

/// Последний кадр вью (зовётся с потока отрисовки).
pub(crate) fn get(id: &str) -> Option<Incoming> {
    VIEWS.lock().ok()?.latest.get(id).cloned()
}

/// The damage of the frames not copied yet, left in place.
pub(crate) fn pending_damage(id: &str) -> Option<Rect> {
    VIEWS.lock().ok()?.damage.get(id).copied()
}

/// The damage to report after `copied` (a frame from [`get`]) was copied
/// into the own texture. It is cleared only while no later frame arrived:
/// a later frame may already be in the copy (CEF shares its textures without
/// a keyed mutex), or may not, so its damage is reported again next time.
pub(crate) fn take_damage(id: &str, copied: u64) -> Option<Rect> {
    let mut views = VIEWS.lock().ok()?;
    let latest = views.latest.get(id).map_or(0, |last| last.frame);
    if latest == copied {
        views.damage.remove(id)
    } else {
        views.damage.get(id).copied()
    }
}

/// Забыть кадры вью (`None` — всех вью).
pub(crate) fn forget(id: Option<&str>) {
    if let Ok(mut views) = VIEWS.lock() {
        match id {
            Some(id) => {
                views.latest.remove(id);
                views.damage.remove(id);
            }
            None => *views = Views::default(),
        }
    }
}

/// Охватывающий прямоугольник кадров CEF (`None`, если список пуст).
pub(crate) fn bounding(rects: &[cef::Rect]) -> Option<Rect> {
    rects
        .iter()
        .filter(|r| r.width > 0 && r.height > 0)
        .map(|r| (r.x, r.y, r.width, r.height))
        .reduce(union)
}

fn union(a: Rect, b: Rect) -> Rect {
    let (l, t) = (a.0.min(b.0), a.1.min(b.1));
    let (r, btm) = ((a.0 + a.2).max(b.0 + b.2), (a.1 + a.3).max(b.1 + b.3));
    (l, t, r - l, btm - t)
}

#[cfg(test)]
mod tests {
    use super::{bounding, union};

    #[test]
    fn rects_unite_into_their_bounding_box() {
        assert_eq!(union((0, 0, 10, 10), (20, 5, 5, 20)), (0, 0, 25, 25));
        let rects = [
            cef::Rect {
                x: 4,
                y: 4,
                width: 2,
                height: 16,
            },
            cef::Rect {
                x: 100,
                y: 0,
                width: 0,
                height: 0,
            },
            cef::Rect {
                x: 10,
                y: 30,
                width: 8,
                height: 8,
            },
        ];
        assert_eq!(bounding(&rects), Some((4, 4, 14, 34)));
        assert_eq!(bounding(&[]), None);
    }
}
