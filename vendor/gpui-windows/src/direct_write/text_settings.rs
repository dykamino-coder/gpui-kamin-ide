//! KaminIDE patch: system text smoothing settings read the way Chromium's
//! `gfx::GetFontRenderParams` (Windows) and `gfx::FontUtilWin` read them, so
//! glyph masks follow the same mode, subpixel order, contrast and gamma.

use windows::{
    Win32::{
        Graphics::{DirectWrite::*, Gdi::*},
        System::Registry::*,
        UI::WindowsAndMessaging::*,
    },
    core::*,
};

use super::text_gamma::TextGammaSettings;

/// How glyph edges are smoothed, as the system asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextSmoothing {
    /// Font smoothing off: bi-level glyphs.
    None,
    /// Standard (grayscale) antialiasing.
    Grayscale,
    /// ClearType with the given subpixel order.
    Subpixel { bgr: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SystemTextSettings {
    pub smoothing: TextSmoothing,
    pub gamma: TextGammaSettings,
}

/// `HKCU\SOFTWARE\Microsoft\Avalon.Graphics\<primary display>`: the key the
/// ClearType tuner writes. Chromium uses DirectWrite's contrast and gamma only
/// when it exists; otherwise DirectWrite's defaults differ from Skia's.
fn text_settings_key() -> Option<HKEY> {
    let mut device = DISPLAY_DEVICEW {
        cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    let mut index = 0;
    while unsafe { EnumDisplayDevicesW(PCWSTR::null(), index, &mut device, 0) }.as_bool() {
        index += 1;
        if (device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE).0 == 0 {
            continue;
        }
        let name = String::from_utf16_lossy(&device.DeviceName);
        let name = name.trim_end_matches('\0');
        let base = name.rsplit('\\').next().unwrap_or(name);
        let path = HSTRING::from(format!("SOFTWARE\\Microsoft\\Avalon.Graphics\\{base}"));
        let mut key = HKEY::default();
        let opened = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_READ, &mut key) };
        return opened.is_ok().then_some(key);
    }
    None
}

fn read_dword(key: HKEY, name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let name = HSTRING::from(name);
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            &name,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value)
}

fn spi_u32(action: SYSTEM_PARAMETERS_INFO_ACTION) -> Option<u32> {
    let mut value = 0u32;
    unsafe {
        SystemParametersInfoW(
            action,
            0,
            Some((&mut value as *mut u32).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    }
    .ok()
    .map(|_| value)
}

/// Subpixel order: the tuner's `PixelStructure` value first (0 none, 1 RGB,
/// 2 BGR, anything else none), then the system orientation; `None` = no
/// subpixel order known, which Chromium renders as grayscale.
fn subpixel_order(key: Option<HKEY>) -> Option<bool> {
    if let Some(structure) = key.and_then(|key| read_dword(key, "PixelStructure")) {
        return match structure {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        };
    }
    match spi_u32(SPI_GETFONTSMOOTHINGORIENTATION)? {
        FE_FONTSMOOTHINGORIENTATIONRGB => Some(false),
        FE_FONTSMOOTHINGORIENTATIONBGR => Some(true),
        _ => None,
    }
}

pub(crate) fn read_system_text_settings(factory: &IDWriteFactory5) -> SystemTextSettings {
    let key = text_settings_key();
    let smoothing = match spi_u32(SPI_GETFONTSMOOTHING) {
        Some(on) if on != 0 => {
            if spi_u32(SPI_GETFONTSMOOTHINGTYPE) == Some(FE_FONTSMOOTHINGCLEARTYPE) {
                match subpixel_order(key) {
                    Some(bgr) => TextSmoothing::Subpixel { bgr },
                    None => TextSmoothing::Grayscale,
                }
            } else {
                TextSmoothing::Grayscale
            }
        }
        _ => TextSmoothing::None,
    };
    let gamma = key
        .and_then(|_| unsafe { factory.CreateRenderingParams() }.ok())
        .map(|params| unsafe {
            TextGammaSettings {
                contrast: params.GetEnhancedContrast(),
                gamma: params.GetGamma(),
            }
        })
        .unwrap_or(TextGammaSettings::CHROMIUM_DEFAULT);
    if let Some(key) = key {
        let _ = unsafe { RegCloseKey(key) };
    }
    SystemTextSettings { smoothing, gamma }
}
