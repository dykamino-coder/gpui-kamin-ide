//! Слои рамки для apply_paint: обычная рамка стороны и двойная (double) двумя полосами.

use super::*;
use crate::document::state::ValueCx;
use crate::style::values::value::px_lossy_zero;

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
pub(crate) fn border_layer(
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<(crate::style::values::value::Color, [f32; 4])> {
    // Рамку `double` рисуют кольца (`double_border`) — поверх плиток и так.
    if double_border(c, vcx).is_some() {
        return None;
    }
    // Слой картинки бывает только у этих двух (см. `render::decorations`).
    // `backdrop-filter`: the filtered backdrop is a child layer, so a quad
    // border would lie under it and get filtered; the element's border
    // belongs above the backdrop (Filter Effects 2 §backdrop-filter).
    let backdrop =
        (c.backdrop_color.is_some() || c.backdrop_ref.is_some()) && !c.backdrop_root_above;
    if (!c.background_paint.paints() || c.bg_image.is_none() && !c.gradient_as_tile()) && !backdrop
    {
        return None;
    }
    // Особые рамки несут свои слои, разные цвета сторон — полосы поверх:
    // всё это уже лежит над плитками (`apply_paint` о них знает).
    if c.border_image.as_ref().is_some_and(|bi| bi.src.is_some())
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
    let uniform = c.used_uniform_border_color()?;
    let w = c.borders();
    let side_px = px_lossy_zero;
    let widths = [
        side_px(w.top),
        side_px(w.right),
        side_px(w.bottom),
        side_px(w.left),
    ];
    if !widths.iter().any(|v| *v > 0.0) {
        return None;
    }
    Some((
        crate::paint::background::border_paint(c, uniform, vcx),
        widths,
    ))
}

/// Рамка `double` (css-backgrounds-3 §4.2): «two parallel solid lines with
/// some space between them». Квад GPUI умеет только сплошную, поэтому обе
/// линии рисуют кольца в `render::decorations`, а квад и слой рамки цвета не
/// получают. `Some((цвет, толщины))`, когда КАЖДАЯ видимая сторона `double`
/// толщиной от 3 px (тоньше линии не разойтись — Blink рисует сплошной) и
/// цвет у сторон один (иначе поверх легли бы полосы сторон).
pub(crate) fn double_border(
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<(crate::style::values::value::Color, [f32; 4])> {
    if c.border_image.as_ref().is_some_and(|bi| bi.src.is_some())
        || c.corner_shaped()
        || c.border_shape.is_some()
    {
        return None;
    }
    let w = c.borders();
    let side_px = px_lossy_zero;
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
    // Only the painted sides count: `border-left: 10px double red` colours
    // one side and leaves the zero-width others unset, and the rings stay
    // one colour (css-backgrounds-3 §4.2; `flex-gap-decorations-004-ref`).
    let mut sides = (0..4)
        .filter(|&i| widths[i] > 0.0)
        .map(|i| c.used_border_color(i));
    let colour = sides.next()?;
    if sides.any(|s| s != colour) {
        return None;
    }
    Some((
        crate::paint::background::border_paint(c, colour, vcx),
        widths,
    ))
}

pub(super) fn paint_border_colors(
    mut d: Div,
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Div {
    // Цвет рамки: единый — прямо в стиль. Разные цвета сторон рисуются
    // полосами в сборщике дерева: у GPUI цвет рамки один на элемент.
    // Рамка-картинка рисуется ВМЕСТО обычной рамки (css-backgrounds-3 §6):
    // толщина остаётся держать раскладку, а цвет не красится — иначе рамка
    // проступала из-под картинки (`border-image-00*`: «no red», а красная
    // рамка видна).
    let uniform = c.used_uniform_border_color();
    let border_image_on = c.border_image.as_ref().is_some_and(|bi| bi.src.is_some());
    // При РАЗНЫХ цветах сторон квад не красится вовсе: его рамка рисуется
    // поверх детей, и красная полоса накрывала цветную
    // (clip-path-polygon-007: `border: red` + `border-left: lime`).
    let mixed = uniform.is_none();
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
    let strips = mixed && c.border_shape.is_none();
    if !border_image_on
        && !crate::paint::background::single_corner::eligible(c)
        && !mixed
        && !strips
        && !c.corner_shaped()
        && c.border_shape.is_none()
        && border_layer(c, vcx).is_none()
        && double_border(c, vcx).is_none()
        && let Some(bc) = uniform
    {
        d = d.border_color(crate::paint::background::border_paint(c, bc, vcx).to_hsla());
    }
    d
}
