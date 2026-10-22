//! Группа свойств `Computed::apply_one`: оформление текста вне реестра —
//! `text-decoration-inset`, `-skip-*`, `text-underline-offset`/`-position`;
//! `text-emphasis-position`, `text-shadow`, `ruby-*` — в подмодуле
//! `emphasis_ruby`. Линии, стиль, цвет и толщина оформления — длинные
//! свойства реестра (`props::text::longhands`).

use crate::document::state::ValueCx;
use crate::style::computed::*;
mod emphasis_ruby;

impl Computed {
    pub(crate) fn apply_text_decor(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "text-underline-offset" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "auto" {
                    self.accept_declaration();
                    self.underline_offset = Some(DecorLen::Auto);
                } else if let Some(l) = parse_decor_length(&t, vcx) {
                    self.accept_declaration();
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
                    self.accept_declaration();
                    self.underline_pos = Some(bits);
                }
            }
            "text-decoration-skip-spaces" => {
                let t = v.trim().to_ascii_lowercase();
                let words: Vec<&str> = t.split_whitespace().collect();
                self.skip_spaces = accept_parsed!(
                    self,
                    match words.as_slice() {
                        ["none"] => Some(0),
                        ["all"] => Some(4),
                        ["start"] => Some(1),
                        ["end"] => Some(2),
                        ["start", "end"] | ["end", "start"] => Some(3),
                        _ => return,
                    }
                );
            }
            "text-decoration-skip-ink" => {
                self.skip_ink = accept_parsed!(
                    self,
                    match v.trim().to_ascii_lowercase().as_str() {
                        "none" => Some(0),
                        "auto" => Some(1),
                        "all" => Some(2),
                        _ => return,
                    }
                );
            }
            "text-decoration-inset" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "auto" {
                    self.accept_declaration();
                    self.td_inset = Some(None);
                    return;
                }
                let words = crate::style::css::value_tokens::split_top(&t);
                let lens: Vec<DecorLen> = words
                    .iter()
                    .filter_map(|w| parse_decor_inset(w, vcx))
                    .collect();
                if lens.len() != words.len() || lens.is_empty() || lens.len() > 2 {
                    return;
                }
                self.td_inset = accept_parsed!(
                    self,
                    Some(Some([lens[0], *lens.get(1).unwrap_or(&lens[0])]))
                );
            }
            _ => self.apply_text_decor_emphasis_ruby(key, v, hit, vcx),
        }
    }
}
