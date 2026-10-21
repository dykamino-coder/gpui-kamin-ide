//! Группа свойств `Computed::apply_one`: позиционирование и видимость —
//! `position`, якорные `anchor-name`/`position-anchor`/`position-area`/
//! `position-try*`/`position-visibility` (css-anchor-position-1), вставки
//! `top`…`inset`, `z-index`, `overflow*` и `overflow-clip-margin`, `float`,
//! `clear`, `opacity`, `visibility`. Разбор якорных значений — в `style::values::position_area`.

use crate::document::state::ValueCx;
mod clip_margin;
mod z_index;
use crate::style::computed::*;
use crate::style::values::position_area::{
    parse_area, parse_try_fallbacks, parse_try_order, parse_visibility,
};
use crate::style::values::value::Len;

impl Computed {
    pub(crate) fn apply_position(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        match key {
            "position" => {
                self.position = apply_parsed!(
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
                .or(self.position)
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
                self.anchor_name = accept_parsed!(self, names);
            }
            // §position-anchor: значение хранится как есть, решается при
            // сборке (`anchor::AnchorPlan::of`) и в `anchor::settle_static`.
            "position-anchor" => {
                self.position_anchor = accept_parsed!(
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
                self.position_area = parsed_value!(
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
                self.position_try_fallbacks = parsed;
                if !self.position_try_fallbacks.is_empty() || v == "none" {
                    self.accept_declaration();
                }
            }
            // §position-try-order-property.
            "position-try-order" => {
                if let Some(o) = parse_try_order(v.trim()) {
                    self.position_try_order = accept_parsed!(self, o);
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
                self.position_try_order = order;
                self.position_try_fallbacks = parsed;
                if !self.position_try_fallbacks.is_empty() || rest == "none" {
                    self.accept_declaration();
                }
            }
            // §position-visibility; легаси `anchors-valid`/`anchors-visible` —
            // псевдонимы (спека разрешает их узнавать).
            "position-visibility" => {
                if v != "always"
                    && (v.is_empty()
                        || !v.split_whitespace().all(|word| {
                            matches!(word, "anchors-valid" | "anchors-visible" | "no-overflow")
                        }))
                {
                    return;
                }
                self.position_visibility = accept_parsed!(self, parse_visibility(v));
            }
            "top" | "right" | "bottom" | "left" => {
                let parsed = apply_parsed!(
                    self,
                    Len::parse(v, vcx).or_else(|| Len::parse_mixed(v, vcx))
                );
                let (side, slot) = match key {
                    "top" => (0, &mut self.inset.top),
                    "right" => (1, &mut self.inset.right),
                    "bottom" => (2, &mut self.inset.bottom),
                    _ => (3, &mut self.inset.left),
                };
                self.inset_inherit[side] = v == "inherit";
                // Keep anchor arithmetic; preserve mixed percentages until layout (§10.9).
                *slot = parsed;
                self.side_seq.inset[side] = self.decl_seq;
            }
            "inset" => {
                self.inset =
                    parsed_value!(self, super::box_model::sides::parsed_sides(v, true, vcx));
                self.side_seq.inset = [self.decl_seq; 4];
            }
            "overflow" => {
                // Запись из двух слов — оси по отдельности
                // (css-overflow-3 §3): `overflow: clip visible`.
                let words: Vec<_> = v.split_whitespace().collect();
                if !(1..=2).contains(&words.len()) {
                    return;
                }
                let (x, y) = parsed_value!(
                    self,
                    parse_overflow(words[0]).zip(parse_overflow(words[words.len() - 1]))
                );
                let (x, y) = (Some(x), Some(y));
                self.scroll.forced = [
                    v.split_whitespace().next() == Some("scroll"),
                    v.split_whitespace().nth(1).or(v.split_whitespace().next()) == Some("scroll"),
                ];
                self.overflow_x = x;
                self.overflow_y = y;
            }
            "overflow-x" => {
                self.overflow_x = apply_parsed!(self, parse_overflow(v));
                self.scroll.forced[0] = v == "scroll";
            }
            "overflow-y" => {
                self.overflow_y = apply_parsed!(self, parse_overflow(v));
                self.scroll.forced[1] = v == "scroll";
            }
            "opacity" => self.opacity = apply_parsed!(self, v.parse().ok()),
            // Поле обрезки: край, по которому режется вылезшее содержимое,
            // отодвигается наружу (css-overflow-3 §5). Запись допускает и
            // указание коробки отсчёта — её мы не различаем, край один.
            "overflow-clip-margin" => {
                self.apply_clip_margin(v, vcx);
            }

            "visibility" => {
                self.hidden = apply_parsed!(
                    self,
                    match v {
                        "hidden" | "collapse" => Some(true),
                        "visible" => Some(false),
                        _ => None,
                    }
                );
                self.collapsed = Some(v == "collapse");
            }
            "z-index" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::Z_INDEX;
                    return;
                }
                self.z_index = parsed_value!(
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
                self.float = apply_parsed!(
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
                self.clear_inherit = v == "inherit";
                self.clear = parsed;
            }
            _ => *hit = false,
        }
    }
}
