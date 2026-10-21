//! Атомарное применение сокращения и отдельных свойств outline.
use super::*;
impl Computed {
    pub(super) fn apply_outline(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) {
        match key {
            "outline" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S;
                    return;
                }
                // Initial medium width is 3px.
                let mut o = self.outline.unwrap_or(Outline {
                    width: Some(Len::Px(3.0)),
                    color: None,
                    offset: None,
                    inset: false,
                    style: None,
                });
                if !border_value_valid(v, vcx) {
                    return;
                }
                for token in split_ws_top(v) {
                    if let Some(st) = outline_style_of(token) {
                        self.accept_declaration();
                        o.style = Some(st);
                    } else if let Some(l) = outline_width_of(token, vcx) {
                        self.accept_declaration();
                        o.width = Some(l);
                    } else if token == "currentcolor" {
                        self.accept_declaration();
                        o.color = None;
                    } else if let Some(c) = Color::parse(token, vcx) {
                        self.accept_declaration();
                        o.color = Some(c);
                    }
                }
                self.outline = Some(o);
            }
            "outline-width" | "outline-color" | "outline-offset" | "outline-style" => {
                if v == "inherit" {
                    self.inherit_bits |= match key {
                        "outline-width" => inh::OUTLINE_W,
                        "outline-color" => inh::OUTLINE_C,
                        "outline-style" => inh::OUTLINE_S,
                        _ => inh::OUTLINE_O,
                    };
                    return;
                }
                let mut o = self.outline.unwrap_or_default();
                match key {
                    "outline-width" if matches!(v, "initial" | "unset") => {
                        o.width = Some(Len::Px(3.0))
                    }
                    "outline-width" => {
                        o.width = apply_parsed!(self, outline_width_of(v, vcx)).or(o.width)
                    }
                    "outline-color" => {
                        o.color = parsed_value!(
                            self,
                            Color::parse(v, vcx)
                                .map(Some)
                                .or_else(|| (v == "currentcolor").then_some(None))
                        )
                    }
                    "outline-style" => o.style = apply_parsed!(self, outline_style_of(v)),
                    // Inset moves the outline inward by its width.
                    _ => {
                        o.inset = v == "inset";
                        o.offset = if o.inset {
                            self.accept_declaration();
                            None
                        } else {
                            apply_parsed!(self, Len::parse(v, vcx))
                        };
                    }
                }
                self.outline = Some(o);
            }
            _ => {}
        }
    }
}

pub(super) fn border_value_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    let (mut width, mut style, mut color) = (false, false, false);
    let words = split_outside_parens(v);
    if words.is_empty() {
        return false;
    }
    for token in words {
        let slot = if line_width(&token, vcx).is_some() {
            &mut width
        } else if outline_style_of(&token).is_some() {
            &mut style
        } else if token == "currentcolor" || Color::parse(&token, vcx).is_some() {
            &mut color
        } else {
            return false;
        };
        if *slot {
            return false;
        }
        *slot = true;
    }
    true
}
