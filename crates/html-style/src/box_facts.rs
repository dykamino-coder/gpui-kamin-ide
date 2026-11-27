//! Данные контекста сборки коробки без зависимостей от style и layout.
//! Параметры сохраняют типы письма и выравнивания владельца без обратного импорта.

#[derive(Clone, Debug, PartialEq)]
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

/// What a box's ancestors in the box tree establish, propagated by the box
/// stage (`style::cascade::inherit::containing`). These describe the box's
/// position in the box tree, not its own CSS values, so they live beside the
/// shared computed style and never copy it (Blink keeps the same facts in the
/// paint property tree, not in `ComputedStyle`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeFacts {
    /// An ancestor clips its contents: non-visible overflow, `contain: paint`,
    /// a clip path or a mask.
    pub fragment_clip_ancestor: bool,
    /// The box is inside a multi-column container (css-multicol-1 §2).
    pub in_multicol: bool,
    /// An ancestor is a Backdrop Root (Filter Effects 2 §3).
    pub backdrop_root_above: bool,
    /// An ancestor establishes the containing block of fixed descendants
    /// (css-transforms-1 §2, css-contain-2 §3.2, css-will-change-1 §2.1).
    pub transform_ancestor: bool,
    /// An ancestor is transformed (css-transforms-1 §6: fixed backgrounds).
    pub transformed_ancestor: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
