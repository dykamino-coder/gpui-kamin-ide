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
