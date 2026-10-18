//! Обтекание по форме `shape-outside`.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::band_flow_host::band_flow_host;
use crate::render::{RenderOpts, blocks};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement};
mod float_place;
use float_place::place_shape_float;
mod shape;
pub(crate) use shape::float_shape_of;
mod host;
use host::{atoms_flow, shape_host_div};

/// Обтекание плавающих блоков ФОРМОЙ (`shape-outside`, css-shapes-1 §2).
///
/// Плавающие дети встают absolute у своей стороны, остальным строится
/// обычный поток, но абзацам передаются ВЫРЕЗЫ — формы в координатах от
/// верха потока; каждая строка абзаца сужается по своей высоте. Формула
/// формы считается от выбранной опорной коробки (по умолчанию margin-box),
/// затем переводится в координаты margin-box (позиция флоата).
pub(crate) fn shape_flow(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    if e.marks
        .shape_flow
        .is_some_and(|h| h.bands == Some(crate::dom::BandPath::Measured))
    {
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
    let vert_lr = inherited.wm().vertical() && !inherited.wm().block_rl();
    let line_left_bottom = vert_lr && inherited.wm().sideways();
    let wall = if cb_w > 0.0 && !inherited.wm().block_rl() && !vert_lr {
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
    let host_side: i32 = if e.marks.shape_flow.is_some_and(|h| h.right) {
        1
    } else {
        -1
    };
    let count: usize = e.marks.shape_flow.map_or(0, |h| h.count);
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
        place_shape_float(
            e,
            inherited,
            opts,
            px_of,
            &mut left,
            &mut right,
            cb_w,
            vert_lr,
            line_left_bottom,
            wall,
            &mut bands,
            &mut floats,
            host_side,
            f,
        );
    }
    let shapes = std::sync::Arc::new((left, right));
    // В вертикальном письме ширина контейнера — блок-прогресс контента
    // (число колонок): полная ширина растягивала бы его на страницу.
    let mut host = shape_host_div(e, inherited, vert_lr, &bands);
    for f in floats {
        host = host.child(f);
    }
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
