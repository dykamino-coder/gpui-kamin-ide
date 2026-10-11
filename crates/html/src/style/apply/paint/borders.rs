//! Слои рамки для apply_paint: обычная рамка стороны и двойная (double) двумя полосами.

use super::*;

/// Рамка ПОВЕРХ слоя картинки: цвет и толщины сторон, если рамку надо
/// рисовать отдельным слоем после плиток фона, а не квадом коробки.
///
/// Квад красит фон и рамку одним примитивом ДО детей, а слой плиток —
/// ребёнок (`render::decorations`), поэтому полупрозрачная или пунктирная
/// рамка оказывалась ПОД картинкой (`origin-border-box`: голубая rgba-рамка
/// накрыта жёлтой плиткой; `css3-background-origin-*`: зелёный квадрат
/// поверх пунктира). css-backgrounds-3 §3.7, прим.: «The background is
/// always drawn behind the border»; Blink `box_fragment_painter.cc`:
/// `PaintFillLayers` → `PaintBorder`. Сплошную непрозрачную рамку
/// `paint_tiles` и раньше обходил ужатием области краски — слой делает то же
/// для любой рамки. `None` — рисовать по-старому.
pub(crate) fn border_layer(c: &Computed) -> Option<(crate::style::values::value::Color, [f32; 4])> {
    // Рамку `double` рисуют кольца (`double_border`) — поверх плиток и так.
    if double_border(c).is_some() {
        return None;
    }
    // Слой картинки бывает только у этих двух (см. `render::decorations`).
    if c.bg_image.is_none() && !c.gradient_as_tile() {
        return None;
    }
    // Особые рамки несут свои слои, разные цвета сторон — полосы поверх:
    // всё это уже лежит над плитками (`apply_paint` о них знает).
    if c.border_image.as_ref().is_some_and(|bi| !bi.src.is_empty())
        || c.corner_shaped()
        || c.border_shape.is_some()
    {
        return None;
    }
    // Обрезка содержимого срезала бы и слой: он лежит В коробке, на
    // отрицательных отступах (`css3-background-size-contain`: пунктирная
    // рамка исчезала при `overflow: hidden`). Такие коробки красит квад.
    if !matches!(
        c.overflow_x,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || !matches!(
        c.overflow_y,
        None | Some(crate::style::computed::Overflow::Visible)
    ) {
        return None;
    }
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.first().filter(|f| sides.iter().all(|s| s == *f));
    if sides.len() > 1 && uniform.is_none() {
        return None;
    }
    let w = c.borders();
    let side_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let widths = [
        side_px(w.top),
        side_px(w.right),
        side_px(w.bottom),
        side_px(w.left),
    ];
    if !widths.iter().any(|v| *v > 0.0) {
        return None;
    }
    // Без цвета — цвет текста, без него чёрный (как у квада).
    let colour = uniform
        .copied()
        .copied()
        .or(c.border_color)
        .or(c.color)
        .unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
    Some((crate::paint::background::border_paint(c, colour), widths))
}

/// Рамка `double` (css-backgrounds-3 §4.2): «two parallel solid lines with
/// some space between them». Квад GPUI умеет только сплошную, поэтому обе
/// линии рисуют кольца в `render::decorations`, а квад и слой рамки цвета не
/// получают. `Some((цвет, толщины))`, когда КАЖДАЯ видимая сторона `double`
/// толщиной от 3 px (тоньше линии не разойтись — Blink рисует сплошной) и
/// цвет у сторон один (иначе поверх легли бы полосы сторон).
pub(crate) fn double_border(
    c: &Computed,
) -> Option<(crate::style::values::value::Color, [f32; 4])> {
    if c.border_image.as_ref().is_some_and(|bi| !bi.src.is_empty())
        || c.corner_shaped()
        || c.border_shape.is_some()
    {
        return None;
    }
    let w = c.borders();
    let side_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let widths = [
        side_px(w.top),
        side_px(w.right),
        side_px(w.bottom),
        side_px(w.left),
    ];
    let mut any = false;
    for (i, width) in widths.iter().enumerate() {
        if *width <= 0.0 {
            continue;
        }
        if c.border_side_styles[i] != Some(10) || *width < 3.0 {
            return None;
        }
        any = true;
    }
    if !any {
        return None;
    }
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.first().filter(|f| sides.iter().all(|s| s == *f));
    if !(sides.is_empty() || sides.len() == 4) || (sides.len() > 1 && uniform.is_none()) {
        return None;
    }
    let colour = uniform
        .copied()
        .copied()
        .or(c.border_color)
        .or(c.color)
        .unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
    Some((crate::paint::background::border_paint(c, colour), widths))
}

pub(super) fn paint_border_colors(mut d: Div, c: &Computed) -> Div {
    // Цвет рамки: единый — прямо в стиль. Разные цвета сторон рисуются
    // полосами в сборщике дерева: у GPUI цвет рамки один на элемент.
    // Рамка-картинка рисуется ВМЕСТО обычной рамки (css-backgrounds-3 §6):
    // толщина остаётся держать раскладку, а цвет не красится — иначе рамка
    // проступала из-под картинки (`border-image-00*`: «no red», а красная
    // рамка видна).
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.first().filter(|f| sides.iter().all(|s| s == *f));
    let border_image_on = c.border_image.as_ref().is_some_and(|bi| !bi.src.is_empty());
    // При РАЗНЫХ цветах сторон квад не красится вовсе: его рамка рисуется
    // поверх детей, и красная полоса накрывала цветную
    // (clip-path-polygon-007: `border: red` + `border-left: lime`).
    let mixed = sides.len() > 1 && uniform.is_none();
    // Цвет не задан вовсе — рамка красится цветом текста, а без него
    // чёрным: начальное значение `border-color` — `currentColor`, начальное
    // `color` — чёрный. Прежде такая рамка не красилась ничем и пропадала,
    // хотя место в раскладке держала (строчный путь это уже делал —
    // `inline::uniform_border`).
    let any_border = {
        let w = c.borders();
        [w.top, w.right, w.bottom, w.left]
            .iter()
            .any(|l| matches!(l, Some(Len::Px(v)) if *v > 0.0))
    };
    let current = any_border.then(|| {
        c.color.unwrap_or(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
    });
    // Фигурные углы (`corner-shape`): рамку красит кольцевой слой по контуру
    // (`render::decorations`), квад цвета не получает — иначе его прямой
    // внутренний угол проступал бы из-под контура (corner-shape-bevel:
    // красный треугольник ~240 px² на угол при допуске 200 px на пару).
    // `border-shape`: рамку целиком рисует слой контура (`render::decorations`),
    // прямоугольная рамка квада проступала бы из-под фигуры.
    // Рамка над слоем картинки — отдельным слоем (`border_layer`,
    // `render::decorations`): квад цвета не получает, иначе рамка легла бы
    // ПОД плитки, а слой — второй раз поверх (полупрозрачная потемнела бы).
    // Полосы сторон (`render::decorations`) рисуются, когда цвет задан не у
    // всех четырёх сторон одинаково, — тогда квад цвета не получает: иначе
    // полупрозрачная сторона ложилась дважды, полосой поверх квада (эталоны
    // `grid-gap-decorations-*` с `border-right: 6px solid rgba(…/.5)` темнели
    // до двух слоёв). Полосы красят и стороны без своего цвета — см. там же.
    let strips =
        !sides.is_empty() && !(sides.len() == 4 && uniform.is_some()) && c.border_shape.is_none();
    if !border_image_on
        && !mixed
        && !strips
        && !c.corner_shaped()
        && c.border_shape.is_none()
        && border_layer(c).is_none()
        && double_border(c).is_none()
        && let Some(bc) = uniform.copied().copied().or(c.border_color).or(current)
    {
        d = d.border_color(crate::paint::background::border_paint(c, bc).to_hsla());
    }
    d
}
