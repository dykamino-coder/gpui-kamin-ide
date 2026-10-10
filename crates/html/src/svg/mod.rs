//! `<svg>` внутри документа.
//!
//! Зачем отдельно: инструмент ShowWidget прямо предлагает модели рисовать
//! графики и диаграммы через SVG, поэтому без него виджеты теряют главное.
//!
//! Как: поддерево `<svg>` сериализуется обратно в разметку и растрируется
//! теми же resvg/usvg, которыми пользуется сам GPUI, а результат отдаётся как
//! готовое изображение. Путь через `svg()`-элемент не годится: он превращает
//! рисунок в одноцветную маску (это иконочный путь), а график обязан
//! сохранить цвета.
//!
//! Плата за такой подход — растр: при увеличении масштаба картинка не
//! пересчитывается. Поэтому рисуем с запасом по плотности.

use crate::dom::{Element, Node};
use clip::{shape_box, translate_only};
use gpui::{AnyElement, IntoElement};
use masks::masked_layers;
use serialize::serialize_sized;
use size::size_of;

mod clip;
mod device_image;
mod masks;
pub(crate) mod raster;
pub mod serialize;
pub(crate) mod size;

/// Во сколько раз растрировать плотнее логического размера: на дробном
/// системном масштабе (125%, 150%) картинка иначе выглядит мыльной.
const DENSITY: f32 = 2.0;

thread_local! {
    /// Размер опорной коробки `view-box` ближайшего вьюпорта (css-masking-1
    /// §5.1 `view-box`): начало — в начале системы координат `viewBox`,
    /// размер — его ширина и высота; без `viewBox` — размер самого `<svg>`.
    static VIEW_BOX: std::cell::Cell<(f32, f32)> =
        const { std::cell::Cell::new((300.0, 150.0)) };
    /// Пишутся дети `<clipPath>`: свой `<clipPath>` рядом с ними синтезировать
    /// нельзя — модель содержимого `<clipPath>` только фигуры, текст и `use`.
    static IN_CLIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Готовый элемент с рисунком либо `None`, если разобрать не удалось.
pub fn element(e: &Element) -> Option<AnyElement> {
    let (w, h) = size_of(e);
    // `overflow: visible` по оси выпускает фигуры за канву (SVG 2 §overflow):
    // растр расширяется до содержимого по свободной оси, а коробка остаётся
    // размером канвы — картинка переполняет её, как в браузере.
    let px_len = |l: Option<crate::style::values::value::Len>| match l {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let child_extent = |horiz: bool| -> f32 {
        let mut m: f32 = 0.0;
        for c in &e.children {
            if let Node::Element(el) = c {
                let own = if horiz {
                    px_len(el.style.width).or_else(|| el.attr("width").and_then(|v| v.parse().ok()))
                } else {
                    px_len(el.style.height)
                        .or_else(|| el.attr("height").and_then(|v| v.parse().ok()))
                };
                let at = if horiz {
                    el.attr("x").and_then(|v| v.parse().ok()).unwrap_or(0.0)
                } else {
                    el.attr("y").and_then(|v| v.parse().ok()).unwrap_or(0.0)
                };
                if let Some(v) = own {
                    m = m.max(at + v);
                }
                // Переполнение считается по stroke-box и с учётом чистого
                // сдвига (SVG 2 §overflow: видна вся отрисовка, включая
                // обводку): `<rect stroke-width=20 transform=translate(10 20)>`
                // в `overflow: visible` канве 230×240 рисуется до 240×250
                // (border-shape-shadow-ref). Поворот и масштаб — как прежде.
                if let Some((bx, by, bw, bh)) = shape_box(el, true) {
                    let (mut dx, mut dy) = el
                        .attr("transform")
                        .and_then(translate_only)
                        .unwrap_or((0.0, 0.0));
                    let pure = |t: &crate::style::computed::Transform| {
                        t.rotate_rad == 0.0
                            && t.skew_rad == (0.0, 0.0)
                            && t.scale == (1.0, 1.0)
                            && t.translate_pct == (0.0, 0.0)
                    };
                    match el.style.transform.as_ref() {
                        Some(t) if pure(t) => {
                            dx += t.translate.0;
                            dy += t.translate.1;
                        }
                        Some(_) => continue,
                        None => {}
                    }
                    if let Some((tx, ty)) = el.style.translate {
                        dx += px_len(Some(tx)).unwrap_or(0.0);
                        dy += px_len(Some(ty)).unwrap_or(0.0);
                    }
                    m = m.max(if horiz { bx + dx + bw } else { by + dy + bh });
                }
            }
        }
        m
    };
    // `contain: paint` перебивает видимое переполнение: край обрезки —
    // коробка плюс `overflow-clip-margin` (css-overflow-3 §overflow-clip).
    let contained = e.style.contain_paint == Some(true);
    let clip_margin = if contained {
        e.style.clip_margin.unwrap_or(0.0)
    } else {
        0.0
    };
    let visible_y =
        !contained && e.style.overflow_y == Some(crate::style::computed::Overflow::Visible);
    let visible_x =
        !contained && e.style.overflow_x == Some(crate::style::computed::Overflow::Visible);
    let rw = if visible_x {
        w.max(child_extent(true))
    } else {
        (w + clip_margin).min(child_extent(true).max(w))
    };
    let rh = if visible_y {
        h.max(child_extent(false))
    } else {
        (h + clip_margin).min(child_extent(false).max(h))
    };
    // CSS-маски на детях рисунка — слоями (см. `masked_layers`).
    if let Some(layers) = masked_layers(e, w, h, rw, rh) {
        return Some(layers);
    }
    // Переполнение ВЛЕВО и ВВЕРХ (SVG 2 §overflow, `overflow: visible`):
    // stroke-box ребёнка с чистым сдвигом уходит за начало канвы — канва
    // растёт в минус, `viewBox` сдвигается (`serialize_sized`), картинка
    // кладётся с отрицательным краем (border-shape-clips-background-ref:
    // круг r=55 на канве 100×100 выступает на 5 px со всех сторон).
    let neg_extent = |horiz: bool| -> f32 {
        let mut m: f32 = 0.0;
        for c in &e.children {
            let Node::Element(el) = c else { continue };
            let Some((bx, by, _, _)) = shape_box(el, true) else {
                continue;
            };
            let (mut dx, mut dy) = el
                .attr("transform")
                .and_then(translate_only)
                .unwrap_or((0.0, 0.0));
            match el.style.transform.as_ref() {
                Some(t)
                    if t.rotate_rad == 0.0
                        && t.skew_rad == (0.0, 0.0)
                        && t.scale == (1.0, 1.0)
                        && t.translate_pct == (0.0, 0.0) =>
                {
                    dx += t.translate.0;
                    dy += t.translate.1;
                }
                Some(_) => continue,
                None => {}
            }
            if let Some((tx, ty)) = el.style.translate {
                dx += px_len(Some(tx)).unwrap_or(0.0);
                dy += px_len(Some(ty)).unwrap_or(0.0);
            }
            let at = if horiz { bx + dx } else { by + dy };
            m = m.max(-at);
        }
        m
    };
    let nx = if visible_x { neg_extent(true) } else { 0.0 };
    let ny = if visible_y { neg_extent(false) } else { 0.0 };
    let (cw, ch) = (rw + nx, rh + ny);
    let mut img = device_image::element(serialize_sized(e, rw, rh, nx, ny), cw, ch)?;
    // CSS-фон самого <svg> (`svg { background: green }`): канва растра
    // прозрачна, фон красится коробкой картинки (svg-scale-001 и родня).
    if let Some(bg) = e.style.background {
        use gpui::Styled as _;
        img = img.bg(bg.to_hsla());
    }
    if rw > w + 0.5 || rh > h + 0.5 || nx > 0.0 || ny > 0.0 {
        Some({
            use gpui::ParentElement as _;
            use gpui::Styled as _;
            gpui::div()
                .w(gpui::px(w))
                .h(gpui::px(h))
                .flex_shrink_0()
                .child(img.absolute().top(gpui::px(-ny)).left(gpui::px(-nx)))
                .into_any_element()
        })
    } else {
        Some(img.into_any_element())
    }
}

#[cfg(test)]
mod tests;
