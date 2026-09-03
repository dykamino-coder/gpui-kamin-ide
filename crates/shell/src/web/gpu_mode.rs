//! Чем рисует окно: видеокартой или процессором (WARP).
//!
//! На машине без GPU (облачная виртуалка, RDP-сессия) DXGI отдаёт единственный
//! адаптер — «Microsoft Basic Render Driver», то есть WARP, программный D3D11.
//! gpui берёт первый адаптер, который умеет D3D11, и молча садится на него
//! (`vendor/gpui/src/platform/windows/directx_devices.rs`). Дальше КАЖДЫЙ кадр
//! окна растеризуется процессором на всех ядрах, причём мимо счётчиков gpui:
//! `Present` только ставит команды в очередь и возвращается за пару
//! миллисекунд, а работа идёт после него.
//!
//! Замер на проде (8 ядер, Xeon, RDP): один CSS-спиннер 19×19 в шапке чата
//! держал 120–139 кадров CEF в секунду, из них 22–27 кадров окна, и стоил
//! **6,8 ядра из 8**. Он же снятый — 0,28 ядра и ноль кадров. Диагностика
//! `[cef] за секунду` объясняла из этого только 0,8 ядра: остальное съедал
//! WARP, которого никто не замерял.
//!
//! Поэтому режим знают двое: насос (`pump.rs`) заказывает кадры реже, а отдача
//! страниц (`scheme.rs`) гасит бесконечные анимации, которые эти кадры и
//! порождают.

use std::sync::OnceLock;
use std::time::Duration;

/// Форс режима: `1` — считать рендер программным, `0` — аппаратным. Нужен
/// стендам (проверить эффект на машине С видеокартой) и как ручной откат,
/// если детект ошибётся на незнакомом адаптере.
const FORCE_ENV: &str = "KAMIN_FORCE_SW_RENDER";

/// Пауза насоса при программном рендере. На проде WARP выдавал 25 кадров окна
/// в секунду и упирался в потолок: кадр стоил ~40 мс процессорного времени,
/// то есть насос со своими 8 мс просто не успевал их заказывать. 100 мс
/// ограничивают поток десятью кадрами в секунду — стриминг ответа и скролл
/// читаются, а счёт идёт на доли ядра вместо шести.
const SOFTWARE_REPAINT_INTERVAL: Duration = Duration::from_millis(100);

/// Обычная пауза — не чаще половины кадра дисплея (см. `pump.rs`).
const HARDWARE_REPAINT_INTERVAL: Duration = Duration::from_millis(8);

/// Вендор Microsoft. WARP и прочие программные адаптеры представляются им;
/// у железа вендор свой (0x10DE NVIDIA, 0x1002 AMD, 0x8086 Intel).
#[cfg(windows)]
const VENDOR_MICROSOFT: u32 = 0x1414;

static SOFTWARE: OnceLock<bool> = OnceLock::new();

/// Рисует ли окно программным растеризатором. Считается ОДИН раз: состав
/// адаптеров за время работы не меняется, а ответ нужен на горячем пути
/// (каждый тик насоса).
pub(crate) fn software_render() -> bool {
    *SOFTWARE.get_or_init(|| {
        let forced = match std::env::var(FORCE_ENV).ok().as_deref() {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        let software = forced.unwrap_or_else(detect);
        if software {
            // В лог: по присланному `diag.log` иначе не отличить «медленно
            // из-за WARP» от «медленно из-за нас».
            println!(
                "[cef] рендер программный (WARP): кадры окна не чаще {} мс, бесконечные анимации страниц гасим",
                SOFTWARE_REPAINT_INTERVAL.as_millis()
            );
        }
        software
    })
}

/// Пауза между заказами перерисовки окна.
pub(crate) fn repaint_interval() -> Duration {
    if software_render() {
        SOFTWARE_REPAINT_INTERVAL
    } else {
        HARDWARE_REPAINT_INTERVAL
    }
}

/// Блок стилей, который приклеивается к КАЖДОЙ отдаваемой странице при
/// программном рендере (`scheme.rs`).
///
/// Гасим не анимацию целиком, а её ПОВТОРЫ: `animation: none` оставило бы
/// невидимым всё, что появляется через `opacity: 0` + `forwards`, а нам нужен
/// ровно один источник кадров — бесконечное вращение. Спиннер довернёт свой
/// круг и замрёт, разовые появления отработают как были.
pub(crate) fn reduced_motion_block() -> &'static str {
    if software_render() {
        r#"<style id="__kaminReducedMotion">*,*::before,*::after{animation-iteration-count:1!important}</style>"#
    } else {
        ""
    }
}

/// Есть ли в системе хоть один аппаратный адаптер.
///
/// Спрашиваем DXGI, а не устройство окна: ответ нужен ДО первого кадра (первая
/// же отдача страницы должна прийти уже с погашенными анимациями), а устройство
/// gpui появляется только на нём. Выбор у gpui тот же — первый адаптер из
/// этого же перечисления, который умеет D3D11.
#[cfg(windows)]
fn detect() -> bool {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

    // Safety: перечисление адаптеров и чтение их описаний, без владения.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            // Без DXGI судить не о чем: молча замедлять окно на догадке хуже,
            // чем оставить всё как есть.
            return false;
        };
        let mut seen = 0u32;
        for index in 0.. {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            seen += 1;
            let described = String::from_utf16_lossy(&desc.Description);
            let name = described.trim_matches(char::from(0));
            let software =
                desc.VendorId == VENDOR_MICROSOFT || name.starts_with("Microsoft Basic Render");
            if !software {
                return false;
            }
            println!("[cef] адаптер {index}: {name} — программный");
        }
        // Ни одного адаптера — перечисление не удалось; см. выше про догадки.
        seen > 0
    }
}

#[cfg(not(windows))]
fn detect() -> bool {
    false
}
