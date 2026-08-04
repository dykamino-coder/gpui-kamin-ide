//! Кадры страниц как GPU-текстуры (`plan/101-cef.md`, шаг B).
//!
//! CEF отдаёт кадр общей текстурой D3D11 (`on_accelerated_paint`). Мы открываем
//! её в устройстве gpui, копируем в свою и кладём в атлас — дальше кадр рисует
//! тот же шейдер, что и обычные картинки, вместе со скруглением углов. Через
//! оперативную память кадр не проходит вовсе, поэтому ресайз догоняется без
//! растяжения.
//!
//! Все текстуры хранятся владеющими ссылками (`gpu_texture.rs`): ручной учёт
//! ссылок здесь уже приводил к падениям — то кадр брался из освобождённой
//! текстуры, то лишнее освобождение рушило устройство.

use super::gpu_texture::GpuTexture;
use super::open_shared::{open_by_handle, warn_once};
use std::collections::HashMap;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{LazyLock, Mutex};

/// Устройство D3D11 окна: кладём с потока отрисовки, читаем с потока CEF.
/// Устройство создано без `SINGLETHREADED`, поэтому открывать текстуры с чужого
/// потока можно.
static DEVICE: AtomicIsize = AtomicIsize::new(0);

/// Устройство окна (для попапов: они открывают буферы тем же устройством).
pub(crate) fn device() -> isize {
    DEVICE.load(Ordering::Relaxed)
}

/// Запомнить устройство окна (с потока отрисовки).
pub(crate) fn remember_device(window: &gpui::Window) {
    if DEVICE.load(Ordering::Relaxed) != 0 {
        return;
    }
    if let Some(dev) = window.d3d_device_raw() {
        DEVICE.store(dev as isize, Ordering::Relaxed);
    }
}

/// Последний кадр вью: открытая общая текстура Chromium и её размер.
#[derive(Clone)]
pub(crate) struct Incoming {
    pub(crate) shared: GpuTexture,
    /// Регион СТРАНИЦЫ внутри буфера (`content_rect`: x, y, w, h) — только
    /// его и рисуем: остальное прозрачно и читалось бы чёрным. Размер самого
    /// буфера не храним: он берётся у своей текстуры, иначе плитка атласа
    /// могла заявить больше, чем в текстуре есть.
    pub(crate) content: (i32, i32, i32, i32),
}

static INCOMING: LazyLock<Mutex<HashMap<String, Incoming>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Принять кадр (зовётся с потока CEF).
///
/// Номер буфера ОТКРЫВАЕМ ЗДЕСЬ: у CEF он живёт только на время вызова, и
/// попытка открыть его позже, с потока отрисовки, даёт `E_INVALIDARG` — панель
/// тогда остаётся пустой (наступал на это дважды). Копировать с этого потока
/// нельзя: контекст D3D11 не потокобезопасен, копия делается в отрисовке.
pub(crate) fn put(id: &str, handle: isize, width: i32, height: i32, content: (i32, i32, i32, i32)) {
    if handle == 0 || width <= 0 || height <= 0 {
        return;
    }
    let device = DEVICE.load(Ordering::Relaxed);
    if device == 0 {
        return;
    }
    let Some(shared) = open_by_handle(device, handle) else {
        return;
    };
    // Размер спрашиваем У САМОЙ текстуры: номера буферов Chromium
    // переиспользует, и кэш открытых мог отдать буфер ПРОШЛОГО размера —
    // копирование между разными размерами слой проверки D3D11 считает ошибкой.
    let desc = shared.desc();
    if desc.Width as i32 != width || desc.Height as i32 != height {
        // Открыли не тот буфер — Chromium уже пересоздал его под другой размер.
        super::diag::mismatch();
        return;
    }
    store(id, shared, content);
}

/// Запомнить последний кадр вью.
fn store(id: &str, shared: GpuTexture, content: (i32, i32, i32, i32)) {
    if let Ok(mut map) = INCOMING.lock() {
        map.insert(id.to_string(), Incoming { shared, content });
    }
}

/// Последний кадр вью (зовётся с потока отрисовки).
pub(crate) fn get(id: &str) -> Option<Incoming> {
    INCOMING.lock().ok()?.get(id).cloned()
}

/// Слот атласа на вью: заводится один раз и живёт до закрытия вью, дальше в нём
/// МЕНЯЕТСЯ содержимое. Так номер записи не освобождается и не достаётся
/// другому вью, а кадр в полёте не начинает рисовать чужую текстуру (из-за
/// этого приложение падало на быстром ресайзе).
///
/// Размер хранится не для красоты: адрес освобождённой текстуры система охотно
/// выдаёт следующей, и по одному совпадению адресов слот считался бы прежним —
/// а в атласе остался бы СТАРЫЙ размер, и выборка уходила за край.
struct Slot {
    texture: GpuTexture,
    width: i32,
    height: i32,
    tile: gpui::ExternalTexture,
    /// Регион страницы, который сейчас ПОКАЗАН (w, h) — по нему решаем, не
    /// хуже ли новый кадр уже показанного.
    content: (i32, i32),
    /// Когда последний раз принимали кадр: страховка от вечного отказа.
    accepted_at: std::time::Instant,
}

static SLOTS: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Получить описатель текстуры для отрисовки. Зовётся с потока отрисовки.
pub(crate) fn texture_for(
    id: &str,
    window: &gpui::Window,
    want: (i32, i32),
) -> Option<gpui::ExternalTexture> {
    let incoming = get(id)?;
    // ПЕРЕХОДНЫЕ кадры ресайза не показываем. При расширении по ширине
    // Chromium сначала присылает кадр с НЕДОРИСОВАННОЙ высотой (content_rect
    // короче панели), следующим — полный: на экране это «прыжок» страницы
    // (жалоба пользователя). Правило: если новый кадр КОРОЧЕ уже показанного
    // по оси, которую никто не менял (want совпадает с показанным), держим
    // прошлый кадр. Страховка от вечного отказа — 250 мс, дальше принимаем
    // что дают.
    {
        let meta = SLOTS.lock().ok().and_then(|m| {
            m.get(id)
                .map(|s| (s.content, s.accepted_at, s.tile.clone()))
        });
        if let Some(((dw, dh), accepted_at, tile)) = meta {
            let (_, _, cw, ch) = incoming.content;
            let axis_kept = |want_axis: i32, shown: i32| (want_axis - shown).abs() <= 2;
            let worse =
                (ch < dh - 2 && axis_kept(want.1, dh)) || (cw < dw - 2 && axis_kept(want.0, dw));
            if worse && accepted_at.elapsed().as_millis() < 250 {
                super::diag::cropped();
                return Some(tile.region(0, 0, dw, dh));
            }
        }
    }
    let device = window.d3d_device_raw()?;
    let context = window.d3d_context_raw()?;
    // Свернули и развернули окно — gpui пересоздал устройство D3D11, и все наши
    // текстуры от прежнего мертвы: панели оставались пустыми (поймано на живом
    // прогоне). Замечаем смену и заводим всё заново.
    let prev = DEVICE.swap(device as isize, Ordering::Relaxed);
    if prev != 0 && prev != device as isize {
        println!("[cef] устройство D3D11 пересоздано — обновляю текстуры");
        forget_device_objects();
    }
    // Копия под захватом keyed mutex: читать общую текстуру Chromium прямо в
    // своей отрисовке нельзя — так делает и эталонный `cef-mixer`.
    let own = super::copy_frame::copy_into_own(id, device, context, &incoming.shared);
    // Жалобы слоя проверки читаем ПОСЛЕ копирования: на нём он и ругается.
    super::d3d_log::drain(device);
    let own = own?;

    let known = SLOTS.lock().ok().and_then(|m| {
        m.get(id)
            .map(|s| (s.texture.clone(), s.width, s.height, s.tile.clone()))
    });
    let remember = |texture: GpuTexture, tile: &gpui::ExternalTexture, w: i32, h: i32| {
        if let Ok(mut map) = SLOTS.lock() {
            map.insert(
                id.to_string(),
                Slot {
                    texture,
                    width: w,
                    height: h,
                    tile: tile.clone(),
                    content: (incoming.content.2, incoming.content.3),
                    accepted_at: std::time::Instant::now(),
                },
            );
        }
    };
    // Размер берём У САМОЙ текстуры, а НЕ у последнего кадра. Когда копия
    // пропущена (производитель держал общую текстуру), `own` остаётся ПРОШЛОЙ,
    // меньшего размера, а плитка атласа заявляла бы новый размер — шейдер
    // сэмплил за край текстуры, и по краям панели шли ЧЁРНЫЕ ПОЛОСЫ
    // (замер стендом `check_web_drag`: кромка 0 при середине 228).
    let own_desc = own.desc();
    let (tex_w, tex_h) = (own_desc.Width as i32, own_desc.Height as i32);
    let (cx_, cy_, cw, ch) = incoming.content;
    // Рисуем ТОЛЬКО страницу (`content_rect`), без прозрачных полей буфера.
    let crop = move |tile: gpui::ExternalTexture| {
        tile.region(cx_.min(tex_w - 1), cy_.min(tex_h - 1), cw, ch)
    };
    match known {
        Some((texture, w, h, tile)) if texture.same_as(&own) && w == tex_w && h == tex_h => {
            // Текстура та же, но регион страницы мог смениться — обновим,
            // иначе фильтр переходных кадров сравнивал бы со старым.
            if let Ok(mut map) = SLOTS.lock()
                && let Some(slot) = map.get_mut(id)
            {
                slot.content = (cw, ch);
                slot.accepted_at = std::time::Instant::now();
            }
            Some(crop(tile))
        }
        Some((_, _, _, tile)) => {
            let raw = own.extra_ref();
            let next = window.update_external_texture(&tile, raw, tex_w, tex_h)?;
            remember(own, &next, tex_w, tex_h);
            Some(crop(next))
        }
        None => {
            let raw = own.extra_ref();
            let Some(tile) = window.register_external_texture(raw, tex_w, tex_h) else {
                warn_once("атлас не принял текстуру");
                return None;
            };
            remember(own, &tile, tex_w, tex_h);
            Some(crop(tile))
        }
    }
}

/// Плитки атласа закрытых вью: выбрасывать можно только с потока отрисовки,
/// поэтому копим и отдаём первому же кадру любого живого вью.
static DEAD_TILES: LazyLock<Mutex<Vec<gpui::ExternalTexture>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Забыть кадры и текстуры ОДНОГО вью (закрытие браузера).
pub(crate) fn forget_view(id: &str) {
    if let Ok(mut map) = INCOMING.lock() {
        map.remove(id);
    }
    if let Ok(mut map) = SLOTS.lock()
        && let Some(slot) = map.remove(id)
        && let Ok(mut dead) = DEAD_TILES.lock()
    {
        dead.push(slot.tile);
    }
}

/// Отложить чужую плитку атласа на выброс (попапы).
pub(crate) fn retire_tile(tile: gpui::ExternalTexture) {
    if let Ok(mut dead) = DEAD_TILES.lock() {
        dead.push(tile);
    }
}

/// Освободить плитки атласа закрытых вью (зовётся из отрисовки живого вью).
pub(crate) fn drain_dead(window: &gpui::Window) {
    let dead: Vec<_> = DEAD_TILES
        .lock()
        .map(|mut d| d.drain(..).collect())
        .unwrap_or_default();
    for tile in dead {
        window.drop_external_texture(&tile);
    }
}

/// Забыть всё, что привязано к прежнему устройству D3D11.
fn forget_device_objects() {
    if let Ok(mut map) = SLOTS.lock() {
        map.clear();
    }
    super::open_shared::forget_all();
    if let Ok(mut map) = INCOMING.lock() {
        map.clear();
    }
    super::copy_frame::forget_all();
}
