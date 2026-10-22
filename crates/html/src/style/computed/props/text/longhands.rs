//! Обработчики длинных свойств текста и оформления реестра: каждое свойство
//! пишет ТОЛЬКО свои поля стиля. Значение проверено разбором (`values`);
//! `inherit` у наследуемого свойства — очистка своих полей (пустое поле
//! берётся от родителя), у ненаследуемого его решает реестр полей
//! `inherited_value` до обработчика.

use super::values;
use crate::document::state::ValueCx;
use crate::style::computed::{Computed, DECOR_THROUGH, DECOR_UNDER};
use crate::style::css::LonghandId;
use crate::style::values::value::StyleColor;

impl Computed {
    pub(crate) fn apply_text_longhand(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        use LonghandId::*;
        let inherit = v.trim().eq_ignore_ascii_case("inherit");
        match id {
            WhiteSpaceCollapse if inherit => {
                self.keep_spaces = None;
                self.preserve_newlines = None;
                self.break_after_spaces = None;
            }
            WhiteSpaceCollapse => {
                if let Some(c) = values::collapse(v) {
                    self.accept_declaration();
                    self.keep_spaces = Some(c.keep_spaces);
                    self.preserve_newlines = Some(c.preserve_newlines);
                    self.break_after_spaces = Some(c.break_after_spaces);
                }
            }
            TextWrapMode if inherit => self.nowrap = None,
            TextWrapMode => {
                if let Some(nowrap) = values::wrap_mode(v) {
                    self.nowrap = accept_parsed!(self, Some(nowrap));
                }
            }
            TextWrapStyle if inherit => self.balance_lines = None,
            TextWrapStyle => {
                if let Some(balance) = values::wrap_style(v) {
                    self.balance_lines = accept_parsed!(self, Some(balance));
                }
            }
            TextEmphasisStyle if inherit => {
                self.text_emphasis = None;
                self.emphasis_auto_shape = false;
            }
            TextEmphasisStyle => {
                if let Some((mark, auto_shape)) = values::emphasis_style(v) {
                    self.text_emphasis = accept_parsed!(self, Some(mark));
                    self.emphasis_auto_shape = auto_shape;
                }
            }
            TextEmphasisColor if inherit => self.emphasis_color = None,
            TextEmphasisColor => {
                if let Some(c) = StyleColor::parse_value(v, vcx) {
                    self.emphasis_color = accept_parsed!(self, Some(c));
                }
            }
            _ if inherit => {}
            TextDecorationLine => {
                if let Some(lines) = values::decoration_line(v) {
                    self.td_lines = accept_parsed!(self, Some(lines));
                    self.underline = Some(lines & DECOR_UNDER != 0);
                    self.line_through = Some(lines & DECOR_THROUGH != 0);
                }
            }
            TextDecorationStyle => {
                if let Some(s) = values::decoration_style(v) {
                    self.td_style = accept_parsed!(self, Some(s));
                }
            }
            TextDecorationColor => {
                if let Some(c) = StyleColor::parse_value(v, vcx) {
                    self.td_color = accept_parsed!(self, c);
                }
            }
            TextDecorationThickness => {
                if let Some(t) = values::decoration_thickness(v, vcx) {
                    self.td_thickness = accept_parsed!(self, Some(t));
                }
            }
            _ => {}
        }
    }
}
