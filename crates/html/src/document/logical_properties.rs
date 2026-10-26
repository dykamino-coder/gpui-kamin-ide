//! Проход документа после каскада: логические стороны → физические и всё, что
//! обязано идти следом. Письмо наследуется, поэтому перевод `margin-block`,
//! `inline-size` и вставок возможен только здесь, когда письмо узла окончательно
//! (после распространения с `<body>`); тем же проходом — слои `:hover`,
//! `::first-letter` и `::first-line`. Далее в строгом порядке: ориентация
//! картинок масок, статические позиции якорей (`anchor::settle_static`) и
//! offset-трансформ (`motion::settle`) — всем нужны уже физические стороны.

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
            let props = WritingProps::of(e.style.cascaded()).inherit(parent);
            let wm = props.wm();
            let parent_vertical = parent.wm().vertical();
            // Ячейка таблицы узнаётся и по тегу: `Display::TableCell` на этом шаге
            // приходит только из авторского CSS.
            let is_cell = matches!(e.tag.as_str(), "td" | "th")
                || e.style.cascaded().display == Some(crate::style::computed::Display::TableCell);
            e.style
                .cascaded_mut()
                .resolve_logical(wm, parent_vertical, is_cell, vcx);
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
                layer
                    .cascaded_mut()
                    .resolve_logical(wm, parent_vertical, is_cell, vcx);
            }
            // Физические поля здесь окончательны; схлопывание изменит их позже.
            // Ортогональный корень потока тоже сохраняет их: его строчный размер —
            // запасное доступное место минус собственные поля строчной оси
            // (css-writing-modes-4 §7.3), которые схлопывание может вынести наружу.
            if e.style.cascaded().clip_ref == Some(1) || wm.vertical() != parent_vertical {
                e.style.cascaded_mut().own_margin = Some(e.style.cascaded().margin);
            }
            walk(&mut e.children, props, vcx);
        }
    }
    walk(&mut nodes, WritingProps::default(), vcx);
    // Зум, каскадное `inherit` ненаследуемых свойств (`z-index`, `clip`,
    // `text-overflow`, свойства реестра) — стадия вычисленных значений
    // (`style::cascade::compute`): раскладка читает вычисленный стиль.
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

/// CSS Images 3 §5.1: image-orientation наследуется и для декоративных картинок.
/// Группировка масок читает стиль до слияния при отрисовке, поэтому разрешаем
/// наследование здесь, не создавая коробок для предков без собственного стиля.
fn settle_mask_orientation(nodes: &mut [Node], parent: Option<bool>) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let orientation = e.style.cascaded().image_orient_none.or(parent);
        if e.style.cascaded().mask_image.is_some() {
            e.style.cascaded_mut().image_orient_none = orientation;
        }
        settle_mask_orientation(&mut e.children, orientation);
    }
}

#[cfg(test)]
mod tests {
    //! Ориентация картинок масок сохраняется при подготовке документа.
    use crate::document::Document;
    use crate::dom::Node;

    #[test]
    fn mask_orientation_inherits_through_unwrapped_ancestors_and_resets() {
        let doc = Document::builder(
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
        )
        .build()
        .unwrap();
        fn collect(nodes: &[Node], flags: &mut Vec<Option<bool>>) {
            for node in nodes {
                if let Node::Element(e) = node {
                    if e.style.cascaded().mask_image.is_some() {
                        flags.push(e.style.cascaded().image_orient_none);
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
        let doc = Document::builder(
            r#"<body><div style="writing-mode: sideways-lr; direction: rtl">
            <p id="a" style="margin-inline-start: 15px; padding-block-start: 5px"></p>
            </div></body>"#,
            "",
        )
        .build()
        .unwrap();
        let p =
            crate::dom::test_support::find(doc.nodes(), &|e| e.attr("id") == Some("a")).expect("p");
        // sideways-lr + rtl: inline-start сверху, block-start слева (§6.4).
        assert_eq!(p.style.cascaded().margin.top, Some(Len::Px(15.0)));
        assert_eq!(p.style.cascaded().padding.left, Some(Len::Px(5.0)));
        assert_eq!(
            (p.style.cascaded().writing_mode, p.style.cascaded().rtl),
            (None, None)
        );
    }
}
