//! Плитки фона: размер, начало, раскладка и отрисовка (paint_tiles).

use crate::paint::background::*;

/// Размер одной плитки в точках по правилам `background-size`.
pub(crate) fn tile_size(i: Intrinsic, box_size: (f32, f32), size: BgSize) -> (f32, f32) {
    let (bw, bh) = box_size;
    // Соотношение для растяжений: своё, иначе — из умолчального размера.
    let auto = default_size(i, box_size);
    let ratio = i
        .ratio
        .unwrap_or_else(|| if auto.1 > 0.0 { auto.0 / auto.1 } else { 1.0 });
    match size {
        BgSize::Auto => auto,
        // Без своего соотношения картинка растягивается на место под фон
        // ЦЕЛИКОМ: сохранять нечего (css-images-3 §5.3).
        BgSize::Cover | BgSize::Contain if i.ratio.is_none() => box_size,
        BgSize::Cover | BgSize::Contain => {
            let (iw, ih) = (ratio.max(0.0001), 1.0);
            let sx = bw / iw;
            let sy = bh / ih;
            // `cover` закрывает коробку целиком, `contain` вписывается в неё.
            let k = if matches!(size, BgSize::Cover) {
                sx.max(sy)
            } else {
                sx.min(sy)
            };
            (iw * k, ih * k)
        }
        // Заданная одна сторона тянет вторую по соотношению — как в CSS.
        BgSize::Fixed(w, h) => match (len_px(w, bw), len_px(h, bh)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if i.ratio.is_some() || i.w.is_some() => (w, w / ratio),
            (Some(w), None) => (w, auto.1),
            (None, Some(h)) if i.ratio.is_some() || i.h.is_some() => (h * ratio, h),
            (None, Some(h)) => (auto.0, h),
            (None, None) => auto,
        },
    }
}

pub(crate) fn len_px(l: Option<Len>, base: f32) -> Option<f32> {
    match l? {
        Len::Px(v) => Some(v),
        Len::Pct(v) => Some(base * v),
        Len::Calc(i) => {
            let s = crate::value::calc_get(i);
            Some(s.px + base * s.pct)
        }
        // Шрифтовые единицы — от запасного кегля, единой точкой.
        l @ (Len::Em(_)
        | Len::EmPx(..)
        | Len::Ch(_)
        | Len::Ic(_)
        | Len::Ex(_)
        | Len::Lh(_)
        | Len::LhPx(..)) => crate::metrics::fallback_len_px(l, "", 16.0),
        Len::Vw(_) | Len::Vh(_) => None,
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => None,
    }
}

/// Смещение первой плитки: проценты считаются от свободного места, как в CSS.
pub(crate) fn origin(pos: BgPos, box_size: (f32, f32), tile: (f32, f32)) -> (f32, f32) {
    let axis = |l: Option<Len>, box_len: f32, tile_len: f32| -> f32 {
        match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(v)) => (box_len - tile_len) * v,
            // `calc(50px + 50%)`: доля — от свободного места, как у чистой
            // доли (css-backgrounds-3 §3.6), точки — как есть. Смесь с
            // третьей природой парой не отдаётся и, как прежде, идёт нулём.
            Some(Len::Calc(i)) => crate::value::calc_get(i)
                .pct_px()
                .map_or(0.0, |(pct, px)| (box_len - tile_len) * pct + px),
            _ => 0.0,
        }
    };
    (
        axis(pos.x, box_size.0, tile.0),
        axis(pos.y, box_size.1, tile.1),
    )
}

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
            Some(u @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_) | Len::Lh(_))) => {
                Some(Len::Px(crate::metrics::spacing_px(Some(u), &family, font)))
            }
            other => other,
        };
        crate::computed::BgPos {
            x: to_px(c.bg_pos.x),
            y: to_px(c.bg_pos.y),
        }
    };
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &family, font);
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
        Some(crate::computed::BgClip::BorderBox) => [
            -px_of(border.top),
            -px_of(border.right),
            -px_of(border.bottom),
            -px_of(border.left),
        ],
        Some(crate::computed::BgClip::ContentBox) => [
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
    let paint_base = if fixed_area { canvas.unwrap_or(bounds) } else { bounds };
    let paint_box = match c.bg_clip {
        Some(crate::computed::BgClip::PaddingBox) | Some(crate::computed::BgClip::Text) => paint_base,
        Some(crate::computed::BgClip::ContentBox) => Bounds {
            origin: gpui::point(
                paint_base.origin.x + px(px_of(c.padding.left)),
                paint_base.origin.y + px(px_of(c.padding.top)),
            ),
            size: gpui::size(
                (paint_base.size.width - px(px_of(c.padding.left) + px_of(c.padding.right))).max(px(0.0)),
                (paint_base.size.height - px(px_of(c.padding.top) + px_of(c.padding.bottom))).max(px(0.0)),
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
                size: gpui::size(paint_base.size.width + px(l + r), paint_base.size.height + px(t + b)),
            }
        }
    };
    // Область позиционирования — по неокруглённой коробке, когда слой её
    // знает (`exact_layer`); краска режется округлённой (`paint_box` выше).
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
        BgSize::Fixed(w, h) => if horizontal { w.is_none() } else { h.is_none() },
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
        if fixed_area { paint_box } else { canvas.unwrap_or(paint_box) },
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
    // Собственная обрезка коробки (`overflow` ≠ visible) режет по её
    // padding-box, а фон по `background-clip` живёт до border-box: маска
    // раздвигается на рамку — ровно на то, что коробка отняла у себя сама
    // (`attachment-local-clipping-image-*`).
    let clips_self = matches!(c.overflow_x, Some(o) if o != crate::computed::Overflow::Visible)
        || matches!(c.overflow_y, Some(o) if o != crate::computed::Overflow::Visible);
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
                sampling::paint_tile(window, cell, corners, image.clone(), &found);
            }
        }
    });
    });
}

/// Размер плитки после подгонки под целое их число (`background-repeat: round`).
///
/// css-backgrounds-3 §3.4: плитка растягивается или сжимается так, чтобы вдоль
/// оси уложилось целое их число без зазоров. Одна плитка — минимум: меньше
/// целой копии не бывает.
pub(crate) fn rounded(mode: Tiling, tile: f32, box_len: f32) -> f32 {
    if mode != Tiling::Round || tile <= 0.0 || box_len <= 0.0 {
        return tile;
    }
    let count = (box_len / tile).round().max(1.0);
    box_len / count
}
