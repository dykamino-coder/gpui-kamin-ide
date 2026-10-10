//! Раскладка и окраска плиток после определения областей фона.

use super::{origin, rounded, tile_size};
use crate::paint::background::*;
use crate::style::computed::{BgPos, BgRepeat, BgSize, Computed, Tiling};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tiles(
    c: &Computed,
    bounds: Bounds<Pixels>,
    canvas: Option<Bounds<Pixels>>,
    window: &mut gpui::Window,
    inset: [f32; 4],
    radius: f32,
    found: Source,
    paint_box: Bounds<Pixels>,
    size: BgSize,
    repeat: BgRepeat,
    pos: BgPos,
    fixed_area: bool,
    border: crate::style::computed::Sides,
    px_of: impl Fn(Option<Len>) -> f32,
) {
    let bounds = exact_layer::positioning(bounds);
    // Место под фон: свой край по `background-origin`.
    let bounds = Bounds {
        origin: gpui::point(
            bounds.origin.x + px(inset[3]),
            bounds.origin.y + px(inset[0]),
        ),
        size: gpui::size(
            bounds.size.width - px(inset[1] + inset[3]),
            bounds.size.height - px(inset[0] + inset[2]),
        ),
    };
    let box_size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let tile = tile_size(found.intrinsic(), box_size, size);
    // Плитка НЕ бывает осмысленно шире считанных коробок: вырожденное
    // соотношение (`viewBox` в миллиарды) давало размер за пределами
    // точности float, и координаты копий разваливались. Видима всё равно
    // только часть в коробке.
    // Потолок — от БОЛЬШЕЙ стороны коробки: при нулевой высоте места под фон
    // (`height: 0; padding-bottom: 100px; background-origin: content-box`)
    // потолок по своей оси выходил 8 точек, и `cover` 100×50 рисовался
    // полоской 100×8 (`background-size-cover-003`).
    let cap = box_size.0.max(box_size.1).max(1.0) * 8.0;
    let tile = (tile.0.min(cap), tile.1.min(cap));
    // Нулевая плитка не рисуется вовсе, а вот МЕЛКАЯ — рисуется: браузер
    // мостит и долями точки. Ограничивается не размер плитки, а их ЧИСЛО.
    if tile.0 <= 0.0 || tile.1 <= 0.0 {
        return;
    }
    // Плитка мельче половины точки неразличима: копии сливаются в сплошную
    // заливку, и мы делаем ровно её — одной растянутой копией. Сливаются
    // только МОСТЯЩИЕСЯ оси (`background-size-near-zero-*`).
    const MERGE: f32 = 0.5;
    let merge = |len: f32, box_len: f32, mode: Tiling| {
        if len < MERGE && mode != Tiling::None {
            box_len
        } else {
            len
        }
    };
    let tile = (
        merge(tile.0, box_size.0, repeat.axis(true)),
        merge(tile.1, box_size.1, repeat.axis(false)),
    );
    // `round` меняет САМ размер плитки, поэтому считается до смещения.
    let before = tile;
    let tile = (
        rounded(repeat.axis(true), tile.0, box_size.0),
        rounded(repeat.axis(false), tile.1, box_size.1),
    );
    // css-backgrounds-3 §3.9, third step: `round` along one axis only with
    // `auto` size along the other rescales that other axis so the original
    // aspect ratio is restored (`background-size-029`: 52px auto + round
    // repeat → 60×60, not 60×52).
    let auto_axis = |horizontal: bool| match size {
        BgSize::Auto => true,
        BgSize::Fixed(w, h) => {
            if horizontal {
                w.is_none()
            } else {
                h.is_none()
            }
        }
        _ => false,
    };
    let (round_x, round_y) = (
        repeat.axis(true) == Tiling::Round,
        repeat.axis(false) == Tiling::Round,
    );
    let tile = if round_x && !round_y && auto_axis(false) && before.0 > 0.0 {
        (tile.0, tile.0 * before.1 / before.0)
    } else if round_y && !round_x && auto_axis(true) && before.1 > 0.0 {
        (tile.1 * before.0 / before.1, tile.1)
    } else {
        tile
    };
    // Плитки МЕРЯЮТСЯ областью позиционирования, а КЛАДУТСЯ по всей краске:
    // у канваса это весь холст, и полоса `repeat-x` обязана выходить за поля
    // корня (`background-root-016`: «extending … to the left and right edges
    // of the page»).
    let clip = sampling::snapped_clip(
        if fixed_area {
            paint_box
        } else {
            canvas.unwrap_or(paint_box)
        },
        window,
    );
    let start = origin(pos, box_size, tile);
    let shift = (
        f64::from(f32::from(bounds.origin.x)) - f64::from(f32::from(clip.origin.x)),
        f64::from(f32::from(bounds.origin.y)) - f64::from(f32::from(clip.origin.y)),
    );
    let span = (f32::from(clip.size.width), f32::from(clip.size.height));
    // `space` раздаёт зазоры внутри ОБЛАСТИ ПОЗИЦИОНИРОВАНИЯ, а не по холсту
    // (css-backgrounds-3 §3.4), поэтому длина ему нужна своя.
    let lay = tile_positions::axis;
    let mut xs = lay(
        repeat.axis(true),
        start.0,
        tile.0,
        shift.0,
        box_size.0,
        span.0,
    );
    let mut ys = lay(
        repeat.axis(false),
        start.1,
        tile.1,
        shift.1,
        box_size.1,
        span.1,
    );
    // Общий потолок числа квадов: потолок НА ОСЬ пропускал произведение
    // (плитка 1x1 на вьюпорт = ~480 тысяч квадов — кадр не заканчивался,
    // hidpi-invert-filter-background висел). Плитки ПРОРЕЖИВАЮТСЯ с
    // укрупнением квада: покрытие коробки сохраняется (для одноцветной
    // 1x1 — точно, узор теряет лишь плотность повтора).
    const MAX_QUADS: usize = 4096;
    let mut tile = tile;
    if xs.len() * ys.len() > MAX_QUADS {
        let k = ((xs.len() * ys.len()) as f32 / MAX_QUADS as f32)
            .sqrt()
            .ceil() as usize;
        xs = xs.into_iter().step_by(k).collect();
        ys = ys.into_iter().step_by(k).collect();
        tile = (tile.0 * k as f32, tile.1 * k as f32);
    }
    let Some(image) = conic::tile(&found, tile, window.scale_factor()) else {
        return;
    };
    let corners = gpui::Corners::all(px(radius));
    let pixelated = c.image_pixelated == Some(true);
    // Собственная обрезка коробки (`overflow` ≠ visible) режет по её
    // padding-box, а фон по `background-clip` живёт до border-box: маска
    // раздвигается на рамку — ровно на то, что коробка отняла у себя сама
    // (`attachment-local-clipping-image-*`).
    let clips_self = matches!(c.overflow_x, Some(o) if o != crate::style::computed::Overflow::Visible)
        || matches!(c.overflow_y, Some(o) if o != crate::style::computed::Overflow::Visible);
    let outer = if clips_self {
        let cur = window.content_mask().bounds;
        Bounds {
            origin: gpui::point(
                cur.origin.x - px(px_of(border.left)),
                cur.origin.y - px(px_of(border.top)),
            ),
            size: gpui::size(
                cur.size.width + px(px_of(border.left) + px_of(border.right)),
                cur.size.height + px(px_of(border.top) + px_of(border.bottom)),
            ),
        }
    } else {
        window.content_mask().bounds
    };
    window.with_content_mask_replaced(gpui::ContentMask { bounds: outer }, |window| {
        window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |window| {
            for y in &ys {
                for x in &xs {
                    // Reduce to Pixels only after cancelling the clip offset.
                    let at = gpui::point(
                        px((f64::from(f32::from(clip.origin.x)) + x) as f32),
                        px((f64::from(f32::from(clip.origin.y)) + y) as f32),
                    );
                    let cell = Bounds {
                        origin: at,
                        size: gpui::size(px(tile.0), px(tile.1)),
                    };
                    sampling::paint_tile(window, cell, corners, image.clone(), &found, pixelated);
                }
            }
        });
    });
}
