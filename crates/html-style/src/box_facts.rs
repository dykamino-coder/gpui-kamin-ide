//! Данные контекста сборки коробки без зависимостей от style и layout.
//! Параметры сохраняют типы письма и выравнивания владельца без обратного импорта.

#[derive(Clone, Debug)]
pub struct BoxContext<W, A> {
    /// Ориентация сетки: horizontal = 1, vertical-lr = 2, vertical-rl = 3.
    pub parent_grid: u8,
    pub parent_grid_align_items: Option<A>,
    pub parent_flex_grid: bool,
    pub parent_lanes: bool,
    pub cb_wm: W,
    pub applicability: ApplicabilityInputs,
}

impl<W: Default, A> Default for BoxContext<W, A> {
    fn default() -> Self {
        Self {
            parent_grid: 0,
            parent_grid_align_items: None,
            parent_flex_grid: false,
            parent_lanes: false,
            cb_wm: W::default(),
            applicability: ApplicabilityInputs::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ApplicabilityInputs {
    pub flex: bool,
    pub rtl: bool,
    pub wrap_reverse: bool,
}

pub struct DecorationInputs<'a, D> {
    pub inherited: &'a [D],
    pub block: bool,
    pub generates_box: bool,
}
