//! Базовые фигуры clip-path/маски (css-shapes-1 §3): polygon(), rect(), xywh(), inset() с правилом заливки и скруглением. Хвост ветви clip-path/mask из Computed::apply_clip_mask.

use super::*;

impl Computed {
    pub(super) fn clip_basic_shape(&mut self, v: &str) {
        if let Some(rest) = v.strip_prefix("polygon(") {
            let rest = match rest.rfind(')') {
                Some(i) => &rest[..i],
                None => rest,
            };
            // Первым может стоять правило намотки (css-shapes-1
            // §3.1): `polygon(evenodd, …)`. Вершин любое число —
            // больше восьми (предел шейдера) и `evenodd` уходят
            // растровой маской-путём при отрисовке.
            // css-shapes-2 §basic-shape-polygon: `<fill-rule>? [round
            // <length>]?` may lead the list; the radius rounds every
            // vertex (a rectangle becomes a rounded rectangle).
            let mut round = None;
            let (rule, rest) = match rest.trim_start().split_once(',') {
                Some((head, tail))
                    if head
                        .split_whitespace()
                        .next()
                        .is_some_and(|w| matches!(w, "nonzero" | "evenodd" | "round")) =>
                {
                    let mut words = head.split_whitespace().peekable();
                    let rule = match words.peek() {
                        Some(&w @ ("nonzero" | "evenodd")) => {
                            words.next();
                            w
                        }
                        _ => "nonzero",
                    };
                    if words.next() == Some("round") {
                        round = words.next().and_then(Len::parse);
                    }
                    (rule, tail)
                }
                _ => ("nonzero", rest),
            };
            self.clip_round_len = round.filter(|l| matches!(l, Len::Px(_)));
            let points: Vec<(Len, Len)> = rest
                .split(',')
                .filter_map(|pair| {
                    let mut it = pair.split_whitespace();
                    let x = Len::parse(it.next()?)?;
                    let y = Len::parse(it.next()?)?;
                    Some((x, y))
                })
                .collect();
            if points.len() >= 3 {
                self.clip_polygon = Some(points);
                self.clip_polygon_evenodd = rule == "evenodd";
            }
        } else if v.starts_with("circle(") || v.starts_with("ellipse(") {
            // Форма растрируется маской: радиус и центр считаются от
            // размеров коробки при отрисовке.
            //
            // Голая `circle()`/`ellipse()` шла запасным путём
            // скруглённой коробки, где радиус — половина МЕНЬШЕЙ из
            // заданных `width`/`height`. При `box-sizing: content-box`
            // это содержимое, а не опорная коробка: у 40×40 с
            // отбивкой 20 и рамкой 20 выходило 20 вместо 60, и
            // `circle()` рисовалась слегка скруглённым прямоугольником
            // вместо вписанного круга. `shape_params` с пустым списком
            // радиусов даёт ровно `closest-side` (css-shapes-1
            // §3.1.1.3) от border-box — то, что и требуется.
            self.clip_shape = Some(format!("shape:{}", v.trim()));
        } else if let Some(rest) = v.strip_prefix("rect(") {
            // Края видимой области (css-shapes-1 §basic-shape):
            // top/right/bottom/left от верхнего-левого угла, auto —
            // край опорной коробки; хвост `round R` — скругление.
            let inner = rest.trim_end_matches(')');
            let sides_part = inner.split("round").next().unwrap_or("").trim();
            let vals: Vec<Option<Len>> = sides_part
                .split_whitespace()
                .map(|t| if t == "auto" { None } else { Len::parse(t) })
                .collect();
            if vals.len() == 4 {
                self.clip_edges = Some([vals[0], vals[1], vals[2], vals[3]]);
                self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
                self.clip_round = inner
                    .split("round")
                    .nth(1)
                    .and_then(uniform_round)
                    .and_then(|l| match l {
                        Len::Px(v) => Some(v),
                        _ => None,
                    })
                    .or(self.clip_round);
            }
        } else if let Some(rest) = v.strip_prefix("xywh(") {
            let inner = rest.trim_end_matches(')');
            let sides_part = inner.split("round").next().unwrap_or("").trim();
            let vals: Vec<Len> = sides_part
                .split_whitespace()
                .filter_map(Len::parse)
                .collect();
            if vals.len() == 4 {
                self.clip_xywh = Some([vals[0], vals[1], vals[2], vals[3]]);
                self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
                self.clip_round = inner
                    .split("round")
                    .nth(1)
                    .and_then(uniform_round)
                    .and_then(|l| match l {
                        Len::Px(v) => Some(v),
                        _ => None,
                    })
                    .or(self.clip_round);
            }
        } else if let Some(rest) = v.strip_prefix("inset(") {
            // Стороны вырезки (css-shapes-1 §3.1.1.1): 1-4 значения
            // TRBL до слова round; доли резолвит отрисовка.
            let sides_part = rest
                .trim_end_matches(')')
                .split("round")
                .next()
                .unwrap_or("")
                .trim();
            let vals: Vec<Len> = sides_part
                .split_whitespace()
                .filter_map(Len::parse)
                .collect();
            let pick = |i: usize| -> Len {
                match vals.len() {
                    1 => vals[0],
                    2 => vals[i % 2],
                    3 => vals[i.min(2)].to_owned(),
                    4 => vals[i],
                    _ => Len::Px(0.0),
                }
            };
            if !vals.is_empty() {
                self.clip_inset = Some([pick(0), pick(1), pick(2), pick(3)]);
            }
            let inner = rest.trim_end_matches(')');
            self.clip_round_len = inner.split("round").nth(1).and_then(uniform_round);
            let radius = inner
                .split("round")
                .nth(1)
                .and_then(uniform_round)
                .and_then(|l| match l {
                    Len::Px(v) => Some(v),
                    Len::Pct(p) => Some(p),
                    l @ (Len::Em(_)
                    | Len::EmPx(..)
                    | Len::Ch(_)
                    | Len::Ic(_)
                    | Len::Ex(_)
                    | Len::Lh(_)
                    | Len::LhPx(..)) => crate::text::metrics::fallback_len_px(l, "", 16.0),
                    Len::Vw(_) | Len::Vh(_) | Len::Calc(_) | Len::Anchor(_) => None,
                    Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
                });
            self.clip_round = Some(radius.unwrap_or(0.0));
        }
    }
}
