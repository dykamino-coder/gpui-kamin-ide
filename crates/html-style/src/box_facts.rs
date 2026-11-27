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
/// paint property tree, not in `ComputedStyle`). Layout's used values that
/// flow down the box tree live here too, so layout never writes the computed
/// style. `C` is the owner's colour type, as `BoxContext` takes its writing
/// mode and alignment without a reverse import.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TreeFacts<C> {
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
    /// The nearest ancestor scrollport's axes, physical width then height,
    /// including an indefinite nearest scroller (css-writing-modes-4 §7.3.1).
    pub orthogonal_scrollport: Option<[AxisSizes; 2]>,
    /// The used `user-select` makes the text unselectable (`Some(true)`);
    /// `auto` resolves against the parent box (css-ui-4 §6.1).
    pub no_select: Option<bool>,
    /// The used `line-clamp` of the nearest clamp container, handed to the
    /// block containers and text inside it (css-overflow-4 §5.1).
    pub line_clamp: Option<u32>,
    /// The ink of the text shadow being painted: the text of the shadow's box
    /// and its descendants draws in it (css-text-decor-3 §4).
    pub text_shadow_ink: Option<C>,
    /// The box is inside a ruby unit laid out as one line (css-ruby-1 §3).
    pub ruby_unit: bool,
    /// Forced breaks inside ruby annotations become spaces (css-ruby-1 §3.4).
    pub ruby_suppress_breaks: bool,
    /// The box is on a line turned for vertical text (css-writing-modes-4 §7.1).
    pub turned_line: bool,
    /// The inline space of an orthogonal flow: the nearest definite ancestor
    /// block size (css-writing-modes-4 §7.3.1).
    pub ortho_limit: Option<f32>,
    /// The used inline constraint of an orthogonal flow root and its in-flow
    /// contents (css-writing-modes-4 §7.3.2).
    pub orthogonal_inline: Option<InlineConstraint>,
}

/// An orthogonal flow's inline percentage basis and shrink-to-fit space
/// (`style::computed::orthogonal`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineConstraint {
    pub available: f32,
    pub fixed: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

/// One axis of a box for an orthogonal flow: its size and min/max limits
/// (`style::computed::orthogonal`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisSizes {
    pub size: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ApplicabilityInputs {
    pub flex: bool,
    pub rtl: bool,
    pub wrap_reverse: bool,
}

pub struct DecorationInputs<'a, D> {
    /// The parent's propagated decorations, shared rather than copied; `None`
    /// where propagation stops (atomic inlines, out-of-flow boxes).
    pub inherited: Option<&'a std::rc::Rc<[D]>>,
    pub block: bool,
    pub generates_box: bool,
}
