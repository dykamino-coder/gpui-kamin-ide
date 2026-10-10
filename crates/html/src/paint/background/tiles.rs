//! Плитки фона: размер, начало, раскладка и отрисовка (paint_tiles).

mod geometry;
pub(super) use geometry::len_px;
use geometry::origin;
pub(super) use geometry::rounded;
pub(crate) use geometry::tile_size;

mod drawing;
pub(super) use drawing::draw_tiles;

use crate::paint::background::*;
use crate::style::computed::{BgRepeat, Computed};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, px};

/// Нарисовать плитки: `area` задаёт РАЗМЕР и НАЧАЛО ОТСЧЁТА, `canvas` — что
/// именно закрашивается.
///
/// Две области нужны одному фону — КАНВАСУ (CSS 2.1 §14.2): краска «extends to
/// cover the entire canvas», а плитки «are sized and positioned relative to the
/// root element's box as if they were painted for that element alone». У
/// обычной коробки области совпадают, и `None` оставляет прежний путь слово в
/// слово.
pub fn paint_tiles(
    c: &Computed,
    bounds: Bounds<Pixels>,
    canvas: Option<Bounds<Pixels>>,
    window: &mut gpui::Window,
) {
    let Some(src) = c.bg_image.clone() else {
        return;
    };
    let size = c.bg_size;
    let repeat = c.bg_repeat.unwrap_or(BgRepeat::Repeat);
    let family = c.font_family.clone().unwrap_or_default();
    let font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    // Смещение плитки — та же длина, что и всюду: `background-position: 3em`
    // сводится к точкам ЗДЕСЬ, где известны кегль и гарнитура. Ниже по пути
    // непроходная единица читалась как ноль, и плитка вставала в угол
    // (`background-root-019`, `-023`).
    let pos = {
        // Переводятся ТОЛЬКО единицы шрифта: всё прочее (точки, проценты,
        // `calc`) ниже по пути уже понимают, а лишний перевод их портит —
        // ЗАМЕРЕНО: сплошной перевод дал CSS2 4614 -> 4610, вся потеря в
        // семье `border-*-width-applies-to-00*`.
        let to_px = |l: Option<Len>| match l {
            Some(u @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_) | Len::Lh(_))) => Some(
                Len::Px(crate::text::metrics::spacing_px(Some(u), &family, font)),
            ),
            other => other,
        };
        crate::style::computed::BgPos {
            x: to_px(c.bg_pos.x),
            y: to_px(c.bg_pos.y),
        }
    };
    let px_of = |l: Option<Len>| crate::text::metrics::spacing_px(l, &family, font);
    let border = c.borders();
    // Отступ слоя от ВНУТРЕННЕГО края рамки: слой лежит внутри коробки и
    // меряется именно им, а `background-origin` может требовать другого края
    // (css-backgrounds-3 §3.6). Положительное значение вжимает внутрь.
    // css-backgrounds-3 §3.6: «If background-attachment is fixed, this
    // property has no effect» — the positioning area is the viewport itself
    // (`background-origin-006`: content-box origin moved a fixed tile by the
    // box's border and padding).
    let fixed_area = c.bg_fixed == Some(true) && canvas.is_some();
    let inset = match c.bg_origin {
        _ if fixed_area => [0.0; 4],
        Some(crate::style::computed::BgClip::BorderBox) => [
            -px_of(border.top),
            -px_of(border.right),
            -px_of(border.bottom),
            -px_of(border.left),
        ],
        Some(crate::style::computed::BgClip::ContentBox) => [
            px_of(c.padding.top),
            px_of(c.padding.right),
            px_of(c.padding.bottom),
            px_of(c.padding.left),
        ],
        _ => [0.0; 4],
    };
    let radius = match c.radius.tl {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // `image-orientation` действует и на ФОНОВУЮ картинку (css-images-3 §5.4,
    // «Applies to: all elements»): развёрнутый и сырой растр — разные
    // картинки с разным природным размером, и ключ обязан их различать
    // (`image-orientation-none-content-images`: четыре `<img>` с
    // `background-image` под `image-orientation: none`).
    let Some(found) = source(&key_exif(&src, c)) else {
        return;
    };
    // Область ПОКРАСКИ (`background-clip`, css-backgrounds-3 §3.7): плитки
    // меряются областью позиционирования, а кладутся по всей краске —
    // border-box по умолчанию заходит под рамку, content-box режется полем
    // (`origin-*`, `background-size-cover-00*`, `background-origin-007`).
    // Слой лежит в padding-box коробки.
    // A fixed layer is positioned against the viewport but PAINTED in its
    // own box (`canvas`): the clip area is measured from that box, exactly as
    // for a scrolling layer (css-backgrounds-3 §3.7 still applies; the
    // transparent dotted border of `background-origin-006` shows the tile).
    let paint_base = if fixed_area {
        canvas.unwrap_or(bounds)
    } else {
        bounds
    };
    let paint_box = match c.bg_clip {
        Some(crate::style::computed::BgClip::PaddingBox)
        | Some(crate::style::computed::BgClip::Text) => paint_base,
        Some(crate::style::computed::BgClip::ContentBox) => Bounds {
            origin: gpui::point(
                paint_base.origin.x + px(px_of(c.padding.left)),
                paint_base.origin.y + px(px_of(c.padding.top)),
            ),
            size: gpui::size(
                (paint_base.size.width - px(px_of(c.padding.left) + px_of(c.padding.right)))
                    .max(px(0.0)),
                (paint_base.size.height - px(px_of(c.padding.top) + px_of(c.padding.bottom)))
                    .max(px(0.0)),
            ),
        },
        _ => {
            // Порядок краски (CSS 2.2 Прил. E, css-backgrounds-3 §3.7): фон
            // лежит ПОД рамкой, рамка рисуется поверх. Слой плитки — ребёнок
            // коробки и красится ПОСЛЕ её рамки, поэтому под сплошной
            // непрозрачной рамкой область краски ужимается до её внутреннего
            // края: результат тот же, что «под рамкой». Пунктир, `double` и
            // полупрозрачная рамка пропускают фон — там border-box целиком
            // (`background-repeat-001`, `c548-ln-ht-001`, `margin-shorthand-001`).
            let covers = |i: usize| {
                let opaque = c.border_colors[i]
                    .or(c.border_color)
                    .is_none_or(|col| col.a >= 1.0);
                // solid / inset / outset / groove / ridge — сплошная краска.
                matches!(c.border_side_styles[i], Some(3..=6) | Some(9)) && opaque
            };
            let ext = |side: Option<Len>, i: usize| if covers(i) { 0.0 } else { px_of(side) };
            let (t, r, b, l) = (
                ext(border.top, 0),
                ext(border.right, 1),
                ext(border.bottom, 2),
                ext(border.left, 3),
            );
            Bounds {
                origin: gpui::point(paint_base.origin.x - px(l), paint_base.origin.y - px(t)),
                size: gpui::size(
                    paint_base.size.width + px(l + r),
                    paint_base.size.height + px(t + b),
                ),
            }
        }
    };
    // Область позиционирования — по неокруглённой коробке, когда слой её
    // знает (`exact_layer`); краска режется округлённой (`paint_box` выше).
    draw_tiles(
        c, bounds, canvas, window, inset, radius, found, paint_box, size, repeat, pos, fixed_area,
        border, px_of,
    );
}
