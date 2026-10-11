//! Правки дерева после разбора: quirks-проценты, цвета правил, run-in, флоаты во флексе, выравнивание, руби.

mod alignment;
mod flow_items;
mod ruby_boxes;
pub(super) use crate::dom::fixup_tree::alignment::align_self_from_dom_parent;
pub(super) use crate::dom::fixup_tree::alignment::grid_table_items_keep_stretch;
pub(super) use crate::dom::fixup_tree::flow_items::flex_items_lose_float;
pub(super) use crate::dom::fixup_tree::flow_items::fold_run_ins;
pub(super) use crate::dom::fixup_tree::ruby_boxes::own_containing_block;
pub(super) use crate::dom::fixup_tree::ruby_boxes::ruby_box_role;
pub(super) use crate::dom::fixup_tree::ruby_boxes::wrap_misparented_ruby;

use crate::dom::*;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

/// Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме quirks
/// доля высоты элемента в потоке ищет опору через предков-блоков с
/// `height: auto` до ближайшего с заданной высотой. Сводится к точкам ЗДЕСЬ, в
/// стиле узла: флоаты и картинки строятся из сырого стиля (`wrap_floats`,
/// `image_with`), и пересчёт только в слитом (`inline::inherit`) до них не
/// доходил (`float-percentage-resolution-quirks-mode`,
/// `intrinsic-percent-replaced-003`). `base` — высота содержимого опоры для
/// детей; гибкий/сеточный/табличный предок с `auto` и абсолют цепочку рвут.
pub(super) fn quirks_percent_heights(nodes: &mut [Node], base: Option<f32>) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        let st = &mut e.style;
        let out_of_flow = matches!(
            st.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        );
        let blockish = matches!(
            st.display,
            None | Some(Display::Block) | Some(Display::InlineBlock) | Some(Display::ListItem)
        );
        if let (Some(Len::Pct(k)), Some(b), false, true) = (st.height, base, out_of_flow, blockish)
        {
            st.height = Some(Len::Px(k * b));
        }
        // Табличные коробки квирка не дают: доля внука ячейки с заданной
        // высотой остаётся `auto` (`percentages-grandchildren-quirks-mode-001`).
        let tabular = matches!(
            st.display,
            Some(
                Display::Table
                    | Display::InlineTable
                    | Display::TableCell
                    | Display::TableRow
                    | Display::TableRowGroup
            )
        );
        let child_base = match st.height {
            _ if tabular => None,
            Some(Len::Px(h)) => Some(h),
            None | Some(Len::Auto) if blockish && !out_of_flow && e.tag != "html" => base,
            _ => None,
        };
        quirks_percent_heights(&mut e.children, child_base);
    }
}

pub(super) type RuleColors = (
    Option<crate::style::values::value::Color>,
    Option<crate::style::computed::GapList<Option<crate::style::values::value::Color>>>,
    Option<crate::style::values::value::Color>,
    Option<crate::style::computed::GapList<Option<crate::style::values::value::Color>>>,
);

/// `column-rule-color: inherit` / `row-rule-color: inherit` — ненаследуемое
/// свойство берёт ВЫЧИСЛЕННОЕ значение ДОМ-родителя (css-cascade-4 §7.2).
/// Отрисовка линеек читает собственный стиль коробки, поэтому слово решается
/// здесь, в дереве, как `display: inherit` выше (`multicol-rule-color-inherit-001`:
/// родитель `column-rule-color: green` при `column-rule-style: none`, ребёнок
/// `inherit` — зелёные линейки, а не `currentcolor` красного текста).
pub(super) fn resolve_rule_color_inherit(nodes: &mut [Node], parent: &RuleColors) {
    use crate::style::computed::inh;
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let s = &mut el.style;
        if s.inherit_bits & inh::COLUMN_RULE_C != 0 {
            s.column_rule_color = parent.0;
            s.column_rule_colors = parent.1.clone();
            s.inherit_bits &= !inh::COLUMN_RULE_C;
        }
        if s.inherit_bits & inh::ROW_RULE_C != 0 {
            s.row_rule_color = parent.2;
            s.row_rule_colors = parent.3.clone();
            s.inherit_bits &= !inh::ROW_RULE_C;
        }
        let own: RuleColors = (
            s.column_rule_color,
            s.column_rule_colors.clone(),
            s.row_rule_color,
            s.row_rule_colors.clone(),
        );
        resolve_rule_color_inherit(&mut el.children, &own);
    }
}

/// `filter: url(#id)` дешёвым слоем рисуется только у коробки БЕЗ содержимого
/// (`interact::FilterLayer`): у коробки с детьми слой лёг бы поверх них.
/// Решается здесь, пока дерево целое — при сборке абсолютные дети уже
/// вынесены в свои слои, и родитель выглядит пустым
/// (`filter-region-transformed-composited-child-001`).
pub(super) fn filter_ref_only_empty(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if el.style.filter_ref.is_some()
            && el.children.iter().any(|c| match c {
                Node::Element(_) => true,
                Node::Text(t) => !t.trim().is_empty(),
            })
        {
            el.style.filter_ref = None;
        }
        filter_ref_only_empty(&mut el.children);
    }
}
