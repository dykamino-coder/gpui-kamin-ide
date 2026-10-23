//! Наследуемые длинные свойства реестра вне шрифта и текста (и ненаследуемые
//! части их сокращений: `text-box-trim`, `max-lines`, `continue`): ОДИН
//! разбор на свойство — им реестр проверяет объявление при разборе блока, им
//! же обработчик пишет поля стиля. Каждое свойство пишет ТОЛЬКО свои поля;
//! `inherit` у наследуемого — очистка своих полей (пустое поле берётся от
//! родителя слиянием `cascade::inherit`), `initial` — запись начального
//! значения реестра тем же разбором.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::{Grammar, LonghandId};

mod breaking;
mod clamp;
mod decor;
mod hyphen_limits;
mod list;
pub use list::MarkerType;
mod spacing;
mod svg_paint;
mod table;
mod text_box_values;
mod typography;
mod ui;
mod writing;

/// Свойство разбирается этим модулем?
pub(crate) fn owns(id: LonghandId) -> bool {
    matches!(
        id.grammar(),
        Grammar::List
            | Grammar::Writing
            | Grammar::Color
            | Grammar::Ui
            | Grammar::Svg
            | Grammar::Table
            | Grammar::Inline
    )
}

/// Годится ли значение (не общее слово, без подстановки) свойству.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    let v = value.trim();
    match id.grammar() {
        Grammar::List => list::accepts(id, v, vcx),
        Grammar::Writing => writing::accepts(id, v),
        Grammar::Color | Grammar::Ui => ui::accepts(id, v, vcx),
        Grammar::Svg => svg_paint::accepts(id, v, vcx),
        Grammar::Table => table::accepts(id, v, vcx),
        Grammar::Inline => spacing::accepts(id, v, vcx)
            .or_else(|| breaking::accepts(id, v))
            .or_else(|| typography::accepts(id, v))
            .or_else(|| clamp::accepts(id, v))
            .or_else(|| decor::accepts(id, v, vcx))
            .unwrap_or(false),
        _ => false,
    }
}

impl Computed {
    /// Записать проверенное значение свойства в его поля. Негодное значение
    /// (CSS движка без проверки разбором) не пишет ничего.
    pub(crate) fn apply_inherited_longhand(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        let v = v.trim();
        match id.grammar() {
            Grammar::List => self.apply_list(id, v, vcx),
            Grammar::Writing => self.apply_writing(id, v),
            Grammar::Color | Grammar::Ui => self.apply_ui(id, v, vcx),
            Grammar::Svg => self.apply_svg_paint(id, v, vcx),
            Grammar::Table => self.apply_table(id, v, vcx),
            _ => {
                let _ = self.apply_spacing(id, v, vcx)
                    || self.apply_breaking(id, v)
                    || self.apply_typography(id, v)
                    || self.apply_clamp(id, v)
                    || self.apply_decor(id, v, vcx);
            }
        }
    }

    /// `inherit` у наследуемого свойства (css-cascade-5 §7.3.1): свои поля
    /// пусты, значение придёт от родителя.
    pub(crate) fn inherit_longhand(&mut self, id: LonghandId) {
        match id.grammar() {
            Grammar::List => self.inherit_list(id),
            Grammar::Writing => self.inherit_writing(id),
            Grammar::Color | Grammar::Ui => self.inherit_ui(id),
            Grammar::Svg => self.inherit_svg_paint(id),
            Grammar::Table => self.inherit_table(id),
            _ => {
                let _ = self.inherit_spacing(id)
                    || self.inherit_breaking(id)
                    || self.inherit_typography(id)
                    || self.inherit_clamp(id)
                    || self.inherit_decor(id);
            }
        }
    }
}
