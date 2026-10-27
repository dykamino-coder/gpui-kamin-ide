//! Таблица при укладке по фрагментаинерам: предикат табличной коробки (`table_box`), мера
//! таблицы без рядов — подписи, рамка и заданная высота (`table_shape`; ряды и ячейки даёт
//! только записанная раскладка, `flow::blocks::table`), полосы повтора `thead`/`tfoot` по
//! разложенным секциям (`repeat_spec`, css-tables-3 §repeated-headers; Blink
//! `table_layout_algorithm.cc`) и признак последовательной таблицы (`serial`).

use crate::document::state::ValueCx;

use crate::dom::Element;
use crate::layout::fragment::flow::blocks::table::TableSections;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::Display;
pub(crate) mod parts;
mod row_shape;
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

/// Мера таблицы без рядов для укладки по фрагментаинерам: подписи обёртки
/// (css-tables-3 §terminology), рамка и отбивка коробки таблицы, пол заданной
/// высоты (CSS 2.1 §17.5.3). Таблица с рядами здесь не мерится (`None`): её
/// ряды, охваты и ячейки даёт раскладка (`flow::blocks::table`).
pub(super) fn table_shape(c: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> Option<Shape> {
    table_shape_bands(c, depth, cx, vcx)
}

/// Повтор секций для стопки: полосы `flow::Repeat` и геометрия укладки.
pub(crate) type RepeatSpec = (
    Option<(f32, f32)>,
    Option<(f32, f32)>,
    crate::layout::fragment::types::RepeatGeom,
);

/// Повтор шапки/подвала таблицы-ребёнка стопки колонок (css-tables-3
/// §repeated-headers; Blink `table_layout_algorithm.cc:1082-1150`): только
/// `column-fill: auto` с заданной высотой и без рядов, когда размер
/// фрагментаинера известен до укладки. Ряды таблицы даёт раскладка копии
/// (`measured`: может ли по стилю повторяться шапка и подвал); полосы решаются
/// по её разложенным секциям (`StackChild::refresh_repeat`), а здесь — пустые
/// полосы под копии. Без записанной раскладки повтора нет.
pub(crate) fn repeat_bands(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    measured: Option<(bool, bool)>,
) -> Option<RepeatSpec> {
    fixed.filter(|_| rows.is_none() && table_box(c))?;
    let (head, foot) = measured?;
    (head || foot).then(|| {
        let band = |on: bool| on.then_some((0.0, 0.0));
        (band(head), band(foot), Default::default())
    })
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
