//! Подготовка таблицы: border-spacing, ряды и число колонок, дети с анонимными объектами; предикаты
//! ячеек.

use crate::dom::{Element, Node};
use crate::layout::box_kind::inline_level_box;
use crate::layout::options::RenderOpts;
use crate::layout::replaced::limits::atom_base_font;
use crate::style::computed::Computed;
use crate::style::computed::FontCx;
use crate::style::computed::stacking::stacking_context;
use crate::style::values::value::Len;

pub(super) fn border_spacing_of(
    e: &Element,
    opts: &RenderOpts,
    inherited: &Computed,
) -> (f32, f32) {
    table_spacing(
        e,
        opts,
        inherited,
        e.style.used().inh.table_list.border_spacing,
    )
}

pub(super) fn table_spacing(
    e: &Element,
    opts: &RenderOpts,
    inherited: &Computed,
    border_spacing: Option<(Option<Len>, Option<Len>)>,
) -> (f32, f32) {
    let vcx = opts.cx.vcx();
    match (
        e.style.cascaded().inh.table_list.border_collapse,
        border_spacing,
    ) {
        (Some(true), _) => (0.0, 0.0),
        (None, _) if e.attr("rules").is_some() => (0.0, 0.0),
        // Заданный `border-spacing` перекрывает умолчание браузера в 2px.
        // Шрифтовые единицы разрешаются по кеглю САМОЙ таблицы: `1em` роняло
        // зазор в ноль, и вся подсемья Хикси с `border-spacing: 1em`
        // расходилась с эталоном ровно на зазор.
        (_, Some((x, y))) => {
            let em = atom_base_font(inherited, opts);
            let px_of = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_))) => {
                    l.to_px(None, FontCx::with_font_px(em), vcx).unwrap_or(0.0)
                }
                _ => 0.0,
            };
            (px_of(x), px_of(y))
        }
        // Начальное значение `border-spacing` — НОЛЬ: два пикселя — это
        // умолчание браузера для ТЕГА `<table>`, и оно приходит сюда
        // каскадом из своего стилевого листа. `div` с `display: table`
        // зазора не имеет.
        _ => (0.0, 0.0),
    }
}

pub(super) fn table_rows_fixed(fixed: Vec<Node>) -> Vec<Node> {
    {
        // Роль группы задаётся ТЕГОМ ИЛИ `display` (§17.5.3): `div` с
        // `table-header-group` встаёт первым так же, как `<thead>`.
        let kind_of = |g: &Element| -> Option<u8> {
            match g.tag.as_str() {
                "thead" => Some(0),
                "tbody" => Some(1),
                "tfoot" => Some(2),
                _ => g.style.used().reset_context.row_group_kind,
            }
        };
        let first_with = |k: u8| -> Option<u64> {
            fixed.iter().find_map(|n| match n {
                Node::Element(g) if kind_of(g) == Some(k) => Some(g.node_id),
                _ => None,
            })
        };
        // Заголовочной и подвальной становится только ПЕРВАЯ группа
        // своего рода; последующие — обычные группы рядов.
        let head = first_with(0);
        let foot = first_with(2);
        let key = |n: &Node| match n {
            Node::Element(g) if Some(g.node_id) == head => 0u8,
            Node::Element(g) if Some(g.node_id) == foot => 2,
            _ => 1,
        };
        let ordered = fixed.windows(2).all(|w| key(&w[0]) <= key(&w[1]));
        if ordered {
            fixed
        } else {
            let mut sorted = fixed;
            sorted.sort_by_key(key);
            sorted
        }
    }
}

/// Есть ли в поддереве ячейки содержимое, которое красится ПОЗЖЕ сросшихся
/// кромок стола: строчный уровень (атомы строки, заменяемые, инлайн-столы —
/// фаза переднего плана), флоаты, позиционированные и контексты наложения
/// (CSS 2.1 прил. E, шаги 5-8; Blink `box_fragment_painter.cc:952-957`
/// красит кромки в `kDescendantBlockBackgroundsOnly`, то есть сразу после
/// фонов поточных блочных потомков). Блочный поточный потомок без этих
/// признаков остаётся под кромками — его в расчёт не берём, спускаясь в его
/// детей. Глубина ограничена: обход идёт у каждой ячейки.
pub(super) fn cell_paints_over(nodes: &[Node], depth: u8) -> bool {
    depth > 0
        && nodes.iter().any(|n| match n {
            Node::Element(k) => {
                inline_level_box(k)
                    || k.style.used().reset_box_layout.float.unwrap_or(0) != 0
                    || matches!(
                        k.style.used().reset_positioned.position,
                        Some(crate::style::computed::Position::Relative)
                            | Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                            | Some(crate::style::computed::Position::Sticky)
                    )
                    || stacking_context(&k.style.used())
                    || cell_paints_over(&k.children, depth - 1)
            }
            Node::Text(_) => false,
        })
}
