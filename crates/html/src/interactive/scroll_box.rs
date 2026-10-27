//! Обёртки интерактивности на этапе сборки элемента: `scrollable` заворачивает коробку с
//! `overflow: auto | scroll` (и `hidden` при scroll-snap) в `ScrollArea`, решая размеры ленты,
//! доли высоты от содержащего блока (CSS 2.1 §10.5), наружные поля и сжатие flex-элемента;
//! `resizable` заворачивает коробку с `resize` в `Resizable`, где размер мыши побеждает
//! разметку. На вход — DOM-элемент и унаследованный стиль, на выход — готовый `AnyElement`.
mod hoisting;
use hoisting::hoist_from_scroll;

use crate::dom::Element;
use crate::layout::box_kind::in_flow;
use crate::layout::build_contracts::Styling;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

/// Обернуть элемент лентой прокрутки, если `overflow` её просит.
///
/// `auto` и `scroll` в CSS означают именно ленту; обрезка без прокрутки —
/// это `hidden`, и подменять одно другим значило терять содержимое.
pub(crate) fn scrollable(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    if e.style.used().scroller || e.tag == "frameset" || crate::layout::box_kind::real_inline(e) {
        return None;
    }
    use crate::style::computed::Overflow;
    // Eligibility follows computed overflow; the initially visible opposite
    // axis becomes auto even when it has no declaration (Overflow 3 §3).
    if ![e.style.used().overflow_x, e.style.used().overflow_y]
        .into_iter()
        .any(|o| matches!(o, Some(Overflow::Scroll | Overflow::Hidden)))
    {
        return None;
    }
    let scroll_style = crate::style::cascade::materialize::box_style(inherited, &e.style, vcx);
    let snap = scroll_style.snap.axis != 0;
    let horizontal = scroll_style.overflow_x == Some(Overflow::Scroll)
        || snap && scroll_style.overflow_x == Some(Overflow::Hidden);
    let vertical = scroll_style.overflow_y == Some(Overflow::Scroll)
        || snap && scroll_style.overflow_y == Some(Overflow::Hidden);
    if !horizontal && !vertical {
        return None;
    }
    let scroll_projection = crate::interactive::scroll_extent::projection(
        opts.cx.scroll.frame.as_ref(),
        inherited.self_node,
        Some(&scroll_style),
        opts.cx.scroll.sticky.as_ref(),
    );
    // Лента и её содержимое — внутри проекции (`ShiftLink`).
    let projected_opts = scroll_projection
        .as_ref()
        .map(|p| opts.inner(|cx| cx.scroll.sticky = Some(p.link().clone())));
    let opts = projected_opts.as_deref().unwrap_or(opts);
    let area_sticky = opts.cx.scroll.sticky.clone();
    let parent_frame = opts.cx.scroll.frame.clone();
    let freeze_scrollbars = opts.cx.deferred.freeze_scrollbars;
    let mut node = e.clone();
    node.style.used_mut().overflow_x = scroll_style.overflow_x;
    node.style.used_mut().overflow_y = scroll_style.overflow_y;
    // Слой ICB закрывается раньше, чем `ScrollArea` позовёт `build`, — поэтому
    // выносим кандидатов сейчас, из ещё не отданного в замыкание клона.
    hoist_from_scroll(&mut node, inherited, opts);
    let (inherited, opts) = (inherited.clone(), opts.clone());
    let build = std::rc::Rc::new(
        move |handle: &gpui::ScrollHandle, h: bool, v: bool, frame| -> AnyElement {
            let vcx = opts.cx.vcx();
            // Keep CSS overflow on the native box; `scroller` only prevents
            // recursive adapters while its ScrollHandle is attached.
            let mut inner = node.clone();
            inner.style.used_mut().scroller = true;
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
            if in_flow(&node.style.used())
                && matches!(inherited.display, None | Some(Display::Block))
                && inherited.border_box != Some(true)
                && let Some(Len::Px(ph)) = inherited.height
                && ph > 0.0
            {
                let of = |l: Option<Len>| match l {
                    Some(Len::Pct(k)) => Some(Len::Px(k * ph)),
                    other => other,
                };
                inner.style.used_mut().height = of(inner.style.used().height);
                inner.style.used_mut().min_height = of(inner.style.used().min_height);
                inner.style.used_mut().max_height = of(inner.style.used().max_height);
            }
            // Наружный отступ принадлежит коробке, а не видимой области:
            // оставленный внутри, он увеличивал ленту на свою величину, и
            // содержимое было видно ниже края панели.
            let outer_margin = inner.style.used().margin;
            inner.style.used_mut().margin = Default::default();
            let target = crate::interactive::scroll_target::Target::new(
                node.node_id,
                handle,
                h,
                v,
                outer_margin,
            );
            let inner_opts = opts.inner(|cx| {
                cx.scroll.target = Some(target.clone());
                cx.scroll.frame = frame;
            });
            let build = inner_opts.cx.contracts.build;
            let built = build.element(&inner, &inherited, &inner_opts);
            if target.applied() {
                return built;
            }
            use gpui::{InteractiveElement, StatefulInteractiveElement};
            let margins = Styling::Margins(&outer_margin);
            let mut d = build
                .styled_div(margins, &opts.cx)
                .id(node.state_id("html-scroll-content", vcx))
                .track_scroll(handle)
                .child(built);
            // Элемент потока родителя — ЭТА обёртка, а не внутренний узел: ей и
            // сжатие, которое блоку в потоке выключено (`flex_shrink: 0` из
            // `blocks()`), а во flex-контексте — 1. Без него автоминимум
            // прокрутки (0) давал ленте ужаться под первым же потолком
            // родителя до нуля (`line-clamp-007`: `overflow:auto`-ребёнок
            // клэмп-контейнера пропадал целиком).
            if let Some(shrink) = node.style.used().flex_shrink {
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
            let sized =
                crate::style::cascade::materialize::box_style(&inherited, &inner.style, vcx);
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
                let lane = if inner.style.used().border_box == Some(true) {
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
                    // выпускал ленту за контейнер (`grid-baseline-003-ref`).
                    let flex_item = matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    );
                    if node.style.used().flex_shrink.is_none() && !flex_item {
                        d = d.flex_shrink_0();
                    }
                }
            }
            d.into_any_element()
        },
    );
    Some(crate::interactive::scroll_extent::project(
        crate::interactive::scroll_area::ScrollArea::new(
            e.state_id("html-scroll-area", vcx),
            horizontal,
            vertical,
            build,
            vcx,
        )
        .with_style(scroll_style, freeze_scrollbars)
        .with_tree(e, parent_frame.as_ref())
        .with_sticky(area_sticky)
        .into_any_element(),
        scroll_projection,
    ))
}

/// Обернуть элемент ручкой изменения размера, если `resize` разрешает.
pub(crate) fn resizable(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if e.style.cascaded().pointer_events_none == Some(true) {
        return None;
    }
    let (horizontal, vertical) = e.style.used().resize?;
    let axis = match (horizontal, vertical) {
        (true, true) => crate::interactive::resizable::ResizeAxis::Both,
        (true, false) => crate::interactive::resizable::ResizeAxis::Horizontal,
        _ => crate::interactive::resizable::ResizeAxis::Vertical,
    };
    let id = e.state_id("html-resize", opts.cx.vcx());
    let node = e.clone();
    let (inherited, opts) = (inherited.clone(), opts.clone());
    let build = std::rc::Rc::new(move |w: Option<f32>, h: Option<f32>| {
        // Заданный мышью размер побеждает разметку — как и в браузере, где
        // он пишется в инлайн-стиль элемента.
        let mut mixed = node.clone();
        mixed.style.used_mut().resize = None;
        if let Some(w) = w {
            mixed.style.used_mut().width = Some(Len::Px(w));
        }
        if let Some(h) = h {
            mixed.style.used_mut().height = Some(Len::Px(h));
        }
        opts.cx.contracts.build.element(&mixed, &inherited, &opts)
    });
    Some(crate::interactive::resizable::Resizable::new(id, axis, build).into_any_element())
}
