//! Группа свойств `Computed::apply_one`: позиционирование и видимость —
//! `position`, якорные `anchor-name`/`position-anchor`/`position-area`/
//! `position-try*`/`position-visibility` (css-anchor-position-1), `z-index`,
//! `overflow*` и `overflow-clip-margin`, `float`,
//! `clear`, `opacity`. Разбор якорных значений — в `style::values::position_area`.

use crate::values::ValueCx;
mod clip_margin;
mod z_index;
use crate::style::computed::*;
use crate::style::values::position_area::{
    parse_area, parse_try_fallbacks, parse_try_order, parse_visibility,
};
use crate::style::values::value::Len;

impl Computed {
    pub fn apply_position(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            "position" => {
                self.reset_positioned.position = apply_parsed!(
                    self,
                    match v {
                        "absolute" => Some(Position::Absolute),
                        "relative" => Some(Position::Relative),
                        "static" => Some(Position::Static),
                        // `fixed` отсчитывается от окна: своей системы отсчёта у
                        // него нет, и сборщик дерева ставит его отдельным слоем.
                        "fixed" => Some(Position::Fixed),
                        "sticky" | "-webkit-sticky" => Some(Position::Sticky),
                        _ => None,
                    }
                )
            }
            // css-anchor-position-1 §anchor-name: `none | <dashed-ident>#`.
            "anchor-name" => {
                let names = if v == "none" {
                    None
                } else {
                    let names: Vec<_> = v.split(',').map(str::trim).collect();
                    if !names.iter().all(|n| {
                        n.starts_with("--") && crate::style::css::selector_tokens::ident(n)
                    }) {
                        return;
                    }
                    Some(names.into_iter().map(str::to_owned).collect())
                };
                self.reset_positioned.anchor_name = accept_parsed!(self, names);
            }
            // §position-anchor: значение хранится как есть, решается при
            // сборке (`anchor::AnchorPlan::of`) и в `anchor::settle_static`.
            "position-anchor" => {
                self.reset_positioned.position_anchor = accept_parsed!(
                    self,
                    Some(match v {
                        "normal" => PositionAnchor::Normal,
                        "none" => PositionAnchor::None,
                        "auto" => PositionAnchor::Auto,
                        "match-parent" => PositionAnchor::MatchParent,
                        name if name.starts_with("--") => PositionAnchor::Named(name.to_string()),
                        _ => return,
                    })
                );
            }
            // §position-area: сетка 3×3 от якоря и содержащего блока; разбор
            // и физическое разрешение — в `anchor`.
            "position-area" => {
                self.reset_positioned.position_area = parsed_value!(
                    self,
                    parse_area(v)
                        .map(Some)
                        .or_else(|| (v == "none").then_some(None))
                );
                if v == "none" {
                    self.accept_declaration();
                }
            }
            // §position-try-fallbacks: список вариантов, разбор — в `anchor`.
            "position-try-fallbacks" => {
                let parsed = parse_try_fallbacks(v);
                if parsed.is_empty() && v != "none" {
                    return;
                }
                self.reset_positioned.position_try_fallbacks = parsed;
                if !self.reset_positioned.position_try_fallbacks.is_empty() || v == "none" {
                    self.accept_declaration();
                }
            }
            // §position-try-order-property.
            "position-try-order" => {
                if let Some(o) = parse_try_order(v.trim()) {
                    self.reset_positioned.position_try_order = accept_parsed!(self, o);
                }
            }
            // Сокращение `position-try: <order>? <fallbacks>` (§position-try-prop):
            // опущенный порядок — `normal`.
            "position-try" => {
                let v = v.trim();
                let (order, rest) = match v.split_once(char::is_whitespace) {
                    Some((a, b)) if parse_try_order(a).is_some() => {
                        (parse_try_order(a).unwrap_or(0), b.trim())
                    }
                    None if parse_try_order(v).is_some() => {
                        (parse_try_order(v).unwrap_or(0), "none")
                    }
                    _ => (0, v),
                };
                let parsed = parse_try_fallbacks(rest);
                if parsed.is_empty() && rest != "none" {
                    return;
                }
                self.reset_positioned.position_try_order = order;
                self.reset_positioned.position_try_fallbacks = parsed;
                if !self.reset_positioned.position_try_fallbacks.is_empty() || rest == "none" {
                    self.accept_declaration();
                }
            }
            // §position-visibility; легаси `anchors-valid`/`anchors-visible` —
            // псевдонимы (спека разрешает их узнавать).
            "position-visibility" => {
                css_wide_reset!(self, reset_positioned.position_visibility);
                let Some(bits) = parse_visibility(v) else {
                    return;
                };
                self.reset_positioned.position_visibility = accept_parsed!(self, bits);
            }
            "overflow" => {
                // Запись из двух слов — оси по отдельности
                // (css-overflow-3 §3): `overflow: clip visible`.
                let words: Vec<_> = v.split_whitespace().collect();
                if !(1..=2).contains(&words.len()) {
                    return;
                }
                let pair = apply_parsed!(
                    self,
                    parse_overflow(words[0]).zip(parse_overflow(words[words.len() - 1]))
                );
                let (x, y) = (pair.map(|p| p.0), pair.map(|p| p.1));
                self.reset_box_layout.scroll.forced = [
                    v.split_whitespace().next() == Some("scroll"),
                    v.split_whitespace().nth(1).or(v.split_whitespace().next()) == Some("scroll"),
                ];
                self.reset_box_layout.overflow_x = x;
                self.reset_box_layout.overflow_y = y;
            }
            "overflow-x" => {
                self.reset_box_layout.overflow_x = apply_parsed!(self, parse_overflow(v));
                self.reset_box_layout.scroll.forced[0] = v == "scroll";
            }
            "overflow-y" => {
                self.reset_box_layout.overflow_y = apply_parsed!(self, parse_overflow(v));
                self.reset_box_layout.scroll.forced[1] = v == "scroll";
            }
            "opacity" => self.reset_effects.opacity = apply_parsed!(self, v.parse().ok()),
            // Поле обрезки: край, по которому режется вылезшее содержимое,
            // отодвигается наружу (css-overflow-3 §5). Запись допускает и
            // указание коробки отсчёта — её мы не различаем, край один.
            "overflow-clip-margin" => {
                self.apply_clip_margin(v, vcx);
            }

            "z-index" => {
                if v == "inherit" {
                    self.reset_cascade.inherit_bits |= inh::Z_INDEX;
                    return;
                }
                self.reset_positioned.z_index = parsed_value!(
                    self,
                    z_index::integer(v, vcx)
                        .map(Some)
                        .or_else(|| (v == "auto").then_some(None))
                );
                if v == "auto" {
                    self.accept_declaration();
                }
            }
            "float" => {
                self.reset_box_layout.float = apply_parsed!(
                    self,
                    match v {
                        "left" => Some(-1),
                        "right" => Some(1),
                        "none" => Some(0),
                        _ => None,
                    }
                )
            }
            "clear" => {
                let parsed = parsed_value!(
                    self,
                    match v {
                        "left" => Some(Some(-1)),
                        "right" => Some(Some(1)),
                        "both" => Some(Some(0)),
                        "none" | "inherit" => Some(None),
                        _ => None,
                    }
                );
                self.reset_context.clear_inherit = v == "inherit";
                self.reset_box_layout.clear = parsed;
            }
            _ => *hit = false,
        }
    }
}
