//! Разбор CSS: декларации из `style=""` и правила из `<style>`.
//!
//! Своя реализация вместо `cssparser`: по замеру нашего же кода 82% селекторов —
//! одиночный класс, глубина не больше трёх, комбинаторов `>`/`+` на весь проект
//! четырнадцать. Полноценная CSS-машина здесь не окупается, а лишняя
//! зависимость — окупается ещё меньше.

use std::collections::HashMap;

pub(super) mod selector_tokens;
mod stylesheet_tokens;
use crate::style::css::stylesheet_tokens::find_matching;
pub(crate) use stylesheet_tokens::next_piece;
mod component_tokens;
mod font_family_values;
pub(crate) use component_tokens::skip_string;
mod priority_tokens;
use crate::style::css::priority_tokens::top_level_bang;
pub(crate) mod custom_properties;
pub(super) mod selector;
mod variable_tokens;
pub(crate) mod variable_values;
pub use crate::style::css::selector::*;
pub(super) mod decls;
pub use crate::style::css::decls::*;
pub(super) mod sheet;
pub use crate::style::css::sheet::*;
mod media;
pub use crate::style::css::media::*;
pub(crate) mod supports;
pub(crate) use crate::style::css::supports::*;
pub(super) mod at_rules;
pub use crate::style::css::at_rules::*;
pub(super) mod keyframes;
pub use crate::style::css::keyframes::*;

/// Пара «свойство: значение». Значение хранится сырым — разбор откладывается
/// до момента применения, чтобы неизвестные свойства не стоили ничего.
pub type Decls = HashMap<String, String>;

/// Разделитель повторных объявлений одного свойства внутри значения.
pub const DECL_SEP: char = char::from_u32(1).unwrap();

/// Служебный ключ со списком свойств В ПОРЯДКЕ ЗАПИСИ.
///
/// Каскад решает порядком объявлений (CSS 2.1 §6.4.1), а словарь его не
/// помнит: `background-color: red; background: green` и обратная запись
/// давали ОДИН исход. Имя начинается со служебного знака — свойства с
/// таким именем в разметке не бывает.
pub const ORDER_KEY: &str = "\u{2}order";

/// Priority is metadata: an escaped `!` inside a custom value is not !important.
pub(super) const CUSTOM_IMPORTANT: &str = "\u{2}important:";

/// Одно правило: с чем сопоставлять и что применять.
#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: Decls,
    /// Порядок в исходнике: при равной специфичности выигрывает последнее.
    pub order: usize,
    /// Откуда правило: 0 — таблица агента, 1 — таблица документа.
    ///
    /// Происхождение СТАРШЕ специфичности (CSS Cascade §6.4.4): авторское
    /// правило перебивает умолчание агента, даже когда специфичность у него
    /// ниже. Пока обе таблицы лежали в одном списке и сравнивались только
    /// специфичностью, `* { margin: 0 }` со специфичностью (0,0,0) проигрывал
    /// нашему `p { margin: 6px 0 }` — то есть не работал ни один reset.
    pub origin: u8,
    /// Каскадный слой (css-cascade-5 §6.4): путь индексов от корня слоёв,
    /// собственные правила слоя — с хвостом `u32::MAX`, поэтому они идут
    /// ПОСЛЕ своих подслоёв; правила вне слоёв — `[u32::MAX]`, последний
    /// неявный слой. Обычные объявления сравниваются по возрастанию пути,
    /// важные — по убыванию.
    pub layer: Vec<u32>,
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod media_probe {
    use super::*;

    /// `0px` кончается на `x`, как короткая запись разрешения `dppx`.
    /// Жадное отрезание давало «0p», разбор проваливался, и вся фича
    /// молча становилась `<general-enclosed>`.
    #[test]
    fn media_without_space_before_paren() {
        let m = Media::default();
        assert!(m.matches("@media(min-width:0px)"), "no-space form");
        assert!(m.matches("@media (min-width:0px)"), "spaced form");
        assert!(m.matches("@media"), "empty query");
    }
}
