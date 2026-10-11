//! Струны полей: сквозное схлопывание, цепочки первого и последнего ребёнка.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::margins::COLLAPSE_CB_WIDTH_PX;
use crate::render::{holds_line_box, in_flow, inline_level_box, own_context, replaced_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;
mod through;
pub(super) use through::adjoin;
pub(crate) use through::float_only_wrapper;
pub(crate) use through::solve;
pub(super) use through::strut_of;
pub(crate) use through::through_strut;
pub(super) use through::through_strut_no_clear;
mod chains;
pub(super) use chains::leading_chain;
pub(crate) use chains::margin_px;
pub(super) use chains::pin_inherited_margins;
pub(super) use chains::trailing_chain;
pub(super) use chains::zero_at;

/// Первый (по направлению итератора) IN-FLOW блочный ребёнок: плавающие,
/// абсолютные и пустые строчные пропускаются, непробельный текст и строчный
/// элемент с содержимым (строчная коробка!) обрывают поиск.
pub(super) fn first_in_flow<'a>(
    it: impl Iterator<Item = (usize, &'a Node)>,
) -> Option<(usize, &'a crate::dom::Element)> {
    for (i, c) in it {
        match c {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return None,
            // Строчный ТЕГ с блочным видом — блочный ребёнок: нижнее поле
            // `<em style="display:block">` схлопывается через родителя
            // (`selectors-001`: под зелёной строкой оставалась красная полоса
            // в 1em). Обратный случай (`div` с `inline-block`) остаётся на
            // прежнем пути: ★ ЗАМЕРЕНО — чистое `inline_level_box(ch)` роняло
            // `image-color-background-size` 0.00 → 4.50.
            Node::Element(ch) if ch.inline && inline_level_box(ch) => {
                // Замещаемый атом (img и родня) — строчная КОРОБКА, а не
                // пустой спан: он рождает line box и рвёт примыкание.
                // Пропуск ронял отступ параграфа перед голым <img> в
                // эталонах (130 пар «эталон рисует img»).
                if ch.children.is_empty()
                    && !matches!(
                        ch.tag.as_str(),
                        "img"
                            | "svg"
                            | "canvas"
                            | "video"
                            | "embed"
                            | "object"
                            | "iframe"
                            | "input"
                            | "br"
                    )
                {
                    continue;
                }
                return None;
            }
            Node::Element(ch) => {
                // Строчный КОНТЕЙНЕР (`inline-block` и родня) рождает
                // строчную коробку и РВЁТ примыкание (CSS 2.1 §8.3.1), в
                // отличие от плавающего и абсолютного, которых в потоке нет
                // вовсе. Пока он просто пропускался, отступ предыдущего
                // блока «протекал» наружу мимо него, и строка вставала на
                // отступ выше (`box-sizing-010`: квадраты разъезжались на
                // кегль).
                // Разбор держит `display: inline` как `InlineBlock` с
                // пометкой `inline_display`, поэтому одного взгляда на
                // display мало: обычный строчный элемент своей коробки не
                // имеет и примыкание НЕ рвёт (пустой `<span>` между блоками
                // прозрачен).
                // Своей коробки у `display: inline` нет, и примыкание он
                // рвёт не собой, а СТРОЧНОЙ КОРОБКОЙ, которую рождает его
                // содержимое: вокруг неё встаёт анонимная блочная коробка
                // (§9.2.1.1). Пустой такой элемент прозрачен, а с текстом
                // внутри обязан оборвать поиск — иначе отступ предыдущего
                // блока протекает под анонимную коробку, и строка встаёт на
                // него выше (`inline-formatting-context-002`).
                if ch.style.inline_display == Some(true) {
                    if replaced_inline(&ch.tag) || holds_line_box(&ch.children) {
                        return None;
                    }
                    continue;
                }
                if ch.style.inline_display != Some(true)
                    && matches!(
                        ch.style.display,
                        Some(Display::InlineBlock)
                            | Some(Display::InlineFlex)
                            | Some(Display::InlineGrid)
                            | Some(Display::InlineTable)
                    )
                {
                    return None;
                }
                if !in_flow(&ch.style) {
                    continue;
                }
                return Some((i, ch));
            }
        }
    }
    None
}

/// Ноль по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и `border: 0`
/// схлопыванию не мешают (CSS 2.1 §8.3.1).
pub(crate) fn zero_len(l: Option<Len>) -> bool {
    matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)))
}

/// Открыт ли ВЕРХНИЙ край коробки для примыкания к полю первого ребёнка: нет
/// ни рамки, ни поля сверху, и коробка не заводит своего контекста (§8.3.1).
pub(crate) fn top_edge_open(e: &Element) -> bool {
    // Доля отступа при содержащем блоке НУЛЕВОЙ ширины вычисляется в ноль
    // (§8.4: проценты — от ширины содержащего блока), и край открыт
    // (§8.3.1 «no top padding»): `margin-collapse-028` — `padding: 50%` внутри
    // `width: 0`, поле внука уходило внутрь, и зелёная полоса ехала на 40 ниже.
    // Вызывается только внутри схлопывания: ширина — уровня или (в цепочке)
    // содержимого родителя `e`, см. `with_inner_cb`.
    let pct_zero = matches!(e.style.padding.top, Some(Len::Pct(_)))
        && COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get) == Some(0.0);
    !own_context(e)
        && (zero_len(e.style.padding.top) || pct_zero)
        && zero_len(e.style.borders().top)
}

/// Струна примыкающих полей (CSS 2.1 §8.3.1): больший положительный и самый
/// отрицательный. Свёртка ассоциативна, поэтому все три случая спеки — сосед,
/// родитель с ребёнком и схлопывание насквозь — считаются одним кодом.
pub(crate) type Strut = (f32, f32);

/// Ненулевые поля, отступы или рамки строчной коробки по СТРОЧНОЙ оси.
/// Такая коробка не даёт строке стать фантомной (css-inline-3
/// §invisible-line-boxes: «no inline boxes with non-zero inline-axis margins,
/// padding, or borders»); блочная ось (`padding-top`, `margin-bottom`) в счёт
/// не идёт, а отрицательное поле — тоже ненулевое (`phantom-line-boxes-004`).
/// Оси физические: в вертикальном письме строчная ось — `top`/`bottom`, там
/// правило пока не различает (тестов нет).
pub(crate) fn inline_axis_edges(c: &Computed) -> bool {
    let nonzero =
        |l: Option<Len>| matches!(l, Some(Len::Px(v) | Len::Pct(v) | Len::Em(v)) if v != 0.0);
    let b = c.borders();
    nonzero(c.margin.left)
        || nonzero(c.margin.right)
        || nonzero(c.padding.left)
        || nonzero(c.padding.right)
        || nonzero(b.left)
        || nonzero(b.right)
}

/// Поле в точках или `None`, если единица нам не по зубам (доля, `vh`,
/// `calc`). Ноль подставлять НЕЛЬЗЯ: ветка насквозь значение ЗАПИСЫВАЕТ
/// обратно, и написанное пропадёт навсегда (`margin-bottom-103`: `50%`
/// превращалось в `0`).
pub(super) fn margin_or_bail(l: Option<Len>, style: &Computed) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(_) => margin_px(l, style),
    }
}
