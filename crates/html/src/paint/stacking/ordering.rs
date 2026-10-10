//! Порядок слоёв окраски; отделён от определения контекстов наложения.

use crate::dom::{Element, Node};
use crate::layout::positioned::predicates::edge_set;
use crate::render::is_blank;
use crate::style::computed::Display;

/// Порядок наложения внутри одного родителя.
///
/// Отрицательный `z-index` кладёт элемент ПОД поток: отложенной отрисовкой это
/// не выражается — она всегда рисует поверх. Зато порядок детей мы задаём
/// сами: такие элементы уходят в начало списка и рисуются раньше.
/// `flex_ctx` — дети гибкого контейнера: у элемента ряда `z-index` кроме
/// auto создаёт контекст наложения и без `position` (css-flexbox-1 §4.3:
/// «z-index values other than auto create a stacking context even if
/// position is static»; `flex-item-z-ordering-001/002`).
pub(crate) fn by_layer(nodes: Vec<Node>, flex_ctx: bool) -> Vec<Node> {
    // Элемент на статической позиции переставлять НЕЛЬЗЯ: место в потоке и
    // есть его координата. `z-index` меняет только порядок отрисовки, а
    // перестановка меняла и раскладку — абсолютный блок с `z-index: -1`
    // уезжал к началу родителя и накрывал собой абзац над собой.
    // Двигать можно только ВНЕПОТОЧНЫЙ элемент: у релятивного слот в потоке и
    // есть его координата, перестановка меняла раскладку всего родителя
    // (красная полоса `overlapped-red` уезжала в начало страницы). Релятивный
    // с отрицательным `z-index` остаётся на месте и рисуется подложкой.
    // Переставлять можно только коробку, чьё положение задано краями ПО ОБЕИМ
    // осям: статической позиции у неё нет вовсе, и место в списке детей на
    // раскладку не влияет. С пустой осью место по ней держит нулевая распорка,
    // стоящая там, где элемент написан, — перестановка увозила бы её к началу
    // родителя.
    let movable = |e: &Element| {
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        // Элемент сетки с ЯВНЫМИ дорожками по обеим осям место в списке детей
        // тоже не держит: его позицию задаёт размещение, а не порядок
        // (css-grid-2 §8). Значит, его можно переставить ради порядка краски
        // (§9.9 шаг 3), как и внепоточную коробку с заданными краями.
        let placed = e.style.grid_col.is_some() && e.style.grid_row.is_some();
        e.style.z_index.is_some_and(|z| z < 0)
            && (placed
                || flex_ctx
                || (matches!(
                    e.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ) && x_set
                    && y_set))
    };
    // §9.9 шаг 8, обратная сторона того же правила: позиционированная коробка
    // без отрицательного `z-index` рисуется ПОВЕРХ содержимого потока. Порядок
    // краски у нас — порядок детей, поэтому написанный ПЕРВЫМ абсолют
    // закрашивался следующим за ним братом (проба: белый квадрат
    // `right-offset-003` пропадал под синим блоком, хотя стоял верно).
    // Условие то же, что у подслоя: края заданы по ОБЕИМ осям, значит места в
    // потоке коробка не держит и перестановка меняет только краску.
    // Пустая СТРОЧНАЯ ось у абсолюта среди одних блочных братьев места в списке
    // тоже не держит: статическая позиция по ней — начальный край содержимого
    // родителя (CSS 2.1 §10.3.7: «the left edge of the containing block to the
    // left margin edge of a hypothetical box»), для блочного уровня от места среди
    // соседей она не зависит (taffy `static_position.x = content_box_inset.left`,
    // `vendor/taffy/src/compute/block.rs:439`). `multicol-spanner-002`: абсолют
    // `top:80px` написан ДО многоколоночника, чья коробка по §column-span держит
    // нижнее поле спаннера (100, фон красный), и без перестановки фон ложился
    // поверх зелёного. Строчный контекст (текст рядом) и вертикальное письмо (там
    // x — блочная ось, позиция зависит от места) — мимо.
    let block_only = nodes.iter().all(|n| match n {
        Node::Element(k) => !k.inline,
        Node::Text(_) => is_blank(n),
    });
    let over = |e: &Element| {
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        let free_inline = !x_set
            && block_only
            && e.style.position == Some(crate::style::computed::Position::Absolute)
            && e.style.vertical != Some(true);
        (matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) && (x_set || free_inline)
            && y_set
            && !e.style.z_index.is_some_and(|z| z < 0))
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): элемент ряда с z-index > 0 —
        // поверх соседей той же перестановкой: css-flexbox 734 -> 733
        // (`flexbox-items-as-stacking-contexts-001` 0.44 -> 0.60), плюсов
        // ноль. Подслой для z < 0 (`movable` выше) оставлен: +1.
    };
    // Переставлять можно только ЧЕРЕЗ ПОТОК: порядок между позиционированными
    // соседями — это их порядок в разметке (§9.9 шаг 8 сохраняет его), и
    // прыжок через такого соседа менял бы наложение (`abspos-013`: красный
    // `fixed` с краями обязан лежать ПОД зелёным `fixed` без краёв).
    // Позиционированным считается и ПОДДЕРЕВО: краску поверх даёт не сам
    // сосед, а его потомок (`position-relative-table-tbody-top`: зелёный
    // `tbody` лежит внутри непозиционированной таблицы, и прыжок абсолюта
    // через неё открывал красный индикатор).
    fn positioned_inside(n: &Node, depth: usize) -> bool {
        match n {
            Node::Element(e) => {
                e.style.position.is_some()
                    || (depth < 8 && e.children.iter().any(|c| positioned_inside(c, depth + 1)))
            }
            Node::Text(_) => false,
        }
    }
    let mut after_positioned = vec![false; nodes.len()];
    let mut seen = false;
    for (i, n) in nodes.iter().enumerate().rev() {
        after_positioned[i] = seen;
        if positioned_inside(n, 0) {
            seen = true;
        }
    }
    // Внутри таблицы порядок детей — это её СТРОЕНИЕ (ряды, группы, ячейки), и
    // перестановка ломает саму решётку, а не краску.
    let table_here = nodes.iter().any(|n| match n {
        Node::Element(e) => {
            e.style.row_group_kind.is_some()
                || e.style.col_role.is_some()
                || e.style.is_caption == Some(true)
                || matches!(
                    e.style.display,
                    Some(Display::TableRow)
                        | Some(Display::TableCell)
                        | Some(Display::TableRowGroup)
                        | Some(Display::Table)
                )
                || matches!(
                    e.tag.as_str(),
                    "tr" | "td" | "th" | "tbody" | "thead" | "tfoot"
                )
        }
        Node::Text(_) => false,
    });
    let over_at = |i: usize, n: &Node| match n {
        Node::Element(e) => over(e) && !after_positioned[i] && !table_here,
        Node::Text(_) => false,
    };
    let touched = nodes.iter().enumerate().any(|(i, n)| {
        over_at(i, n)
            || match n {
                Node::Element(e) => movable(e),
                Node::Text(_) => false,
            }
    });
    if !touched {
        return nodes;
    }
    let keys: Vec<i32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| match n {
            Node::Element(e) if movable(e) => e.style.z_index.unwrap_or(0).min(0),
            _ if over_at(i, n) => 1,
            _ => 0,
        })
        .collect();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|i| keys[*i]);
    let mut taken: Vec<Option<Node>> = nodes.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| taken[i].take()).collect()
}
