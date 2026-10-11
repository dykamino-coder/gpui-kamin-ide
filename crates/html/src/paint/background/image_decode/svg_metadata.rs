//! Размер и фон SVG для декодирования фоновых изображений.

use super::Intrinsic;

/// Своя величина рисунка: `width`/`height` корневого тега, иначе `viewBox`.
///
/// Разбирается по тексту, а не деревом: дерево документа рисунка нам не нужно
/// нигде больше, а растеризатору всё равно идёт исходная разметка.
/// Нулевая ось `viewBox`: соотношение вырождено, рисовать нечего.
pub(super) fn degenerate_viewbox(markup: &str) -> bool {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => return false,
    };
    let Some(at) = head.find("viewBox=") else {
        return false;
    };
    let rest = head[at + 8..].trim_start();
    let Some(quote) = rest.chars().next() else {
        return false;
    };
    let Some(vb) = rest[1..].split(quote).next() else {
        return false;
    };
    let nums: Vec<f32> = vb
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    nums.len() == 4 && (nums[2] <= 0.0 || nums[3] <= 0.0)
}

/// Фон КАНВЫ рисунка: `style="background: …"` на корневом `<svg>`.
///
/// Это CSS-свойство замещаемого корня, а не SVG-контент — растеризатор его
/// не рисует, и рисунок из одного фона выходил прозрачным (box-sizing-007).
pub(crate) fn svg_root_background(markup: &str) -> Option<crate::style::values::value::Color> {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => return None,
    };
    let at = head.find("style=")?;
    let rest = head[at + 6..].trim_start();
    let quote = rest.chars().next()?;
    let style = rest[1..].split(quote).next()?;
    let decls = crate::style::css::parse_decls(style);
    let v = decls
        .get("background")
        .or_else(|| decls.get("background-color"))?;
    crate::style::values::value::Color::parse(v.split_whitespace().next()?)
}

pub(crate) fn svg_size(markup: &str) -> Intrinsic {
    let head = match markup.find("<svg") {
        Some(at) => {
            &markup[at..markup[at..]
                .find('>')
                .map(|e| at + e)
                .unwrap_or(markup.len())]
        }
        None => markup,
    };
    let raw = |name: &str| -> Option<&str> {
        let at = head.find(&format!("{name}="))?;
        let rest = head[at + name.len() + 1..].trim_start();
        let quote = rest.chars().next()?;
        Some(rest[1..].split(quote).next()?.trim())
    };
    // Доля СВОЕЙ величиной не является: она считается от места под фон, то
    // есть сторона у рисунка отсутствует (SVG §7.2 и css-images-3 §4.1).
    let side = |name: &str| -> Option<f32> {
        let v = raw(name)?;
        if v.ends_with('%') {
            return None;
        }
        v.trim_end_matches("px")
            .parse()
            .ok()
            .filter(|n: &f32| *n > 0.0)
    };
    let ratio = raw("viewBox").and_then(|vb| {
        let nums: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0).then(|| nums[2] / nums[3])
    });
    let (w, h) = (side("width"), side("height"));
    Intrinsic {
        w,
        h,
        // Обе стороны заданы — соотношение из них, иначе из `viewBox`.
        ratio: match (w, h) {
            (Some(w), Some(h)) => Some(w / h),
            _ => ratio,
        },
    }
}
