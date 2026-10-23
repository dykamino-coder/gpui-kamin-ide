//! ADR P-19 TRANSITIONAL(F2-5): оценочные полосы повторов сохраняются до продолжений секций и ячеек.
//! Полосы формы таблицы: группы, ряды, шапки и подвалы (table_shape_bands).

use super::TableBands;
use super::parts::{cell_settings, is_row, ordered_parts};
use super::row_cuts::RowRef;
use super::row_shape::row_band_shape;
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::flow::blocks::table::{row_avoid, row_force};
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::anon::fixup_table_children;
use crate::style::values::value::Len;

pub(super) fn table_shape_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    bands: &mut TableBands,
    vcx: ValueCx<'_>,
) -> Option<Shape> {
    // Повтор секций вызывает оценку напрямую, минуя fragment_size::measure.
    #[cfg(feature = "wpt-harness")]
    if let Some(probe) = cx
        .probe
        .or_else(|| vcx.lines.map(|lines| &lines.opts.frag.probe))
    {
        probe.estimate("table.estimate");
    }
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    if depth == 0
        || c.style.cascaded().wm().vertical()
        || c.style.cascaded().border_collapse == Some(true)
    {
        return None;
    }
    // Заданная высота таблицы БОЛЬШЕ не повод отказаться от меры: она просто
    // ПОЛ коробки рядов (CSS 2.1 §17.5.3 — используемая высота есть большая
    // из заданной и суммы рядов; css-tables-3 §terminology кладёт `height` на
    // table grid box, а §style-overrides отдаёт обёртке только `position`,
    // `float`, `margin`-* и края — `height` среди них НЕТ). Пока отказ стоял,
    // многоколоночник с такой таблицей не фрагментировался ВОВСЕ: снимок
    // `specified-block-size-002` — зелёное (10,67)..(134,566), то есть 403 css
    // высоты во всю ширину при эталоне (10,67)..(134,191); снимок
    // `specified-block-size-003` — колонки 2-4 пусты, 11750 точек красного
    // фона многоколоночника.
    // Проценты и прочие единицы, как и прежде, — отказ от меры целиком:
    // разрешать их некому, а недомер увёл бы разрез не туда.
    let spec_of = |l: &Option<Len>| match l {
        None => Some(None),
        Some(Len::Px(v)) => Some(Some(*v)),
        _ => None,
    };
    let spec_h = spec_of(&c.style.computed().height)?;
    let spec_min_h = spec_of(&c.style.computed().min_height)?;
    let b = c.style.computed().borders();
    let mt = px_of(&c.style.computed().margin.top).unwrap_or(0.0);
    let mb = px_of(&c.style.computed().margin.bottom).unwrap_or(0.0);
    let top = px_of(&c.style.computed().padding.top)? + px_of(&b.top)?;
    let bot = px_of(&c.style.computed().padding.bottom)? + px_of(&b.bottom)?;
    let (spacing, cell_cx) = cell_settings(c, cx)?;
    // Дети чинятся ТЕМ ЖЕ `fixup_table_children`, которым их чинит рисователь
    // `layout::table::table` (css-tables-3 §3 fixup): бесхозная ячейка,
    // блок или текст прямо в таблице получают анонимный ряд, не-ячейка внутри
    // ряда — анонимную ячейку, `display: contents` растворяется, а ряды внутри
    // групп чинит рекурсивный заход. Мера по СЫРОМУ дереву отдавала бы `None` на
    // любом ребёнке, которому нужна анонимная коробка; у колонок `None` — отказ
    // от укладки целиком, и многоколоночник не фрагментировался бы вовсе:
    // первая колонка переполнялась, остальные пустовали
    // (`table-border-000`, `table-cell-border-001`, `overflow-scroll-row`).
    // `fixed` объявлен ДО `parts`: `parts` держит ссылки внутрь него.
    let fixed = fixup_table_children(&c.children, vcx);
    // Подпись — НЕ ряд и не группа: она живёт в анонимной ОБЁРТКЕ таблицы, а
    // не в коробке рядов (css-tables-3 §terminology: table wrapper box —
    // «A block container box generated around table grid boxes to account for
    // any space occupied by each table-caption it owns»; table grid box —
    // «A block-level box containing the table-internal boxes, EXCLUDING its
    // captions»). `fixup_table_children` её не чинит (ветка `is_cap` кладёт
    // подпись как есть), и в разборе ниже она уходила бы в `_ => return None`:
    // таблица с подписью в колонках не фрагментировалась бы ВОВСЕ — первая
    // колонка переполнялась, остальные пустовали (`table-border-004`).
    // Сторона — с самой подписи, при пустоте — с таблицы (наследование
    // `caption-side`), ровно как в рисователе `table()`; порядок внутри
    // стороны — разметки (`sections-and-captions-mixed-order`: сверху 1 и 2,
    // снизу 14 и 15…20).
    let is_cap_kid =
        |e: &Element| e.tag == "caption" || e.style.computed().is_caption == Some(true);
    let mut caps_top: Vec<&Element> = Vec::new();
    let mut caps_bot: Vec<&Element> = Vec::new();
    // Части в порядке отрисовки: первая заголовочная группа — вперёд,
    // первая подвальная — назад, остальное как в разметке (`table()`).
    let parts = ordered_parts(&fixed, |e| {
        if is_cap_kid(e) {
            if e.style
                .cascaded()
                .caption_bottom
                .or(c.style.cascaded().caption_bottom)
                == Some(true)
            {
                caps_bot.push(e);
            } else {
                caps_top.push(e);
            }
            return true;
        }
        false
    })?;
    let mut rows: Vec<RowRef> = Vec::new();
    for (gi, (_, e)) in parts.iter().enumerate() {
        if is_row(e) {
            rows.push(RowRef {
                row: e,
                group: gi,
                avoid: false,
                fb: row_force(e, false, vcx),
                fa: row_force(e, true, vcx),
                ab: row_avoid(e, false, vcx),
                aa: row_avoid(e, true, vcx),
            });
            continue;
        }
        let inner: Vec<&Element> = e
            .children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                Node::Element(r) if is_row(r) => Some(r),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let last = inner.len().saturating_sub(1);
        for (i, r) in inner.iter().enumerate() {
            rows.push(RowRef {
                row: r,
                group: gi,
                avoid: e.style.computed().break_inside_avoid,
                fb: row_force(r, false, vcx) || (i == 0 && e.style.computed().break_before_force),
                fa: row_force(r, true, vcx) || (i == last && e.style.computed().break_after_force),
                // Край ГРУППЫ — тот же перенос, что у принудительных выше:
                // `break-before: avoid` группы действует на её ПЕРВОМ ряду,
                // `break-after: avoid` — на ПОСЛЕДНЕМ (css-break-4 §break-between,
                // «Applies to: … table row groups, table rows»).
                ab: row_avoid(r, false, vcx) || (i == 0 && e.style.computed().break_before_avoid),
                aa: row_avoid(r, true, vcx) || (i == last && e.style.computed().break_after_avoid),
            });
        }
    }
    // Таблица ИЗ ОДНИХ ПОДПИСЕЙ мерится: коробка рядов у неё пуста (только
    // рамка и отбивка), но обёртка несёт подписи и их точки разреза.
    // `table-border-004`: подпись 110 + пустая коробка `border-width:20px 0`
    // (20 + 20) + подпись 250 = 400 = ровно четыре колонки по 100.
    // Таблица ИЗ ОДНОЙ ЗАДАННОЙ ВЫСОТЫ мерится так же, как из одних подписей:
    // рядов нет, но коробка есть и её высоту знает стиль
    // (`specified-block-size-002`: пустая `display: table; height: 400px` =
    // ровно 4 колонки по 100; `table-border-007`: рамка 10 + 180 + 10 = 200 =
    // две колонки по 100 — проба `p-tb-007` = 0.00).
    if rows.is_empty()
        && caps_top.is_empty()
        && caps_bot.is_empty()
        && spec_h.is_none()
        && spec_min_h.is_none()
    {
        return None;
    }
    row_band_shape(
        c, depth, cx, bands, spec_h, spec_min_h, mt, mb, top, bot, spacing, cell_cx, caps_top,
        caps_bot, parts, rows, vcx,
    )
}
