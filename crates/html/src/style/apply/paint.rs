//! Покраска: фон, рамки, двойная рамка, слой рамки (apply_paint).

use crate::style::apply::*;
use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

/// Рамка `double` (css-backgrounds-3 §4.2): «two parallel solid lines with
/// some space between them». Квад GPUI умеет только сплошную, поэтому обе
/// линии рисуют кольца в `render::decorations`, а квад и слой рамки цвета не
/// получают. `Some((цвет, толщины))`, когда КАЖДАЯ видимая сторона `double`
/// толщиной от 3 px (тоньше линии не разойтись — Blink рисует сплошной) и
/// цвет у сторон один (иначе поверх легли бы полосы сторон).
pub(crate) fn double_border(c: &Computed) -> Option<(crate::style::values::value::Color, [f32; 4])> {
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
    let widths = [side_px(w.top), side_px(w.right), side_px(w.bottom), side_px(w.left)];
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
    if !matches!(c.overflow_x, None | Some(crate::style::computed::Overflow::Visible))
        || !matches!(c.overflow_y, None | Some(crate::style::computed::Overflow::Visible))
    {
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
    let widths = [side_px(w.top), side_px(w.right), side_px(w.bottom), side_px(w.left)];
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

pub(super) fn apply_paint(mut d: Div, c: &Computed) -> Div {
    d.style().css_border_snap = Some(true);
    // Atomic paint (CSS 2.1 Appendix E step 7.2.1.4 for inline-blocks, step 5
    // for floats; css-flexbox-1 §5.4 and css-grid-1 §9: flex and grid items
    // "paint exactly the same as inline blocks"): the box's own line content
    // is painted with it, not after later siblings' backgrounds
    // (`grid-lanes` items with overlapping negative margins).
    if c.parent_flex_grid
        || c.float.is_some_and(|f| f != 0)
        || matches!(
            c.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
    {
        d.style().paint_atomic = Some(true);
    }
    // An integer `z-index` on a positioned box (or a flex/grid item, css-flexbox-1
    // §5.4) makes it a stacking context (CSS 2.1 §9.9.1): its line content stays
    // inside it — a `z-index: -1` box's text no longer paints above later flow
    // backgrounds (`line-breaking-ic-001`).
    if c.z_index.is_some()
        && (c.parent_flex_grid
            || matches!(
                c.position,
                Some(Position::Relative)
                    | Some(Position::Absolute)
                    | Some(Position::Fixed)
                    | Some(Position::Sticky)
            ))
    {
        d.style().paint_stacking = Some(true);
    }
    // Смешивание больше не живёт на заливке: раньше блендер знал четыре
    // формулы и красил только фон узла, а CSS смешивает ВСЁ поддерево целиком.
    // Теперь оно считается при сборке буфера группы (см. `render::grouped`).
    // Фон, обрезанный внутренним краем (`background-clip`), красит не сама
    // коробка, а отдельный слой внутри неё (`render::clip_layer`): коробка в
    // раскладке красится целиком, вместе с рамкой и полями.
    if c.color_clip().is_none() {
        if let Some(g) = &c.gradient {
            // Градиенту с размером/повтором/позицией нужна механика плитки —
            // его рисует слой-картинка (см. render::decorations), заливка
            // красила бы всю коробку.
            if !c.gradient_as_tile() {
                d = d.bg(fill(g));
            }
            if let Some(bg) = c.background {
                d = d.bg(gpui::Background::from(bg.to_hsla()));
            }
        } else if let Some(bg) = c.background {
            d = d.bg(gpui::Background::from(bg.to_hsla()));
        }
    }
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
    let strips = !sides.is_empty() && !(sides.len() == 4 && uniform.is_some()) && c.border_shape.is_none();
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
    if c.border_dashed == Some(true) {
        d = d.border_dashed();
    }
    if c.border_dotted == Some(true) {
        d.style().border_style = Some(gpui::BorderStyle::Dotted);
    }
    if let Some(o) = c.opacity {
        d = d.opacity(o);
    }
    // `visibility: hidden` — элемент занимает своё место, но не рисуется.
    if c.hidden == Some(true) {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    // Элемент, не ловящий курсор, не меняет и его форму.
    if let Some(name) = c
        .cursor
        .as_ref()
        .filter(|_| c.pointer_events_none != Some(true))
    {
        // Набор GPUI совпадает с CSS почти буква в букву; неизвестное имя
        // оставляем без изменений, а не подменяем стрелкой.
        let style = match name.as_str() {
            "pointer" => Some(gpui::CursorStyle::PointingHand),
            "text" | "vertical-text" => Some(gpui::CursorStyle::IBeam),
            "crosshair" => Some(gpui::CursorStyle::Crosshair),
            "grab" => Some(gpui::CursorStyle::OpenHand),
            "grabbing" | "move" | "all-scroll" => Some(gpui::CursorStyle::ClosedHand),
            "default" | "auto" => Some(gpui::CursorStyle::Arrow),
            "not-allowed" | "no-drop" => Some(gpui::CursorStyle::OperationNotAllowed),
            "context-menu" => Some(gpui::CursorStyle::ContextualMenu),
            "copy" => Some(gpui::CursorStyle::DragCopy),
            "alias" => Some(gpui::CursorStyle::DragLink),
            "ew-resize" | "col-resize" => Some(gpui::CursorStyle::ResizeLeftRight),
            "ns-resize" | "row-resize" => Some(gpui::CursorStyle::ResizeUpDown),
            "e-resize" => Some(gpui::CursorStyle::ResizeRight),
            "w-resize" => Some(gpui::CursorStyle::ResizeLeft),
            "n-resize" => Some(gpui::CursorStyle::ResizeUp),
            "s-resize" => Some(gpui::CursorStyle::ResizeDown),
            "nwse-resize" | "nw-resize" | "se-resize" => {
                Some(gpui::CursorStyle::ResizeUpLeftDownRight)
            }
            "nesw-resize" | "ne-resize" | "sw-resize" => {
                Some(gpui::CursorStyle::ResizeUpRightDownLeft)
            }
            _ => None,
        };
        if let Some(st) = style {
            d.style().mouse_cursor = Some(st);
        }
    }
    // Тень без своего цвета — цветом текста ЭТОГО элемента (метка:
    // отрицательная альфа; css-backgrounds-3 §7, currentColor).
    let shadow_colour = |sh: &crate::style::computed::Shadow| {
        if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        }
    };
    // У `border-shape` обе тени повторяют фигуру и рисуются растром
    // (`render::grouped` — наружные, `render::decorations` — внутренние);
    // прямоугольные примитивы квада легли бы поверх и мимо фигуры.
    if !c.inset_shadows.is_empty() && c.border_shape.is_none() {
        d.style().inset_box_shadow = Some(
            c.inset_shadows
                .iter()
                // css-backgrounds-3 §7.1: «The first shadow is on top»; the
                // list is painted bottom-up, so the last one goes first.
                .rev()
                // Резкую внутреннюю рисует слой-кольцо в декорациях: примитив
                // с нулевым размытием вырождается в шейдере (как у внешней).
                .filter(|s| s.blur > 0.0)
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    // Шейдер gpui читает `blur_radius` как σ гаусса, а в CSS
                    // радиус размытия — вдвое больше σ (css-backgrounds-3
                    // §box-shadow: «blur radius … resulting shadow must
                    // approximate … a Gaussian blur with a standard deviation
                    // equal to half the blur radius»; Blink `BlurAsSigma`).
                    // Прежде тень выходила вдвое шире — как у `text-shadow`
                    // (`text_shadow_layers`), которая σ уже делит.
                    blur_radius: px(s.blur * 0.5),
                    spread_radius: px(s.spread),
                    inset: true,
                })
                .collect(),
        );
    }
    if !c.shadows.is_empty() && c.border_shape.is_none() {
        d = d.shadow(
            c.shadows
                .iter()
                // css-backgrounds-3 §7.1: the first shadow is on top.
                .rev()
                // Резкую тень (без размытия) рисует слой-квад в декорациях:
                // примитив с нулевым размытием вырождается в шейдере.
                .filter(|s| s.blur > 0.0)
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    // Шейдер gpui читает `blur_radius` как σ гаусса, а в CSS
                    // радиус размытия — вдвое больше σ (css-backgrounds-3
                    // §box-shadow: «blur radius … resulting shadow must
                    // approximate … a Gaussian blur with a standard deviation
                    // equal to half the blur radius»; Blink `BlurAsSigma`).
                    // Прежде тень выходила вдвое шире — как у `text-shadow`
                    // (`text_shadow_layers`), которая σ уже делит.
                    blur_radius: px(s.blur * 0.5),
                    spread_radius: px(s.spread),
                    inset: false,
                })
                .collect::<Vec<_>>(),
        );
    }
    d
}
