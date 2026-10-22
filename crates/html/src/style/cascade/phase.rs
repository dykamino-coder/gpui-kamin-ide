//! Порядок вычисления стиля элемента: фазы, в которых применяются
//! победители каскада (css-cascade-5 §4 — значения проходят «cascaded →
//! specified → computed», и вычисление одних свойств опирается на
//! вычисленные значения других). Свойство, от которого зависит вычисление
//! или сопоставление других, применяется в более ранней фазе; итоги фазы
//! передаются дальше явным входом [`ComputeCx`], а не чтением наполовину
//! применённого стиля.
//!
//! Список выведен из зависимостей нашего кода, а не перенесён у движков
//! (Blink `StyleCascade::ApplyCascadeAffecting`/`ApplyHighPriority`, Stylo
//! `PRIORITARY_PROPERTY_DEPENDENCIES` — сверено, совпадает по существу):
//!
//! * [`Phase::CascadeAffecting`] — `writing-mode`, `direction`,
//!   `text-orientation` (сопоставление логических свойств с физическими,
//!   css-writing-modes-4 §6, css-logical-1 §4; замер `ch`/`ic` в
//!   вертикальном письме, css-values-4 §6.1.1) и `zoom` (css-viewport-1:
//!   множитель всех длин).
//! * [`Phase::Font`] — шрифт: `font`, его длинные и сбрасываемые свойства
//!   (они делят с сокращением порядок объявлений), `font-size-adjust`
//!   (метрики `ex`/`ch`), `math-depth`/`math-style` (кегль `math`); цвет:
//!   `color-scheme` (`light-dark()`, системные цвета, css-color-adjust-1
//!   §2) и `color` (`currentcolor`, css-color-4 §6.4). Отсюда — кегль корня
//!   для `rem`/`rlh` (css-values-4 §6.1.4).
//! * [`Phase::Rest`] — все остальные свойства; друг от друга они не
//!   зависят на применении.
//!
//! [`ComputeCx`]: super::compute_cx::ComputeCx

use crate::style::computed::font_members;

/// Фаза применения свойства; порядок вариантов — порядок фаз.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    /// Письмо и масштаб: от них зависит сопоставление логических свойств и
    /// все длины.
    CascadeAffecting,
    /// Шрифт и цвет: база единиц шрифта и `currentcolor`.
    Font,
    /// Всё остальное.
    Rest,
}

impl Phase {
    /// Фазы в порядке применения.
    pub const ALL: [Phase; 3] = [Phase::CascadeAffecting, Phase::Font, Phase::Rest];

    /// Фаза свойства по имени (длинного свойства или сокращения: сокращение
    /// стоит в фазе своих длинных свойств).
    pub fn of(name: &str) -> Phase {
        match name {
            "writing-mode" | "direction" | "text-orientation" | "zoom" => Phase::CascadeAffecting,
            "font" | "font-size-adjust" | "math-depth" | "math-style" | "color-scheme"
            | "color" => Phase::Font,
            _ if font_members::contains(name) => Phase::Font,
            _ => Phase::Rest,
        }
    }
}

#[cfg(test)]
mod tests;
