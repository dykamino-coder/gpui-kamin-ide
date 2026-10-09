//! Computed::apply_one: text-decoration*, text-underline-*, text-emphasis*, text-shadow, ruby-*.

use crate::style::computed::*;
use crate::style::values::value::Color;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_text_decor(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "text-decoration" | "text-decoration-line" | "-webkit-text-decoration-line" => {
                // css-text-decor-4 §2: сокращение — `<line> || <style> ||
                // <color> || <thickness>`, все подсвойства сбрасываются.
                // Недействительный токен делает объявление НЕВАЛИДНЫМ целиком
                // (§4.2): `text-decoration: diagonal` оставляет прежнее
                // значение (`c71-fwd-parsing-003`).
                let lower = v.trim().to_ascii_lowercase();
                let words = crate::paint::background::split_top(&lower);
                if words.is_empty() {
                    return;
                }
                let short = key == "text-decoration";
                let (mut lines, mut none) = (0u8, false);
                let (mut style, mut color, mut thick) = (None, None, None);
                let mut current = false;
                for t in &words {
                    let t = *t;
                    match t {
                        "none" if lines == 0 && !none => none = true,
                        "underline" if lines & DECOR_UNDER == 0 && !none => lines |= DECOR_UNDER,
                        "overline" if lines & DECOR_OVER == 0 && !none => lines |= DECOR_OVER,
                        "line-through" if lines & DECOR_THROUGH == 0 && !none => {
                            lines |= DECOR_THROUGH
                        }
                        // Мигание и пометки правописания линий не рисуют.
                        "blink" | "spelling-error" | "grammar-error" if !none => {}
                        _ if !short => return,
                        _ if style.is_none() && parse_decor_style(t).is_some() => {
                            style = parse_decor_style(t)
                        }
                        "currentcolor" if color.is_none() && !current => current = true,
                        _ if color.is_none() && !current && Color::parse(t).is_some() => {
                            color = Color::parse(t)
                        }
                        _ if thick.is_none() && parse_decor_thickness(t).is_some() => {
                            thick = parse_decor_thickness(t)
                        }
                        _ => return,
                    }
                }
                self.td_lines = Some(lines);
                if short {
                    self.td_style = style;
                    self.td_color = color;
                    self.td_thickness = thick;
                }
                self.underline = Some(lines & DECOR_UNDER != 0);
                self.line_through = Some(lines & DECOR_THROUGH != 0);
            }
            "text-decoration-style" | "-webkit-text-decoration-style" => {
                if let Some(s) = parse_decor_style(&v.trim().to_ascii_lowercase()) {
                    self.td_style = Some(s);
                }
            }
            "text-decoration-color" | "-webkit-text-decoration-color" => {
                let t = v.trim();
                if t.eq_ignore_ascii_case("currentcolor") {
                    self.td_color = None;
                } else if let Some(c) = Color::parse(t) {
                    self.td_color = Some(c);
                }
            }
            "text-decoration-thickness" => {
                if let Some(t) = parse_decor_thickness(&v.trim().to_ascii_lowercase()) {
                    self.td_thickness = Some(t);
                }
            }
            "text-underline-offset" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "auto" {
                    self.underline_offset = Some(DecorLen::Auto);
                } else if let Some(l) = parse_decor_length(&t) {
                    self.underline_offset = Some(l);
                }
            }
            "text-underline-position" => {
                let t = v.trim().to_ascii_lowercase();
                let mut bits = 0u8;
                let words: Vec<&str> = t.split_whitespace().collect();
                let ok = match words.as_slice() {
                    ["auto"] => true,
                    ["from-font"] => {
                        bits = UPOS_FROM_FONT;
                        true
                    }
                    ws if !ws.is_empty() && ws.len() <= 2 => ws.iter().all(|w| {
                        let b = match *w {
                            "under" => UPOS_UNDER,
                            "left" => UPOS_LEFT,
                            "right" => UPOS_RIGHT,
                            _ => return false,
                        };
                        let side = UPOS_LEFT | UPOS_RIGHT;
                        if bits & b != 0 || (b & side != 0 && bits & side != 0) {
                            return false;
                        }
                        bits |= b;
                        true
                    }),
                    _ => false,
                };
                if ok {
                    self.underline_pos = Some(bits);
                }
            }
            "text-decoration-skip-spaces" => {
                let t = v.trim().to_ascii_lowercase();
                let words: Vec<&str> = t.split_whitespace().collect();
                self.skip_spaces = match words.as_slice() {
                    ["none"] => Some(0),
                    ["all"] => Some(4),
                    ["start"] => Some(1),
                    ["end"] => Some(2),
                    ["start", "end"] | ["end", "start"] => Some(3),
                    _ => return,
                };
            }
            "text-decoration-skip-ink" => {
                self.skip_ink = match v.trim().to_ascii_lowercase().as_str() {
                    "none" => Some(0),
                    "auto" => Some(1),
                    "all" => Some(2),
                    _ => return,
                };
            }
            "text-decoration-inset" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "auto" {
                    self.td_inset = Some(None);
                    return;
                }
                let words = crate::paint::background::split_top(&t);
                // Смесь доли и точек доживает до отрисовки: доля — от
                // ширины украшенного прогона (css-text-decor-4 §4.1).
                let one = |w: &str| match crate::style::values::value::calc_pct_px(w) {
                    Some((k, p)) if k != 0.0 => Some(DecorLen::Mix(k, p)),
                    _ => parse_decor_length(w),
                };
                let lens: Vec<DecorLen> = words.iter().filter_map(|w| one(w)).collect();
                if lens.len() != words.len() || lens.is_empty() || lens.len() > 2 {
                    return;
                }
                self.td_inset = Some(Some([lens[0], *lens.get(1).unwrap_or(&lens[0])]));
            }
            "text-emphasis"
            | "text-emphasis-style"
            | "text-emphasis-color"
            | "text-emphasis-position" => {
                // css-text-decor-3 §5: знак задаётся словом (форма +
                // заливка) или строкой; `none` его снимает. Цвет знака —
                // свой (`emphasis_color`), по умолчанию цвет текста.
                if key == "text-emphasis-position" {
                    self.emphasis_under = v.split_whitespace().any(|w| w == "under");
                    return;
                }
                if key == "text-emphasis-color" {
                    let t = v.trim();
                    if t.eq_ignore_ascii_case("currentcolor") {
                        self.emphasis_color = None;
                    } else if let Some(c) = Color::parse(t) {
                        self.emphasis_color = Some(c);
                    }
                    return;
                }
                let v = v.trim();
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
                            ) && let Some(c) = Color::parse(w)
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
                    self.text_emphasis = None;
                    return;
                }
                // Строка в кавычках — первый её знак (§5.1: «only the first
                // character is used»).
                if let Some(q) = v.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let body = v.trim_matches(q);
                    self.text_emphasis = body.chars().next().map(|c| c.to_string());
                    return;
                }
                let open = v.split_whitespace().any(|w| w == "open");
                let shape = v
                    .split_whitespace()
                    .find(|w| {
                        matches!(
                            *w,
                            "dot" | "circle" | "double-circle" | "triangle" | "sesame"
                        )
                    })
                    .unwrap_or("circle");
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
                self.text_emphasis = Some(mark.to_string());
            }
            "ruby-position" => {
                // css-ruby-1 §4.1: `under` — под базой; `over`, `alternate`
                // (первый уровень) и `inter-character` (в горизонтали — пока
                // как `over`) — над ней.
                self.ruby_under = Some(v.split_whitespace().any(|w| w == "under"));
            }
            "ruby-align" => {
                self.ruby_align = match v.trim() {
                    "start" => Some(RubyAlign::Start),
                    "center" => Some(RubyAlign::Center),
                    "space-between" => Some(RubyAlign::SpaceBetween),
                    "space-around" => Some(RubyAlign::SpaceAround),
                    _ => self.ruby_align,
                };
            }
            "ruby-overhang" => {
                self.ruby_overhang = match v.trim() {
                    "auto" => Some(RubyOverhang::Auto),
                    "none" => Some(RubyOverhang::None),
                    "spaces" => Some(RubyOverhang::Spaces),
                    _ => self.ruby_overhang,
                };
            }
            "ruby-merge" => {
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
            // `shadow_raw` уже рисовал свою `box-shadow` в `em` (0.08 → 6.25).
            "text-shadow" if has_font_units(v) => self.text_shadow_raw = Some(v.to_string()),
            "text-shadow" => {
                self.text_shadow_raw = None;
                self.text_shadow_none = v.trim().eq_ignore_ascii_case("none");
                let mut list = parse_shadows(v).into_iter();
                self.text_shadow = list.next();
                self.text_shadow_rest = list.collect();
            }
            _ => *hit = false,
        }
    }
}
