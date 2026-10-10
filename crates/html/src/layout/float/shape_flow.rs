//! Обтекание по форме `shape-outside`.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::band_flow_host::band_flow_host;
use crate::layout::float::band_host::{BandPiece, band_piece, px_margin, px_margin_box};
use crate::layout::float::float_atom::band_atom;
use crate::layout::list::list_item;
use crate::layout::replaced::image::image;
use crate::paint::effects::grouped::grouped;
use crate::render::{RenderOpts, blocks, inline_level, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// Обтекание плавающих блоков ФОРМОЙ (`shape-outside`, css-shapes-1 §2).
///
/// Плавающие дети встают absolute у своей стороны, остальным строится
/// обычный поток, но абзацам передаются ВЫРЕЗЫ — формы в координатах от
/// верха потока; каждая строка абзаца сужается по своей высоте. Формула
/// формы считается от выбранной опорной коробки (по умолчанию margin-box),
/// затем переводится в координаты margin-box (позиция флоата).
pub(crate) fn shape_flow(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    if e.attr("bands") == Some("m") {
        return band_flow_host(e, inherited, opts);
    }
    let px_of = |l: &Option<Len>| match l {
        None => 0.0,
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let mut left: Vec<crate::layout::float::shapes::FloatShape> = Vec::new();
    let mut right: Vec<crate::layout::float::shapes::FloatShape> = Vec::new();
    // Ширина содержащего блока: от неё считаются доли формы и поля
    // (`shape-margin: 5%`), она же — дальний край для правила 7 §9.5.1.
    // Известна только точками: непроходную единицу `px_of` глушит в ноль.
    let cb_w = px_of(&e.style.width);
    // Стенка РАЗМЕЩЕНИЯ. Без известной ширины переноса флоатов здесь нет
    // совсем, а вертикальное письмо не переносит никогда — полосам в обоих
    // случаях ставится заведомо недостижимая стенка. 8192 = 2^13: обратный
    // перевод правого края в отступ от своей стороны (`wall - fx - mw`)
    // остаётся точным до 2^-11 точки, на два порядка точнее допуска полос.
    const NO_WALL: f32 = 8192.0;
    // `vertical-lr`/`sideways-lr`: та же вертикаль, но блок-старт — ЛЕВЫЙ
    // край (css-writing-modes-4 §2.1). У `sideways-lr` к тому же line-left —
    // НИЗ (§6.3): `float: left` прижимается к низу, и вырез меряется от него.
    let vert_lr = inherited.vertical == Some(true) && inherited.vertical_rl != Some(true);
    let line_left_bottom = vert_lr && inherited.sideways == Some(true);
    let wall = if cb_w > 0.0 && inherited.vertical_rl != Some(true) && !vert_lr {
        cb_w
    } else {
        NO_WALL
    };
    // Полосы занятости (CSS 2.1 §9.5.1) держат ПРЯМОУГОЛЬНУЮ занятость
    // margin-box и отвечают только за размещение. Точная форма выреза
    // (`shape-outside`) в полосы не попадает вовсе и идёт отдельными
    // списками `left`/`right`: форма меняет область ОБТЕКАНИЯ, но не
    // позицию самого флоата (css-shapes-1 §1).
    let mut bands = crate::layout::float::bands::FloatBands::new(wall);
    let mut floats: Vec<AnyElement> = Vec::new();
    let mut rest: Vec<Node> = Vec::new();
    let host_side: i32 = if e.attr("side") == Some("right") {
        1
    } else {
        -1
    };
    let count: usize = e.attr("count").and_then(|c| c.parse().ok()).unwrap_or(0);
    for (idx, n) in e.children.iter().enumerate() {
        let Node::Element(f) = n else {
            rest.push(n.clone());
            continue;
        };
        if idx >= count {
            rest.push(n.clone());
            continue;
        }
        // Сторона — с САМОГО флоата: бандовый хост собирает пробег обеих
        // сторон и `float` с детей не снимает. Атрибут `side` остаётся
        // запасным ответом для живого пути `shape-outside`.
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
            &mut floats,
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
    let shapes = std::sync::Arc::new((left, right));
    // В вертикальном письме ширина контейнера — блок-прогресс контента
    // (число колонок): полная ширина растягивала бы его на страницу.
    let mut host = shape_host_div(e, inherited, vert_lr, &bands);
    for f in floats {
        host = host.child(f);
    }
    // Блочные коробки среди полос (§9.5, последний абзац): «The border box of
    // a table, a block-level replaced element, or an element in the normal
    // flow that establishes a new block formatting context must not overlap
    // the margin box of any floats in the same block formatting context».
    // Такая коробка ищет ОКНО на всю свою высоту и съезжает вниз, пока не
    // найдёт (`bands.place_among`), а не встаёт «ниже всех флоатов»
    // (`floats-wrap-bfc-004`: BFC встаёт на y=6, оставаясь сбоку от флоата с
    // низом 20).
    //
    // Ветка включается только на бандовом хосте и только когда в хвосте есть
    // хоть один НЕ-атом: сплошные инлайн-блоки обязаны идти строчным потоком
    // `FlowRow` — они делят строку, а здесь каждый кусок берёт свою.
    //
    // Вертикальное письмо сюда не пускается: дальняя стенка полос там
    // недостижимая (`NO_WALL`, `:4845-4850`), и окна не сузятся.
    let band_host = e.attr("bands") == Some("1");
    let host = match band_host_flow(e, inherited, opts, vert_lr, bands, &rest, host, band_host) {
        Ok(value) => return value,
        Err(host) => host,
    };
    // Картина из инлайн-блоков с известными размерами — построчный поток
    // атомов (FlowRow): flex-переносом вырезы по строкам не выразить, а
    // абзац таких детей не набирает.
    let atoms: Vec<crate::layout::fragment::types::FlowChild> = Vec::new();
    let atoms_ok = true;
    let (host, shapes) = match atoms_flow(
        e,
        inherited,
        opts,
        vert_lr,
        line_left_bottom,
        &rest,
        shapes,
        host,
        atoms,
        atoms_ok,
    ) {
        Ok(value) => return value,
        Err(x) => x,
    };
    let mut flowed = inherited.clone();
    flowed.flow_shapes = Some(shapes);
    host.children(blocks(&rest, &flowed, opts))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn float_shape_of(
    vert_lr: bool,
    line_left_bottom: bool,
    f: &Element,
    side: i32,
    ml: f32,
    mt: f32,
    bl: f32,
    bt: f32,
    pl: f32,
    pt: f32,
    cw: f32,
    chh: f32,
    mw: f32,
    mh: f32,
    raw: String,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    off: f32,
    sm: f32,
    vert_rl: bool,
) -> super::shapes::FloatShape {
    if !vert_rl && let Some(at) = raw.find("circle(").or_else(|| raw.find("ellipse(")) {
        let inner = &raw[at..];
        let inner = match inner.find(')') {
            Some(end) => &inner[..=end],
            None => inner,
        };
        match crate::paint::background::shape_params(inner, bw, bh, 1.0) {
            Some((cx, cy, rx, ry)) => {
                // Координаты — от опорной коробки; переводим к margin-box.
                let (cx, cy) = (cx + bx, cy + by);
                let cx = if side < 0 { cx } else { mw - cx };
                let (rx, ry) = (rx + sm, ry + sm);
                // css-shapes-1 §3.1: «When a shape is used to define a
                // float area, the shape is clipped to the float's margin
                // box». Аналитический эллипс клипа не знает: круг
                // `at left top` второго флоата лез на радиус ВЫШЕ своего
                // флоата и выталкивал коробки под него (`circle-032`:
                // длинная коробка на y=180 вместо 60), `circle(100%)`
                // отдавал экстент шире margin-box (`circle-041`: 164 при
                // 120). Вылезающий эллипс идёт профилем по точке высоты:
                // тот же срез `ellipse_cut`, по высоте только [0, mh), по
                // оси зажат [0, mw] — как растровый путь
                // (`background::shape_profile`). Лежащий внутри — прежней
                // аналитикой, ни на сотую не меняется.
                const EPS: f32 = 0.01;
                let spills = cy - ry < -EPS || cy + ry > mh + EPS || cx + rx > mw + EPS;
                if spills {
                    let rows = mh.ceil().max(1.0) as usize;
                    let ext: Vec<f32> = (0..rows)
                        .map(|r| {
                            let y0 = r as f32;
                            let v = crate::layout::float::shapes::ellipse_cut(
                                cy,
                                rx,
                                ry,
                                cx,
                                y0,
                                (y0 + 1.0).min(mh),
                            )
                            .clamp(0.0, mw);
                            if v > 0.0 { off + v } else { 0.0 }
                        })
                        .collect();
                    crate::layout::float::shapes::FloatShape::Profile {
                        top: 0.0,
                        ext: std::sync::Arc::new(ext),
                    }
                } else {
                    crate::layout::float::shapes::FloatShape::Ellipse {
                        top: 0.0,
                        cx: cx + off,
                        cy,
                        rx,
                        ry,
                    }
                }
            }
            None => crate::layout::float::shapes::FloatShape::Band {
                top: 0.0,
                h: mh,
                w: off + mw + sm,
            },
        }
    } else {
        // Geometric rounded boxes stay continuous; raster shapes retain dilation.
        let radius_of = |c: &Option<crate::style::values::value::Len>| match c {
            Some(crate::style::values::value::Len::Px(v)) => (*v, *v),
            Some(crate::style::values::value::Len::Pct(k)) => (k * bw, k * bh),
            _ => (0.0, 0.0),
        };
        let sb = crate::paint::background::ShapeBox {
            mw,
            mh,
            rx: bx,
            ry: by,
            rw: bw,
            rh: bh,
            cx: ml + bl + pl,
            cy: mt + bt + pt,
            cw,
            ch: chh,
            // Elliptical corners (`60px 40px`, css-backgrounds-3 §5.1) keep
            // both radii (`shape-outside-border-box-border-radius-007`).
            // Each axis resolves a percentage against its own box side.
            radius: {
                let ell = f.style.radius_ell.unwrap_or([None; 4]);
                let axis = |l: crate::style::values::value::Len, base: f32| match l {
                    crate::style::values::value::Len::Px(v) => v,
                    crate::style::values::value::Len::Pct(k) => k * base,
                    _ => 0.0,
                };
                let pair = |i: usize, c: &Option<crate::style::values::value::Len>| match ell[i] {
                    Some((x, y)) => (axis(x, bw), axis(y, bh)),
                    None => radius_of(c),
                };
                [
                    pair(0, &f.style.radius.tl),
                    pair(1, &f.style.radius.tr),
                    pair(2, &f.style.radius.br),
                    pair(3, &f.style.radius.bl),
                ]
            },
            threshold: f.style.shape_threshold.unwrap_or(0.0),
        };
        if let Some(shape) = (!vert_rl && sm <= 0.0)
            .then(|| crate::paint::background::rounded_float(&raw, &sb, side))
            .flatten()
        {
            crate::layout::float::shapes::FloatShape::RoundedBox {
                top: 0.0,
                off,
                shape: std::sync::Arc::new(shape),
            }
        } else {
            let profile = if vert_rl {
                // Профиль адресуется от блок-старта: у `vertical-rl` это
                // правый край (так его и строит `shape_profile_block`), у
                // `vertical-lr` — левый, то есть тот же профиль задом наперёд.
                let pside = if line_left_bottom { -side } else { side };
                crate::paint::background::shape_profile_block(&raw, &sb, sm.max(0.0), pside).map(
                    |mut p| {
                        if vert_lr {
                            p.reverse();
                        }
                        p
                    },
                )
            } else {
                crate::paint::background::shape_profile(&raw, &sb, sm.max(0.0), side)
            };
            match profile {
                Some(ext) => crate::layout::float::shapes::FloatShape::Profile {
                    top: 0.0,
                    ext: std::sync::Arc::new(
                        ext.into_iter()
                            .map(|v| if v > 0.0 && !vert_rl { off + v } else { v })
                            .collect(),
                    ),
                },
                None if vert_rl => {
                    // Непонятная запись в вертикали: занята вся блок-ось
                    // margin-box на всю его инлайн-ось.
                    crate::layout::float::shapes::FloatShape::Band {
                        top: 0.0,
                        h: mw,
                        w: mh,
                    }
                }
                None => {
                    // Непонятная запись: прямоугольник опорной коробки со
                    // стороны текста.
                    let w_cut = if side < 0 { bx + bw } else { mw - bx };
                    crate::layout::float::shapes::FloatShape::Band {
                        top: by,
                        h: bh,
                        w: off + w_cut + sm,
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_float_holder(
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

fn shape_host_div(
    e: &Element,
    inherited: &Computed,
    vert_lr: bool,
    bands: &super::bands::FloatBands,
) -> gpui::Div {
    if inherited.vertical_rl == Some(true) || vert_lr {
        div().relative()
    } else if e.attr("inflow-height") != Some("1") && bands.bottom(None) > 0.0 {
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: гейтить охват флоатов признаком «коробка
        // образует свой контекст форматирования», как это делают Blink
        // (`block_layout_algorithm.cc`: `IsNewFormattingContext()`) и Servo
        // (`BlockFormattingContext::layout`). Предикат был полный: флоат,
        // внепоточность, `inline-*`, таблица, ячейка, гибкий, сетка,
        // `flow-root`, `contain`, обрезка по `overflow`, корень и тело.
        // Срез флоатов (333 пары, 233 зелёных): 230. Приобретений ноль,
        // потери — `clear-003` 0.00 → 3.84, `floats-005` 0.00 → 0.72,
        // `floats-wrap-top-below-bfc-001l` 0.01 → 0.61.
        // Причина: у нас флоаты в этом хосте АБСОЛЮТНЫЕ, и `min_h` держит не
        // только §10.6.7, но и высоту, которую по спеке дают ОЧИСТИВШИЕ их
        // братья в потоке. Возвращать вместе с клиренсом как величиной в
        // потоке (шаг F6 из `target/scout-float-bands-design.md`).
        // §10.6.7: хост обязан охватить флоаты высотой — здесь они
        // абсолютные и сами её не растят.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (10.09, `scout-floatover-2026-09.md`, 9
        // хунков): отрицательное `margin-top` у блочного ребёнка обнуляет
        // авто-высоту родителя (шесть проб `k1`-`k6`: верные 100/120/150/
        // 130/100/140, у нас 0.8 во всех шести — места детей при этом
        // верны, ломается только §10.6.7). Скаут закрывал это ОБЁРТКОЙ
        // заданной высоты и замерил ей восемь своих проб в 0.00.
        // Срез 3018 пар (флоаты + clamp + 700 текстовых): 2264 -> 2265,
        // и вся прибавка — от ДРУГОГО патча в пачке. Своё: `floats-135`
        // 0.00 взята, `margin-collapse-158` 0.06 -> 1.13 потеряна.
        // Бисект однозначен: тот же corpus без этого патча оставляет
        // `margin-collapse-158` = 0.06. Обёртка заданной высоты рвёт
        // §8.3.1 — сквозь неё перестают схлопываться поля. Находку
        // (обнуление высоты) держать, лечение искать без обёртки:
        // §10.6.7 считает низ содержимого от КРАЁВ детей с учётом
        // отрицательных полей, а не от суммы высот.
        div().relative().w_full().min_h(px(bands.bottom(None)))
    } else {
        div().relative().w_full()
    }
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
fn band_host_flow(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    bands: super::bands::FloatBands,
    rest: &Vec<Node>,
    mut host: gpui::Div,
    band_host: bool,
) -> Result<AnyElement, gpui::Div> {
    if band_host
        && inherited.vertical_rl != Some(true)
        && !vert_lr
        && rest
            .iter()
            .any(|n| matches!(band_piece(n), Some(BandPiece::Bfc) | Some(BandPiece::Strut)))
    {
        // Потолок потока: низ предыдущего куска (правило 5 §9.5.1). Бежит по
        // кускам и служит стартом поиска окна для следующего.
        let mut y = 0.0f32;
        for n in rest {
            let Node::Element(c) = n else {
                continue;
            };
            let (Some(kind), Some((mw, mh))) = (band_piece(n), px_margin_box(&c.style)) else {
                continue;
            };
            if matches!(kind, BandPiece::Strut) {
                // Распорка своего контекста не заводит: её border-box флоаты
                // перекрывают (обтекают только строки), а показать ей нечего
                // — от неё нужна одна высота.
                y += mh;
                continue;
            }
            let (ml, mr, mt, mb) = (
                px_margin(&c.style.margin.left).unwrap_or(0.0),
                px_margin(&c.style.margin.right).unwrap_or(0.0),
                px_margin(&c.style.margin.top).unwrap_or(0.0),
                px_margin(&c.style.margin.bottom).unwrap_or(0.0),
            );
            // §9.5, последний абзац, дословно: «The border box of a table, a
            // block-level replaced element, or an element in the normal flow
            // that establishes a new block formatting context … must not
            // overlap the margin box of any floats». Требование стоит на
            // BORDER-box коробки; её собственные поля в перечень не входят
            // ВООБЩЕ. Поэтому окно ищется под border-box, а поля работают
            // только по блочной оси: верхнее опускает потолок поиска, нижнее
            // задаёт потолок следующего куска.
            //
            // Так же у Blink (`block_layout_algorithm.cc:2164-2172`):
            // «Margins are applied from the content-box, not the layout
            // opportunity area», и проверка влезания там идёт по
            // `fragment.InlineSize()` / `fragment.BlockSize()` (`:2206`,
            // `:2283`) — то есть по border-box фрагмента.
            //
            // Отпечаток числа (`new-fc-beside-float-with-margin`): коробка
            // 50 точек с `margin-right: 1px` рядом с флоатом 50 в блоке 100.
            // margin-box 51 в окно 50 не влезал, и коробка уезжала на y=100
            // под флоат; border-box 50 влезает ровно, и она встаёт на y=0
            // сбоку — как и требует `meta assert` теста.
            //
            // border-box выводится вычитанием полей из уже посчитанного
            // margin-box: `px_margin_box` складывает width + padding + border
            // + margin теми же `px_margin`, поэтому разность точна.
            let (bw, bh) = (mw - ml - mr, mh - mt - mb);
            let (l, top, _avail) = bands.place_among(bw, bh, y + mt);
            y = top + bh + mb;
            // Коробка прижимается к инлайн-НАЧАЛУ полосы. `margin: auto`
            // прижимом не считается СОЗНАТЕЛЬНО: эталоны `-001r` выравнивают
            // свои коробки `text-align: right`, о котором `FlowRow` не знает
            // вовсе, и учесть одно без другого — значит развести пару
            // (см. scout-bfcdraft §0.3(г)).
            let mut inner = c.clone();
            // Поля кладёт держатель — на самой коробке они сдвинули бы её
            // ещё раз (та же причина, что у флоатов, `:5092-5095`).
            inner.style.margin = crate::style::computed::Sides::default();
            let merged = inherit(inherited, &c.style);
            let built = styled_div_with(&inner, &merged)
                .children(blocks(&inner.children, &merged, opts))
                .into_any_element();
            host = host.child(div().absolute().left(px(l + ml)).top(px(top)).child(built));
        }
        // §10.6.7 плюс собственная высота потока: держатели абсолютные и сами
        // хост не растят. `min_h` переопределяет поставленный на `:5129` —
        // это и нужно, там учтены только флоаты.
        let bottom = if e.attr("inflow-height") == Some("1") {
            y
        } else {
            bands.bottom(None).max(y)
        };
        return Ok(host.min_h(px(bottom)).into_any_element());
    }
    Err(host)
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
fn atoms_flow(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    line_left_bottom: bool,
    rest: &Vec<Node>,
    shapes: std::sync::Arc<(
        Vec<super::shapes::FloatShape>,
        Vec<super::shapes::FloatShape>,
    )>,
    host: gpui::Div,
    mut atoms: Vec<crate::layout::fragment::types::FlowChild>,
    mut atoms_ok: bool,
) -> Result<
    AnyElement,
    (
        gpui::Div,
        std::sync::Arc<(
            Vec<super::shapes::FloatShape>,
            Vec<super::shapes::FloatShape>,
        )>,
    ),
> {
    for n in rest {
        match n {
            Node::Text(t) => {
                if !t.trim().is_empty() {
                    atoms_ok = false;
                    break;
                }
            }
            Node::Element(c) => {
                let inline_box = matches!(
                    c.style.display,
                    Some(Display::InlineBlock) | Some(Display::InlineFlex)
                ) || (c.tag == "img" && inline_level(c));
                // Размер атома — MARGIN-box: эталон
                // `floats-wrap-top-below-003l-ref` держится на
                // `margin-top: 25px; margin-right: 250px` у второй коробки, а
                // без полей она встаёт вплотную и уезжает на 25 точек вверх.
                let dims = px_margin_box(&c.style);
                // Пустая коробка без размеров — разделитель разметки
                // (незакрытый div в хвосте) — просто пропускается.
                let empty = !inline_box
                    && c.children
                        .iter()
                        .all(|n| matches!(n, Node::Text(t) if t.trim().is_empty()))
                    && dims == Some((0.0, 0.0))
                    && c.style.background.is_none();
                if empty {
                    continue;
                }
                match (inline_box, dims) {
                    (true, Some((w, h))) if w > 0.0 && h > 0.0 => {
                        atoms.push(band_atom(c, inherited, opts).unwrap());
                    }
                    _ => {
                        atoms_ok = false;
                        break;
                    }
                }
            }
        }
    }
    if atoms_ok && !atoms.is_empty() {
        let rtl = inherited.rtl == Some(true);
        // Вертикальное письмо (`vertical-rl`, `sideways-rl`): формы уже
        // построены в осях письма (`background::shape_profile_block`) —
        // индекс равен расстоянию от блок-старта (правого края), значение —
        // экстенту вдоль физической вертикали от своей line-стороны.
        // Транспонировать их второй раз нечего.
        //
        // Прежний `transpose` схлопывал `Profile` в полосу максимального
        // экстента (`w: max(ext)`) — то есть терял форму целиком, а её несут
        // ВСЕ произвольные фигуры: `inset` с `round`, `polygon`, `path()`,
        // `shape()`, слово-коробка с `border-radius`, картинка, градиент.
        // У `Band` он вдобавок не менял оси местами: `h` брался из высоты
        // margin-box, хотя по блок-оси лежит его ШИРИНА.
        //
        // Сторона сохраняется отдельными списками: `float: left` — line-left
        // = верх, `float: right` — line-right = низ (css-writing-modes-4
        // §6.3), и от `direction` это не зависит. А `direction: rtl`
        // разворачивает инлайн-ось, и коробки идут от НИЖНЕГО края — эталоны
        // семейства (`shape-outside-inset-023-ref` и родня) меряют свой
        // `inset-inline-start` именно снизу.
        if inherited.vertical_rl == Some(true) || vert_lr {
            let mut row =
                crate::layout::fragment::types::FlowRow::new(atoms, shapes, rtl).vertical_rl();
            if vert_lr {
                row = row.block_lr();
            }
            // `sideways-lr`: инлайн-ось идёт СНИЗУ вверх (line-left — низ,
            // css-writing-modes-4 §6.3), строки ряда кладутся от нижнего края.
            if line_left_bottom {
                row = row.inline_up();
            }
            if let Some(Len::Px(v)) = e.style.height {
                row = row.inline_limit(v);
            }
            return Ok(host.child(row).into_any_element());
        }
        return Ok(host
            .child(crate::layout::fragment::types::FlowRow::new(
                atoms, shapes, rtl,
            ))
            .into_any_element());
    }
    Err((host, shapes))
}
