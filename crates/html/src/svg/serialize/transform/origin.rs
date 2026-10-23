//! Разбор презентационного `transform-origin` SVG-элемента относительно используемой опорной
//! коробки (css-transforms-1 §transform-origin): ключевые слова осей в любом порядке, проценты
//! от коробки, абсолютные единицы в px; длины отсчитываются от рамки фигуры только при
//! `transform-box: fill-box`, по умолчанию — от вьюпорта.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
pub(super) fn parse(
    raw: &str,
    bounds: (f32, f32, f32, f32),
    box_relative: bool,
) -> Option<(f32, f32)> {
    let (fx, fy, fw, fh) = bounds;
    let side = |t: &str, base: f32, off: f32| -> Option<f32> {
        let t = t.trim();
        Some(match t {
            "left" | "top" => off,
            "center" => off + base * 0.5,
            "right" | "bottom" => off + base,
            _ if t.ends_with('%') => {
                off + t.trim_end_matches('%').parse::<f32>().ok()? / 100.0 * base
            }
            _ => {
                // Абсолютные единицы (css-values §6.2): 1in = 96px.
                let (num, k) = if let Some(n) = t.strip_suffix("px") {
                    (n, 1.0)
                } else if let Some(n) = t.strip_suffix("cm") {
                    (n, 96.0 / 2.54)
                } else if let Some(n) = t.strip_suffix("mm") {
                    (n, 96.0 / 25.4)
                } else if let Some(n) = t.strip_suffix("in") {
                    (n, 96.0)
                } else if let Some(n) = t.strip_suffix("pt") {
                    (n, 96.0 / 72.0)
                } else if let Some(n) = t.strip_suffix("pc") {
                    (n, MEDIUM_FONT_SIZE_PX)
                } else if let Some(n) = t.strip_suffix('q').or_else(|| t.strip_suffix('Q')) {
                    (n, 96.0 / 101.6)
                } else {
                    (t, 1.0)
                };
                // Длина от рамки фигуры — только при
                // `transform-box: fill-box`; по умолчанию (`view-box`)
                // отсчёт от вьюпорта, то есть без сдвига на `off`
                // (`svg-origin-length-*` зелены именно так).
                let v = num.trim().parse::<f32>().ok()? * k;
                return Some(if box_relative { off + v } else { v });
            }
        })
    };
    let toks: Vec<&str> = raw.split_whitespace().collect();
    // Осевые ключевые слова (css-transforms §4): одиночный `top`/`bottom`
    // — это ось Y с центром по X; пара слов может идти в любом порядке,
    // но `top 100%` невалидна — тогда точка отсчёта остаётся `0 0`
    // (None = без origin-обёртки).
    let vert_only = |t: &str| matches!(t.trim(), "top" | "bottom");
    let horiz_only = |t: &str| matches!(t.trim(), "left" | "right");
    let keyword = |t: &str| matches!(t.trim(), "top" | "bottom" | "left" | "right" | "center");
    let (ox, oy) = match toks.as_slice() {
        [a] if vert_only(a) => (fx + fw * 0.5, side(a, fh, fy)?),
        [a] => (side(a, fw, fx)?, fy + fh * 0.5),
        // Пара слов ОДНОЙ оси невалидна (css-transforms-1 §transform-origin):
        // `top bottom`, `left right` — объявление отбрасывается целиком.
        [a, b] if (vert_only(a) && vert_only(b)) || (horiz_only(a) && horiz_only(b)) => {
            return None;
        }
        [a, b] if vert_only(a) || horiz_only(b) => {
            // Обратный порядок допустим только у ПАРЫ ключевых слов.
            if keyword(a) && keyword(b) {
                (side(b, fw, fx)?, side(a, fh, fy)?)
            } else {
                return None;
            }
        }
        [a, b] => (side(a, fw, fx)?, side(b, fh, fy)?),
        _ => return None,
    };
    Some((ox, oy))
}
