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

use super::d3d_device::Device;
use super::gpu_texture::GpuTexture;
use super::open_shared::warn_once;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::device_cache::sync_device;
pub(crate) use super::device_cache::{device, remember_device, with_current_device};

mod incoming;
pub(crate) use incoming::{bounding, get, put};

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
    /// `TILE_GENERATION` value of `tile`.
    generation: u64,
    /// Регион страницы, который сейчас ПОКАЗАН (w, h) — по нему решаем, не
    /// хуже ли новый кадр уже показанного.
    content: (i32, i32),
    /// Когда последний раз принимали кадр: страховка от вечного отказа.
    accepted_at: std::time::Instant,
    /// The page frame copied into `texture` last (`incoming::Incoming::frame`)
    /// and the region of it that is shown.
    frame: u64,
    crop: (i32, i32, i32, i32),
    /// The last call showed an older region instead of the new frame (a
    /// transitional resize frame): what is painted is not `crop`.
    held: bool,
}

/// What a sprite of the view samples: the same key means the same atlas tile
/// and region, so a newer copy into the texture needs no new scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameKey {
    tile: u64,
    size: (i32, i32),
    crop: (i32, i32, i32, i32),
}

/// Bumped whenever a view's atlas entry is registered or refilled.
static TILE_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The key of the frame `texture_for` returned last for view `id`.
pub(crate) fn shown_key(id: &str) -> Option<FrameKey> {
    let map = SLOTS.lock().ok()?;
    let slot = map.get(id).filter(|slot| !slot.held)?;
    Some(FrameKey {
        tile: slot.generation,
        size: (slot.width, slot.height),
        crop: slot.crop,
    })
}

static SLOTS: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Получить описатель текстуры для отрисовки. Зовётся с потока отрисовки.
pub(crate) fn texture_for(
    id: &str,
    window: &gpui::Window,
    want: (i32, i32),
) -> Option<gpui::ExternalTexture> {
    let device = unsafe { Device::from_owned(window.d3d_device_raw())? };
    sync_device(device.clone());
    let context = unsafe {
        super::d3d_device::take_owned::<windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext>(
            window.d3d_context_raw(),
        )?
    };
    let incoming = get(id)?;
    if !incoming.shared.belongs_to(device.raw()) {
        return None;
    }
    // This frame is in the own texture already (the pump shows page frames
    // without a window draw, `page_frame.rs`): a later paint copies nothing.
    let copied_already = SLOTS.lock().ok().and_then(|m| {
        m.get(id)
            .filter(|s| s.frame == incoming.frame && !s.held)
            .map(|s| s.tile.region(s.crop.0, s.crop.1, s.crop.2, s.crop.3))
    });
    if copied_already.is_some() {
        return copied_already;
    }
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
                if let Ok(mut map) = SLOTS.lock()
                    && let Some(slot) = map.get_mut(id)
                {
                    slot.held = true;
                }
                return Some(tile.region(0, 0, dw, dh));
            }
        }
    }
    // Копия под захватом keyed mutex: читать общую текстуру Chromium прямо в
    // своей отрисовке нельзя — так делает и эталонный `cef-mixer`.
    let context_raw = windows::core::Interface::as_raw(&context);
    let own = super::copy_frame::copy_into_own(id, device.raw(), context_raw, &incoming.shared);
    // Жалобы слоя проверки читаем ПОСЛЕ копирования: на нём он и ругается.
    super::d3d_log::drain(device.raw());
    let own = own?;
    // Пустую копию не видит ни один счётчик — проба (INC-2026-0073).
    super::blank_probe::check(id, device.raw(), context_raw, &own, incoming.content);

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
                    generation: TILE_GENERATION
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    content: (incoming.content.2, incoming.content.3),
                    accepted_at: std::time::Instant::now(),
                    frame: 0,
                    crop: (0, 0, 0, 0),
                    held: false,
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
    let crop = (cx_.min(tex_w - 1), cy_.min(tex_h - 1), cw, ch);
    let tile = match known {
        Some((texture, w, h, tile)) if texture.same_as(&own) && w == tex_w && h == tex_h => {
            // Текстура та же, но регион страницы мог смениться — обновим,
            // иначе фильтр переходных кадров сравнивал бы со старым.
            if let Ok(mut map) = SLOTS.lock()
                && let Some(slot) = map.get_mut(id)
            {
                slot.content = (cw, ch);
                slot.accepted_at = std::time::Instant::now();
            }
            tile
        }
        Some((_, _, _, tile)) => {
            let raw = own.extra_ref();
            let next = window.update_external_texture(&tile, raw, tex_w, tex_h)?;
            remember(own, &next, tex_w, tex_h);
            next
        }
        None => {
            let raw = own.extra_ref();
            let Some(tile) = window.register_external_texture(raw, tex_w, tex_h) else {
                warn_once("атлас не принял текстуру");
                return None;
            };
            remember(own, &tile, tex_w, tex_h);
            tile
        }
    };
    // Копия выше принесла в свою текстуру кадры, пришедшие с прошлой
    // отрисовки: окно перерисует только то, что в них меняли (`incoming`).
    let copied = super::copy_frame::take_copied(id);
    if copied && let Some((x, y, w, h)) = incoming::take_damage(id, incoming.frame) {
        window.damage_external_texture(&tile, x, y, w, h);
    }
    if let Ok(mut map) = SLOTS.lock()
        && let Some(slot) = map.get_mut(id)
    {
        slot.crop = crop;
        slot.held = false;
        if copied {
            slot.frame = incoming.frame;
        }
    }
    Some(tile.region(crop.0, crop.1, crop.2, crop.3))
}

/// Submit the copies queued on the window's context. A copy made outside a
/// window draw has no present behind it to submit it, and its `ReleaseSync`
/// would wait in our queue while the producer waits to acquire the texture
/// (CEF shutdown then took up to 74 s).
pub(crate) fn submit_copies(window: &gpui::Window) {
    let context = unsafe {
        super::d3d_device::take_owned::<windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext>(
            window.d3d_context_raw(),
        )
    };
    if let Some(context) = context {
        unsafe { context.Flush() };
    }
}

/// Плитки атласа закрытых вью: выбрасывать можно только с потока отрисовки,
/// поэтому копим и отдаём первому же кадру любого живого вью.
static DEAD_TILES: LazyLock<Mutex<Vec<gpui::ExternalTexture>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Забыть кадры и текстуры ОДНОГО вью (закрытие браузера).
pub(crate) fn forget_view(id: &str) {
    incoming::forget(Some(id));
    super::page_frame::forget(id);
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
pub(super) fn forget_device_objects() {
    if let Ok(mut dead) = DEAD_TILES.lock() {
        dead.clear();
    }
    super::popup::forget_device_objects();
    if let Ok(mut map) = SLOTS.lock() {
        map.clear();
    }
    super::open_shared::forget_all();
    incoming::forget(None);
    super::copy_frame::forget_all();
}
