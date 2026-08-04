//! Владеющая ссылка на текстуру D3D11, которую можно передавать между потоками.
//!
//! Раньше текстуры хранились сырыми указателями, а счётчик ссылок велся руками
//! — и каждая ошибка в этом учёте оборачивалась падением: то кадр рисовался из
//! уже освобождённой текстуры, то устройство рушилось от лишнего освобождения.
//! Здесь ссылку держит обычный объект: язык освобождает её сам, ровно один раз.
//!
//! Передача между потоками безопасна: устройство создано без
//! `D3D11_CREATE_DEVICE_SINGLETHREADED`, а сама текстура — это просто ресурс,
//! который открывает поток CEF, а рисует поток отрисовки.

#[cfg(windows)]
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D};

/// Текстура вместе с правом её отпустить.
#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct GpuTexture(ID3D11Texture2D);

#[cfg(windows)]
// SAFETY: см. пояснение в шапке файла — ресурс не привязан к потоку.
unsafe impl Send for GpuTexture {}
#[cfg(windows)]
unsafe impl Sync for GpuTexture {}

#[cfg(windows)]
impl GpuTexture {
    /// Взять во владение указатель, который нам уже отдали со счётчиком.
    pub(crate) unsafe fn from_owned(raw: *mut std::ffi::c_void) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Some(Self(unsafe { windows::core::Interface::from_raw(raw) }))
    }

    /// Описание текстуры: размер и формат.
    pub(crate) fn desc(&self) -> D3D11_TEXTURE2D_DESC {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { self.0.GetDesc(&mut desc) };
        desc
    }

    /// Тот же ресурс без права владения — для вызовов D3D.
    pub(crate) fn raw(&self) -> &ID3D11Texture2D {
        &self.0
    }

    /// Ещё одна ссылка сырым указателем — для того, кто владеет сам (атлас).
    pub(crate) fn extra_ref(&self) -> *mut std::ffi::c_void {
        windows::core::Interface::into_raw(self.0.clone())
    }

    /// Одна ли это текстура с другой.
    pub(crate) fn same_as(&self, other: &Self) -> bool {
        use windows::core::Interface;
        self.0.as_raw() == other.0.as_raw()
    }
}
