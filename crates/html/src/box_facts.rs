//! Данные контекста сборки коробки без зависимостей от style и layout.
//! Параметры сохраняют типы письма и выравнивания владельца без обратного импорта.

#[derive(Clone, Debug)]
pub(crate) struct BoxContext<W, A> {
    /// Ориентация сетки: horizontal = 1, vertical-lr = 2, vertical-rl = 3.
    pub(crate) parent_grid: u8,
    pub(crate) parent_grid_align_items: Option<A>,
    pub(crate) parent_flex_grid: bool,
    pub(crate) parent_lanes: bool,
    pub(crate) cb_wm: W,
    pub(crate) applicability: ApplicabilityInputs,
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
pub(crate) struct ApplicabilityInputs {
    pub(crate) flex: bool,
    pub(crate) rtl: bool,
    pub(crate) wrap_reverse: bool,
}

pub(crate) struct DecorationInputs<'a, D> {
    pub(crate) inherited: &'a [D],
    pub(crate) block: bool,
    pub(crate) generates_box: bool,
}
