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
    /// `cb_width` / `avail_w` come from outside the tracked subtree; next to
    /// `cell_bfc`, so the fat sink of `widths` keeps the struct's size.
    cb_outside: Cell<bool>,
    avail_outside: Cell<bool>,
}

/// Where the outside width reads of a tracked subtree go
/// (`BoxConstraints::track_widths`): a flag of one subtree, or the record
/// state of a flow that tracks its children one by one in its own context
/// (`layout::reuse::FlowReuse`), which then needs no flag of its own.
pub(crate) trait WidthSink {
    fn note(&self);
}

impl WidthSink for Cell<bool> {
    fn note(&self) {
        self.set(true);
    }
}

/// The sink of a subtree tracked inside a tracked one
/// (`BoxConstraints::track_widths_nested`).
struct NestedSink {
    own: Rc<Cell<bool>>,
    outer: Rc<dyn WidthSink>,
}

impl WidthSink for NestedSink {
    fn note(&self) {
        self.own.set(true);
        self.outer.note();
    }
}

/// Read set of the widths of a subtree kept across frames (`layout::reuse`):
/// the subtree's build output stays valid under another available width
/// unless the build consumed a width that came from outside the subtree.
/// A value is *outside* while it is inherited (the child's context is a copy
/// of the parent's); a value the subtree sets itself (a definite own width,
/// a measured column) is its own, and any outside width it was computed from
/// was already read through an accessor. Like the revalidation selectors of
/// Servo's style sharing: what the build consulted is recorded, not guessed.
///
/// The set is in cells, so a flow can record the reads of each of its
/// children's builds in its own context, without a copy per child
/// (`BoxConstraints::track_widths_in_place`).
#[derive(Default)]
struct WidthReads {
    sink: Cell<Option<Rc<dyn WidthSink>>>,
}

impl WidthReads {
    fn note(&self, outside: bool) {
        if !outside {
            return;
        }
        let sink = self.sink.take();
        if let Some(sink) = &sink {
            sink.note();
        }
        self.sink.set(sink);
    }

    fn sink(&self) -> Option<Rc<dyn WidthSink>> {
        let sink = self.sink.take();
        self.sink.set(sink.clone());
        sink
    }
}

impl Clone for WidthReads {
    fn clone(&self) -> Self {
        Self {
            sink: Cell::new(self.sink()),
        }
    }
}

impl std::fmt::Debug for WidthReads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidthReads")
            .field("tracked", &self.sink().is_some())
            .finish()
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
        self.widths.note(self.cb_outside.get());
        self.cb_width
    }

    /// Available width of `width: auto` blocks (see the field); a read of an
    /// outside width is recorded.
    pub(crate) fn avail_w(&self) -> Option<f32> {
        self.widths.note(self.avail_outside.get());
        self.avail_w
    }

    /// A containing-block width the box sets itself.
    pub(crate) fn set_cb_width(&mut self, width: Option<f32>) {
        self.cb_width = width;
        self.cb_outside.set(false);
    }

    /// An available width the box sets itself.
    pub(crate) fn set_avail_w(&mut self, width: Option<f32>) {
        self.avail_w = width;
        self.avail_outside.set(false);
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
            self.widths.note(self.avail_outside.get());
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
    pub(crate) fn track_widths(&mut self, sink: Rc<dyn WidthSink>) {
        self.track_widths_in_place(sink);
    }

    /// `track_widths` in this shared context, until `untrack_widths`: the
    /// subtrees built with it and with its copies record into `sink`. For a
    /// context that tracks nothing itself (a document's root flow).
    pub(crate) fn track_widths_in_place(&self, sink: Rc<dyn WidthSink>) {
        debug_assert!(
            self.widths.sink().is_none() && !self.cb_outside.get() && !self.avail_outside.get(),
            "a tracking context"
        );
        self.widths.sink.set(Some(sink));
        self.cb_outside.set(true);
        self.avail_outside.set(true);
    }

    /// `track_widths` of a subtree inside a tracked one (a table cell inside a
    /// recorded block): its outside reads go to `sink`, and to the sink of
    /// the enclosing subtree while a width of this context comes from outside
    /// that one too. Which of the two widths a read consumed is not told
    /// apart there: the enclosing subtree may be told of a read of a width set
    /// inside it, which only makes its record fit fewer widths.
    pub(crate) fn track_widths_nested(&mut self, sink: Rc<Cell<bool>>) {
        let outer = self
            .widths
            .sink()
            .filter(|_| self.cb_outside.get() || self.avail_outside.get());
        let sink: Rc<dyn WidthSink> = match outer {
            Some(outer) => Rc::new(NestedSink { own: sink, outer }),
            None => sink,
        };
        self.widths.sink.set(Some(sink));
        self.cb_outside.set(true);
        self.avail_outside.set(true);
    }

    /// A subtree that is not built again (a replayed table cell) read an
    /// outside width when it was built: tell the enclosing tracked subtree,
    /// as the build would have (see `track_widths_nested`).
    pub(crate) fn note_nested_read(&self) {
        self.widths
            .note(self.cb_outside.get() || self.avail_outside.get());
    }

    /// End `track_widths_in_place`: the context tracks nothing again.
    pub(crate) fn untrack_widths(&self) {
        self.widths.sink.set(None);
        self.cb_outside.set(false);
        self.avail_outside.set(false);
    }
}

/// A width a consumer reads only where it resolves against it (a float fit
/// check, a percentage): an outside width is read, and so recorded
/// (`BoxConstraints::track_widths`), only then.
#[derive(Clone, Copy)]
pub(crate) enum WidthBasis<'a> {
    Known(Option<f32>),
    Read(&'a dyn Fn() -> Option<f32>),
}

impl WidthBasis<'_> {
    pub(crate) fn get(self) -> Option<f32> {
        match self {
            Self::Known(width) => width,
            Self::Read(read) => read(),
        }
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
#[cfg(test)]
mod tests;
