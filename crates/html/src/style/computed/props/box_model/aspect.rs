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
                let body = v
                    .split_whitespace()
                    .filter(|t| !t.eq_ignore_ascii_case("auto"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ratio = match body.split_once('/') {
                    Some((a, b)) => match (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
                        (Ok(a), Ok(b)) if b != 0.0 => Some(a / b),
                        _ => None,
                    },
                    None => body.trim().parse().ok(),
                }
                // Вырожденное отношение (ноль или бесконечность в любой части)
                // ведёт себя как `auto` (css-sizing-4 Overview.bs:533: «If the
                // <ratio> is degenerate, the property instead behaves as
                // auto»; css-values-4 Overview.bs:1665): `0/1` давал
                // соотношение 0, и коробка `width: 100px` получала бесконечную
                // высоту (`zero-or-infinity-001`).
                .filter(|r: &f32| r.is_finite() && *r > 0.0);
                self.note_parsed(&ratio);
                if v == "auto" {
                    self.accept_declaration();
                }
                if v.split_whitespace().any(|t| t.eq_ignore_ascii_case("auto")) {
                    self.aspect_ratio_auto = ratio;
                    self.aspect_ratio = None;
                } else {
                    self.aspect_ratio_auto = None;
                    self.aspect_ratio = ratio;
                }
            }
            "frame-sizing" => {
                self.frame_sizing_height = accept_parsed!(
                    self,
                    matches!(v.trim().to_ascii_lowercase().as_str(), "content-height")
                );
            }
            _ => *hit = false,
        }
    }
}
