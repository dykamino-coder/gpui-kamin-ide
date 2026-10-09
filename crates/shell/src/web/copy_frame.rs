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
static RETRIES: LazyLock<Mutex<HashMap<String, super::keyed_access::RetryBudget>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static RECOVERY: LazyLock<Mutex<HashMap<String, super::keyed_access::RetryBudget>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn frame_arrived(id: &str) {
    if let Ok(mut retries) = RETRIES.lock() {
        retries.remove(id);
    }
}

/// Забыть свою текстуру одного вью (закрытие браузера).
pub(crate) fn forget_view(id: &str) {
    frame_arrived(id);
    if let Ok(mut recovery) = RECOVERY.lock() {
        recovery.remove(id);
    }
    if let Ok(mut map) = OWN.lock() {
        map.remove(id);
    }
}

/// Забыть свои текстуры: после пересоздания устройства D3D11 они мертвы.
pub(crate) fn forget_all() {
    if let Ok(mut recovery) = RECOVERY.lock() {
        recovery.clear();
    }
    if let Ok(mut retries) = RETRIES.lock() {
        retries.clear();
    }
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
        // Указатели устройства и контекста нам одолжены окном: оборачиваем на
        // время вызова и возвращаем как было.
        let device: ID3D11Device = Interface::from_raw(device_raw);
        let context: ID3D11DeviceContext = Interface::from_raw(context_raw);

        let own = (|| {
            // Прежняя своя текстура годится, только если описание сходится.
            // Ищем среди запомненных размеров — при осцилляции ресайза нужный
            // почти всегда уже создан.
            let fitting = OWN.lock().ok().and_then(|m| {
                m.get(id).and_then(|pool| {
                    pool.iter()
                        .find(|texture| {
                            let desc = texture.desc();
                            desc.Width == src_desc.Width
                                && desc.Height == src_desc.Height
                                && desc.Format == src_desc.Format
                        })
                        .cloned()
                })
            });
            let fresh = fitting.is_none();
            let own = match fitting.clone() {
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
                    GpuTexture::from_owned(created?.into_raw())?
                }
            };

            // Raw HRESULT: WAIT_TIMEOUT и WAIT_ABANDONED положительны, но
            // не дают права читать поверхность или вызывать ReleaseSync.
            let mutex: Option<IDXGIKeyedMutex> = shared.raw().cast().ok();
            let status = mutex.as_ref().map(|m| super::keyed_mutex::acquire(m));
            let access = super::keyed_access::with_access(
                status,
                || context.CopyResource(own.raw(), shared.raw()),
                || {
                    if let Some(m) = &mutex {
                        let _ = m.ReleaseSync(0);
                    }
                },
            );
            match access {
                super::keyed_access::Access::Retry => {
                    super::diag::busy();
                    if RETRIES
                        .lock()
                        .is_ok_and(|mut retries| retries.entry(id.to_string()).or_default().take())
                    {
                        super::repaint_requested();
                    }
                    // Только успешно скопированный прежний кадр. Новая own
                    // текстура до CopyResource не содержит валидных пикселей.
                    return fitting;
                }
                super::keyed_access::Access::Recreate => {
                    let view = id.strip_suffix("::popup").unwrap_or(id);
                    let mut budget = RECOVERY.lock().ok()?.get(view).cloned().unwrap_or_default();
                    let retry = budget.take();
                    forget_view(view);
                    super::shared_texture::forget_view(view);
                    super::popup::forget_view(view);
                    RECOVERY.lock().ok()?.insert(view.to_string(), budget);
                    // Поверхность принадлежит producer: простое повторное
                    // открытие abandoned handle её не восстанавливает.
                    if retry {
                        super::browsers::close(view);
                        super::repaint_requested();
                    }
                    return None;
                }
                super::keyed_access::Access::Copied => {
                    if let Ok(mut recovery) = RECOVERY.lock() {
                        recovery.remove(id.strip_suffix("::popup").unwrap_or(id));
                    }
                    frame_arrived(id);
                }
            }
            if fresh && let Ok(mut map) = OWN.lock() {
                let pool = map.entry(id.to_string()).or_default();
                pool.insert(0, own.clone());
                pool.truncate(OWN_KEEP);
            }
            Some(own)
        })();

        let _ = Interface::into_raw(device);
        let _ = Interface::into_raw(context);
        own
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
#[path = "copy_frame_tests.rs"]
mod tests;
