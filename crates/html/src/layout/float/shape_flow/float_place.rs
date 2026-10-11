//! Размещение одного флоата с формой: размеры, полоса, форма выреза, держатель.

use super::float_shape_of;
use crate::dom::Element;
use crate::layout::list::list_item;
use crate::layout::replaced::image::image;
use crate::paint::effects::grouped::grouped;
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

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
        && let Some((w, h)) = crate::paint::background::intrinsic_px(&u)
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
            let vertical = inherited.vertical_rl == Some(true) || vert_lr;
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
    let vert_rl = inherited.vertical_rl == Some(true) || vert_lr;
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

#[allow(clippy::too_many_arguments)]
pub(super) fn push_float_holder(
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    line_left_bottom: bool,
    floats: &mut Vec<AnyElement>,
    f: &Element,
    side: i32,
    ml: f32,
    mr: f32,
    mt: f32,
    mb: f32,
    fy: f32,
    off: f32,
) {
    let mut copy = f.clone();
    // Сторона и очистка уже прочитаны выше; гасить их надо ДО слияния:
    // у бандового хоста `float` доживает до сюда, а слитый стиль с
    // `float` заводит лишний контекст обрезки.
    copy.style.float = None;
    copy.style.clear = None;
    let mut merged = inherit(inherited, &copy.style);
    // Поля кладёт держатель (позиция absolute от края) — на самой
    // коробке они сдвигали бы её обратно (float: right с margin-left
    // вылезал за правый край контейнера). Снимать их надо И СО СЛИТОГО
    // стиля: коробку строит он, и через него поле возвращалось —
    // четвёрка флоатов с `margin: 10px` уезжала на поле целиком
    // (`floats-014`).
    copy.style.margin = crate::style::computed::Sides::default();
    merged.margin = crate::style::computed::Sides::default();
    // Маска и обрезка формой живут в буфере группы (`grouped`): у флоата
    // с `shape-outside` этот путь был не пройден вовсе, и `clip-path`
    // на нём не резал НИЧЕГО — коробка рисовалась целым прямоугольником,
    // тогда как эталон (тот же флоат без `shape-outside`) идёт обычным
    // путём и маску получает. Стиль берётся с самой коробки (`copy.style`,
    // поля уже сняты выше — их несёт держатель), как на пути замещаемых
    // и внепоточных (`:7718`, `:7722`).
    let built = if copy.tag == "img" {
        grouped(image(&copy), &copy.style)
    } else if copy.style.display == Some(Display::ListItem) {
        // CSS Lists 3 §2: a floated list item keeps its marker.
        grouped(
            list_item::render_with_style(&copy, inherited, &merged, opts),
            &copy.style,
        )
    } else {
        grouped(
            styled_div_with(&copy, &merged)
                .children(blocks(&copy.children, &merged, opts))
                .into_any_element(),
            &copy.style,
        )
    };
    let holder = if inherited.vertical_rl == Some(true) || vert_lr {
        // Вертикальное письмо: блок-старт — ПРАВЫЙ край, колонки
        // флоатов идут влево; инлайн-старт — верх, а у float:right
        // (line-right) — НИЗ (css-writing-modes §7,
        // shape-outside-circle-049 и родня). У `vertical-lr` блок-старт —
        // левый край.
        let col = if vert_lr {
            div().absolute().left(px(off + ml))
        } else {
            div().absolute().right(px(off + mr))
        };
        if (side < 0) != line_left_bottom {
            col.top(px(mt))
        } else {
            col.bottom(px(mb))
        }
    } else if side < 0 {
        div().absolute().left(px(off + ml)).top(px(mt + fy))
    } else {
        div().absolute().right(px(off + mr)).top(px(mt + fy))
    };
    floats.push(holder.child(built).into_any_element());
}
