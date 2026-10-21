//! Computed::apply_text_decor, хвост цепочки: text-emphasis*, ruby-*, text-shadow. Ветви в исходном
//! порядке после text-decoration*.

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
            "text-emphasis"
            | "text-emphasis-style"
            | "text-emphasis-color"
            | "text-emphasis-position" => {
                // css-text-decor-3 §5: знак задаётся словом (форма +
                // заливка) или строкой; `none` его снимает. Цвет знака —
                // свой (`emphasis_color`), по умолчанию цвет текста.
                if key == "text-emphasis-position" {
                    self.accept_declaration();
                    self.emphasis_position_auto = Some(v.trim() == "auto");
                    self.emphasis_under = v.split_whitespace().any(|w| w == "under");
                    self.emphasis_left = v.split_whitespace().any(|w| w == "left");
                    return;
                }
                if key == "text-emphasis-color" {
                    let t = v.trim();
                    if t.eq_ignore_ascii_case("currentcolor") {
                        self.accept_declaration();
                        self.emphasis_color = None;
                    } else if let Some(c) = Color::parse(t, vcx) {
                        self.accept_declaration();
                        self.emphasis_color = Some(c);
                    }
                    return;
                }
                let v = v.trim();
                self.emphasis_auto_shape = false;
                // Сокращение `text-emphasis` несёт и цвет (§5.3): слово,
                // которое разбирается как цвет, в стиль знака не идёт.
                let style_words: Vec<&str> =
                    if key == "text-emphasis" && !v.starts_with(['"', '\'']) {
                        let mut kept = Vec::new();
                        for w in v.split_whitespace() {
                            if w.eq_ignore_ascii_case("currentcolor") {
                                self.emphasis_color = None;
                            } else if !matches!(
                                w,
                                "none"
                                    | "open"
                                    | "filled"
                                    | "dot"
                                    | "circle"
                                    | "double-circle"
                                    | "triangle"
                                    | "sesame"
                            ) && let Some(c) = Color::parse(w, vcx)
                            {
                                self.emphasis_color = Some(c);
                            } else {
                                kept.push(w);
                            }
                        }
                        kept
                    } else {
                        v.split_whitespace().collect()
                    };
                let joined = style_words.join(" ");
                let v = joined.as_str();
                if v == "none" || v.is_empty() {
                    self.accept_declaration();
                    self.text_emphasis = Some(String::new());
                    return;
                }
                // Строка в кавычках — первый её знак (§5.1: «only the first
                // character is used»).
                if let Some(q) = v.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    self.accept_declaration();
                    let body =
                        crate::style::computed::parse_util::unescape_content(v.trim_matches(q));
                    self.text_emphasis = Some(
                        body.chars()
                            .next()
                            .map_or_else(String::new, |c| c.to_string()),
                    );
                    return;
                }
                let open = v.split_whitespace().any(|w| w == "open");
                let shape = v.split_whitespace().find(|w| {
                    matches!(
                        *w,
                        "dot" | "circle" | "double-circle" | "triangle" | "sesame"
                    )
                });
                self.emphasis_auto_shape = shape.is_none();
                let shape = shape.unwrap_or("circle");
                let mark = match (shape, open) {
                    ("dot", false) => '\u{2022}',
                    ("dot", true) => '\u{25E6}',
                    ("circle", false) => '\u{25CF}',
                    ("circle", true) => '\u{25CB}',
                    ("double-circle", false) => '\u{25C9}',
                    ("double-circle", true) => '\u{25CE}',
                    ("triangle", false) => '\u{25B2}',
                    ("triangle", true) => '\u{25B3}',
                    ("sesame", false) => '\u{FE45}',
                    _ => '\u{FE46}',
                };
                self.text_emphasis = accept_parsed!(self, Some(mark.to_string()));
            }
            "ruby-position" => {
                self.accept_declaration();
                // css-ruby-1 §4.1: `under` — под базой; `over`, `alternate`
                // (первый уровень) и `inter-character` (в горизонтали — пока
                // как `over`) — над ней.
                self.ruby_under = Some(v.split_whitespace().any(|w| w == "under"));
            }
            "ruby-align" => {
                self.ruby_align = record_parsed!(
                    self,
                    match v.trim() {
                        "start" => Some(RubyAlign::Start),
                        "center" => Some(RubyAlign::Center),
                        "space-between" => Some(RubyAlign::SpaceBetween),
                        "space-around" => Some(RubyAlign::SpaceAround),
                        _ => None,
                    }
                )
                .or(self.ruby_align);
            }
            "ruby-overhang" => {
                self.ruby_overhang = record_parsed!(
                    self,
                    match v.trim() {
                        "auto" => Some(RubyOverhang::Auto),
                        "none" => Some(RubyOverhang::None),
                        "spaces" => Some(RubyOverhang::Spaces),
                        _ => None,
                    }
                )
                .or(self.ruby_overhang);
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
                self.text_shadow_raw = accept_parsed!(self, Some(v.to_string()))
            }
            "text-shadow" => {
                self.text_shadow_raw = None;
                self.text_shadow_none = v.trim().eq_ignore_ascii_case("none");
                let mut list = parse_shadows(v, vcx).into_iter();
                self.text_shadow = record_parsed!(self, list.next());
                if self.text_shadow_none {
                    self.accept_declaration();
                }
                self.text_shadow_rest = list.collect();
            }
            _ => *hit = false,
        }
    }
}
