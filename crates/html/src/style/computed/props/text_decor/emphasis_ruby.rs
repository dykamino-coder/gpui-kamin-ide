//! Computed::apply_text_decor, хвост цепочки: text-emphasis-position, ruby-*, text-shadow.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_text_decor_emphasis_ruby(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // css-text-decor-4 §3.2: сторона знака; в сокращение
            // `text-emphasis` не входит.
            "text-emphasis-position" => {
                let words: Vec<_> = v.split_whitespace().collect();
                if words.is_empty()
                    || words
                        .iter()
                        .any(|w| !matches!(*w, "auto" | "over" | "under" | "left" | "right"))
                {
                    return;
                }
                self.accept_declaration();
                self.emphasis_position_auto = Some(v.trim() == "auto");
                self.emphasis_under = v.split_whitespace().any(|w| w == "under");
                self.emphasis_left = v.split_whitespace().any(|w| w == "left");
            }
            "ruby-position" => {
                let words: Vec<_> = v.split_whitespace().collect();
                if words.is_empty()
                    || words
                        .iter()
                        .any(|w| !matches!(*w, "over" | "under" | "alternate" | "inter-character"))
                {
                    return;
                }
                self.accept_declaration();
                // css-ruby-1 §4.1: `under` — под базой; `over`, `alternate`
                // (первый уровень) и `inter-character` (в горизонтали — пока
                // как `over`) — над ней.
                self.ruby_under = Some(v.split_whitespace().any(|w| w == "under"));
            }
            "ruby-align" => {
                self.ruby_align = apply_parsed!(
                    self,
                    match v.trim() {
                        "start" => Some(RubyAlign::Start),
                        "center" => Some(RubyAlign::Center),
                        "space-between" => Some(RubyAlign::SpaceBetween),
                        "space-around" => Some(RubyAlign::SpaceAround),
                        _ => None,
                    }
                );
            }
            "ruby-overhang" => {
                self.ruby_overhang = apply_parsed!(
                    self,
                    match v.trim() {
                        "auto" => Some(RubyOverhang::Auto),
                        "none" => Some(RubyOverhang::None),
                        "spaces" => Some(RubyOverhang::Spaces),
                        _ => None,
                    }
                );
            }
            "ruby-merge" => {
                if matches!(v, "separate" | "merge" | "auto") {
                    self.accept_declaration();
                }
                self.ruby_merge = match v {
                    "separate" | "initial" => Some(0),
                    "merge" => Some(1),
                    "auto" => Some(2),
                    "inherit" | "unset" => None,
                    _ => self.ruby_merge,
                };
            }
            // Тень в единицах шрифта — строкой до своего кегля (`resolve_em`):
            // `parse_shadows` такую тень пропускает, и `1em 0em purple` у
            // эталона `box-shadow-multiple-001-ref` пропадала, пока тест после
            // `shadow_raw` уже рисовал свою `box-shadow` в `em`.
            "text-shadow" if has_font_units(v) => {
                if !box_shadow_valid(v, vcx) || v.split_whitespace().any(|w| w == "inset") {
                    return;
                }
                self.text_shadow_raw = accept_parsed!(self, Some(v.to_string()))
            }
            "text-shadow" => {
                css_wide_reset!(
                    self,
                    text_shadow,
                    text_shadow_raw,
                    text_shadow_none,
                    text_shadow_rest
                );
                if !box_shadow_valid(v, vcx) || v.split_whitespace().any(|w| w == "inset") {
                    return;
                }
                self.text_shadow_raw = None;
                self.text_shadow_none = v.trim().eq_ignore_ascii_case("none");
                let mut list = parse_shadows(v, vcx).into_iter();
                self.text_shadow = accept_parsed!(self, list.next());
                if self.text_shadow_none {
                    self.accept_declaration();
                }
                self.text_shadow_rest = list.collect();
            }
            _ => *hit = false,
        }
    }
}
