//! Прокручиваемые коробки.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::positioned::predicates::{edge_set, stays_positioned};
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::inside as inside_deferred;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::{RenderOpts, blocks, element, in_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// Обернуть элемент лентой прокрутки, если `overflow` её просит.
///
/// `auto` и `scroll` в CSS означают именно ленту; обрезка без прокрутки —
/// это `hidden`, и подменять одно другим значило терять содержимое.
/// Вынуть из поддерева ленты прокрутки абсолютных потомков, которым лента не
/// содержащий блок.
///
/// §11.1.1: предок обрезает ТОЛЬКО того потомка, для которого он содержащий
/// блок. У абсолютного элемента без позиционированного предка содержащий блок
/// — область просмотра (§10.1 п.4), и `overflow: scroll|auto` его не касается.
///
/// Лента строит поддерево в замыкании, которое зовёт
/// `ScrollArea::request_layout` — уже после `icb_close()`, и `icb_push` вернул
/// бы элемент назад «рисовать на месте». Поэтому кандидатов вынимаем ЗДЕСЬ.
///
/// Условия — те же, что у `to_icb`, плюс два ужесточения: только
/// НЕПОСРЕДСТВЕННЫЕ дети ленты и только при ОБЕИХ заданных осях (по свободной
/// оси место сообщает щуп, а он остался бы в замыкании).
fn hoist_from_scroll(e: &mut Element, inherited: &Computed, opts: &RenderOpts) {
    use crate::style::computed::Position;
    if !crate::layout::positioned::containing_block::icb_active() || inside_deferred() {
        return;
    }
    let merged = crate::style::cascade::inherit::inherit(inherited, &e.style);
    // Лента внутри позиционированного предка не выносит ничего: у её потомков
    // содержащий блок есть.
    if merged.cb_ancestor || crate::text::inline::establishes_cb(&merged) {
        return;
    }
    if matches!(
        merged.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
    ) {
        return;
    }
    let take: Vec<bool> = (0..e.children.len())
        .map(|i| {
            let Node::Element(c) = &e.children[i] else {
                return false;
            };
            let x_set = edge_set(c.style.inset.left) || edge_set(c.style.inset.right);
            let y_set = edge_set(c.style.inset.top) || edge_set(c.style.inset.bottom);
            c.style.position == Some(Position::Absolute)
                && c.style.z_index.unwrap_or(0) >= 0
                && x_set
                && y_set
                && !stays_positioned(&e.children[i + 1..])
        })
        .collect();
    if !take.iter().any(|t| *t) {
        return;
    }
    let mut keep = Vec::with_capacity(e.children.len());
    for (i, child) in std::mem::take(&mut e.children).into_iter().enumerate() {
        if !take[i] {
            keep.push(child);
            continue;
        }
        // `blocks` на одном узле повторяет ВЕСЬ путь `to_icb`: строит элемент
        // и отдаёт открытому слою ICB. При обеих заданных осях щуп не нужен,
        // поэтому список возвращается пустым.
        let left = blocks(std::slice::from_ref(&child), &merged, opts);
        if left.is_empty() {
            continue;
        }
        // Условия предиката разошлись с `to_icb`: узел остаётся на месте, а не
        // теряется.
        drop(left);
        keep.push(child);
    }
    e.children = keep;
}

pub(crate) fn scrollable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    use crate::style::computed::Overflow;
    let horizontal = e.style.overflow_x == Some(Overflow::Scroll);
    let vertical = e.style.overflow_y == Some(Overflow::Scroll);
    if !horizontal && !vertical {
        return None;
    }
    let mut node = e.clone();
    // Слой ICB закрывается раньше, чем `ScrollArea` позовёт `build`, — поэтому
    // выносим кандидатов сейчас, из ещё не отданного в замыкание клона.
    hoist_from_scroll(&mut node, inherited, opts);
    let node = node;
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(
        move |handle: &gpui::ScrollHandle, h: bool, v: bool| -> AnyElement {
            let _depth = DepthScope::enter(depth);
            // Внутренний узел рисуется без прокрутки: ею занимается лента.
            let mut inner = node.clone();
            inner.style.overflow_x = None;
            inner.style.overflow_y = None;
            inner.style.scroller = true;
            // Доли высоты коробки — от её СОДЕРЖАЩЕГО блока (CSS 2.1 §10.5,
            // §10.7: «calculated with respect to the height of the generated
            // box's containing block»), а не от ленты: лента `d` своей высоты
            // не имеет, и под ней `height`/`min-height`/`max-height` в долях
            // решались как `auto`/`0`/`none`. Blink обёртки не заводит: доля
            // решается от `PercentageResolutionBlockSize` родителя
            // (`length_utils.cc:200-213`), и сквозь анонимную прослойку база
            // тоже идёт родительская (`CalculateChildPercentageSize`,
            // `length_utils.cc:1814-1818`). Прежде это прятало сжатие ленты
            // (`flex_shrink` 1 у обёртки по умолчанию); после P4 (сжатие узла на
            // обёртке) лента в потоке не жмётся, и эталон
            // `fieldset-as-item-overflow-ref` (`max-height: 100%` под
            // `height: 100px`) вылезал вниз на 100 px. Развязка та же, что у
            // `pct_height_to_px`: только поточный ребёнок блочного родителя с
            // высотой в точках (у гибкого и сеточного долю решает раскладка, у
            // абсолюта содержащий блок другой); `border-box` родителя — мимо,
            // его `height` не высота содержимого.
            if in_flow(&node.style)
                && matches!(inherited.display, None | Some(Display::Block))
                && inherited.border_box != Some(true)
                && let Some(Len::Px(ph)) = inherited.height
                && ph > 0.0
            {
                let of = |l: Option<Len>| match l {
                    Some(Len::Pct(k)) => Some(Len::Px(k * ph)),
                    other => other,
                };
                inner.style.height = of(inner.style.height);
                inner.style.min_height = of(inner.style.min_height);
                inner.style.max_height = of(inner.style.max_height);
            }
            // Наружный отступ принадлежит коробке, а не видимой области:
            // оставленный внутри, он увеличивал ленту на свою величину, и
            // содержимое было видно ниже края панели.
            let outer_margin = inner.style.margin;
            inner.style.margin = Default::default();
            let (built, native_box) = crate::interactive::scroll_target::build(node.node_id, handle, h, v, outer_margin,
                || element(&inner, &inherited, &opts));
            if native_box { return built; }
            use gpui::{InteractiveElement, StatefulInteractiveElement};
            let mut d = crate::style::apply::margins(div(), &outer_margin)
                .id(gpui::ElementId::Integer(node.node_id + 1))
                .track_scroll(handle)
                .child(built);
            // Элемент потока родителя — ЭТА обёртка, а не внутренний узел: ей и
            // сжатие, которое блоку в потоке выключено (`flex_shrink: 0` из
            // `blocks()`), а во flex-контексте — 1. Без него автоминимум
            // прокрутки (0) давал ленте ужаться под первым же потолком
            // родителя до нуля (`line-clamp-007`: `overflow:auto`-ребёнок
            // клэмп-контейнера пропадал целиком).
            if let Some(shrink) = node.style.flex_shrink {
                d.style().flex_shrink = Some(shrink);
            }
            if h {
                d = d.overflow_x_scroll();
            }
            if v {
                d = d.overflow_y_scroll();
            }
            // Лента — видимая область КОРОБКИ (CSS 2.1 §11.1.1: «content is
            // clipped»), а без своего размера она растягивалась на всю ширину
            // родителя: `inner` с `width: 200px` переполнение уже не режет
            // (`overflow` с него снят выше), и глиф за краем коробки был виден
            // целиком (`min-height-104/106`: красный третий «X» Ahem на
            // 200…300 px). Ширина ленты — рамочная ширина коробки; считается
            // только из точек, иначе лента остаётся прежней.
            // Размеры — из СЛИТОГО стиля: в собственном стиле узла `16ch` и
            // `10em` ещё не решены в точки (их решает `inherit`), и лента без
            // ширины не резала ничего (`white-space-pre-wrap-trailing-spaces-
            // 021`: висящие пробелы `overflow: auto`-коробки шириной в `ch`
            // выходили за её край).
            let sized = crate::style::cascade::inherit::inherit(&inherited, &inner.style);
            if h && let Some(Len::Px(w)) = sized.width {
                let side = |l: Option<Len>| match l {
                    None | Some(Len::Auto) => Some(0.0),
                    Some(Len::Px(v)) => Some(v),
                    _ => None,
                };
                let b = sized.borders();
                let edges = side(sized.padding.left)
                    .zip(side(sized.padding.right))
                    .zip(side(b.left).zip(side(b.right)))
                    .map(|((pl, pr), (bl, br))| pl + pr + bl + br);
                let lane = if inner.style.border_box == Some(true) {
                    Some(w)
                } else {
                    edges.map(|e| w + e)
                };
                if let Some(lw) = lane {
                    d = d.w(px(lw));
                    // Сжатие ленты решает РОДИТЕЛЬ. В блочном потоке `blocks()` уже
                    // записал `flex_shrink: 0`, и он перенесён на ленту выше. У
                    // элемента гибкого ряда действует свой `flex-shrink`
                    // (начальное 1, css-flexbox-1 Overview.bs:2586-2588; автоминимум
                    // ленты прокрутки — ноль, :1301): `width: 200px; min-width: 0`
                    // в контейнере 100 обязан ужаться до 100. Безусловный ноль
                    // выпускал ленту за контейнер (`grid-baseline-003-ref`,
                    // 0.00 → 0.72).
                    let flex_item = matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    );
                    if node.style.flex_shrink.is_none() && !flex_item {
                        d = d.flex_shrink_0();
                    }
                }
            }
            d.into_any_element()
        },
    );
    Some(
        crate::interactive::scroll_area::ScrollArea::new(
            gpui::ElementId::Integer(e.node_id),
            horizontal,
            vertical,
            build,
        )
        .into_any_element(),
    )
}

/// Обернуть элемент ручкой изменения размера, если `resize` разрешает.
pub(crate) fn resizable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if e.style.pointer_events_none == Some(true) {
        return None;
    }
    let (horizontal, vertical) = e.style.resize?;
    let axis = match (horizontal, vertical) {
        (true, true) => crate::interactive::resizable::ResizeAxis::Both,
        (true, false) => crate::interactive::resizable::ResizeAxis::Horizontal,
        _ => crate::interactive::resizable::ResizeAxis::Vertical,
    };
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |w: Option<f32>, h: Option<f32>| {
        let _depth = DepthScope::enter(depth);
        // Заданный мышью размер побеждает разметку — как и в браузере, где
        // он пишется в инлайн-стиль элемента.
        let mut mixed = node.clone();
        mixed.style.resize = None;
        if let Some(w) = w {
            mixed.style.width = Some(Len::Px(w));
        }
        if let Some(h) = h {
            mixed.style.height = Some(Len::Px(h));
        }
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::interactive::resizable::Resizable::new(gpui::ElementId::Integer(e.node_id), axis, build)
            .into_any_element(),
    )
}
