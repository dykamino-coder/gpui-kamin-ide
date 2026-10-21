//! Computed::apply_border, хвост цепочки: ширины сторон, логические кромки, цвета и стили сторон,
//! border-style, border-spacing, outline*, border-image*. Ветви в исходном порядке после ветвей
//! углов.

use super::*;
mod outline;
use outline::border_value_valid;

impl Computed {
    pub(super) fn apply_border_sides_outline(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        if key.ends_with("-color") && v.eq_ignore_ascii_case("currentcolor") {
            self.accept_declaration();
        }
        if key.starts_with("border-") && (key.contains("inline") || key.contains("block")) {
            let prop = key.rsplit('-').next().unwrap_or("");
            let words = split_outside_parens(v);
            let valid = match prop {
                "width" => {
                    !words.is_empty()
                        && words.len() <= 2
                        && words.iter().all(|w| line_width(w, vcx).is_some())
                }
                "style" => {
                    !words.is_empty()
                        && words.len() <= 2
                        && words.iter().all(|w| border_style_rank(w).is_some())
                }
                "color" => {
                    !words.is_empty()
                        && words.len() <= 2
                        && words
                            .iter()
                            .all(|w| w == "currentcolor" || Color::parse(w, vcx).is_some())
                }
                _ => border_value_valid(v, vcx),
            };
            if !valid {
                return;
            }
            self.accept_declaration();
        }
        match key {
            "border-top-width" => {
                self.border_width.top =
                    apply_parsed!(self, line_width(v, vcx)).or(self.border_width.top)
            }
            "border-right-width" => {
                self.border_width.right =
                    apply_parsed!(self, line_width(v, vcx)).or(self.border_width.right)
            }
            "border-bottom-width" => {
                self.border_width.bottom =
                    apply_parsed!(self, line_width(v, vcx)).or(self.border_width.bottom)
            }
            "border-left-width" => {
                self.border_width.left =
                    apply_parsed!(self, line_width(v, vcx)).or(self.border_width.left)
            }
            "border-inline" => {
                let l = self.logical();
                l.border[1] = Some(v.to_string());
                l.border[3] = Some(v.to_string());
            }
            "border-block" => {
                let l = self.logical();
                l.border[0] = Some(v.to_string());
                l.border[2] = Some(v.to_string());
            }
            "border-block-start" => self.logical().border[0] = Some(v.to_string()),
            "border-block-end" => self.logical().border[2] = Some(v.to_string()),
            "border-inline-start" => self.logical().border[3] = Some(v.to_string()),
            "border-inline-end" => self.logical().border[1] = Some(v.to_string()),
            "border-block-width"
            | "border-block-style"
            | "border-block-color"
            | "border-inline-width"
            | "border-inline-style"
            | "border-inline-color" => {
                let parts = split_outside_parens(v);
                let (a, b) = match parts.as_slice() {
                    [a] => (a, a),
                    [a, b] => (a, b),
                    _ => return,
                };
                let (axis, prop) = key["border-".len()..].split_once('-').unwrap_or_default();
                self.apply_one(&format!("border-{axis}-start-{prop}"), a, vcx);
                self.apply_one(&format!("border-{axis}-end-{prop}"), b, vcx);
            }
            "border-block-start-color"
            | "border-block-end-color"
            | "border-inline-start-color"
            | "border-inline-end-color"
            | "border-block-start-width"
            | "border-block-end-width"
            | "border-inline-start-width"
            | "border-inline-end-width"
            | "border-block-start-style"
            | "border-block-end-style"
            | "border-inline-start-style"
            | "border-inline-end-style" => {
                let rest = &key["border-".len()..];
                let (side, prop) = if let Some(p) = rest.strip_prefix("block-start-") {
                    (0u8, p)
                } else if let Some(p) = rest.strip_prefix("inline-end-") {
                    (1, p)
                } else if let Some(p) = rest.strip_prefix("block-end-") {
                    (2, p)
                } else if let Some(p) = rest.strip_prefix("inline-start-") {
                    (3, p)
                } else {
                    return;
                };
                let prop: &'static str = match prop {
                    "color" => "color",
                    "width" => "width",
                    _ => "style",
                };
                self.logical().border_long.push((side, prop, v.to_string()));
            }
            "border-top-color" => {
                self.border_colors[0] = parsed_value!(
                    self,
                    side_color(v, self.color, vcx)
                        .map(Some)
                        .or_else(|| (v == "currentcolor").then_some(None))
                )
            }
            "border-right-color" => {
                self.border_colors[1] = parsed_value!(
                    self,
                    side_color(v, self.color, vcx)
                        .map(Some)
                        .or_else(|| (v == "currentcolor").then_some(None))
                )
            }
            "border-bottom-color" => {
                self.border_colors[2] = parsed_value!(
                    self,
                    side_color(v, self.color, vcx)
                        .map(Some)
                        .or_else(|| (v == "currentcolor").then_some(None))
                )
            }
            "border-left-color" => {
                self.border_colors[3] = parsed_value!(
                    self,
                    side_color(v, self.color, vcx)
                        .map(Some)
                        .or_else(|| (v == "currentcolor").then_some(None))
                )
            }
            "border-top-style" => self.set_border_style(v, Some(0)),
            "border-right-style" => self.set_border_style(v, Some(1)),
            "border-bottom-style" => self.set_border_style(v, Some(2)),
            "border-left-style" => self.set_border_style(v, Some(3)),
            "border-style" => {
                let side = |list: &[&str], i: usize| -> String {
                    let pick = match (list.len(), i) {
                        (1, _) => 0,
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i.min(list.len().saturating_sub(1)),
                    };
                    list[pick].to_ascii_lowercase()
                };
                let list: Vec<&str> = v.split_whitespace().collect();
                if list.is_empty()
                    || list.len() > 4
                    || !list.iter().all(|v| border_style_rank(v).is_some())
                {
                    return;
                }
                if list.len() <= 4 && list.iter().all(|v| border_style_rank(v).is_some()) {
                    self.accept_declaration();
                }
                self.border_dashed = Some(side(&list, 0) == "dashed");
                self.border_dotted = Some(side(&list, 0) == "dotted");
                let widths = [
                    &mut self.border_width.top,
                    &mut self.border_width.right,
                    &mut self.border_width.bottom,
                    &mut self.border_width.left,
                ];
                for (i, w) in widths.into_iter().enumerate() {
                    let one = side(&list, i);
                    let on = border_style(&one);
                    self.border_visible[i] = Some(on);
                    // CSS 2.1 §17.6.2.1: hidden wins collapsed border conflicts.
                    if let Some(rank) = border_style_rank(&one) {
                        self.border_side_styles[i] = Some(rank);
                    }
                    // Initial medium width is 3px when a style makes it visible.
                    if on && w.is_none() {
                        *w = Some(Len::Px(3.0));
                    }
                    if one == "none" || one == "hidden" {
                        *w = Some(Len::Px(0.0));
                    }
                }
            }
            "border-spacing" => {
                let words = split_outside_parens(v);
                if !(1..=2).contains(&words.len()) {
                    return;
                }
                let (a, b) = parsed_value!(
                    self,
                    line_width(&words[0], vcx).zip(line_width(&words[words.len() - 1], vcx))
                );
                self.border_spacing = Some((Some(a), Some(b)));
            }
            "outline" | "outline-width" | "outline-color" | "outline-offset" | "outline-style" => {
                self.apply_outline(key, v, vcx)
            }
            "border-image"
            | "border-image-source"
            | "border-image-slice"
            | "border-image-width"
            | "border-image-outset"
            | "border-image-repeat" => self.set_border_image(key, v, vcx),
            _ => *hit = false,
        }
    }
}
