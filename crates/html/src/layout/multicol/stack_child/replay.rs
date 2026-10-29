//! Допуск ребёнка стопки к краске одной главной копией, воспроизведённой в каждом куске
//! (`ColumnStack::replay`): в его поддереве нет краски, зависящей от фрагмента.

use crate::dom::{Element, Node};
use crate::style::computed::{Display, Position};

/// Поддерево рисуется записью главной копии: первое — допуск, второе — в нём есть
/// художники, читающие границы фрагмента, которые в записи режут себя по кускам сами
/// (`effect_fragment::pieces`): срез тени по внутренним краям (css-break-3 §5.4),
/// опорная коробка плоского преобразования во фрагменте (§5.5), фильтр. Остаются на
/// копиях: объёмные преобразования и перспектива (их плоскости решаются на отрисовке по
/// накопленной матрице), фильтры по ссылке и фона, линейки промежутков и вложенная
/// фрагментация (многоколоночник рисует свои куски сам); отложенная краска
/// позиционированных и контекстов наложения идёт вне записи; сросшиеся кромки таблицы
/// решают стыки по маске своего куска (`EdgePainter`). Таблица с раздельными рамками
/// рисуется одинаково в любом куске: ячейки — параллельные потоки одной раскладки.
/// Обход — итеративный.
pub(crate) fn replayable(root: &Element) -> (bool, bool) {
    let mut stack = vec![root];
    let mut effects = false;
    while let Some(e) = stack.pop() {
        let s = e.style.used();
        // Владение, окна и расход элементов flex и сетки переносит допуск узлов записи
        // (`element::admission::items`).
        let per_copy = table_part(e) && s.border_collapse == Some(true);
        // Позиционированные коробки и контексты наложения рисуются отложенно, в фазе
        // своего контекста (CSS 2.1 Appendix E, `PaintLast`), а не внутри записи копии.
        let deferred = !matches!(s.position, None | Some(Position::Static))
            || s.z_index.is_some()
            || s.opacity.is_some_and(|o| o < 1.0);
        let solid = s.transform.is_some_and(|t| t.has_3d)
            || s.perspective.is_some()
            || s.preserve_3d == Some(true);
        if per_copy
            || deferred
            || solid
            || s.filter_ref.is_some()
            || s.backdrop_filter_set
            || s.column_count.is_some()
            || s.column_width.is_some()
            || s.column_rule_width.is_some()
            || s.column_rule_styles.is_some()
            || s.row_rule_width.is_some()
            || s.row_rule_styles.is_some()
        {
            return (false, false);
        }
        effects |= !s.shadows.is_empty()
            || s.transform.is_some()
            || s.rotate_prop.is_some()
            || s.scale_prop.is_some()
            || s.filter.is_some();
        stack.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    (true, effects)
}

/// Часть таблицы (её сросшиеся кромки решают стыки по маске своего куска).
fn table_part(e: &Element) -> bool {
    matches!(
        e.style.used().display,
        Some(
            Display::Table
                | Display::InlineTable
                | Display::TableRowGroup
                | Display::TableRow
                | Display::TableCell
        )
    ) || matches!(
        e.tag.as_str(),
        "table" | "caption" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th"
    )
}

/// Часть таблицы без таблицы-родителя: стопка раскладывает её анонимной обёрткой
/// (CSS 2.1 §17.2.1), а не элементом ребёнка.
fn orphan_part(parent: &Element, e: &Element) -> bool {
    let table = |e: &Element| {
        matches!(
            e.style.used().display,
            Some(
                Display::Table | Display::InlineTable | Display::TableRowGroup | Display::TableRow
            )
        ) || matches!(e.tag.as_str(), "table" | "thead" | "tbody" | "tfoot" | "tr")
    };
    let whole = matches!(
        e.style.used().display,
        Some(Display::Table | Display::InlineTable)
    ) || e.tag == "table";
    table_part(e) && !whole && !table(parent)
}

/// Хост стопки, чьи дети могут рисоваться записью главной копии: в поддереве нет
/// частей таблицы, раскладываемых анонимной обёрткой.
pub(crate) fn table_free(root: &Element) -> bool {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        for c in e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            _ => None,
        }) {
            if orphan_part(e, c) {
                return false;
            }
            stack.push(c);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::replayable;

    fn first(html: &str) -> crate::dom::Element {
        let tree = crate::dom::parse(html, "");
        fn find(nodes: &[crate::dom::Node]) -> Option<&crate::dom::Element> {
            nodes.iter().find_map(|n| match n {
                crate::dom::Node::Element(e) if e.tag == "section" => Some(e),
                crate::dom::Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(&tree).unwrap().clone()
    }

    #[test]
    fn fragment_aware_painters_keep_their_copies() {
        let of = |html: &str| replayable(&first(html));
        assert_eq!(
            of("<section><p style='background:green'>a</p></section>"),
            (true, false)
        );
        assert_eq!(
            of("<section><p style='box-shadow:0 0 4px red'>a</p></section>"),
            (true, true)
        );
        assert_eq!(
            of("<section><div style='transform:rotate(1deg)'>a</div></section>"),
            (true, true)
        );
        assert_eq!(
            of("<section><div style='transform:rotateX(1deg)'>a</div></section>"),
            (false, false)
        );
        assert_eq!(
            of("<section><div style='columns:2'>a</div></section>"),
            (false, false)
        );
        assert_eq!(
            of("<section><div style='position:absolute'>a</div></section>"),
            (false, false)
        );
        assert_eq!(
            of("<section><div style='display:grid'>a</div></section>"),
            (true, false)
        );
    }
}
