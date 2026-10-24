//! Мера таблицы для укладки по фрагментаинерам (`table_shape`): ряды стопкой с
//! `border-spacing`, точки класса A между рядами (css-break-4 §possible-breaks,
//! css-tables-3 §fragmentation), монолитные и принудительные разрывы рядов и групп, подписи
//! вокруг коробки рядов. Здесь же полосы повтора `thead`/`tfoot` во фрагментах
//! (`repeat_bands`, css-tables-3 §repeated-headers; Blink `table_layout_algorithm.cc`)
//! и предикат табличной коробки (`table_box`). Подмодули: геометрия рядов, точки разреза,
//! формы рядов и подписей, полосы секций, последовательная таблица (`serial`).

use crate::document::state::ValueCx;

use crate::dom::Element;
use crate::layout::fragment::flow::blocks::table::TableSections;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::Display;
pub(crate) mod parts;
mod row_geometry;
pub(crate) use row_geometry::row_geometry;
mod row_cuts;
mod row_shape;
use row_cuts::{RowRef, table_row_cuts};
mod shape_bands;
use shape_bands::table_shape_bands;
mod serial;
use crate::tolerance::COORD_EPS;
pub(crate) use serial::serial_table_parallel;

/// Табличная коробка — по тегу или по `display`.
pub(crate) fn table_box(c: &Element) -> bool {
    c.tag == "table"
        || matches!(
            c.style.used().display,
            Some(Display::Table) | Some(Display::InlineTable)
        )
}

/// Мера таблицы для укладки по фрагментаинерам (css-break-4
/// §possible-breaks, класс A: «table row group boxes, table row boxes»;
/// css-tables-3 §fragmentation). Ряды — стопка: высота ряда — наибольшая
/// из мер его ячеек (ячейка — обычная блочная мера с рамкой и отбивкой),
/// между рядами и вокруг них — `border-spacing`, снаружи — отступ и рамка
/// таблицы. `break-inside: avoid` ряда или группы — монолитный диапазон
/// (css-break-4 §breaking-rules, Rule 2), `break-before/after` ряда или
/// группы — принудительный разрыв на границе ряда. `thead` встаёт первым,
/// `tfoot` — последним, как в `table()`. Заданная высота — ПОЛ коробки рядов
/// (CSS 2.1 §17.5.3, css-tables-3 §terminology: `height` относится к table
/// grid box, обёртка лишь несёт подписи); растянутая коробка раздаёт остаток
/// рядам и потому идёт сплошным блоком без внутренних точек. Охват `rowspan`
/// ограничен группой; ячейка растит исходный ряд лишь при равной высоте
/// ряда и охвата. Неизмеримое распределение высоты охвата,
/// сросшиеся рамки, вертикальное письмо, монолит при раздаче surplus рядам
/// и неизмеримая ячейка — `None`: таблица идёт цельным куском измеренной
/// высоты без точек, как прежде.
pub(super) fn table_shape(c: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> Option<Shape> {
    table_shape_bands(c, depth, cx, &mut TableBands::default(), vcx)
}

/// Полосы первой шапки и первого подвала таблицы в координатах её меры:
/// `(верх секции, высота секции)` и `break-inside: avoid*` секции, плюс
/// вертикальный `border-spacing`. Нужны повтору секций во фрагментах
/// (`repeat_bands`).
#[derive(Default, Clone, Copy)]
struct TableBands {
    pub(crate) head: Option<(f32, f32)>,
    pub(crate) foot: Option<(f32, f32)>,
    pub(crate) head_avoid: bool,
    pub(crate) foot_avoid: bool,
    pub(crate) spacing: f32,
    /// Коробка рядов `[верх, низ)` — без подписей обёртки.
    pub(crate) box_top: f32,
    pub(crate) box_end: f32,
}

/// Повтор секций для стопки: полосы `flow::Repeat` и геометрия укладки.
pub(crate) type RepeatSpec = (
    Option<(f32, f32)>,
    Option<(f32, f32)>,
    crate::layout::fragment::types::RepeatGeom,
);

/// Повтор шапки/подвала таблицы-ребёнка стопки колонок (css-tables-3
/// §repeated-headers; Blink `table_layout_algorithm.cc:1082-1150`): секция
/// повторяется, если у неё `break-inside: avoid*` и блочный размер не больше
/// четверти фрагментаинера («block-size of the section is one quarter or less
/// than that of the fragmentainer»). Размер фрагментаинера Blink знает только
/// вне первого прохода балансировки (`HasKnownFragmentainerBlockSize`), поэтому
/// здесь — только `column-fill: auto` с заданной высотой и без рядов. Ответ —
/// полосы для `flow::Repeat`: шапка `(верх секции, секция + зазор под ней)`,
/// подвал `(верх секции − зазор, зазор + секция)`. У таблицы, чьи ряды даёт
/// раскладка копии (`measured`: может ли по стилю повторяться шапка и
/// подвал), полосы решаются по её разложенным секциям
/// (`StackChild::refresh_repeat`), а здесь — пустые полосы под копии.
pub(crate) fn repeat_bands(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    measured: Option<(bool, bool)>,
    cx: ShapeCx<'_>,
    vcx: ValueCx<'_>,
) -> Option<RepeatSpec> {
    let per = fixed.filter(|_| rows.is_none() && table_box(c))?;
    match measured {
        Some((head, foot)) => (head || foot).then(|| {
            let band = |on: bool| on.then_some((0.0, 0.0));
            (band(head), band(foot), Default::default())
        }),
        None => repeat_bands_in(c, per, cx, vcx),
    }
}

/// Known page and column fragmentainers share the repeat reservation algorithm.
pub(crate) fn repeat_bands_in(
    c: &Element,
    per: f32,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<RepeatSpec> {
    if !table_box(c) {
        return None;
    }
    let mut b = TableBands::default();
    table_shape_bands(c, 4, cx, &mut b, vcx)?;
    repeat_spec(
        TableSections {
            head: b.head.map(|(at, h)| (at, h, b.head_avoid)),
            foot: b.foot.map(|(at, h)| (at, h, b.foot_avoid)),
            spacing: b.spacing,
            box_top: b.box_top,
            box_end: b.box_end,
        },
        per,
    )
}

/// Повтор секций таблицы, чьи ряды даёт раскладка копии: полосы решаются
/// после неё (`repeat_spec` по разложенным секциям), а здесь — копии для
/// полос шапки и подвала, которые по стилю (`break-inside: avoid*`) могут
/// повторяться; без таких секций повтора нет.
pub(crate) fn repeat_candidates(
    head: bool,
    foot: bool,
    head_els: impl FnOnce() -> Vec<gpui::AnyElement>,
    foot_els: impl FnOnce() -> Vec<gpui::AnyElement>,
) -> Option<crate::layout::fragment::types::Repeat> {
    (head || foot).then(|| crate::layout::fragment::types::Repeat {
        head: head.then_some((0.0, 0.0)),
        foot: foot.then_some((0.0, 0.0)),
        geom: Default::default(),
        head_els: if head { head_els() } else { Vec::new() },
        foot_els: if foot { foot_els() } else { Vec::new() },
    })
}

/// Полосы повтора секций `sec` во фрагментаинере размера `per`: секция
/// повторяется, если у неё `break-inside: avoid*` и блочный размер не больше
/// четверти фрагментаинера (css-tables-3 §repeated-headers; Blink
/// `table_layout_algorithm.cc:1086-1121`). Шапка —
/// `(верх секции, секция + зазор под ней)`, подвал — `(верх секции − зазор,
/// зазор + секция)`.
pub(crate) fn repeat_spec(sec: TableSections, per: f32) -> Option<RepeatSpec> {
    let max = per / 4.0;
    let fits = |&(_, h, avoid): &(f32, f32, bool)| avoid && h > COORD_EPS && h <= max + COORD_EPS;
    let head = sec
        .head
        .filter(fits)
        .map(|(at, h, _)| (at, h + sec.spacing));
    let foot = sec
        .foot
        .filter(fits)
        .map(|(at, h, _)| (at - sec.spacing, h + sec.spacing));
    if head.is_none() && foot.is_none() {
        return None;
    }
    let geom = crate::layout::fragment::types::RepeatGeom {
        head: head.map_or(0.0, |h| h.1),
        foot: foot.map_or(0.0, |f| f.1),
        head_end: head.map_or(0.0, |(at, h)| at + h),
        foot_at: foot.map_or(f32::MAX, |(at, _)| at),
        foot_end: foot.map_or(f32::MAX, |(at, h)| at + h),
        box_top: sec.box_top,
        box_end: sec.box_end,
    };
    Some((head, foot, geom))
}

/// `RepeatGeom` для щупов укладки (`grow_pushed`, план `clone`): та же мера,
/// что у `StackChild` в сборке стопки.
pub(crate) fn repeat_leads(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    cx: ShapeCx<'_>,
    vcx: ValueCx<'_>,
) -> crate::layout::fragment::types::RepeatGeom {
    repeat_bands(c, fixed, rows, None, cx, vcx).map_or_else(Default::default, |r| r.2)
}

#[cfg(test)]
mod hint_tests;
