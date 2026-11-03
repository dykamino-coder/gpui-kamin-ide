//! Bounded memo of used fonts (face selection and size adjustment) of one document.
//! A result is a function of the computed font group, the language, the glyph
//! orientation, the family list, the computed size, the font snapshot generation
//! and the provider; entries keep their font group alive, so identity is stable.

use super::DocumentValues;
use crate::style::computed::{InheritedFont, StyleGroup};
use kamin_font_facts::font_facts::{FontOrientation, UsedFont};
use std::cell::Cell;

const CAPACITY: usize = 16;

/// Inputs of one used font; borrowed for lookup, owned only on insertion.
pub struct UsedFontKey<'a> {
    pub font: &'a StyleGroup<InheritedFont>,
    pub language: Option<&'a str>,
    pub orientation: FontOrientation,
    pub family: &'a str,
    pub computed_px: f32,
}

struct Entry {
    font: StyleGroup<InheritedFont>,
    language: Option<String>,
    orientation: FontOrientation,
    family: String,
    computed_px: u32,
    generation: u64,
    provider: *const (),
    value: UsedFont,
}

#[derive(Default)]
pub(super) struct UsedFonts {
    /// Taken out for each lookup, so no borrow outlives it and `select` may re-enter.
    entries: Cell<Vec<Entry>>,
    next: Cell<usize>,
}

impl Entry {
    fn matches(&self, key: &UsedFontKey<'_>, generation: u64, provider: *const ()) -> bool {
        self.generation == generation
            && self.provider == provider
            && self.computed_px == key.computed_px.to_bits()
            && self.orientation == key.orientation
            && self.font.ptr_eq(key.font)
            && self.family == key.family
            && self.language.as_deref() == key.language
    }
}

impl DocumentValues {
    /// The used font for `key`, computed by `select` on a miss.
    pub fn used_font_memo(
        &self,
        key: UsedFontKey<'_>,
        select: impl FnOnce() -> UsedFont,
    ) -> UsedFont {
        let generation = self.font_generation() + self.font_facts.registry_generation.get();
        let provider = std::rc::Rc::as_ptr(&self.font_provider()) as *const ();
        let memo = &self.font_facts.used;
        let entries = memo.entries.take();
        let hit = entries
            .iter()
            .find(|e| e.matches(&key, generation, provider))
            .map(|e| e.value.clone());
        memo.entries.set(entries);
        if let Some(value) = hit {
            return value;
        }
        let value = select();
        let entry = Entry {
            font: key.font.clone(),
            language: key.language.map(str::to_owned),
            orientation: key.orientation,
            family: key.family.to_owned(),
            computed_px: key.computed_px.to_bits(),
            generation,
            provider,
            value: value.clone(),
        };
        let mut entries = memo.entries.take();
        if entries.len() < CAPACITY {
            entries.push(entry);
        } else {
            let at = memo.next.get() % CAPACITY;
            entries[at] = entry;
            memo.next.set(at + 1);
        }
        memo.entries.set(entries);
        value
    }
}

#[cfg(test)]
mod tests;
