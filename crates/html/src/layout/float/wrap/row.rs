//! Ряд обтекания: флоаты по сторонам и колонка потока; хвост под накрытием флоатом.

use crate::dom::{Element, Node};
use crate::style::computed::{Align, Computed, Display, FlexDir};
use crate::style::values::value::Len;
use std::ops::ControlFlow;

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_beside_covered(
    nodes: &[Node],
    out: &mut Vec<Node>,
    i: &mut usize,
    side: i8,
    floaters: &mut Vec<Element>,
    j: usize,
    separates: impl Fn(&Element) -> bool,
    rest: &mut Vec<Node>,
    out_of_flow: &mut [Node],
    clearance_strut: bool,
    covered: Option<((f32, f32), (f32, f32))>,
) -> ControlFlow<()> {
    if side < 0
        && floaters.len() == 1
        && out_of_flow.is_empty()
        // Хвост из одной распорки клиренса наложением не выражается:
        // §9.5.2 держит зазор следующей коробки от НИЗА ФЛОАТА, а
        // конструкция ниже флоат из потока убирает.
        && !clearance_strut
        // Следом РАЗДЕЛИТЕЛЬ (`separates`): его верх §9.5.2 считает от
        // НИЗА ФЛОАТА, а наложение возвращает поток на низ соседа — без
        // поля распорки нет, и очищающий брат вставал на низ соседа.
        && !nodes
            .get(j)
            .is_some_and(|n| matches!(n, Node::Element(next) if separates(next)))
        && let Some(((_, fh), (_, th))) = covered
    {
        let mut lone = floaters.remove(0);
        // Ряда нет — сжатие плавающего куска, заданное подготовкой выше,
        // здесь не при чём.
        lone.style.flex_shrink = None;
        lone.style.margin.top = Some(Len::Px(-th));
        lone.style.margin.bottom = Some(Len::Px(th - fh));
        out.extend(std::mem::take(rest));
        out.push(Node::Element(lone));
        *i = j;
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}

pub(super) fn push_float_row(
    out: &mut Vec<Node>,
    floaters: Vec<Element>,
    sides: Vec<i8>,
    rest: Vec<Node>,
) {
    let хвост_с_полями = rest.iter().any(|n| match n {
        Node::Element(e) => [e.style.margin.left, e.style.margin.right]
            .iter()
            .any(|m| !matches!(m, None | Some(Len::Px(0.0)))),
        Node::Text(_) => false,
    });
    let mut column = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "div".into(),
        style: Computed {
            flex_grow: Some(1.0),
            // ЗАМЕРЕНО: CSS2 5336 → 5338 (+10/−8), CSS3 2418 → 2417
            // (+1/−2), итого +1. Приобретения — обтекание текстом
            // (`floats-rule3-outside-right-001` 1.84 → 0.00,
            // `floats-wrap-bfc-002/003-*-overflow` 5-8 → 0.00, четвёрка
            // `float-nowrap-*`). Потери — БФК со СВОИМИ полями рядом с
            // флоатом (`floats-wrap-bfc-with-margin-004/005/008/009`,
            // `floats-132`, `floats-rule7-outside-left-001`): им остаток
            // ряда достаётся без учёта их полей. Чинится каналом «поле
            // БФК входит в остаток», которого в ряду нет.
            // Колонка обтекания берёт ОСТАТОК ряда, а не своё содержимое:
            // при основе «по содержимому» её max-content складывался с
            // шириной флоата, ряд переносился, и `float: right` уезжал
            // ПОД текст к левому краю вместо правого края той же строки
            // (проба: `float:right` 60 точек и три слова в двухстах).
            flex_basis: (!хвост_с_полями).then_some(Len::Px(0.0)),
            flex_shrink: Some(1.0),
            min_width: (!хвост_с_полями).then_some(Len::Px(0.0)),
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: rest,
        attrs: vec![],
        inline: false,
    };
    column.style.display = Some(Display::Block);
    let mut row_children: Vec<Node> = vec![];
    let paired: Vec<(i8, Element)> = sides.iter().copied().zip(floaters).collect();
    row_children.extend(
        paired
            .iter()
            .filter(|(s, _)| *s < 0)
            .map(|(_, f)| Node::Element(f.clone())),
    );
    row_children.push(Node::Element(column));
    // Прижатые вправо идут справа налево в порядке разметки.
    //
    // Правило 9 §9.5.1: правый флоат — «as far to the right as possible».
    // Ряд переносит, и правый, не влезший рядом с левым (правило 3),
    // уезжает на свою строку — там флекс ставит его в НАЧАЛО строки
    // (`c414-flt-fit-005/006`: x = 0 вместо 5em). `margin-left: auto`
    // первому правому ряда возвращает прижим: авто-поле получает только
    // остаток ПОСЛЕ гибких длин (taffy `flexbox.rs:311` раньше `:358`),
    // поэтому на строке с колонкой `flex-grow: 1` оно нулевое и колонку не
    // сжимает. Флоат без своей ширины (`fit-content` — обёртка сеткой) и
    // с авторским левым полем не трогаем.
    let mut first_right = true;
    for (_, f) in paired.iter().rev().filter(|(s, _)| *s > 0) {
        let mut f = f.clone();
        if first_right
            && matches!(f.style.margin.left, None | Some(Len::Px(0.0)))
            && !matches!(f.style.width, None | Some(Len::FitContent))
        {
            f.style.margin.left = Some(Len::Auto);
        }
        first_right = false;
        row_children.push(Node::Element(f));
    }
    out.push(Node::Element(Element {
        list_item: None,
        node_id: 0,
        anim: None,
        // Метка для сборщика дерева: у ряда обтекания текст ещё режется
        // по нижнему краю плавающего блока (см. `float_flow`).
        tag: "kamin-float".into(),
        style: Computed {
            display: Some(Display::Flex),
            flex_dir: Some(FlexDir::Row),
            align_items: Some(Align::Start),
            // Плавающие блоки, которым не хватило ширины, уходят НИЖЕ
            // (CSS 2.1 §9.5.1): ряд обязан переносить.
            flex_wrap: Some(true),
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: row_children,
        attrs: vec![],
        inline: false,
    }));
}
