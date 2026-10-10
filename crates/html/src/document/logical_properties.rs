//! Logical properties for document; split out to keep the owning module within 250 lines.

use crate::dom::Node;

/// Разложить логические стороны и размеры по физическим — по всему дереву.
///
/// Какая сторона логического начала физическая, знает только письмо, а оно
/// НАСЛЕДУЕТСЯ. Пока перевод шёл при разборе, он молча считал письмо
/// горизонтальным: `margin-block` в вертикальном тексте разворачивал отступ
/// не по той оси, а `inline-size` задавал не ту сторону коробки.
///
/// Проход идёт после каскада и после распространения письма с `<body>` —
/// то есть в единственной точке, где письмо узла уже окончательно.
pub(super) fn resolve_logical(mut nodes: Vec<Node>) -> Vec<Node> {
    fn walk(nodes: &mut [Node], mode: (Option<bool>, Option<bool>, Option<bool>, Option<bool>)) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            // Письмо и направление наследуются; свои значения сильнее.
            // `sideways` — часть ЗНАЧЕНИЯ `writing-mode`, а оно наследуемое
            // (css-writing-modes-4 §3.1, `Inherited: yes`). Без него потомок
            // блока `sideways-lr`, у которого своего письма нет, разбирал
            // логические стороны по строке `vertical-lr` таблицы
            // §Abstract-Physical Mapping: `inline-start` уезжал с НИЖНЕГО
            // края на верхний (эталон `initial-letter-block-position-
            // margins-slr-ref` ставил `margin-inline-start: 15px` сверху).
            let own = (
                e.style.vertical.or(mode.0),
                e.style.vertical_rl.or(mode.1),
                e.style.rtl.or(mode.2),
                e.style.sideways.or(mode.3),
            );
            let (was_v, was_rl, was_rtl, was_sw) = (
                e.style.vertical,
                e.style.vertical_rl,
                e.style.rtl,
                e.style.sideways,
            );
            e.style.vertical = own.0;
            e.style.vertical_rl = own.1;
            e.style.rtl = own.2;
            e.style.sideways = own.3;
            // Табличность ячейки на этом шаге держится ТЕГОМ:
            // `Display::TableCell` приходит только из авторского CSS
            // (замеренный откат в шапке `resolve_logical`), поэтому
            // проверяются оба признака.
            let is_cell = matches!(e.tag.as_str(), "td" | "th")
                || e.style.display == Some(crate::style::computed::Display::TableCell);
            e.style.resolve_logical(mode.0, is_cell);
            // Слои `:hover`, `::first-letter` и `::first-line` — ТЕМ ЖЕ
            // проходом. Слой собирается копией стиля элемента ДО этого
            // прохода (`dom.rs:2473 layer()`), поэтому логические стороны
            // остаются у него в `Computed::logical` и на физические поля не
            // ложатся НИКОГДА. Из-за этого `::first-letter
            // { margin-block-start: 10px }` не доезжал до буквицы:
            // `render::initial_letter_float` читает у слоя `margin.top` и
            // родню, а там `None`.
            for layer in [
                e.hover.as_mut(),
                e.first_letter.as_mut(),
                e.first_line.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                let (lv, lrl, lrtl, lsw) =
                    (layer.vertical, layer.vertical_rl, layer.rtl, layer.sideways);
                layer.vertical = own.0;
                layer.vertical_rl = own.1;
                layer.rtl = own.2;
                layer.sideways = own.3;
                layer.resolve_logical(mode.0, is_cell);
                layer.vertical = lv;
                layer.vertical_rl = lrl;
                layer.rtl = lrtl;
                layer.sideways = lsw;
            }
            // Унаследованное обратно снимается: наследованием занимается
            // сборщик дерева, и оставленное здесь значение завело бы узлу
            // собственную коробку (см. `has_box_style`).
            e.style.vertical = was_v;
            e.style.vertical_rl = was_rl;
            e.style.rtl = was_rtl;
            e.style.sideways = was_sw;
            walk(&mut e.children, own);
        }
    }
    walk(&mut nodes, (None, None, None, None));
    // `zoom` (css-viewport-1): длины под зумом домножаются ЗДЕСЬ, отдельным
    // проходом по собственным стилям — до слияния и до раскладки, а не в
    // `inline::inherit` (см. ★ перед ней). Идёт после `walk`: логические
    // стороны уже физические. Страницу без `zoom` проход не меняет: у неё ни
    // одного элемента с `zoom`, и ни одна ветка записи не исполняется.
    crate::style::zoom::resolve(&mut nodes);
    // `z-index: inherit` и `clip: inherit` разбор выражает только разрядом
    // `inherit_bits`, а значение родителя кладёт слияние (`inline::inherit`) —
    // в СЛИТЫЙ стиль. Сборщик же дерева решает слой и обрезку по
    // СОБСТВЕННОМУ (`defers(&e.style, …)`, `below`, `grouped(…, &e.style)`),
    // и там оставалось прежнее объявление: `z-index: -1; z-index: inherit`
    // клал зелёный под поток (`z-index-014`), `clip: inherit` не резал
    // ничего (`clip-102`). CSS 2.1 §6.2.1: «the property takes the same
    // computed value as the property for the element's parent».
    settle_explicit_inherit(&mut nodes, None);
    // Якорные вставки, которым не разрешиться никогда (нет имени и якоря
    // по умолчанию; имя, которого в документе нет), сводятся к запасному
    // значению или к `auto` ЗДЕСЬ — сборщик дерева выбирает статическую
    // позицию по `edge_set`, а логические вставки к этому шагу уже легли на
    // физические стороны (`Computed::resolve_logical` выше).
    crate::layout::positioned::anchor::settle::settle_static(&mut nodes);
    // motion-1: offset-трансформ — вторым проходом по СОБРАННОМУ дереву, где у
    // каждой коробки есть родитель. Идёт после `zoom::resolve` (длины уже
    // домножены) и после `resolve_logical` (стороны уже физические).
    crate::animation::motion::settle(&mut nodes);
    nodes
}

/// Явное `inherit` у `z-index` и `clip` — в собственный стиль элемента.
///
/// Оба свойства ненаследуемые, поэтому собственный стиль родителя и есть его
/// вычисленное значение (как у `zoom::explicit`). Проход сверху вниз: у
/// родителя цепочка `inherit` к этому шагу уже разрешена.
pub(super) fn settle_explicit_inherit(
    nodes: &mut [Node],
    parent: Option<&crate::style::computed::Computed>,
) {
    use crate::style::computed::inh;
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        if let Some(p) = parent {
            if e.style.inherit_bits & inh::Z_INDEX != 0 {
                e.style.z_index = p.z_index;
            }
            if e.style.inherit_bits & inh::CLIP != 0 {
                e.style.clip_rect = p.clip_rect;
                e.style.clip_len = p.clip_len;
            }
            if e.style.grid_areas_inherit {
                e.style.grid_areas = p.grid_areas.clone();
            }
            if e.style.text_overflow_inherit {
                e.style.ellipsis = p.ellipsis;
                e.style.overflow_marker = p.overflow_marker.clone();
            }
        }
        settle_explicit_inherit(&mut e.children, Some(&e.style));
    }
}
