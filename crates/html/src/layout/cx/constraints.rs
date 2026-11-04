//! Доступные размеры и BFC: входы, которые родитель задаёт дочерней коробке.

use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct BoxConstraints {
    /// Containing-block width in px when the parent's style gives it. A
    /// replaced element without intrinsic size but with a ratio takes its
    /// width from the block equation, i.e. from the containing block (CSS 2.1
    /// §10.3.2, last case); 300×150 applies only when there is none.
    cb_width: Option<f32>,
    /// Width a block-level child with `width: auto` gets in the current
    /// block container: its stretch-fit size (CSS 2.1 §10.3.3; css-sizing-3:
    /// the inline size of an in-flow block-level box is definite). Needed by
    /// `repeat(auto-fill | auto-fit, …)` (css-grid-1 §7.2.3.2) and by
    /// percentages of inline boxes, whose containing block is the paragraph's
    /// block. `None` — honestly unknown (flex, grid, table, inline-block,
    /// float, abspos, multicol, vertical writing).
    avail_w: Option<f32>,
    /// Definite content height of the current block container, for
    /// percentage heights of embedded viewports (CSS 2.1 §10.5).
    pub(crate) avail_h: Option<f32>,
    /// The block list being built is a table cell's (or caption's) own
    /// content: a BFC root whose floats it contains (§10.6.7).
    pub(crate) cell_bfc: bool,
    /// Reads of widths that come from outside a retained subtree.
    widths: WidthReads,
}

/// Read set of the widths of a subtree kept across frames (`layout::reuse`):
/// the subtree's build output stays valid under another available width
/// unless the build consumed a width that came from outside the subtree.
/// A value is *outside* while it is inherited (the child's context is a copy
/// of the parent's); a value the subtree sets itself (a definite own width,
/// a measured column) is its own, and any outside width it was computed from
/// was already read through an accessor. Like the revalidation selectors of
/// Servo's style sharing: what the build consulted is recorded, not guessed.
#[derive(Clone, Debug, Default)]
struct WidthReads {
    sink: Option<Rc<Cell<bool>>>,
    cb: bool,
    avail: bool,
}

impl WidthReads {
    fn note(&self, outside: bool) {
        if outside && let Some(sink) = &self.sink {
            sink.set(true);
        }
    }
}

impl PartialEq for BoxConstraints {
    fn eq(&self, other: &Self) -> bool {
        self.cb_width == other.cb_width
            && self.avail_w == other.avail_w
            && self.avail_h == other.avail_h
            && self.cell_bfc == other.cell_bfc
    }
}

impl BoxConstraints {
    /// Containing-block width (see the field); a read of an outside width
    /// is recorded.
    pub(crate) fn cb_width(&self) -> Option<f32> {
        self.widths.note(self.widths.cb);
        self.cb_width
    }

    /// Available width of `width: auto` blocks (see the field); a read of an
    /// outside width is recorded.
    pub(crate) fn avail_w(&self) -> Option<f32> {
        self.widths.note(self.widths.avail);
        self.avail_w
    }

    /// A containing-block width the box sets itself.
    pub(crate) fn set_cb_width(&mut self, width: Option<f32>) {
        self.cb_width = width;
        self.widths.cb = false;
    }

    /// An available width the box sets itself.
    pub(crate) fn set_avail_w(&mut self, width: Option<f32>) {
        self.avail_w = width;
        self.widths.avail = false;
    }

    /// The available width of a child box: `inner(outer)`. When the result
    /// does not need the outer width (`inner(None)` is definite), it is the
    /// child's own; otherwise it is derived from the outer width and stays
    /// outside, so only a later consumer records the read.
    pub(crate) fn derive_avail_w(&mut self, inner: impl Fn(Option<f32>) -> Option<f32>) {
        match inner(None) {
            Some(own) => self.set_avail_w(Some(own)),
            None => self.avail_w = inner(self.avail_w),
        }
    }

    /// `f(avail_w)`, recording the read only when the result depends on the
    /// width (differs from `f(None)`).
    pub(crate) fn with_avail_w<T: PartialEq>(&self, f: impl Fn(Option<f32>) -> T) -> T {
        let blind = f(None);
        if self.avail_w.is_none() {
            return blind;
        }
        let seen = f(self.avail_w);
        if seen != blind {
            self.widths.note(self.widths.avail);
        }
        seen
    }

    /// Both widths without recording a read: only for keys that compare
    /// them as a whole (`layout::reuse::FlowKey`).
    pub(crate) fn key_widths(&self) -> (Option<f32>, Option<f32>) {
        (self.cb_width, self.avail_w)
    }

    /// Start recording outside width reads of a subtree into `sink`: both
    /// widths of this context come from outside it.
    pub(crate) fn track_widths(&mut self, sink: Rc<Cell<bool>>) {
        self.widths = WidthReads {
            sink: Some(sink),
            cb: true,
            avail: true,
        };
    }
}

/// Входы повторной меры. Размер страницы не заменяет процентную базу родителя:
/// оба участвуют в зависимости снимка (CSS Sizing 3 §percentage-sizing).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FragmentConstraints {
    pub(crate) area: (f32, f32),
    pub(crate) parent: BoxConstraints,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_consumed_outside_widths_are_recorded() {
        let sink = Rc::new(Cell::new(false));
        let mut outer = BoxConstraints::default();
        outer.set_avail_w(Some(800.0));
        outer.set_cb_width(Some(800.0));
        outer.track_widths(sink.clone());
        // A derived width stays outside without a read.
        let mut child = outer.clone();
        child.derive_avail_w(|w| w.map(|w| w - 10.0));
        assert!(!sink.get());
        // A width that does not change the result is not a read.
        assert_eq!(child.with_avail_w(|_| 1), 1);
        assert!(!sink.get());
        // A definite own width is the subtree's own.
        let mut own = child.clone();
        own.derive_avail_w(|_| Some(300.0));
        own.set_cb_width(Some(300.0));
        assert_eq!((own.avail_w(), own.cb_width()), (Some(300.0), Some(300.0)));
        assert!(!sink.get());
        assert_eq!(child.avail_w(), Some(790.0));
        assert!(sink.get());
        assert_eq!(child, {
            let mut plain = BoxConstraints::default();
            plain.set_avail_w(Some(790.0));
            plain.set_cb_width(Some(800.0));
            plain
        });
    }
}
