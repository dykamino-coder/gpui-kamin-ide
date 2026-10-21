//! Проход документа после каскада: логические стороны → физические и всё, что
//! обязано идти следом. Письмо наследуется, поэтому перевод `margin-block`,
//! `inline-size` и вставок возможен только здесь, когда письмо узла окончательно
//! (после распространения с `<body>`); тем же проходом — слои `:hover`,
//! `::first-letter` и `::first-line`. Далее в строгом порядке:
//! `inherited_value::settle`, `zoom::resolve`, явное `inherit` у `z-index`/`clip`,
//! статические позиции якорей (`anchor::settle_static`) и offset-трансформ
//! (`motion::settle`) — всем нужны уже физические стороны.

use crate::document::state::ValueCx;
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
pub(super) fn resolve_logical(mut nodes: Vec<Node>, vcx: ValueCx<'_>) -> Vec<Node> {
    use crate::style::computed::writing_mode::WritingProps;
    fn walk(nodes: &mut [Node], parent: WritingProps, vcx: ValueCx<'_>) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            // `writing-mode` и `direction` наследуются (css-writing-modes-4 §3.1, §2.1):
            // логические стороны коробки раскладываются по унаследованному значению, а в
            // собственном стиле его нет (слияние с родителем делает сборщик дерева).
            let props = WritingProps::of(&e.style).inherit(parent);
            let wm = props.wm();
            let parent_vertical = parent.wm().vertical();
            // Ячейка таблицы узнаётся и по тегу: `Display::TableCell` на этом шаге
            // приходит только из авторского CSS.
            let is_cell = matches!(e.tag.as_str(), "td" | "th")
                || e.style.display == Some(crate::style::computed::Display::TableCell);
            e.style.resolve_logical(wm, parent_vertical, is_cell, vcx);
            // Слои `:hover`, `::first-letter` и `::first-line` — копии стиля элемента,
            // снятые до этого прохода, поэтому логические стороны ложатся на их
            // физические поля тем же письмом, что у элемента
            // (`::first-letter { margin-block-start }` доезжает до буквицы).
            for layer in [
                e.hover.as_mut(),
                e.first_letter.as_mut(),
                e.first_line.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                layer.resolve_logical(wm, parent_vertical, is_cell, vcx);
            }
            // Physical margins are final here; collapsing rewrites them later.
            // An orthogonal flow root keeps them too: its inline size is the
            // fallback available space minus its own inline-axis margins
            // (css-writing-modes-4 §7.3), which collapsing may hoist away.
            if e.style.clip_ref == Some(1) || wm.vertical() != parent_vertical {
                e.style.own_margin = Some(e.style.margin);
            }
            walk(&mut e.children, props, vcx);
        }
    }
    walk(&mut nodes, WritingProps::default(), vcx);
    // `zoom` (css-viewport-1): длины под зумом домножаются ЗДЕСЬ, отдельным
    // проходом по собственным стилям — до слияния и до раскладки, а не в
    // `style::cascade::inherit` (см. заметку перед ней). Идёт после `walk`: логические
    // стороны уже физические. Страницу без `zoom` проход не меняет: у неё ни
    // одного элемента с `zoom`, и ни одна ветка записи не исполняется.
    crate::style::cascade::inherited_value::settle(&mut nodes, vcx);
    crate::style::zoom::resolve(&mut nodes);
    // `z-index: inherit` и `clip: inherit` разбор выражает только разрядом
    // `inherit_bits`, а значение родителя кладёт слияние (`style::cascade::inherit`) —
    // в СЛИТЫЙ стиль. Сборщик же дерева решает слой и обрезку по
    // СОБСТВЕННОМУ (`defers(&e.style, …)`, `below`, `grouped(…, &e.style)`),
    // и там оставалось прежнее объявление: `z-index: -1; z-index: inherit`
    // клал зелёный под поток (`z-index-014`), `clip: inherit` не резал
    // ничего (`clip-102`). CSS 2.1 §6.2.1: «the property takes the same
    // computed value as the property for the element's parent».
    settle_explicit_inherit(&mut nodes, None);
    settle_mask_orientation(&mut nodes, None);
    // Якорные вставки, которым не разрешиться никогда (нет имени и якоря
    // по умолчанию; имя, которого в документе нет), сводятся к запасному
    // значению или к `auto` ЗДЕСЬ — сборщик дерева выбирает статическую
    // позицию по `edge_set`, а логические вставки к этому шагу уже легли на
    // физические стороны (`Computed::resolve_logical` выше).
    crate::layout::positioned::anchor::settle::settle_static(&mut nodes, vcx);
    // motion-1: offset-трансформ — вторым проходом по СОБРАННОМУ дереву, где у
    // каждой коробки есть родитель. Идёт после `zoom::resolve` (длины уже
    // домножены) и после `resolve_logical` (стороны уже физические).
    crate::animation::motion::settle(&mut nodes, vcx);
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

/// CSS Images 3 §5.1: image-orientation inherits for decorative images too.
/// Mask grouping consumes the element style before the render-time merge, so
/// resolve this inherited input here, without giving unstyled ancestors a box.
fn settle_mask_orientation(nodes: &mut [Node], parent: Option<bool>) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let orientation = e.style.image_orient_none.or(parent);
        if e.style.mask_image.is_some() {
            e.style.image_orient_none = orientation;
        }
        settle_mask_orientation(&mut e.children, orientation);
    }
}

#[cfg(test)]
mod tests {
    //! Image orientation survives document preparation for CSS masks.
    use crate::document::Document;
    use crate::dom::Node;

    #[test]
    fn mask_orientation_inherits_through_unwrapped_ancestors_and_resets() {
        let doc = Document::new(
            r#"<style>
        html { image-orientation: none }
        .mask { mask-image: url(unused.jpg) }
        .reset { image-orientation: from-image }
        </style><body style="margin:0">
        <div class="mask"></div>
        <section class="reset"><div class="mask"></div></section>
        <section><div class="mask" style="image-orientation:from-image"></div></section>
        </body>"#,
            "",
        );
        fn collect(nodes: &[Node], flags: &mut Vec<Option<bool>>) {
            for node in nodes {
                if let Node::Element(e) = node {
                    if e.style.mask_image.is_some() {
                        flags.push(e.style.image_orient_none);
                    }
                    collect(&e.children, flags);
                }
            }
        }
        let mut flags = Vec::new();
        collect(doc.nodes(), &mut flags);
        assert_eq!(flags, [Some(true), Some(false), Some(false)]);
    }

    #[test]
    fn logical_sides_map_by_the_inherited_mode_without_touching_own_styles() {
        use crate::style::values::value::Len;
        let doc = Document::new(
            r#"<body><div style="writing-mode: sideways-lr; direction: rtl">
            <p id="a" style="margin-inline-start: 15px; padding-block-start: 5px"></p>
            </div></body>"#,
            "",
        );
        fn find(nodes: &[Node]) -> Option<&crate::dom::Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.attrs.iter().any(|(k, v)| k == "id" && v == "a") => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let p = find(doc.nodes()).expect("p");
        // sideways-lr + rtl: inline-start is the top, block-start the left (§6.4).
        assert_eq!(p.style.margin.top, Some(Len::Px(15.0)));
        assert_eq!(p.style.padding.left, Some(Len::Px(5.0)));
        assert_eq!((p.style.writing_mode, p.style.rtl), (None, None));
    }
}
