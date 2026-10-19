//! Размещение одного флоата с формой: размеры, полоса, форма выреза, держатель.

use super::float_shape_of;
use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

mod holder;
use holder::push_float_holder;

#[allow(clippy::too_many_arguments)]
pub(super) fn place_shape_float(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    px_of: impl Fn(&Option<Len>) -> f32,
    left: &mut Vec<super::super::shapes::FloatShape>,
    right: &mut Vec<super::super::shapes::FloatShape>,
    cb_w: f32,
    vert_lr: bool,
    line_left_bottom: bool,
    wall: f32,
    bands: &mut super::super::bands::FloatBands,
    floats: &mut Vec<AnyElement>,
    host_side: i32,
    f: &Element,
) {
    let vcx = opts.cx.vcx();
    let side = f
        .style
        .float
        .filter(|v| *v != 0)
        .map(i32::from)
        .unwrap_or(host_side);
    let b = f.style.borders();
    let (ml, mr) = (px_of(&f.style.margin.left), px_of(&f.style.margin.right));
    let (mt, mb) = (px_of(&f.style.margin.top), px_of(&f.style.margin.bottom));
    let (bl, br_) = (px_of(&b.left), px_of(&b.right));
    let (bt, bb) = (px_of(&b.top), px_of(&b.bottom));
    let (pl, pr) = (px_of(&f.style.padding.left), px_of(&f.style.padding.right));
    let (pt, pb) = (px_of(&f.style.padding.top), px_of(&f.style.padding.bottom));
    let (mut cw, mut chh) = (px_of(&f.style.width), px_of(&f.style.height));
    // `box-sizing: border-box` — заданная длина ВКЛЮЧАЕТ отступы и рамку
    // (css-ui-3 §5.1), а дальше здесь считается контентная. Без вычитания
    // опорная коробка выходила шире содержащего блока, и вырез уводил
    // строки в минус (`shape-outside-content-box-003`, `-padding-box-003`).
    if f.style.border_box == Some(true) {
        if cw > 0.0 {
            cw = (cw - pl - pr - bl - br_).max(0.0);
        }
        if chh > 0.0 {
            chh = (chh - pt - pb - bt - bb).max(0.0);
        }
    }
    // Флоат без своих размеров с картинкой-формой: размер — интринзик
    // картинки (частый паттерн shape-image-тестов).
    if cw <= 0.0
        && chh <= 0.0
        && let Some(raw0) = f.style.shape_outside.as_deref()
        && raw0.contains("url(")
        && let Some(u) = crate::style::computed::parse_url(raw0)
        && let Some((w, h)) = crate::paint::background::intrinsic_px(&u, vcx)
    {
        // Своя величина, а не размер растра: SVG растрируется вдвое
        // плотнее (`background::intrinsic_px`).
        cw = w;
        chh = h;
    }
    let (mw, mh) = (
        ml + bl + pl + cw + pr + br_ + mr,
        mt + bt + pt + chh + pb + bb + mb,
    );
    let raw = f.style.shape_outside.clone().unwrap_or_default();
    // Опорная коробка формы: margin-box по умолчанию (css-shapes §3).
    let (bx, by, bw, bh) = if raw.contains("border-box") {
        (ml, mt, mw - ml - mr, mh - mt - mb)
    } else if raw.contains("padding-box") {
        (
            ml + bl,
            mt + bt,
            mw - ml - mr - bl - br_,
            mh - mt - mb - bt - bb,
        )
    } else if raw.contains("content-box") {
        (ml + bl + pl, mt + bt + pt, cw, chh)
    } else {
        (0.0, 0.0, mw, mh)
    };
    // Размещение по правилам 1-9 §9.5.1. `clear` сюда не доезжает:
    // группу и хвост `wrap_floats` рвёт на первом же `clear` своей
    // стороны.
    // `clear` берётся с самого флоата: у бандового хоста пробег на нём не
    // рвётся, и очистку исполняют полосы. На живом пути `shape-outside`
    // группа рвётся раньше, и `clear` там всегда `None`.
    let (fx, fy) = bands.add_float(side as i8, mw, mh, f.style.clear);
    // Форма выреза и держатель адресуются ОТ СВОЕЙ стороны, а полосы
    // считают обе границы от инлайн-начала: перевод здесь и только здесь.
    let off = if side < 0 { fx } else { wall - fx - mw };
    let sm = match f.style.shape_margin {
        Some(Len::Px(v)) => v,
        // Доля — от ИНЛАЙН-размера содержащего блока (css-shapes-1
        // §shape-margin-property): в вертикальном письме это его высота,
        // которую несёт хост (`shape-outside-linear-gradient-012`:
        // `shape-margin: 25%` у блока 100×200 — 25, а не 50).
        Some(Len::Pct(p)) => {
            let vertical = inherited.wm().block_rl() || vert_lr;
            match e.style.height {
                Some(Len::Px(h)) if vertical => p * h,
                _ => p * cb_w,
            }
        }
        _ => 0.0,
    };
    // Вертикальное письмо (`vertical-rl`, `sideways-rl`): форма обтекания
    // адресуется в ЛОГИЧЕСКИХ осях — блок-ось горизонтальна и идёт от
    // правого края, инлайн-ось вертикальна, line-left = верх,
    // line-right = низ (css-writing-modes-4 §6.3). Ни `FloatShape::
    // Ellipse`, ни строчный `Profile` этого не выражают, поэтому здесь
    // ВСЕ фигуры идут одним растровым путём и режутся столбцами.
    let vert_rl = inherited.wm().block_rl() || vert_lr;
    let shape = float_shape_of(
        vert_lr,
        line_left_bottom,
        f,
        side,
        ml,
        mt,
        bl,
        bt,
        pl,
        pt,
        cw,
        chh,
        mw,
        mh,
        raw,
        bx,
        by,
        bw,
        bh,
        off,
        sm,
        vert_rl,
        vcx,
    );
    let mut shape = shape;
    if fy > 0.0 {
        shape.shift_top(fy);
    }
    if side < 0 {
        left.push(shape);
    } else {
        right.push(shape);
    }
    // Сам флоат — absolute у своей стороны.
    push_float_holder(
        inherited,
        opts,
        vert_lr,
        line_left_bottom,
        floats,
        f,
        side,
        ml,
        mr,
        mt,
        mb,
        fy,
        off,
    );
}
