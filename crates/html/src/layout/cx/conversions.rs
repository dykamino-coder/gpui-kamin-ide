//! Стили элементов GPUI, уже переведённые из стилей коробок в этой сборке документа: вход
//! (значения коробки, её размеры, контекст значений) → общий `StyleRefinement`.
//!
//! Память — состояние раскладки документа (`LayoutState`), но действует только внутри его
//! сборки ([`StyleConversions::build`]: `render::render`, `render::render_block`): перевод
//! читает арену `calc()` кадра, шрифты и семейство документа, которые между сборками
//! меняются, поэтому сборка начинает и кончает пустой памятью, а коробки вне сборки
//! переводятся без неё. Выделений на сборку или на `LayoutCx` у памяти нет, кроме кольца
//! кандидатов, которое живёт только во время сборки.

use crate::layout::block::sizing::BoxDimensions;
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use gpui::StyleRefinement;
use std::cell::Cell;
use std::rc::Rc;

/// Distinct recent inputs kept for reuse. Boxes of a page repeat a few styles, often
/// interleaved (table cells, list items with nested blocks): as in GPUI's refinement share
/// and Stylo's sharing cache, a bounded set of the newest distinct inputs.
const CANDIDATES: usize = 32;

/// Balance of lookups at which they pause: a miss takes one from it, a hit adds `HIT_WORTH`.
/// A miss compares the box with every candidate by value and then copies it into the memory;
/// on distinct styles (flex items with their own `flex` each) that costs more than the
/// conversions it saves, and a rare hit among them (p03: 338 hits and 1 047 misses when every
/// hit restarted lookups) must not restart full lookups. The balance never falls below this
/// threshold, so a run of hits resumes reuse after a few probes.
const PAUSE_AT: i8 = -64;

/// What a hit adds to the balance, and the most the balance keeps: a page of repeated styles
/// that turns to distinct ones pauses after at most `BALANCE_CAP - PAUSE_AT` misses.
const HIT_WORTH: i8 = 2;
const BALANCE_CAP: i8 = 64;

/// While paused, every this many boxes one is still looked up and remembered, so that a
/// later run of repeated styles resumes reuse. Divides 256: the count wraps.
const PROBE_EVERY: u8 = 16;

#[derive(Default)]
pub(crate) struct StyleConversions {
    /// Taken out while read or written: the memory never lends itself across a call.
    recent: Cell<Vec<Conversion>>,
    /// Slot of the next new entry once `recent` is full.
    next: Cell<u8>,
    /// Hits and misses so far, weighed by `HIT_WORTH` (see `PAUSE_AT`).
    balance: Cell<i8>,
    /// Boxes passed over since lookups paused.
    passed: Cell<u8>,
    /// Builds of the document in progress (a nested document builds inside its host's).
    builds: Cell<u8>,
}

/// A document build in progress: the memory is empty when it starts and when it ends.
pub(crate) struct Build<'a>(&'a StyleConversions);

impl Drop for Build<'_> {
    fn drop(&mut self) {
        self.0.clear();
        self.0.builds.set(self.0.builds.get().saturating_sub(1));
    }
}
/// What the memory says about a box about to be converted.
pub(crate) enum Lookup {
    /// An equal box was converted earlier in this build.
    Hit(Rc<StyleRefinement>),
    /// Convert and `record` the result.
    Miss,
    /// Convert without recording: reuse is paused on a run of distinct styles.
    Pass,
}

struct Conversion {
    input: BoxStyle,
    dimensions: BoxDimensions,
    values: Values,
    style: Rc<StyleRefinement>,
}

/// The parts of the value context a conversion reads.
#[derive(Clone, Copy, PartialEq)]
struct Values {
    doc: *const crate::values::DocumentValues,
    frame: bool,
    viewport: (f32, f32),
    scheme: crate::style::values::value::Scheme,
    clearance_possible: bool,
}

impl Values {
    fn of(vcx: ValueCx<'_>) -> Self {
        Self {
            doc: vcx.doc,
            frame: vcx.frame,
            viewport: vcx.viewport,
            scheme: vcx.scheme,
            clearance_possible: vcx.clearance_possible,
        }
    }
}

impl StyleConversions {
    /// Start a build of the document; dropping the result ends it. A nested build empties the
    /// memory too: it only loses candidates of the enclosing build.
    pub(crate) fn build(&self) -> Build<'_> {
        self.builds.set(self.builds.get().saturating_add(1));
        self.clear();
        Build(self)
    }

    /// The memory while the document is being built, none outside its builds.
    pub(crate) fn active(&self) -> Option<&Self> {
        (self.builds.get() > 0).then_some(self)
    }

    /// Forget every candidate and free their memory.
    fn clear(&self) {
        drop(self.recent.take());
        self.next.set(0);
        self.balance.set(0);
        self.passed.set(0);
    }
    /// Look a box up unless reuse is paused, keeping the balance that pauses it.
    pub(crate) fn lookup(
        &self,
        input: &BoxStyle,
        dimensions: &BoxDimensions,
        vcx: ValueCx<'_>,
    ) -> Lookup {
        if self.balance.get() <= PAUSE_AT {
            self.passed.set(self.passed.get().wrapping_add(1));
            if !self.passed.get().is_multiple_of(PROBE_EVERY) {
                return Lookup::Pass;
            }
        }
        match self.find(input, dimensions, vcx) {
            Some(style) => {
                self.balance
                    .set((self.balance.get() + HIT_WORTH).min(BALANCE_CAP));
                self.passed.set(0);
                Lookup::Hit(style)
            }
            None => {
                self.balance.set((self.balance.get() - 1).max(PAUSE_AT));
                Lookup::Miss
            }
        }
    }

    /// The element style converted earlier in this build from an equal input.
    pub(crate) fn find(
        &self,
        input: &BoxStyle,
        dimensions: &BoxDimensions,
        vcx: ValueCx<'_>,
    ) -> Option<Rc<StyleRefinement>> {
        let values = Values::of(vcx);
        let recent = self.recent.take();
        let (len, next) = (recent.len(), usize::from(self.next.get()));
        let found = (1..=len)
            .map(|back| &recent[(next + len - back) % len])
            .find(|c| {
                c.values == values && c.dimensions == *dimensions && c.input.same_box_values(input)
            })
            .map(|c| c.style.clone());
        self.recent.set(recent);
        found
    }

    /// Remember the element style converted from `input`.
    pub(crate) fn record(
        &self,
        input: &BoxStyle,
        dimensions: BoxDimensions,
        vcx: ValueCx<'_>,
        style: Rc<StyleRefinement>,
    ) {
        let entry = Conversion {
            input: input.clone(),
            dimensions,
            values: Values::of(vcx),
            style,
        };
        let mut recent = self.recent.take();
        if recent.capacity() == 0 {
            recent.reserve_exact(CANDIDATES);
        }
        let next = usize::from(self.next.get());
        let next = if recent.len() < CANDIDATES {
            recent.push(entry);
            recent.len() % CANDIDATES
        } else {
            // A dropped style that no box holds any more becomes the allocation of a later
            // element's style (`gpui::recycle_style_refinement`): a converted style its box
            // then changes (copy on write) costs no allocation more than converting without
            // the memory.
            let dropped = std::mem::replace(&mut recent[next], entry);
            gpui::recycle_style_refinement(dropped.style);
            (next + 1) % CANDIDATES
        };
        self.next.set(next as u8);
        self.recent.set(recent);
    }
}

#[cfg(test)]
#[path = "conversions_tests.rs"]
mod tests;
