//! Разбор CSS: декларации из `style=""` и правила из `<style>`.
//!
//! Своя реализация вместо `cssparser`: по замеру нашего же кода 82% селекторов —
//! одиночный класс, глубина не больше трёх, комбинаторов `>`/`+` на весь проект
//! четырнадцать. Полноценная CSS-машина здесь не окупается, а лишняя
//! зависимость — окупается ещё меньше.

pub(crate) mod resource_rules;
pub(crate) mod selector_tokens;
mod stylesheet_tokens;
use crate::style::css::stylesheet_tokens::find_matching;
pub(crate) use stylesheet_tokens::next_piece;
mod component_tokens;
mod font_family_values;
pub(crate) mod value_tokens;
pub(crate) use component_tokens::skip_string;
mod priority_tokens;
use crate::style::css::priority_tokens::top_level_bang;
pub(crate) mod custom_properties;
pub(super) mod selector;
mod variable_tokens;
pub(crate) use variable_tokens::block_end;
pub(crate) mod variable_values;
pub use crate::style::css::selector::*;
pub(super) mod declaration;
pub use crate::style::css::declaration::{DeclBlock, Declaration, VarMap};
pub(super) mod decls;
pub use crate::style::css::decls::*;
pub(crate) use decls::split_top_level;
pub(super) mod sheet;
pub use crate::style::css::sheet::*;
mod media;
pub use crate::style::css::media::*;
pub(crate) mod supports;
pub(crate) use crate::style::css::supports::*;
// The separately maintained at-rule registry exposes its heterogeneous CSS records.
#[allow(clippy::type_complexity)]
pub(super) mod at_rules;
pub use crate::style::css::at_rules::*;
pub(super) mod keyframes;
pub use crate::style::css::keyframes::*;

/// Одно правило: с чем сопоставлять и что применять.
#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: DeclBlock,
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
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let m = Media::default();
        assert!(m.matches("@media(min-width:0px)", vcx), "no-space form");
        assert!(m.matches("@media (min-width:0px)", vcx), "spaced form");
        assert!(m.matches("@media", vcx), "empty query");
    }
}
