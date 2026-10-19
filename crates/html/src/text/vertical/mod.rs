//! Вертикальный текст.
mod combined_upright;
pub(crate) mod measurements;
mod root_left;
mod vertical_text;
pub use crate::text::vertical::combined_upright::CombinedUpright;
pub use crate::text::vertical::root_left::record_root_left;
pub use crate::text::vertical::vertical_text::VerticalText;

// owner: A

use gpui::{Bounds, Pixels};

mod combined_geometry;
pub(crate) mod combined_text;
mod vertical_line_baseline;
mod vertical_style;

thread_local! {
    /// Сборщик внутреннего строчного размера повёрнутого текста (`None` —
    /// закрыт): наибольшая длина строки всех `VerticalText`, разложенных,
    /// пока он открыт.
    pub static VT_INLINE_MAX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

// Где на экране оказалась распорка — и где оказался её заместитель.
//
// Позиционированный элемент по CSS рисуется ПОВЕРХ обычного содержимого, а
// порядок отрисовки у нас — порядок детей. Отложенная отрисовка для этого не
// годится (вложенная в GPUI запрещена, а без вложенности рушится раскладка),
// поэтому элемент уходит последним ребёнком и возвращается на место сдвигом:
// щуп запоминает, где стояла распорка, заместитель — где встал сам, разница
// и есть нужный сдвиг.
thread_local! {
    /// Рамка `VerticalText`, внутри которого сейчас идёт подготовка.
    ///
    /// Повёрнутый абзац подготавливается в ДО-ПОВОРОТНОЙ системе, а матрица
    /// поворота живёт только в `paint`. Щуп статической позиции пишет дырку
    /// именно в подготовке, поэтому без этой рамки `LatePlace` читает
    /// до-поворотную точку как экранную, и коробка уезжает на колонку.
    pub(crate) static VT_FRAME: std::cell::Cell<Option<Bounds<Pixels>>> =
        const { std::cell::Cell::new(None) };
    /// Сторона поворота этой рамки: `sideways-lr` вертится против часовой.
    pub(crate) static VT_CCW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Block-axis phase of the clockwise frame's pre-turn origin: the flat
    /// content starts at `frame.origin.y + phase`, so that pixel edges rounded
    /// before the turn land on the physical pixel grid after it.
    pub(crate) static VT_PHASE: std::cell::Cell<Pixels> = const { std::cell::Cell::new(Pixels::ZERO) };
}

/// Экранная точка для до-поворотной, если мы внутри повёрнутого абзаца.
/// Mapping follows the actual clockwise/counter-clockwise text frame.
/// Идёт ли сейчас подготовка ПОВЁРНУТОГО абзаца (`VerticalText`).
pub fn in_rotated_frame() -> bool {
    VT_FRAME.with(|c| c.get()).is_some()
}

pub(crate) fn vt_map(hole: Bounds<Pixels>, thickness: Pixels) -> Bounds<Pixels> {
    let Some(vt) = VT_FRAME.with(|c| c.get()) else {
        return hole;
    };
    let pre_x = hole.origin.x - vt.origin.x;
    let pre_y = hole.origin.y - vt.origin.y;
    let phase = VT_PHASE.with(|c| c.get());
    // `sideways-lr` вертится ПРОТИВ часовой (см. `VerticalText::paint`):
    // до-поворотная `(px, py)` от угла рамки становится экранной
    // `(x + py, y + h - px - thickness)` — зеркало обычного случая по обеим
    // осям (css-writing-modes-4: строчная ось снизу вверх, over слева).
    if VT_CCW.with(|c| c.get()) {
        return Bounds {
            origin: gpui::point(
                vt.origin.x + pre_y,
                vt.origin.y + vt.size.height - pre_x - thickness,
            ),
            size: hole.size,
        };
    }
    Bounds {
        origin: gpui::point(
            vt.origin.x + vt.size.width - (pre_y - phase) - thickness,
            vt.origin.y + pre_x,
        ),
        size: hole.size,
    }
}
