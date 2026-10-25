//! Доступные размеры и BFC: входы, которые родитель задаёт дочерней коробке.

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BoxConstraints {
    /// Containing-block width in px when the parent's style gives it. A
    /// replaced element without intrinsic size but with a ratio takes its
    /// width from the block equation, i.e. from the containing block (CSS 2.1
    /// §10.3.2, last case); 300×150 applies only when there is none.
    pub(crate) cb_width: Option<f32>,
    /// Width a block-level child with `width: auto` gets in the current
    /// block container: its stretch-fit size (CSS 2.1 §10.3.3; css-sizing-3:
    /// the inline size of an in-flow block-level box is definite). Needed by
    /// `repeat(auto-fill | auto-fit, …)` (css-grid-1 §7.2.3.2) and by
    /// percentages of inline boxes, whose containing block is the paragraph's
    /// block. `None` — honestly unknown (flex, grid, table, inline-block,
    /// float, abspos, multicol, vertical writing).
    pub(crate) avail_w: Option<f32>,
    /// Definite content height of the current block container, for
    /// percentage heights of embedded viewports (CSS 2.1 §10.5).
    pub(crate) avail_h: Option<f32>,
    /// The block list being built is a table cell's (or caption's) own
    /// content: a BFC root whose floats it contains (§10.6.7).
    pub(crate) cell_bfc: bool,
}

/// Входы повторной меры. Размер страницы не заменяет процентную базу родителя:
/// оба участвуют в зависимости снимка (CSS Sizing 3 §percentage-sizing).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FragmentConstraints {
    pub(crate) area: (f32, f32),
    pub(crate) parent: BoxConstraints,
}
