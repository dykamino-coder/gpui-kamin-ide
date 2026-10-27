//! TRANSITIONAL(F2-5): Полосы секций и повторов вычисляет рекурсивный estimator; удалить в P18 после продолжений секций и ячеек из P17.
//! Мера таблицы без рядов (table_shape_bands): подписи, рамка с отбивкой, заданная высота.

use super::parts::{cell_settings, is_row, ordered_parts};
use super::row_shape::row_band_shape;
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::dom::whitespace::is_blank;
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::anon::fixup_table_children;
use crate::style::values::value::Len;

pub(super) fn table_shape_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<Shape> {
    // Вход оценки таблицы виден отдельно от общей меры.
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
    let spec_h = spec_of(&c.style.used().height)?;
    let spec_min_h = spec_of(&BoxDimensions::for_element(c, &c.style.used()).minimum[1])?;
    let b = c.style.used().borders();
    let mt = px_of(&c.style.used().margin.top).unwrap_or(0.0);
    let mb = px_of(&c.style.used().margin.bottom).unwrap_or(0.0);
    let top = px_of(&c.style.used().padding.top)? + px_of(&b.top)?;
    let bot = px_of(&c.style.used().padding.bottom)? + px_of(&b.bottom)?;
    let (spacing, _) = cell_settings(c, cx)?;
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
    let is_cap_kid = |e: &Element| e.tag == "caption" || e.style.used().is_caption == Some(true);
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
    // Ряды и содержимое ячеек кладёт только раскладка таблицы: их точки и
    // высоты даёт записанная сетка (`flow::blocks::table`), а не оценка ячеек.
    // Здесь меряются лишь подписи и коробка без рядов.
    if parts
        .iter()
        .any(|(_, e)| is_row(e) || e.children.iter().any(|n| !is_blank(n)))
    {
        return None;
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
    if caps_top.is_empty() && caps_bot.is_empty() && spec_h.is_none() && spec_min_h.is_none() {
        return None;
    }
    row_band_shape(
        c, depth, cx, spec_h, spec_min_h, mt, mb, top, bot, spacing, caps_top, caps_bot, vcx,
    )
}

#[cfg(test)]
mod tests {
    //! The estimate measures only a table without rows; rows come from the laid-out table.

    use crate::dom::Node;
    use crate::layout::fragment::ShapeCx;

    fn table(nodes: &[Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "table" => Some(e),
            Node::Element(e) => table(&e.children),
            _ => None,
        })
    }

    fn shape(html: &str) -> Option<crate::layout::fragment::Shape> {
        let tree = crate::dom::parse(html, "");
        let t = table(&tree).expect("table");
        super::super::table_shape(t, 4, ShapeCx::COLUMNS, tree.values())
    }

    #[test]
    fn rows_and_cells_are_never_estimated() {
        for body in [
            "<tr><td><div style='height:50px'></div></td></tr>",
            "<tbody><tr><td rowspan=2>x</td></tr><tr></tr></tbody>",
            "<tr></tr>",
        ] {
            let html = format!("<table style='border-spacing:0'>{body}</table>");
            assert!(shape(&html).is_none(), "{body}");
        }
    }

    #[test]
    fn a_table_without_rows_keeps_its_box_and_captions() {
        let s = shape(
            "<table style='border-spacing:0;height:100px;border-top:10px solid;border-bottom:10px solid'></table>",
        )
        .expect("box only");
        assert_eq!(s.height, 100.0);
        assert_eq!(s.solid, vec![(0.0, 10.0), (90.0, 100.0)]);
        let bare = shape("<table style='height:4px'></table>").expect("box");
        let s = shape(
            "<table style='height:4px'><caption><div style='height:30px'></div></caption></table>",
        )
        .expect("caption only");
        // The wrapper stacks the caption box above the table box.
        assert!(
            s.height >= bare.height + 30.0,
            "{} {}",
            s.height,
            bare.height
        );
        assert!(s.cuts.iter().any(|&(need, _)| need >= 30.0));
    }
}
