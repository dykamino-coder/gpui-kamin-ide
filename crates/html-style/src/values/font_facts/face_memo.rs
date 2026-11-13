//! Память набора одного документа: имя семейства, под которым файл знает система шрифтов,
//! и итоговые возможности OpenType (css-fonts-4 §7.2).
//!
//! Оба значения — функция группы шрифта, языка, ориентации глифов, запрета необязательных
//! лигатур (css-text-3 §8.2) и снимка реестра шрифтов, а не коробки: коробки с этими входами
//! (ячейки таблицы, пункты списка, абзацы) получают один готовый результат, а не собирают
//! список тегов заново. Значения отдаются общим указателем, поэтому повторное чтение не
//! выделяет память. Запись держит только группу шрифта: группу потока текста она не
//! держит, иначе её запись в коробке копировала бы группу вместо правки на месте.
//!
//! Записи действуют для текущего снимка реестра: новый снимок сбрасывает их
//! (`DocumentValues::font_snapshot`). Полная память сбрасывается целиком. Записи с равными
//! значениями делят одно семейство, один список возможностей и одну строку языка.

use super::DocumentValues;
use crate::style::computed::{Computed, InheritedFont, StyleGroup};
use kamin_font_facts::font_facts::FontOrientation;
use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const CAPACITY: usize = 16;

/// What a box's text is set with, shared by every box of the same font inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct UsedFace {
    /// The declared family under the name the font system knows; none when no family is named.
    pub family: Option<Arc<str>>,
    /// The resolved OpenType features, in the order of css-fonts-4 §7.2.
    pub features: Arc<Vec<(String, u32)>>,
}

struct Entry {
    font: StyleGroup<InheritedFont>,
    /// A thin pointer: the entry stays at six words.
    lang: Option<Rc<String>>,
    orientation: FontOrientation,
    ligatures_off: bool,
    value: UsedFace,
}

#[derive(Default)]
pub(super) struct UsedFaces {
    /// Taken out for each lookup, so no borrow outlives it.
    entries: Cell<Vec<Entry>>,
}

impl UsedFaces {
    /// Forget the faces of a replaced registry snapshot.
    pub(super) fn clear(&self) {
        self.entries.take();
    }
}

impl DocumentValues {
    /// The family and features `style` sets its text with, derived once per distinct input.
    pub fn used_face(&self, style: &Computed) -> UsedFace {
        // Сначала снимок: новый снимок сбрасывает записи старого.
        self.font_snapshot();
        let orientation = style.font_request("", 0.0).orientation;
        let ligatures_off = style.optional_ligatures_off();
        let lang = style.inh.text_flow.lang.as_deref();
        let memo = &self.font_facts.faces;
        let entries = memo.entries.take();
        let hit = entries
            .iter()
            .find(|e| {
                e.orientation == orientation
                    && e.ligatures_off == ligatures_off
                    && e.lang.as_deref().map(String::as_str) == lang
                    && e.font == style.inh.font
            })
            .map(|e| e.value.clone());
        memo.entries.set(entries);
        if let Some(value) = hit {
            return value;
        }
        let (family, features) = self.derive_face(style);
        let mut entries = memo.entries.take();
        let value = UsedFace {
            family: family.map(|family| {
                entries
                    .iter()
                    .find_map(|e| e.value.family.clone().filter(|known| **known == *family))
                    .unwrap_or_else(|| Arc::from(&*family))
            }),
            features: entries
                .iter()
                .find(|e| *e.value.features == features)
                .map_or_else(|| Arc::new(features), |e| e.value.features.clone()),
        };
        let lang = lang.map(|lang| {
            entries
                .iter()
                .find_map(|e| e.lang.clone().filter(|known| **known == *lang))
                .unwrap_or_else(|| Rc::new(lang.to_owned()))
        });
        if entries.len() == CAPACITY {
            entries.clear();
        } else if entries.len() == entries.capacity() && !entries.is_empty() {
            // Половина сверху, а не удвоение: память живёт весь документ, а различных
            // лиц у страницы немного.
            entries.reserve_exact((entries.len() / 2).max(1).min(CAPACITY - entries.len()));
        }
        entries.push(Entry {
            font: style.inh.font.clone(),
            lang,
            orientation,
            ligatures_off,
            value: value.clone(),
        });
        memo.entries.set(entries);
        value
    }

    /// The family under its system name and the feature list of `style`, not yet shared.
    fn derive_face<'a>(&self, style: &'a Computed) -> (Option<Cow<'a, str>>, Vec<(String, u32)>) {
        let family = style.inh.font.font_family.as_deref().unwrap_or("");
        // Имя из разметки может быть придуманным (`@font-face`): система шрифтов знает файл
        // под его собственным именем, а лицо среди нескольких выбирает запрос стиля.
        let named = (!family.is_empty()).then(|| {
            self.font_snapshot()
                .alias_for(family, style.font_request(family, 0.0))
                .map_or(Cow::Borrowed(family), Cow::Owned)
        });
        (named, style.used_features(self.values()))
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

    #[test]
    fn equal_values_of_other_groups_share_one_family_and_feature_list() {
        let doc = DocumentValues::detached(Default::default());
        let light = style("font-family: Arial; font-weight: 300", &doc);
        let bold = style("font-family: Arial; font-weight: 700", &doc);
        let (a, b) = (doc.used_face(&light), doc.used_face(&bold));
        assert!(!light.inh.font.ptr_eq(&bold.inh.font));
        assert!(Arc::ptr_eq(&a.features, &b.features));
        assert!(Arc::ptr_eq(
            a.family.as_ref().unwrap(),
            b.family.as_ref().unwrap()
        ));
    }

    #[test]
    fn only_the_language_and_ligatures_of_the_text_flow_are_inputs() {
        let doc = DocumentValues::detached(Default::default());
        let plain = style("font-family: Arial", &doc);
        let mut indented = plain.clone();
        indented.inh.text_flow.orphans = Some(4);
        assert!(!plain.inh.text_flow.ptr_eq(&indented.inh.text_flow));
        let (a, b) = (doc.used_face(&plain), doc.used_face(&indented));
        assert!(Arc::ptr_eq(&a.features, &b.features));
        assert_eq!(doc.font_facts.faces.entries.take().len(), 1);
        let mut spaced = plain.clone();
        spaced.inh.text_flow.letter_spacing = Some(crate::style::values::value::Len::Px(1.0));
        let mut greek = plain;
        greek.inh.text_flow.lang = Some("el".into());
        assert_eq!(
            *doc.used_face(&spaced).features,
            spaced.used_features(doc.values())
        );
        assert_eq!(
            *doc.used_face(&greek).features,
            greek.used_features(doc.values())
        );
        assert_eq!(doc.font_facts.faces.entries.take().len(), 2);
    }

    #[test]
    fn a_new_registry_snapshot_drops_the_entries_of_the_old_one() {
        let doc = DocumentValues::detached(Default::default());
        for weight in [300, 400, 700] {
            let mut one = Computed::default();
            one.inh.font.font_weight = Some(weight);
            doc.used_face(&one);
        }
        doc.font_registry_changed();
        doc.used_face(&Computed::default());
        assert_eq!(doc.font_facts.faces.entries.take().len(), 1);
    }
}
