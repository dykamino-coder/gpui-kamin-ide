//! Обработчики длинных свойств текста и оформления реестра: каждое свойство
//! пишет ТОЛЬКО свои поля стиля. Значение проверено разбором (`values`);
//! `inherit` у наследуемого свойства — очистка своих полей (пустое поле
//! берётся от родителя), у ненаследуемого его решает реестр полей
//! `inherited_value` до обработчика.

use super::values;
use crate::style::computed::{Computed, DECOR_THROUGH, DECOR_UNDER};
use crate::style::css::LonghandId;
use crate::style::values::value::StyleColor;
use crate::values::ValueCx;

impl Computed {
    pub fn apply_text_longhand(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        use LonghandId::*;
        let inherit = v.trim().eq_ignore_ascii_case("inherit");
        match id {
            WhiteSpaceCollapse if inherit => {
                self.inh.text_flow.keep_spaces = None;
                self.inh.text_flow.preserve_newlines = None;
                self.inh.text_flow.break_after_spaces = None;
            }
            WhiteSpaceCollapse => {
                if let Some(c) = values::collapse(v) {
                    self.accept_declaration();
                    self.inh.text_flow.keep_spaces = Some(c.keep_spaces);
                    self.inh.text_flow.preserve_newlines = Some(c.preserve_newlines);
                    self.inh.text_flow.break_after_spaces = Some(c.break_after_spaces);
                }
            }
            TextWrapMode if inherit => self.inh.text_flow.nowrap = None,
            TextWrapMode => {
                if let Some(nowrap) = values::wrap_mode(v) {
                    self.inh.text_flow.nowrap = accept_parsed!(self, Some(nowrap));
                }
            }
            TextWrapStyle if inherit => self.inh.text_flow.balance_lines = None,
            TextWrapStyle => {
                if let Some(balance) = values::wrap_style(v) {
                    self.inh.text_flow.balance_lines = accept_parsed!(self, Some(balance));
                }
            }
            TextEmphasisStyle if inherit => {
                self.inh.text_ink.text_emphasis = None;
                self.inh.text_ink.emphasis_auto_shape = false;
            }
            TextEmphasisStyle => {
                if let Some((mark, auto_shape)) = values::emphasis_style(v) {
                    self.inh.text_ink.text_emphasis = accept_parsed!(self, Some(mark));
                    self.inh.text_ink.emphasis_auto_shape = auto_shape;
                }
            }
            TextEmphasisColor if inherit => self.inh.text_ink.emphasis_color = None,
            TextEmphasisColor => {
                if let Some(c) = StyleColor::parse_value(v, vcx) {
                    self.inh.text_ink.emphasis_color = accept_parsed!(self, Some(c));
                }
            }
            _ if inherit => {}
            TextDecorationLine => {
                if let Some(lines) = values::decoration_line(v) {
                    self.reset_text_box.td_lines = accept_parsed!(self, Some(lines));
                    self.reset_text_box.underline = Some(lines & DECOR_UNDER != 0);
                    self.reset_text_box.line_through = Some(lines & DECOR_THROUGH != 0);
                }
            }
            TextDecorationStyle => {
                if let Some(s) = values::decoration_style(v) {
                    self.reset_text_box.td_style = accept_parsed!(self, Some(s));
                }
            }
            TextDecorationColor => {
                if let Some(c) = StyleColor::parse_value(v, vcx) {
                    self.reset_text_box.td_color = accept_parsed!(self, c);
                }
            }
            TextDecorationThickness => {
                if let Some(t) = values::decoration_thickness(v, vcx) {
                    self.reset_text_box.td_thickness = accept_parsed!(self, Some(t));
                }
            }
            _ => {}
        }
    }
}
