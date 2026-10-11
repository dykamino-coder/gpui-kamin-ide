//! Полосы таблицы при фрагментации.
// owner: A

use crate::dom::Element;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::Display;
mod row_cuts;
mod row_shape;
use row_cuts::{RowRef, table_row_cuts};
mod shape_bands;
use shape_bands::table_shape_bands;

/// Табличная коробка — по тегу или по `display`.
pub(crate) fn table_box(c: &Element) -> bool {
    c.tag == "table"
        || matches!(
            c.style.display,
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
/// рядам и потому идёт сплошным блоком без внутренних точек. `rowspan`,
/// сросшиеся рамки, вертикальное письмо, монолит внутри при заданной высоте
/// и неизмеримая ячейка — `None`: таблица идёт цельным куском измеренной
/// высоты без точек, как прежде.
pub(super) fn table_shape(c: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    table_shape_bands(c, depth, cx, &mut TableBands::default())
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
type RepeatSpec = (
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
/// подвал `(верх секции − зазор, зазор + секция)`.
pub(crate) fn repeat_bands(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
) -> Option<RepeatSpec> {
    let per = fixed.filter(|_| rows.is_none() && table_box(c))?;
    let mut b = TableBands::default();
    table_shape_bands(c, 4, ShapeCx::COLUMNS, &mut b)?;
    let max = per / 4.0;
    let head = b
        .head
        .filter(|&(_, h)| b.head_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at, h + b.spacing));
    let foot = b
        .foot
        .filter(|&(_, h)| b.foot_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at - b.spacing, h + b.spacing));
    if head.is_none() && foot.is_none() {
        return None;
    }
    let geom = crate::layout::fragment::types::RepeatGeom {
        head: head.map_or(0.0, |h| h.1),
        foot: foot.map_or(0.0, |f| f.1),
        head_end: head.map_or(0.0, |(at, h)| at + h),
        foot_at: foot.map_or(f32::MAX, |(at, _)| at),
        foot_end: foot.map_or(f32::MAX, |(at, h)| at + h),
        box_top: b.box_top,
        box_end: b.box_end,
    };
    Some((head, foot, geom))
}

/// `RepeatGeom` для щупов укладки (`grow_pushed`, план `clone`): та же мера,
/// что у `StackChild` в сборке стопки.
pub(crate) fn repeat_leads(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
) -> crate::layout::fragment::types::RepeatGeom {
    repeat_bands(c, fixed, rows).map_or_else(Default::default, |r| r.2)
}
