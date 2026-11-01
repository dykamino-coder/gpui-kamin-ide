//! Применение наследуемого оформления.
use super::*;
impl Computed {
    pub(in crate::style::computed::props::inherited) fn apply_decor(
        &mut self,
        id: LonghandId,
        v: &str,
        vcx: ValueCx<'_>,
    ) -> bool {
        use LonghandId::*;
        match id {
            TextShadow => self.apply_text_shadow(v, vcx),
            TextUnderlineOffset => {
                self.inh.text_ink.underline_offset =
                    underline_offset(v, vcx).or(self.inh.text_ink.underline_offset)
            }
            TextUnderlinePosition => {
                self.inh.text_ink.underline_pos =
                    underline_position(v).or(self.inh.text_ink.underline_pos)
            }
            TextDecorationSkipInk => {
                self.inh.text_ink.skip_ink = skip_ink(v).or(self.inh.text_ink.skip_ink)
            }
            TextDecorationSkipSpaces => {
                self.inh.text_ink.skip_spaces = skip_spaces(v).or(self.inh.text_ink.skip_spaces)
            }
            TextEmphasisPosition => match emphasis_position(v) {
                Some(None) => {
                    self.inh.text_ink.emphasis_position_auto = Some(true);
                    (
                        self.inh.text_ink.emphasis_under,
                        self.inh.text_ink.emphasis_left,
                    ) = (false, false);
                }
                Some(Some((under, left))) => {
                    self.inh.text_ink.emphasis_position_auto = Some(false);
                    (
                        self.inh.text_ink.emphasis_under,
                        self.inh.text_ink.emphasis_left,
                    ) = (under, left);
                }
                None => {}
            },
            RubyPosition => {
                self.inh.text_flow.ruby_under = ruby_position(v).or(self.inh.text_flow.ruby_under)
            }
            RubyAlign => {
                self.inh.text_flow.ruby_align = ruby_align(v).or(self.inh.text_flow.ruby_align)
            }
            RubyOverhang => {
                self.inh.text_flow.ruby_overhang =
                    ruby_overhang(v).or(self.inh.text_flow.ruby_overhang)
            }
            RubyMerge => {
                self.inh.text_flow.ruby_merge = ruby_merge(v).or(self.inh.text_flow.ruby_merge)
            }
            _ => return false,
        }
        true
    }

    /// Тень в единицах шрифта ждёт кегля своего элемента сырой записью
    /// (`resolve_em` разбирает её снова уже в точках): наследуется
    /// вычисленное значение.
    fn apply_text_shadow(&mut self, v: &str, vcx: ValueCx<'_>) {
        if !text_shadow_ok(v, vcx) {
            return;
        }
        self.reset_text_box.text_shadow_none = v.eq_ignore_ascii_case("none");
        if has_font_units(v) {
            self.reset_text_box.text_shadow_raw = Some(v.to_string());
            self.inh.text_ink.text_shadow = None;
            self.inh.text_ink.text_shadow_rest.clear();
            return;
        }
        self.reset_text_box.text_shadow_raw = None;
        let mut list = parse_shadows(v, vcx).into_iter();
        self.inh.text_ink.text_shadow = list.next();
        self.inh.text_ink.text_shadow_rest = list.collect();
    }

    pub(in crate::style::computed::props::inherited) fn inherit_decor(
        &mut self,
        id: LonghandId,
    ) -> bool {
        use LonghandId::*;
        match id {
            TextShadow => {
                self.inh.text_ink.text_shadow = None;
                self.reset_text_box.text_shadow_raw = None;
                self.reset_text_box.text_shadow_none = false;
                self.inh.text_ink.text_shadow_rest.clear();
            }
            TextUnderlineOffset => self.inh.text_ink.underline_offset = None,
            TextUnderlinePosition => self.inh.text_ink.underline_pos = None,
            TextDecorationSkipInk => self.inh.text_ink.skip_ink = None,
            TextDecorationSkipSpaces => self.inh.text_ink.skip_spaces = None,
            TextEmphasisPosition => {
                self.inh.text_ink.emphasis_position_auto = None;
                (
                    self.inh.text_ink.emphasis_under,
                    self.inh.text_ink.emphasis_left,
                ) = (false, false);
            }
            RubyPosition => self.inh.text_flow.ruby_under = None,
            RubyAlign => self.inh.text_flow.ruby_align = None,
            RubyOverhang => self.inh.text_flow.ruby_overhang = None,
            RubyMerge => self.inh.text_flow.ruby_merge = None,
            _ => return false,
        }
        true
    }
}
