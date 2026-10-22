//! Неизменяемый рисунок SVG как картинка: исходная разметка и её природный размер
//! (`width`/`height` корня, иначе соотношение `viewBox`) плюс признак нулевой оси
//! `viewBox`. Разбирается текст открывающего тега корня, а не дерево: дерево рисунку
//! как картинке не нужно, растеризатору всё равно идёт исходная разметка.

use super::natural::Intrinsic;
use std::sync::Arc;

/// Рисунок: разметка и природный размер. Растрируется под размер места потребителем.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VectorImage {
    pub(crate) markup: Arc<str>,
    pub(crate) natural: Intrinsic,
}

/// Открывающий тег корня `<svg …` без `>`; `None`, если корня нет.
pub(crate) fn root_tag(markup: &str) -> Option<&str> {
    let at = markup.find("<svg")?;
    let end = markup[at..]
        .find('>')
        .map(|e| at + e)
        .unwrap_or(markup.len());
    Some(&markup[at..end])
}

/// Нулевая ось `viewBox`: соотношение вырождено, рисовать нечего.
pub(crate) fn degenerate_viewbox(markup: &str) -> bool {
    let Some(head) = root_tag(markup) else {
        return false;
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

/// Своя величина рисунка: `width`/`height` корневого тега, иначе `viewBox`.
pub(crate) fn svg_size(markup: &str) -> Intrinsic {
    let head = root_tag(markup).unwrap_or(markup);
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

/// Разметка похожа на SVG: корневой тег ищется в первых 4096 байтах.
///
/// Ищем корневой тег, а не начало файла: перед ним стоят и объявление XML,
/// и комментарий с лицензией — с них начинается добрая половина рисунков
/// набора (`background-size/vector/support/*`). Окно широкое: комментарий
/// в `colors-16x8-parDefault.svg` длиннее 512 байт, и рисунок не
/// распознавался вовсе.
pub(crate) fn looks_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(4096)];
    std::str::from_utf8(head)
        .ok()
        .map(|t| t.contains("<svg"))
        .unwrap_or(false)
}
