//! image-set(): выбор кандидата по типу и плотности, разрешение картинки (`x`/`dppx`/`dpi`).

use super::*;

/// Выбор кандидата `image-set()` (css-images-4 §2.5).
///
/// * `None` — запись НЕГОДНА (отрицательное разрешение числом, вложенный
///   `image-set()`, чужое слово): объявление отбрасывается, прежнее живёт;
/// * `Some(None)` — запись годна, но пригодных кандидатов нет: «invalid image»;
/// * `Some(Some(src))` — СЫРАЯ запись выбранного `<image>` (`url(...)`,
///   градиент); строка-адрес оборачивается в `url(...)` (§2.5: «Each
///   `<string>` inside image-set() represents a `<url>`»).
pub(in crate::style::computed) fn image_set_pick(inner: &str) -> Option<Option<String>> {
    let mut options: Vec<(String, f32)> = vec![];
    for cand in crate::style::css::split_args(inner) {
        let mut image: Option<String> = None;
        let mut res: Option<f32> = None;
        let mut type_ok = true;
        for token in split_outside_parens(cand.trim()) {
            let low = token.to_ascii_lowercase();
            if let Some(body) = low.strip_prefix("type(") {
                // Неподдержанный тип снимает КАНДИДАТА, а не запись (§2.5).
                // Список — форматы, которые читает `background::decode`.
                let mime = body.trim_end_matches(')').trim().trim_matches(is_quote);
                type_ok &= matches!(
                    mime,
                    "image/png"
                        | "image/jpeg"
                        | "image/gif"
                        | "image/webp"
                        | "image/bmp"
                        | "image/svg+xml"
                );
                continue;
            }
            if let Some(r) = image_resolution(&low) {
                // Отрицательное ЧИСЛО вне диапазона по определению — ошибка
                // разбора. Из `calc()` оно приходит вычисленным: запись годна,
                // непригоден только кандидат (`negative-resolution-3`).
                if r < 0.0 && !low.starts_with("calc(") {
                    return None;
                }
                res = Some(r);
                continue;
            }
            if image.is_none() {
                if low.starts_with("image-set(") || low.starts_with("-webkit-image-set(") {
                    return None;
                }
                if token.len() >= 2 && token.starts_with(is_quote) {
                    image = Some(format!("url({token})"));
                    continue;
                }
                if low.contains('(') {
                    image = Some(token.clone());
                    continue;
                }
            }
            return None;
        }
        let Some(image) = image else { continue };
        let res = res.unwrap_or(1.0);
        // Шаг 1 (тип), нулевая/отрицательная плотность (картинке не из чего
        // взять природный размер) и шаг 2 (дубль разрешения среди оставшихся).
        if !type_ok || res <= 0.0 || options.iter().any(|(_, r)| *r == res) {
            continue;
        }
        options.push((image, res));
    }
    // Шаг 4 отдан UA: наименьшее разрешение, которого хватает на плотность
    // 1x, иначе наибольшее (Blink `CSSImageSetValue::GetBestOption`).
    let best = options
        .iter()
        .filter(|(_, r)| *r >= 1.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .or_else(|| options.iter().max_by(|a, b| a.1.total_cmp(&b.1)));
    Some(best.map(|(src, _)| src.clone()))
}

/// `<resolution>` в `dppx` (css-values-4 §6.3), в том числе внутри `calc()`.
/// Единицы разрешения переписываются точками (`1x` → `1px`, `96dpi` → `1px`),
/// арифметику считает готовый разборщик длин. `None` — в записи нет ни одного
/// разрешения либо она не сводится к числу.
fn image_resolution(token: &str) -> Option<f32> {
    if !token.is_ascii() {
        return None;
    }
    let b = token.as_bytes();
    let mut expr = String::with_capacity(token.len() + 8);
    let mut found = false;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        // Имя (`calc`, `url`, слово пути) переписывается целиком: цифра внутри
        // имени числом не начинается.
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_') {
                i += 1;
            }
            expr.push_str(&token[s..i]);
            continue;
        }
        let number =
            c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|n| n.is_ascii_digit()));
        if !number {
            expr.push(c as char);
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        let n: f32 = token[s..i].parse().ok()?;
        let u = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let k = match &token[u..i] {
            "" => {
                expr.push_str(&token[s..i]);
                continue;
            }
            "x" | "dppx" => 1.0,
            "dpi" => 1.0 / 96.0,
            "dpcm" => 2.54 / 96.0,
            _ => return None,
        };
        found = true;
        expr.push_str(&format!("{}px", n * k));
    }
    if !found {
        return None;
    }
    match Len::parse(&expr)? {
        Len::Px(v) => Some(v),
        _ => None,
    }
}
