//! Реестр свойств (Blink `css_properties.json5`, Stylo `longhands.toml` /
//! `shorthands.toml`): длинные свойства с начальным значением, флагом
//! наследования, грамматикой и логической группой; сокращения с их длинными
//! свойствами; общие ключевые слова css-cascade-5 §7.3. Реестр растёт по
//! семействам: свойство вне него разбирается и применяется по имени, как
//! прежде.

mod aliases;
mod font_shorthands;
mod grammar;
mod grid_shorthands;
mod inline_shorthands;
mod layer_shorthands;
#[cfg(test)]
mod layer_tests;
mod layer_words;
mod layout_shorthands;
#[cfg(test)]
mod layout_tests;
mod line_shorthands;
mod longhands;
mod mask_shorthands;
mod rule_shorthands;
mod shorthand_ids;
mod shorthands;
#[cfg(test)]
mod tests;
mod text_shorthands;
mod variant_shorthands;

use crate::values::ValueCx;
pub use aliases::{legacy_alias_value, legacy_name_alias};
pub use grammar::Grammar;
pub use layer_shorthands::split_position;
pub use layer_words::{Words as LayerWords, layer_list, layer_tokens};
pub use longhands::{BoxGroup, LonghandId, LonghandSet, Slot};
pub use shorthands::ShorthandId;

/// Общее ключевое слово (css-cascade-5 §7.3, css-values-4 §3.1.1): годится
/// любому свойству и только целым значением.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CssWide {
    Initial,
    Inherit,
    Unset,
    Revert,
    RevertLayer,
}

impl CssWide {
    /// Значение целиком — общее слово (без учёта регистра)?
    pub fn parse(value: &str) -> Option<Self> {
        let v = value.trim();
        [
            ("initial", Self::Initial),
            ("inherit", Self::Inherit),
            ("unset", Self::Unset),
            ("revert", Self::Revert),
            ("revert-layer", Self::RevertLayer),
        ]
        .into_iter()
        .find(|(name, _)| v.eq_ignore_ascii_case(name))
        .map(|(_, w)| w)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Inherit => "inherit",
            Self::Unset => "unset",
            Self::Revert => "revert",
            Self::RevertLayer => "revert-layer",
        }
    }
}

/// Значение с подстановкой (`var()`, `attr()`, `sibling-index()`): разбор
/// откладывается до вычисления (css-variables-1 §3, css-values-5 §7.7).
pub fn has_substitution(value: &str) -> bool {
    crate::style::css::variable_values::has_var(value)
        || crate::css_syntax::functions(value).any(|f| {
            ["attr", "sibling-index", "sibling-count"]
                .iter()
                .any(|name| f.name.eq_ignore_ascii_case(name))
        })
}

/// Заданное значение объявления в блоке (css-cascade-5 §4: «declared
/// value»). Текст значения остаётся в [`Declaration::value`](super::Declaration).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Specified {
    /// Обычное значение: у длинного свойства реестра — уже проверенное
    /// разбором, у остальных — сырое, его проверяет применение.
    Value,
    /// Общее ключевое слово.
    Wide(CssWide),
    /// Значение с подстановкой; разбирается после неё. У длинного свойства из
    /// сокращения текст — значение сокращения целиком (Blink
    /// `CSSPendingSubstitutionValue`, Stylo `WithVariables { from_shorthand }`).
    Pending(Option<ShorthandId>),
}

impl Specified {
    /// Вид значения по тексту.
    pub fn of(value: &str) -> Self {
        match CssWide::parse(value) {
            Some(w) => Self::Wide(w),
            None if has_substitution(value) => Self::Pending(None),
            None => Self::Value,
        }
    }
}

/// Дописать в блок разобранное объявление `name: value`: сокращение реестра —
/// его длинными свойствами на одном месте блока, длинное свойство реестра —
/// только годным значением. Без документа (`vcx`) значения длинных свойств не
/// проверяются: так разбирается лишь CSS самого движка (умолчания, подсказки).
pub fn push_declared(
    out: &mut super::DeclBlock,
    name: String,
    value: &str,
    important: bool,
    vcx: Option<ValueCx<'_>>,
) {
    // The old name is parsed with its own, wider grammar (Blink
    // `UseAliasParsing`) and only then renamed.
    let legacy = legacy_alias_value(&name, value);
    let value: &str = &legacy;
    let name = legacy_name_alias(&name).map_or(name, str::to_string);
    let pos = out.next_pos();
    if let Some(shorthand) = ShorthandId::from_name(&name) {
        for (id, specified, text) in shorthand.expand(value, vcx).unwrap_or_default() {
            out.push_decl(super::Declaration {
                name: id.name().to_string(),
                value: text,
                important,
                pos,
                id: Some(id),
                specified,
            });
        }
        return;
    }
    let decl = super::Declaration::new(name, value, important, pos);
    if let (Some(id), Specified::Value, Some(vcx)) = (decl.id, decl.specified, vcx)
        && !id.accepts(value, vcx)
    {
        return;
    }
    out.push_decl(decl);
}
