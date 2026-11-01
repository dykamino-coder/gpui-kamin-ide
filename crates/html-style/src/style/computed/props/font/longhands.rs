//! Обработчики длинных свойств шрифта реестра: каждое свойство пишет ТОЛЬКО
//! свои поля стиля. Значение проверено разбором (`values`); `inherit` —
//! очистка своих полей: пустое поле у наследуемого свойства берётся от
//! родителя при наследовании.

use super::values::{self, FontSize as Size, GROUP_TAGS};
use crate::style::computed::Computed;
use crate::style::css::LonghandId;
use crate::values::ValueCx;

impl Computed {
    pub(super) fn apply_font_longhand(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        use LonghandId::*;
        let inherit = v.trim().eq_ignore_ascii_case("inherit");
        match id {
            FontStyle if inherit => {
                (
                    self.inh.font.italic,
                    self.inh.font.oblique,
                    self.inh.font.font_oblique_angle,
                ) = (None, None, None);
            }
            FontStyle => {
                if let Some((italic, oblique, angle)) = values::font_style(v) {
                    self.accept_declaration();
                    self.inh.font.italic = Some(italic);
                    self.inh.font.oblique = Some(oblique);
                    self.inh.font.font_oblique_angle = angle;
                }
            }
            FontVariantAlternates if inherit => self.inh.font.font_alternates = None,
            FontVariantAlternates => {
                if let Some(alternates) =
                    crate::style::computed::font_alternates::parse(v, false, vcx)
                {
                    self.inh.font.font_alternates = accept_parsed!(self, Some(alternates));
                }
            }
            FontVariantEmoji if inherit => self.inh.font.font_variant_emoji = None,
            FontVariantEmoji => {
                if let Some(e) = values::emoji(v) {
                    self.inh.font.font_variant_emoji = accept_parsed!(self, Some(e));
                }
            }
            FontVariantLigatures | FontVariantCaps | FontVariantNumeric | FontVariantEastAsian
            | FontVariantPosition => self.set_variant_group(id, v, inherit),
            FontWeight if inherit => {
                (
                    self.inh.font.font_weight,
                    self.reset_cascade.font_weight_step,
                ) = (None, 0)
            }
            FontWeight => {
                if let Some((weight, step)) = values::font_weight(v, vcx) {
                    self.accept_declaration();
                    (
                        self.inh.font.font_weight,
                        self.reset_cascade.font_weight_step,
                    ) = (weight, step);
                }
            }
            FontWidth if inherit => self.inh.font.font_stretch = None,
            FontWidth => {
                if let Some(pct) = values::font_width(v, vcx) {
                    self.inh.font.font_stretch = accept_parsed!(self, Some(pct));
                }
            }
            FontSize if inherit => {
                (self.inh.font.font_size, self.reset_cascade.font_size_step) = (None, 0);
                self.reset_svg_math.math_font_size = false;
            }
            FontSize => {
                let Some(size) = values::font_size(v, vcx) else {
                    return;
                };
                self.accept_declaration();
                (
                    self.inh.font.font_size,
                    self.reset_cascade.font_size_step,
                    self.reset_svg_math.math_font_size,
                ) = match size {
                    Size::Math => (None, 0, true),
                    Size::Keyword(len, step) => (Some(len), step, false),
                    Size::Length(len) => (Some(len), 0, false),
                };
            }
            LineHeight if inherit => self.inh.font.line_height = None,
            LineHeight => {
                if let Some(lh) = values::line_height(v, vcx) {
                    self.inh.font.line_height = accept_parsed!(self, Some(lh));
                }
            }
            FontFamily => crate::style::computed::font_family::apply(self, v, vcx),
            FontKerning if inherit => self.inh.font.font_kerning = None,
            FontKerning => {
                if let Some(k) = values::kerning(v) {
                    self.inh.font.font_kerning = accept_parsed!(self, Some(k));
                }
            }
            FontSizeAdjust if inherit => self.inh.font.font_size_adjust = None,
            FontSizeAdjust => {
                if let Some(adjust) = values::size_adjust(v, vcx) {
                    self.inh.font.font_size_adjust = accept_parsed!(self, Some(adjust));
                }
            }
            FontFeatureSettings if inherit => self.inh.font.font_settings = None,
            FontFeatureSettings => {
                if let Some(list) = crate::style::computed::feature_list(v) {
                    self.inh.font.font_settings = accept_parsed!(self, Some(list));
                }
            }
            FontSynthesisWeight
            | FontSynthesisStyle
            | FontSynthesisSmallCaps
            | FontSynthesisPosition => self.set_synthesis(id, v, inherit),
            _ => {}
        }
    }

    /// `font-variant-*` с тегами: своя группа тегов и свой разряд «задано».
    fn set_variant_group(&mut self, id: LonghandId, v: &str, inherit: bool) {
        let Some(group) = values::variant_group(id) else {
            return;
        };
        let tags = if inherit {
            None
        } else {
            match values::variant_tags(id, v) {
                Some(tags) => Some(tags),
                None => return,
            }
        };
        let own = GROUP_TAGS[group];
        self.inh
            .font
            .font_features
            .retain(|(t, _)| !own.contains(&t.as_str()));
        match tags {
            Some(tags) => {
                self.accept_declaration();
                self.reset_cascade.font_variant_set |= 1 << group;
                self.inh
                    .font
                    .font_features
                    .extend(tags.into_iter().map(|(t, on)| (t.to_string(), on)));
            }
            None => self.reset_cascade.font_variant_set &= !(1 << group),
        }
    }

    /// `font-synthesis-*`: разрешена ли подмена; у `-style` ещё признак
    /// «только наклон» (`oblique-only`).
    fn set_synthesis(&mut self, id: LonghandId, v: &str, inherit: bool) {
        let value = if inherit {
            None
        } else {
            match values::synthesis(id, v) {
                Some(s) => Some(s),
                None => return,
            }
        };
        self.accept_declaration();
        let allow = value.map(|s| s != 0);
        match id {
            LonghandId::FontSynthesisWeight => self.inh.font.font_synth.0 = allow,
            LonghandId::FontSynthesisStyle => {
                self.inh.font.font_synth.1 = allow;
                self.inh.font.font_oblique_only = value.map(|s| s == 2);
            }
            LonghandId::FontSynthesisSmallCaps => self.inh.font.font_synth.2 = allow,
            _ => self.inh.font.font_synth_position = allow,
        }
    }
}
