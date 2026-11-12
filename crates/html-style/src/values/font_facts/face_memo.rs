//! Память набора одного документа: имя семейства, под которым файл знает система шрифтов,
//! и итоговые возможности OpenType (css-fonts-4 §7.2) — по унаследованным группам стиля.
//!
//! Оба значения — функция групп шрифта и потока текста, ориентации глифов и снимка реестра
//! шрифтов, а не коробки: коробки с общими группами (ячейки таблицы, пункты списка, абзацы)
//! получают один готовый результат, а не собирают список тегов заново. Значения отдаются
//! общим указателем, поэтому повторное чтение не выделяет память. Запись держит свои
//! группы, так что их адрес не достаётся другому стилю, пока она жива.

use super::DocumentValues;
use crate::style::computed::{Computed, InheritedFont, InheritedTextFlow, StyleGroup};
use kamin_font_facts::font_facts::FontOrientation;
use std::cell::Cell;
use std::sync::Arc;

const CAPACITY: usize = 16;

/// What a box's text is set with, shared by every box of the same inherited groups.
#[derive(Clone, Debug, PartialEq)]
pub struct UsedFace {
    /// The declared family under the name the font system knows; none when no family is named.
    pub family: Option<Arc<str>>,
    /// The resolved OpenType features, in the order of css-fonts-4 §7.2.
    pub features: Arc<Vec<(String, u32)>>,
}

struct Entry {
    font: StyleGroup<InheritedFont>,
    text_flow: StyleGroup<InheritedTextFlow>,
    orientation: FontOrientation,
    generation: u64,
    value: UsedFace,
}

#[derive(Default)]
pub(super) struct UsedFaces {
    /// Taken out for each lookup, so no borrow outlives it.
    entries: Cell<Vec<Entry>>,
    next: Cell<usize>,
}

impl DocumentValues {
    /// The family and features `style` sets its text with, derived once per distinct input.
    pub fn used_face(&self, style: &Computed) -> UsedFace {
        let orientation = style.font_request("", 0.0).orientation;
        let generation = self.font_snapshot().generation;
        let memo = &self.font_facts.faces;
        let entries = memo.entries.take();
        let hit = entries
            .iter()
            .find(|e| {
                e.generation == generation
                    && e.orientation == orientation
                    && e.font == style.inh.font
                    && e.text_flow == style.inh.text_flow
            })
            .map(|e| e.value.clone());
        memo.entries.set(entries);
        if let Some(value) = hit {
            return value;
        }
        let value = self.derive_face(style);
        let entry = Entry {
            font: style.inh.font.clone(),
            text_flow: style.inh.text_flow.clone(),
            orientation,
            generation,
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

    fn derive_face(&self, style: &Computed) -> UsedFace {
        let family = style.inh.font.font_family.as_deref().unwrap_or("");
        // Имя из разметки может быть придуманным (`@font-face`): система шрифтов знает файл
        // под его собственным именем, а лицо среди нескольких выбирает запрос стиля.
        let named = (!family.is_empty()).then(|| {
            self.font_snapshot()
                .alias_for(family, style.font_request(family, 0.0))
                .map_or_else(|| Arc::from(family), Arc::from)
        });
        UsedFace {
            family: named,
            features: Arc::new(style.used_features(self.values())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::parse_decls;

    fn style(css: &str, doc: &DocumentValues) -> Computed {
        let mut style = Computed::default();
        style.apply_decls_with_vars(&parse_decls(css), &Default::default(), doc.values());
        style
    }

    #[test]
    fn boxes_of_the_same_groups_share_one_result() {
        let doc = DocumentValues::detached(Default::default());
        let first = style("font-family: Arial; font-kerning: none", &doc);
        let second = first.clone();
        let (a, b) = (doc.used_face(&first), doc.used_face(&second));
        assert!(Arc::ptr_eq(&a.features, &b.features));
        assert_eq!(a.family.as_deref(), Some("Arial"));
        assert_eq!(*a.features, first.used_features(doc.values()));
    }

    #[test]
    fn a_changed_input_derives_again() {
        let doc = DocumentValues::detached(Default::default());
        let plain = style("font-family: Arial", &doc);
        let spaced = style("font-family: Arial; letter-spacing: 2px", &doc);
        let nameless = style("font-weight: 700", &doc);
        let first = doc.used_face(&plain);
        let other = doc.used_face(&spaced);
        assert_eq!(*other.features, spaced.used_features(doc.values()));
        assert_ne!(first.features, other.features);
        assert_eq!(doc.used_face(&nameless).family, None);
        doc.font_registry_changed();
        let again = doc.used_face(&plain);
        assert!(!Arc::ptr_eq(&first.features, &again.features));
        assert_eq!(first, again);
    }

    #[test]
    fn the_memo_stays_bounded() {
        let doc = DocumentValues::detached(Default::default());
        for weight in 0..(CAPACITY * 3) {
            let mut one = Computed::default();
            one.inh.font.font_weight = Some(weight as u16 + 1);
            doc.used_face(&one);
        }
        assert_eq!(doc.font_facts.faces.entries.take().len(), CAPACITY);
    }
}
