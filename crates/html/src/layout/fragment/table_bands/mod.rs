//! Таблица при укладке по фрагментаинерам: предикат табличной коробки (`table_box`), полосы
//! повтора `thead`/`tfoot` по разложенным секциям (`repeat_spec`, css-tables-3
//! §repeated-headers; Blink `table_layout_algorithm.cc`) и признак последовательной таблицы
//! (`serial`). Оценки таблицы нет: подписи, коробку, ряды и ячейки даёт только записанная
//! раскладка (`flow::blocks::table`).

use crate::dom::Element;
use crate::layout::fragment::flow::blocks::table::TableSections;
use crate::layout::fragment::types::Repeat;
use crate::style::computed::Display;
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
) -> Option<Repeat> {
    (head || foot).then(|| Repeat {
        head: head.then_some((0.0, 0.0)),
        foot: foot.then_some((0.0, 0.0)),
        geom: Default::default(),
        row_ends: Vec::new(),
        head_els: if head { head_els() } else { Vec::new() },
        foot_els: if foot { foot_els() } else { Vec::new() },
    })
}

/// Полосы повтора секций `sec`, решённые во фрагментаинере размера `per`, где
/// начинается таблица: секция повторяется, если у неё `break-inside: avoid*` и
/// блочный размер не больше четверти фрагментаинера (css-tables-3
/// §repeated-headers: «up to one quarter»; Blink `table_layout_algorithm.cc:1086-1121`
/// — `block_size > max_section_block_size` отказ, ровно четверть повторяется).
/// Шапка — `(верх секции − край, край + секция + зазор под ней)`, подвал —
/// `(верх секции − зазор, зазор + секция + край)`; край коробки есть только
/// при сросшихся рамках (`TableSections::edge_top`).
pub(crate) fn repeat_spec(sec: &TableSections, per: f32) -> Option<RepeatSpec> {
    let max = per / 4.0;
    let fits = |&(_, h, avoid): &(f32, f32, bool)| avoid && h > COORD_EPS && h <= max + COORD_EPS;
    let head = sec.head.filter(fits);
    let foot = sec.foot.filter(fits);
    let head_band = head.map(|(at, h, _)| (at - sec.edge_top, sec.edge_top + h + sec.spacing));
    let foot_band = foot.map(|(at, h, _)| (at - sec.spacing, sec.spacing + h + sec.edge_bottom));
    if head_band.is_none() && foot_band.is_none() {
        return None;
    }
    let geom = crate::layout::fragment::types::RepeatGeom {
        head: head_band.map_or(0.0, |h| h.1),
        foot: foot_band.map_or(0.0, |f| f.1),
        head_end: head_band.map_or(0.0, |(at, h)| at + h),
        foot_at: foot_band.map_or(f32::MAX, |(at, _)| at),
        foot_end: foot_band.map_or(f32::MAX, |(at, h)| at + h),
        box_top: sec.box_top,
        box_end: sec.box_end,
        head_need: head.map_or(0.0, |s| 4.0 * s.1),
        foot_need: foot.map_or(0.0, |s| 4.0 * s.1),
    };
    Some((head_band, foot_band, geom))
}

impl Repeat {
    /// Полосы по разложенным секциям `sec` во фрагментаинере размера `per`
    /// (`repeat_spec`); `false` — повторяемой секции нет, повтора нет вовсе.
    pub(crate) fn refresh(&mut self, sec: &TableSections, per: f32) -> bool {
        let Some((head, foot, geom)) = repeat_spec(sec, per) else {
            return false;
        };
        self.head = head;
        self.foot = foot;
        self.geom = geom;
        self.row_ends = sec.row_ends.clone();
        true
    }
}

#[cfg(test)]
mod tests;
