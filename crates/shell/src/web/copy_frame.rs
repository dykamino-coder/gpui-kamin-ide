//! Копия кадра CEF в НАШУ текстуру — под захватом keyed mutex.
//!
//! Так делает эталонный `cef-mixer`, и это единственный правильный способ:
//! общая текстура Chromium защищена `IDXGIKeyedMutex`, и читать её напрямую из
//! своей отрисовки нельзя. Без захвата драйвер тормозит и подвисает — то самое
//! «залипло при быстром ресайзе».
//!
//! Копия делается на ГПУ (`CopyResource`) и на потоке отрисовки: контекст D3D11
//! не потокобезопасен, а на этом потоке им и так пользуется gpui.

use super::gpu_texture::GpuTexture;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Наши приватные текстуры по вью: НЕСКОЛЬКО последних размеров, а не одна.
///
/// При ресайзе кадры идут вперемешку — старый размер (перевёрстка ещё не
/// доехала) и новый; с одной текстурой каждый такой перескок = `CreateTexture2D`
/// заново, а это десятки мс на RDP и ровно тот случай, когда панель «залипает».
/// Держим до `OWN_KEEP` штук и выбираем подходящую по описанию.
static OWN: LazyLock<Mutex<HashMap<String, Vec<GpuTexture>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Сколько размеров помнить на вью. Двух хватает на «туда-обратно» при драге,
/// третий — запас на промежуточный размер; больше держать незачем: каждая
/// текстура это w×h×4 байт видеопамяти.
const OWN_KEEP: usize = 3;

/// Забыть свою текстуру одного вью (закрытие браузера).
pub(crate) fn forget_view(id: &str) {
    if let Ok(mut map) = OWN.lock() {
        map.remove(id);
    }
}

/// Забыть свои текстуры: после пересоздания устройства D3D11 они мертвы.
pub(crate) fn forget_all() {
    if let Ok(mut map) = OWN.lock() {
        map.clear();
    }
}

/// Скопировать кадр CEF в свою текстуру и вернуть её.
///
/// `shared` — открытая текстура Chromium, `device`/`context` — наши.
#[cfg(windows)]
pub(crate) fn copy_into_own(
    id: &str,
    device_raw: *mut std::ffi::c_void,
    context_raw: *mut std::ffi::c_void,
    shared: &GpuTexture,
) -> Option<GpuTexture> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11DeviceContext,
        ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIKeyedMutex;
    use windows::core::Interface;

    if device_raw.is_null() || context_raw.is_null() {
        return None;
    }
    // Описание источника: копирование требует полного совпадения размера,
    // формата и числа уровней, поэтому свою текстуру строим по нему.
    let src_desc = shared.desc();

    unsafe {
        // Borrowed pointers: owning snapshots живут у caller до конца копии.
        let device = ID3D11Device::from_raw_borrowed(&device_raw)?;
        let context = ID3D11DeviceContext::from_raw_borrowed(&context_raw)?;

        let owner = context.GetDevice().ok()?;
        if owner.as_raw() != device_raw || !shared.belongs_to(device_raw) {
            return None;
        }
        (|| {
            // Прежняя своя текстура годится, только если описание сходится.
            // Ищем среди запомненных размеров — при осцилляции ресайза нужный
            // почти всегда уже создан.
            let fitting = OWN.lock().ok().and_then(|m| {
                m.get(id).and_then(|pool| {
                    pool.iter()
                        .find(|texture| {
                            let desc = texture.desc();
                            texture.belongs_to(device_raw)
                                && desc.Width == src_desc.Width
                                && desc.Height == src_desc.Height
                                && desc.Format == src_desc.Format
                        })
                        .cloned()
                })
            });
            let own = match fitting {
                Some(texture) => texture,
                None => {
                    let mut desc = src_desc;
                    // Своя текстура обычная: не общая, только для чтения шейдером.
                    desc.Usage = D3D11_USAGE_DEFAULT;
                    desc.BindFlags = D3D11_BIND_SHADER_RESOURCE.0 as u32;
                    desc.CPUAccessFlags = 0;
                    desc.MiscFlags = 0;
                    let mut created: Option<ID3D11Texture2D> = None;
                    device
                        .CreateTexture2D(&desc, None, Some(&mut created))
                        .ok()?;
                    let texture = GpuTexture::from_owned(created?.into_raw())?;
                    if let Ok(mut m) = OWN.lock() {
                        // Свежую — в начало: она же самая нужная, и при
                        // переполнении вылетает самый давний размер.
                        let pool = m.entry(id.to_string()).or_default();
                        pool.insert(0, texture.clone());
                        pool.truncate(OWN_KEEP);
                    }
                    texture
                }
            };

            // Захват общей текстуры. Если ключа нет — копируем как есть.
            let mutex: Option<IDXGIKeyedMutex> = shared.raw().cast().ok();
            if let Some(m) = &mutex
                && m.AcquireSync(0, 16).is_err()
            {
                // Производитель держит кадр. Ждать нельзя — встанет поток
                // отрисовки; но и молчать нельзя: кадр остался бы лежать
                // непоказанным до следующего события мыши («анимация идёт
                // только когда двигаю мышь»). Заказываем ещё проход.
                super::diag::busy();
                super::repaint_requested();
                return Some(own);
            }
            context.CopyResource(own.raw(), shared.raw());
            if let Some(m) = &mutex {
                let _ = m.ReleaseSync(0);
            }
            Some(own)
        })()
    }
}

#[cfg(not(windows))]
pub(crate) fn copy_into_own(
    _id: &str,
    _device_raw: *mut std::ffi::c_void,
    _context_raw: *mut std::ffi::c_void,
    _shared: &GpuTexture,
) -> Option<GpuTexture> {
    None
}

#[cfg(all(test, windows))]
#[path = "device_copy_tests.rs"]
mod device_tests;
