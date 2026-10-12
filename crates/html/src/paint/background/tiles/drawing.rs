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
    // CSS Backgrounds 3 §3.9: cover may require a tile much larger than
    // the painting area. Cap the allocated raster, never this concrete size,
    // its aspect ratio, or the free space used by background-position.
    // Нулевая плитка не рисуется вовсе, а вот МЕЛКАЯ — рисуется: браузер
    // мостит и долями точки. Ограничивается не размер плитки, а их ЧИСЛО.
    if tile.0 <= 0.0 || tile.1 <= 0.0 || !tile.0.is_finite() || !tile.1.is_finite() {
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
    let start = c.background_position_used(
        origin(pos, box_size, tile),
        (box_size.0 - tile.0, box_size.1 - tile.1),
    );
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
    // Скругление — у ОБЛАСТИ краски (css-backgrounds-3 §5.3: фон режется
    // кривой края коробки), а не у каждой плитки: угол плитки скругляется
    // радиусом своего угла коробки, только когда он и есть угол области
    // краски (`origin-border-box_with_radius`: плитка 160 в коробке 514×288
    // скруглялась всеми четырьмя углами).
    let r = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v.max(0.0),
        _ => 0.0,
    };
    let radii = [
        r(c.radius.tl),
        r(c.radius.tr),
        r(c.radius.br),
        r(c.radius.bl),
    ];
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    let (pl, pt) = (f32::from(clip.origin.x), f32::from(clip.origin.y));
    let (pr, pb) = (
        pl + f32::from(clip.size.width),
        pt + f32::from(clip.size.height),
    );
    let corners_of = |cell: Bounds<Pixels>| {
        let (l, t) = (f32::from(cell.origin.x), f32::from(cell.origin.y));
        let (rr, b) = (
            l + f32::from(cell.size.width),
            t + f32::from(cell.size.height),
        );
        gpui::Corners {
            top_left: px(if near(l, pl) && near(t, pt) {
                radii[0]
            } else {
                0.0
            }),
            top_right: px(if near(rr, pr) && near(t, pt) {
                radii[1]
            } else {
                0.0
            }),
            bottom_right: px(if near(rr, pr) && near(b, pb) {
                radii[2]
            } else {
                0.0
            }),
            bottom_left: px(if near(l, pl) && near(b, pb) {
                radii[3]
            } else {
                0.0
            }),
        }
    };
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
                    let corners = corners_of(cell);
                    sampling::paint_tile(window, cell, corners, image.clone(), &found, pixelated);
                }
            }
        });
    });
}
