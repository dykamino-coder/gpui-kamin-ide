//! Липкие коробки `position: sticky`.
// owner: A

use crate::render::*;

pub mod element;

/// Обернуть элемент преобразованием, если оно задано.
///
/// Сюда же попадает вертикальное письмо: строки, идущие сверху вниз, — это
/// повёрнутый на четверть оборота блок. Глифы при этом ложатся боком, как в
/// браузере для латиницы (`text-orientation: mixed`).
/// Распорка, снимающая коробку родителя и видимую часть ленты.
///
/// Абсолютная и во весь родитель: так её собственный прямоугольник и есть
/// коробка родителя, а обрезка на замере — это видимая часть прокрутки.
pub(crate) fn sticky_probe(frame: crate::interact::StickyCell) -> AnyElement {
    gpui::canvas(
        move |bounds, window, _| {
            frame.set(crate::interact::StickyFrame {
                container: Some(bounds),
                viewport: Some(window.content_mask().bounds),
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Обернуть элемент липкой рамкой, если `position: sticky`.
///
/// Отложенный проход нужен из-за порядка: прилипший заголовок рисуется до
/// содержимого, которое под ним проезжает, и без переноса в конец кадра это
/// содержимое его закрашивало бы.
pub(crate) fn sticky_wrap(
    el: AnyElement,
    c: &Computed,
    frame: &crate::interact::StickyCell,
    allowed: bool,
) -> AnyElement {
    if c.position != Some(crate::computed::Position::Sticky) {
        return el;
    }
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Auto) | None => None,
        // Проценты у порога считаются от видимой части; её размер известен
        // только на отрисовке, поэтому берём ноль — как `top: 0`.
        Some(_) => Some(0.0),
    };
    let mut wrapper = crate::interact::Sticky::new(el, frame.clone());
    wrapper.top = side(c.inset.top);
    wrapper.bottom = side(c.inset.bottom);
    wrapper.left = side(c.inset.left);
    wrapper.right = side(c.inset.right);
    // Внутри отложенного поддерева липкий элемент рисуется на месте:
    // откладывать повторно нельзя. Без единого порога (все четыре `auto`)
    // липкий не сдвигается вовсе — css-position-3 §sticky-pos: «If both
    // inset properties in a given axis are auto, no offsets are added in that
    // axis», — и красится как `relative`, в порядке дерева (CSS 2.1 прил. E,
    // шаг 8). Отложенный, он ложился ПОВЕРХ следующего позиционированного
    // брата (`position-sticky-stacking-context-002`: красная половина
    // липкого вылезала поверх `relative`-брата). Servo такому узлу липкой
    // рамки не заводит (`stacking_context.rs`, ветка всех `auto`).
    let thresholds = wrapper.top.is_some()
        || wrapper.bottom.is_some()
        || wrapper.left.is_some()
        || wrapper.right.is_some();
    if !allowed || !thresholds {
        return wrapper.into_any_element();
    }
    gpui::deferred(wrapper)
        .with_priority(c.z_index.unwrap_or(0).max(0) as usize)
        .into_any_element()
}
