//! Computed::apply_box_model, хвост цепочки: раскладочные свойства коробки
//! (aspect-ratio, frame-sizing). Стороны и размеры — в `sides`.

use super::*;

impl Computed {
    pub(super) fn apply_box_model_aspect(&mut self, key: &str, v: &str, hit: &mut bool) {
        match key {
            // --- Раскладка --------------------------------------------------
            "aspect-ratio" => {
                // `auto <ratio>` (css-sizing-4 §5.1): слово `auto` означает,
                // что у замещаемого ПРИРОДНОЕ соотношение сильнее заявленного,
                // а у остальных коробок действует заявленное. Отбрасывать
                // слово и отдавать отношение всем подряд нельзя
                // (`replaced-element-020/029/030`): слово живёт флагом, и
                // отрисовка замещаемого его учитывает (`image_with::ratio_of`).
                let (auto, ratio) = parsed_value!(self, aspect_ratio(v));
                if auto {
                    self.aspect_ratio_auto = ratio;
                    self.aspect_ratio = None;
                } else {
                    self.aspect_ratio_auto = None;
                    self.aspect_ratio = ratio;
                }
            }
            "frame-sizing" => {
                self.frame_sizing_height = parsed_value!(
                    self,
                    match v {
                        "content-height" => Some(true),
                        "fixed" => Some(false),
                        _ => None,
                    }
                );
            }
            _ => *hit = false,
        }
    }
}

/// `auto || <ratio>` (css-sizing-4 §5.1). Вырожденное отношение (ноль или
/// бесконечность в любой части) ведёт себя как `auto` (css-values-4
/// §ratio): `0/1` давал соотношение 0 и бесконечную высоту
/// (`zero-or-infinity-001`).
fn aspect_ratio(v: &str) -> Option<(bool, Option<f32>)> {
    let mut auto = false;
    let mut body = Vec::new();
    for token in v.split_whitespace() {
        if token.eq_ignore_ascii_case("auto") {
            if auto {
                return None;
            }
            auto = true;
        } else {
            body.push(token);
        }
    }
    if body.is_empty() {
        return auto.then_some((true, None));
    }
    let body = body.join(" ");
    let (a, b) = body.split_once('/').unwrap_or((&body, "1"));
    let a: f32 = a.trim().parse().ok()?;
    let b: f32 = b.trim().parse().ok()?;
    if a < 0.0 || b < 0.0 {
        return None;
    }
    let ratio = a / b;
    Some((auto, (ratio.is_finite() && ratio > 0.0).then_some(ratio)))
}
