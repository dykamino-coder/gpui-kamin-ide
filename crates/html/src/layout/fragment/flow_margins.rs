//! Блочные поля ребёнка стопки фрагментов до её укладки.

/// Схлопнутые блочные поля ребёнка стопки в CSS px: стопка кладёт их сама и
/// усекает у разрыва (css-break-3 §5.2), копия коробки идёт без них. Высоту и
/// точки разрыва ребёнка даёт раскладка его копии, а не этот тип.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FlowMargins {
    pub(crate) margin_top: f32,
    pub(crate) margin_bottom: f32,
}
