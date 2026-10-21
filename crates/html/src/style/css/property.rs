//! Реестр свойств (Blink `css_properties.json5`, Stylo `longhands.toml` /
//! `shorthands.toml`): длинные свойства с начальным значением, флагом
//! наследования, грамматикой и логической группой; сокращения с их длинными
//! свойствами; общие ключевые слова css-cascade-5 §7.3. Реестр растёт по
//! семействам: свойство вне него разбирается и применяется по имени, как
//! прежде.

mod longhands;
mod shorthands;
#[cfg(test)]
mod tests;

use crate::document::state::ValueCx;
pub use longhands::{BoxGroup, Grammar, LonghandId, Slot};
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
        || value.contains("attr(")
        || value.contains("sibling-")
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

/// Новое имя свойства для устаревшего (css-cascade-5 §2.1 «legacy name
/// alias»: при разборе старое имя превращается в новое, и оба имени делят
/// одно место каскада). Список — Blink `css_properties.json5` `alias_for` для
/// свойств, чей разбор у нас одинаков под обоими именами.
pub fn legacy_name_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "-webkit-appearance" => "appearance",
        "-webkit-background-clip" => "background-clip",
        "-webkit-user-select" => "user-select",
        "-webkit-mask-clip" => "mask-clip",
        "-webkit-mask-origin" => "mask-origin",
        "-webkit-mask-position" => "mask-position",
        "-webkit-mask-repeat" => "mask-repeat",
        "-webkit-mask-size" => "mask-size",
        _ => return None,
    })
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
