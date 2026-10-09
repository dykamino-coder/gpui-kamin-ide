//! Computed::apply_one: position, anchor*, inset/top..., z-index, overflow*, float, clear, opacity, visibility.

use crate::style::computed::*;
use crate::style::values::value::Len;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_position(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {

            "position" => {
                self.position = match v {
                    "absolute" => Some(Position::Absolute),
                    "relative" => Some(Position::Relative),
                    "static" => Some(Position::Static),
                    // `fixed` отсчитывается от окна: своей системы отсчёта у
                    // него нет, и сборщик дерева ставит его отдельным слоем.
                    "fixed" => Some(Position::Fixed),
                    "sticky" | "-webkit-sticky" => Some(Position::Sticky),
                    _ => self.position,
                }
            }
            // css-anchor-position-1 §anchor-name: `none | <dashed-ident>#`.
            "anchor-name" => {
                self.anchor_name = (v != "none")
                    .then(|| {
                        v.split(',')
                            .map(|n| n.trim().to_string())
                            .filter(|n| n.starts_with("--"))
                            .collect::<Vec<_>>()
                    })
                    .filter(|names| !names.is_empty());
            }
            // §position-anchor: значение хранится как есть, решается при
            // сборке (`anchor::AnchorPlan::of`) и в `anchor::settle_static`.
            "position-anchor" => {
                self.position_anchor = Some(match v {
                    "normal" => PositionAnchor::Normal,
                    "none" => PositionAnchor::None,
                    "auto" => PositionAnchor::Auto,
                    "match-parent" => PositionAnchor::MatchParent,
                    name if name.starts_with("--") => PositionAnchor::Named(name.to_string()),
                    _ => return,
                });
            }
            // §position-area: сетка 3×3 от якоря и содержащего блока; разбор
            // и физическое разрешение — в `anchor`.
            "position-area" => {
                self.position_area = crate::anchor::parse_area(v);
            }
            // §position-try-fallbacks: список вариантов, разбор — в `anchor`.
            "position-try-fallbacks" => {
                self.position_try_fallbacks = crate::anchor::parse_try_fallbacks(v);
            }
            // §position-try-order-property.
            "position-try-order" => {
                if let Some(o) = crate::anchor::parse_try_order(v.trim()) {
                    self.position_try_order = o;
                }
            }
            // Сокращение `position-try: <order>? <fallbacks>` (§position-try-prop):
            // опущенный порядок — `normal`.
            "position-try" => {
                let v = v.trim();
                let (order, rest) = match v.split_once(char::is_whitespace) {
                    Some((a, b)) if crate::anchor::parse_try_order(a).is_some() => {
                        (crate::anchor::parse_try_order(a).unwrap_or(0), b.trim())
                    }
                    None if crate::anchor::parse_try_order(v).is_some() => {
                        (crate::anchor::parse_try_order(v).unwrap_or(0), "none")
                    }
                    _ => (0, v),
                };
                self.position_try_order = order;
                self.position_try_fallbacks = crate::anchor::parse_try_fallbacks(rest);
            }
            // §position-visibility; легаси `anchors-valid`/`anchors-visible` —
            // псевдонимы (спека разрешает их узнавать).
            "position-visibility" => {
                self.position_visibility = crate::anchor::parse_visibility(v);
            }
            "top" | "right" | "bottom" | "left" => {
                let (side, slot) = match key {
                    "top" => (0, &mut self.inset.top),
                    "right" => (1, &mut self.inset.right),
                    "bottom" => (2, &mut self.inset.bottom),
                    _ => (3, &mut self.inset.left),
                };
                self.inset_inherit[side] = v == "inherit";
                // Keep anchor arithmetic; preserve mixed percentages until layout (§10.9).
                *slot = Len::parse(v).or_else(|| Len::parse_mixed(v));
                self.side_seq.inset[side] = self.decl_seq;
            }
            "inset" => {
                self.inset = Sides::shorthand(v);
                self.side_seq.inset = [self.decl_seq; 4];
            }
            "overflow" => {
                // Запись из двух слов — оси по отдельности
                // (css-overflow-3 §3): `overflow: clip visible`.
                let mut it = v.split_whitespace();
                let x = it.next().and_then(parse_overflow);
                let y = it.next().and_then(parse_overflow).or(x);
                self.overflow_x = x;
                self.overflow_y = y;
            }
            "overflow-x" => self.overflow_x = parse_overflow(v),
            "overflow-y" => self.overflow_y = parse_overflow(v),
            "opacity" => self.opacity = v.parse().ok(),
            // Поле обрезки: край, по которому режется вылезшее содержимое,
            // отодвигается наружу (css-overflow-3 §5). Запись допускает и
            // указание коробки отсчёта — её мы не различаем, край один.
            "overflow-clip-margin" => {
                // `inherit` у НЕнаследуемого свойства — вычисленное значение
                // родителя целиком (css-cascade-4 §inherit). Ветка длины ниже
                // слово не понимала, и объявление пропадало
                // (`overflow-clip-margin-009`: поле 20 родителя не доезжало).
                if v.trim() == "inherit" {
                    self.inherit_bits |= inh::CLIP_MARGIN;
                    return;
                }
                // `<visual-box> || <length>`: коробка отсчёта и поле, в любом
                // порядке, любая часть может отсутствовать (умолчание —
                // padding-box, поле 0).
                let mut margin = None;
                let mut bx = None;
                for w in v.split_whitespace() {
                    match w {
                        "border-box" => bx = Some(2u8),
                        "padding-box" => bx = Some(1),
                        "content-box" => bx = Some(0),
                        t => {
                            // Отрицательная длина — ВТЯЖКА внутрь коробки
                            // (css-overflow-4 §overflow-clip-margin:
                            // «Negative values indicate insets»), а не
                            // негодное объявление.
                            if let Some(Len::Px(px)) = Len::parse(t) {
                                margin = Some(px);
                            }
                        }
                    }
                }
                if margin.is_some() || bx.is_some() {
                    self.clip_margin = Some(margin.unwrap_or(0.0));
                    self.clip_margin_box = bx;
                }
            }
            "visibility" => {
                self.hidden = Some(v == "hidden" || v == "collapse");
                self.collapsed = Some(v == "collapse");
            }
            "z-index" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::Z_INDEX;
                    return;
                }
                // Целое за пределами i32 КЛАМПИТСЯ, а не падает в auto
                // (z-index-001: -2147483649 обязан остаться меньше -100).
                self.z_index = v
                    .parse::<i64>()
                    .ok()
                    .map(|n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                    // `<integer>` from a math function (css-values-4 §10.9):
                    // rounded to the nearest integer, halves toward +∞
                    // (`calc-positive-fraction-001`: `calc(3 / 2)` → 2).
                    .or_else(|| {
                        (!v.trim_start().starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+'))
                            .then(|| crate::style::values::value::number(v))
                            .flatten()
                            .map(|x| {
                                let r = if x.is_nan() { 0.0 } else { (x as f64 + 0.5).floor() };
                                r.clamp(i32::MIN as f64, i32::MAX as f64) as i32
                            })
                    });
            }
            "float" => {
                self.float = match v {
                    "left" => Some(-1),
                    "right" => Some(1),
                    _ => Some(0),
                }
            }
            "clear" => {
                self.clear_inherit = v == "inherit";
                self.clear = match v {
                    "left" => Some(-1),
                    "right" => Some(1),
                    "both" => Some(0),
                    _ => None,
                }
            }
            _ => *hit = false,
        }
    }
}
