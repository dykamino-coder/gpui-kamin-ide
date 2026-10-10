//! Пробег флоатов: сбор соседних флоатов, хвост до разделителя, зазор и распорка очищающего соседа.

use crate::dom::{Element, Node};
use crate::layout::block::struts::{margin_px, through_strut};
use crate::layout::float::clear::clears_side;
use crate::layout::float::initial_letter::px_margin_w;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::writing_mode::native_vertical;
use crate::render::is_blank;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(super) fn run_min_width(nodes: &[Node], i: usize, j: usize) -> f32 {
    nodes[i..j]
        .iter()
        .filter_map(|n| match n {
            Node::Element(f) if f.style.float.is_some_and(|s| s != 0) => {
                Some(px_margin_w(&f.style).unwrap_or(0.0))
            }
            _ => None,
        })
        .fold(f32::INFINITY, f32::min)
}

pub(super) fn strut_for_next(
    cb_top_open: bool,
    nodes: &mut [Node],
    out: &[Node],
    j: usize,
    separates: impl Fn(&Element) -> bool,
    rest: &mut Vec<Node>,
) -> bool {
    let laid_out = out
        .iter()
        .rev()
        .find(|n| !is_blank(n))
        .is_none_or(|n| match n {
            Node::Element(prev) => through_strut(prev).is_none(),
            Node::Text(_) => true,
        });
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): убрать подмену целиком. §9.5.2 хочет
    // `max(низ флоатов, своё место + поле)`, а распорка даёт
    // `низ флоатов + поле` и гасит поле — оно выпадает из схлопывания с
    // полем родителя и следующего брата. Но без распорки хуже: срез из 17
    // пар жилы зазора дал 0 зелёных и до, и после, а три пары просели —
    // `margin-collapse-122` 1.55 → 2.94, `-125` 1.54 → 3.82,
    // `-142` 2.47 → «красное видно». Значит распорка держит положение, и
    // чинить надо не её удаление, а канал «поле участвует в схлопывании,
    // не двигая коробку».
    // Распорка — ПРОТЕЗ §9.5.2, а не коробка разметки: она ничего не
    // красит, и накрывать её флоатом (`covered_flow_tail` ниже) нечего.
    // Зато наложение снимает с флоата плавающую природу, и очищающая
    // коробка, ради которой распорка и поставлена, теряет тот нижний
    // край флоата, от которого считает зазор. Флаг гасит наложение
    // ровно на этом случае (проба §2.2: `margin-collapse-clear-003`,
    // `-009` и `nested-clearance-new-formatting-context` — все три
    // потери держит распорка `0 × остаток поля`, прошедшая гейт
    // `covered_flow_tail`, потому что ширины у неё нет, а
    // `px_of2(None) = 0`).
    let mut clearance_strut = false;
    clearance_before_next(
        cb_top_open,
        nodes,
        out,
        j,
        separates,
        rest,
        laid_out,
        &mut clearance_strut,
    );
    clearance_strut
}

pub(super) fn split_rest(
    nodes: &[Node],
    j: &mut usize,
    separates: impl Fn(&Element) -> bool,
) -> (Vec<Node>, Vec<Node>) {
    let mut rest: Vec<Node> = vec![];
    let mut out_of_flow: Vec<Node> = vec![];
    while *j < nodes.len() {
        if let Node::Element(next) = &nodes[*j]
            && (separates(next) || next.style.float.is_some_and(|f| f != 0))
        {
            break;
        }
        if let Node::Element(next) = &nodes[*j]
            && matches!(
                next.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && !at_static_position(&next.style)
        {
            out_of_flow.push(nodes[*j].clone());
            *j += 1;
            continue;
        }
        rest.push(nodes[*j].clone());
        *j += 1;
    }
    (rest, out_of_flow)
}

pub(super) fn gather_floaters(
    parent: &Computed,
    nodes: &[Node],
    i: usize,
    side: i8,
    floaters: &mut Vec<Element>,
    sides: &mut Vec<i8>,
    j: &mut usize,
) {
    while *j < nodes.len() {
        if is_blank(&nodes[*j]) {
            *j += 1;
            continue;
        }
        let Node::Element(next) = &nodes[*j] else {
            break;
        };
        let Some(next_side) = next.style.float.filter(|f| *f != 0) else {
            break;
        };
        // `clear` у соседа обрывает ряд: он обязан начать свой. Так
        // написаны эталоны WPT — колонка из `float: right` + `clear: both`.
        if *j > i && clears_side(next.style.clear, side) {
            break;
        }
        let mut floater = next.clone();
        native_vertical::claim_float_inline_size(&mut floater.style, parent, &floater.children);
        floater.style.float = None;
        // ПРОБОВАЛИ И ОТКАТИЛИ: помечать плавающий кусок блочным
        // (`display: block` + `inline = false`), как велит CSS 2.1 §9.7.
        // Замер: css-text 1003 → 998, flexbox 318 → 319 — итог в минус.
        // Строчная природа картинки нужна ряду обтекания: как блок она
        // перестаёт участвовать в общей строке текста рядом с собой.
        // Плавающий блок не растягивается и не сжимается — он занимает
        // свою ширину, остальное достаётся соседям.
        floater.style.flex_shrink = Some(0.0);
        // Плавающий блок сжимается ДО СОДЕРЖИМОГО, но не шире доступного
        // места. Ключевым словом `fit-content` это писалось раньше, и
        // выходило дороже: слово заворачивает коробку в сетку, а дорожка
        // сетки не считает БОКОВЫЕ ПОЛЯ ребёнка — `margin: 1px` съедал два
        // пикселя ширины, текст переставал помещаться и рвался посреди
        // слова (`word-space-transform-010`, где эталон — 21 одинаковая
        // коробка). Поэтому коробке С ПОЛЯМИ ширина не задаётся вовсе, а
        // потолком служит родитель.
        //
        // Всем остальным остаётся `fit-content`: потолок в родителя не
        // равен ему по смыслу. В родителе НУЛЕВОЙ ширины он обнуляет
        // коробку, тогда как по CSS плавающая коробка не уже минимального
        // содержимого и просто вылезает наружу
        // (`white-space-intrinsic-size-001`).
        let side_margin = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)));
        if floater.style.width.is_none() {
            if side_margin(&floater.style.margin.left) || side_margin(&floater.style.margin.right) {
                floater.style.max_width = floater.style.max_width.or(Some(Len::Pct(1.0)));
            } else {
                floater.style.width = Some(Len::FitContent);
            }
        }
        floaters.push(floater);
        sides.push(next_side);
        *j += 1;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn clearance_before_next(
    cb_top_open: bool,
    nodes: &mut [Node],
    out: &[Node],
    j: usize,
    separates: impl Fn(&Element) -> bool,
    rest: &mut Vec<Node>,
    laid_out: bool,
    clearance_strut: &mut bool,
) {
    if let Some(Node::Element(next)) = nodes.get(j)
        && separates(next)
        && let Some(top) = margin_px(next.style.margin.top, &next.style).filter(|v| *v > 0.0)
    {
        // §9.5.2 считает клиренс от ГИПОТЕТИЧЕСКОЙ позиции — «where the
        // actual top border edge would have been if the element's 'clear'
        // property had been none». Если пробег флоатов стоит в САМОМ
        // НАЧАЛЕ блока с открытым верхним краем, такой позиции не
        // существует: верхнее поле очищающей коробки схлопнулось бы с
        // полем блока (§8.3.1, «top margin of an in-flow block element
        // collapses with its first in-flow block-level child's top margin
        // if the element has no top border, no top padding, and the child
        // has no clearance») и увезло бы флоат ВНИЗ ВМЕСТЕ С СОБОЙ — то
        // есть мимо флоата не прошло бы ни при каком поле.
        //
        // Blink зовёт это примыкающим флоатом и решает до раскладки:
        // `block_layout_algorithm.cc:156-168`
        // (`HasClearancePastAdjoiningFloats` — «floats that would
        // otherwise (if 'clear' were 'none') be pulled down by the BFC
        // block offset of the child… we know for sure that we get
        // clearance, even before layout»), `:1796-1800` (флоат становится
        // примыкающим ровно при неразрешённом `BfcBlockOffset()`) и
        // `:2355-2367` («the child's margins won't have any effect»;
        // позиция берётся из `ExclusionSpace::ClearanceOffset`, то есть =
        // низ пробега).
        //
        // Низ пробега у нас и так даёт ряд обтекания, поэтому весь ответ —
        // НЕ ставить распорку и погасить поле: коробка встанет ровно под
        // рядом, каким бы большим поле ни было
        // (`negative-clearance-after-adjoining-float`: поле 200 при
        // флоате 50 — коробка обязана стоять на 50, а не на 200).
        let adjoining = cb_top_open && out.iter().all(is_blank) && rest.iter().all(is_blank);
        if !adjoining && laid_out {
            rest.push(Node::Element(Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "div".into(),
                style: Computed {
                    display: Some(Display::Block),
                    height: Some(Len::Px(top)),
                    ..Computed::default()
                },
                hover: None,
                first_letter: None,
                first_line: None,
                children: vec![],
                attrs: vec![],
                inline: false,
            }));
            *clearance_strut = true;
        }
        if (adjoining || *clearance_strut)
            && let Some(Node::Element(next)) = nodes.get_mut(j)
        {
            next.style.margin.top = Some(Len::Px(0.0));
        }
    }
}
