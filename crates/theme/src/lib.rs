//! kamin-theme: палитры + цветовые примитивы (plan/20).
//! Чистый крейт (без gpui) — конверсия в gpui::Rgba живёт в shell.

pub mod color;
pub mod palette;

pub use color::Color;
pub use palette::{DARK, LIGHT, Palette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
}

impl ThemeKind {
    /// Базовая палитра kind'а; активная contributed-тема (если задана)
    /// перекрывает её целиком.
    pub fn palette(self) -> &'static Palette {
        if let Some(p) = contributed() {
            return p;
        }
        match self {
            ThemeKind::Dark => &DARK,
            ThemeKind::Light => &LIGHT,
        }
    }

    /// Базовая палитра БЕЗ учёта contributed (для построения оверрайдов).
    pub fn palette_base(self) -> &'static Palette {
        match self {
            ThemeKind::Dark => &DARK,
            ThemeKind::Light => &LIGHT,
        }
    }
}

use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

static CONTRIBUTED: AtomicPtr<Palette> = AtomicPtr::new(std::ptr::null_mut());

// Текущий kind для мест без доступа к RootView.theme (тултипы, file_list)
static CURRENT_KIND: AtomicU8 = AtomicU8::new(0);

/// Зафиксировать активный kind (вызывается при каждой смене темы).
pub fn set_current_kind(k: ThemeKind) {
    CURRENT_KIND.store(matches!(k, ThemeKind::Light) as u8, Ordering::Release);
    bump_generation();
}

/// Поколение оформления: растёт при смене темы, палитры расширения и
/// иконочной темы. Кэшируемым панелям это единственный способ узнать, что
/// картинка устарела: палитра и иконки живут в статиках, а не в пропсах, и
/// `ThemeKind` при смене contributed-темы обычно остаётся тем же (dark→dark).
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Отметить, что оформление изменилось.
pub fn bump_generation() {
    GENERATION.fetch_add(1, Ordering::Release);
}

/// Текущее поколение оформления (для пропсов компонентов).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Текущий kind — light? (для тем-зависимых теней и т.п.)
pub fn current_is_light() -> bool {
    CURRENT_KIND.load(Ordering::Acquire) == 1
}

/// Активная палитра с учётом contributed — для кода вне RootView.
pub fn current_palette() -> &'static Palette {
    let k = if CURRENT_KIND.load(Ordering::Acquire) == 1 {
        ThemeKind::Light
    } else {
        ThemeKind::Dark
    };
    k.palette()
}

/// Активная contributed-палитра (VS Code-тема расширения) или None.
pub fn contributed() -> Option<&'static Palette> {
    let ptr = CONTRIBUTED.load(Ordering::Acquire);
    // Палитры «утекают» намеренно: рендер раздаёт &'static; смена темы редка
    unsafe { ptr.as_ref() }
}

/// Установить/сбросить contributed-палитру (None → базовые DARK/LIGHT).
pub fn set_contributed(p: Option<Palette>) {
    bump_generation();
    let ptr = match p {
        Some(pal) => Box::leak(Box::new(pal)) as *mut Palette,
        None => std::ptr::null_mut(),
    };
    CONTRIBUTED.store(ptr, Ordering::Release);
}
