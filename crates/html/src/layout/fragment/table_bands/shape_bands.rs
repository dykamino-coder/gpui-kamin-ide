//! Полосы формы таблицы: группы, ряды, шапки и подвалы (table_shape_bands).

use super::TableBands;
use super::row_cuts::{RowRef, row_avoid, row_force};
use super::row_shape::row_band_shape;
use crate::dom::{Element, Node};
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::anon::fixup_table_children;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn table_shape_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    bands: &mut TableBands,
) -> Option<Shape> {
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    if depth == 0 || c.style.vertical == Some(true) || c.style.border_collapse == Some(true) {
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
    let spec_h = spec_of(&c.style.height)?;
    let spec_min_h = spec_of(&c.style.min_height)?;
    let b = c.style.borders();
    let mt = px_of(&c.style.margin.top).unwrap_or(0.0);
    let mb = px_of(&c.style.margin.bottom).unwrap_or(0.0);
    let top = px_of(&c.style.padding.top)? + px_of(&b.top)?;
    let bot = px_of(&c.style.padding.bottom)? + px_of(&b.bottom)?;
    // Умолчание тега `<table>` (2px) приходит каскадом; у `display: table`
    // зазора нет. Презентационные `cellspacing`/`cellpadding` — как в
    // `table()`: ниже авторского, выше умолчания браузера
    // (`block-page-break-inside-avoid-11`: мера 198 при рисунке 192 —
    // ложный разрыв внутри `avoid`).
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..cx
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Дети чинятся ТЕМ ЖЕ `fixup_table_children`, которым их чинит рисователь
    // `table()` (render.rs:13551; css-tables-3 §3 fixup): бесхозная ячейка,
    // блок или текст прямо в таблице получают анонимный ряд, не-ячейка внутри
    // ряда — анонимную ячейку, `display: contents` растворяется, а ряды внутри
    // групп чинит рекурсивный заход. Прежде мера шла по СЫРОМУ дереву, и любой
    // ребёнок, которому нужна анонимная коробка, отдавал `None`; у колонок
    // `None` — отказ от укладки целиком, и многоколоночник не фрагментировался
    // вовсе: первая колонка переполнялась, остальные пустовали
    // (`table-border-000` 6.25 — зелёное до y=566 при коробке до y=191,
    // `table-cell-border-001` 2.08, `overflow-scroll-row`).
    // `fixed` объявлен ДО `parts`: `parts` держит ссылки внутрь него.
    let fixed = fixup_table_children(&c.children);
    // Подпись — НЕ ряд и не группа: она живёт в анонимной ОБЁРТКЕ таблицы, а
    // не в коробке рядов (css-tables-3 §terminology: table wrapper box —
    // «A block container box generated around table grid boxes to account for
    // any space occupied by each table-caption it owns»; table grid box —
    // «A block-level box containing the table-internal boxes, EXCLUDING its
    // captions»). `fixup_table_children` её не чинит (ветка `is_cap` →
    // `out.push(child.clone())`, render.rs:16726), и в разборе ниже она
    // уходила в `_ => return None`: таблица с подписью в колонках не
    // фрагментировалась ВОВСЕ — первая колонка переполнялась, остальные
    // пустовали. Снимок `table-border-004`: красное 41,67..134,191 — три
    // колонки из четырёх пусты (11750 точек фона многоколоночника).
    // Сторона — с самой подписи, при пустоте — с таблицы (наследование
    // `caption-side`), ровно как в рисователе `table()`; порядок внутри
    // стороны — разметки (`sections-and-captions-mixed-order`: сверху 1 и 2,
    // снизу 14 и 15…20).
    let is_cap_kid = |e: &Element| e.tag == "caption" || e.style.is_caption == Some(true);
    let mut caps_top: Vec<&Element> = Vec::new();
    let mut caps_bot: Vec<&Element> = Vec::new();
    // Части в порядке отрисовки: первая заголовочная группа — вперёд,
    // первая подвальная — назад, остальное как в разметке (`table()`).
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        if is_cap_kid(e) {
            if e.style.caption_bottom.or(c.style.caption_bottom) == Some(true) {
                caps_bot.push(e);
            } else {
                caps_top.push(e);
            }
            continue;
        }
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    let mut rows: Vec<RowRef> = Vec::new();
    for (gi, (_, e)) in parts.iter().enumerate() {
        if is_row(e) {
            rows.push(RowRef {
                row: e,
                group: gi,
                avoid: false,
                fb: row_force(e, false),
                fa: row_force(e, true),
                ab: row_avoid(e, false),
                aa: row_avoid(e, true),
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
                avoid: e.style.break_inside_avoid,
                fb: row_force(r, false) || (i == 0 && e.style.break_before_force),
                fa: row_force(r, true) || (i == last && e.style.break_after_force),
                // Край ГРУППЫ — тот же перенос, что у принудительных выше:
                // `break-before: avoid` группы действует на её ПЕРВОМ ряду,
                // `break-after: avoid` — на ПОСЛЕДНЕМ (css-break-4 §break-between,
                // «Applies to: … table row groups, table rows»).
                ab: row_avoid(r, false) || (i == 0 && e.style.break_before_avoid),
                aa: row_avoid(r, true) || (i == last && e.style.break_after_avoid),
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
        c, depth, cx, bands, px_of, spec_h, spec_min_h, mt, mb, top, bot, spacing, cell_cx,
        caps_top, caps_bot, parts, rows,
    )
}
