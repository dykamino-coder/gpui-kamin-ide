//! Шрифты прогонов текста, уже выведенные в этой сборке документа: вход (унаследованные
//! группы шрифта и потока текста, ориентация письма, базовый шрифт и кегль) → готовый шрифт
//! прогона и его кегль.
//!
//! Шрифт прогона — функция унаследованных групп, а не коробки: все куски текста с теми же
//! группами получают один результат (Blink держит готовый `Font` в общем вычисленном стиле).
//! Группы делятся между элементом и потомками, пока потомок их не меняет, поэтому на
//! странице одинаковых строк вывод (список запасных семейств, алиасы `@font-face`,
//! возможности OpenType) делается один раз, а не на каждый кусок.
//!
//! Память — состояние раскладки документа (`LayoutState`) и действует только внутри его
//! сборки ([`RunFaces::build`]), как `StyleConversions`: реестр шрифтов и семейство
//! документа меняются между сборками, поэтому сборка начинает и кончает пустой памятью.

use crate::font_facts::FontOrientation;
use crate::style::computed::{Computed, InheritedFont, InheritedTextFlow, StyleGroup};
use crate::values::ValueCx;
use std::cell::Cell;

/// Distinct recent inputs kept: a page sets its text in a few fonts, each in two roles.
const CAPACITY: usize = 16;

/// The font of a run and its own size, when the run's style declares one.
pub(crate) type RunFace = (gpui::Font, Option<gpui::Pixels>);

/// What the font is derived for: the two derivations differ for the same style.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    /// The font a run of the style's text is set in.
    Run,
    /// The font the style's own text is measured with.
    Measure,
}

struct Entry {
    role: Role,
    /// The groups are held, so their identity is stable while the entry lives.
    font: StyleGroup<InheritedFont>,
    text_flow: StyleGroup<InheritedTextFlow>,
    orientation: FontOrientation,
    base: gpui::Font,
    base_px: u32,
    doc: *const crate::values::DocumentValues,
    generation: u64,
    face: RunFace,
}

#[derive(Default)]
pub(crate) struct RunFaces {
    /// Taken out while read or written: the memory never lends itself across a call.
    entries: Cell<Vec<Entry>>,
    /// Slot of the next entry once the memory is full.
    next: Cell<usize>,
    /// Builds of the document in progress (a nested document builds inside its host's).
    builds: Cell<u8>,
}

/// A document build in progress: the memory is empty when it starts and when it ends.
pub(crate) struct Build<'a>(&'a RunFaces);

impl Drop for Build<'_> {
    fn drop(&mut self) {
        self.0.clear();
        self.0.builds.set(self.0.builds.get().saturating_sub(1));
    }
}

impl RunFaces {
    /// Start a build of the document; dropping the result ends it.
    pub(crate) fn build(&self) -> Build<'_> {
        self.builds.set(self.builds.get().saturating_add(1));
        self.clear();
        Build(self)
    }

    /// The memory while the document is being built, none outside its builds.
    pub(crate) fn active(&self) -> Option<&Self> {
        (self.builds.get() > 0).then_some(self)
    }

    fn clear(&self) {
        drop(self.entries.take());
        self.next.set(0);
    }

    /// The face derived earlier in this build from equal inputs, else `derive`d and kept.
    pub(crate) fn face(
        &self,
        role: Role,
        style: &Computed,
        base: &gpui::Font,
        base_px: f32,
        vcx: ValueCx<'_>,
        derive: impl FnOnce() -> RunFace,
    ) -> RunFace {
        let orientation = style.font_request("", 0.0).orientation;
        let doc: *const crate::values::DocumentValues = vcx.doc;
        let generation = vcx.doc.font_snapshot().generation;
        let entries = self.entries.take();
        let hit = entries
            .iter()
            .find(|e| {
                e.role == role
                    && e.base_px == base_px.to_bits()
                    && e.orientation == orientation
                    && e.doc == doc
                    && e.generation == generation
                    && e.font == style.inh.font
                    && e.text_flow == style.inh.text_flow
                    && e.base == *base
            })
            .map(|e| e.face.clone());
        self.entries.set(entries);
        if let Some(face) = hit {
            return face;
        }
        // `derive` reads the document's fonts and may come back here: nothing is held.
        let face = derive();
        let entry = Entry {
            role,
            font: style.inh.font.clone(),
            text_flow: style.inh.text_flow.clone(),
            orientation,
            base: base.clone(),
            base_px: base_px.to_bits(),
            doc,
            generation,
            face: face.clone(),
        };
        let mut entries = self.entries.take();
        if entries.len() < CAPACITY {
            entries.push(entry);
        } else {
            let at = self.next.get() % CAPACITY;
            entries[at] = entry;
            self.next.set(at + 1);
        }
        self.entries.set(entries);
        face
    }
}

#[cfg(test)]
#[path = "run_faces_tests.rs"]
mod tests;
