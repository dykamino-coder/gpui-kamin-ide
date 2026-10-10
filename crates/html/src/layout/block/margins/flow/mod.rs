//! Схлопывание полей в потоке: поглощение полей детей и сквозные коробки.

use crate::dom::Node;
use crate::layout::block::struts::margin_px;
use crate::render::{inline_level, is_blank};
use crate::style::values::value::Len;
mod absorb;
pub(super) use absorb::absorb_margin;
pub(super) use absorb::collapses_through;

/// `lead` — собственное поле КОНТЕЙНЕРА по ведущей стороне оси потока
/// (`margin-left` при `vertical-lr`, `margin-right` при `vertical-rl`), если
/// эта сторона открыта — без рамки и внутреннего отступа. CSS 2.1 §8.3.1:
/// «The top margin of an in-flow block element collapses with its first
/// in-flow block-level child's top margin if the element has no top border,
/// no top padding»; css-writing-modes-4 §7.1 переносит это на `margin-left`
/// / `margin-right` в вертикальном письме («in a vertical-rl writing mode it
/// takes part in margin collapsing in place of margin-bottom»).
/// `None` — сторона запечатана либо контейнер — корень (§8.3.1: поля корня
/// не схлопываются).
pub(crate) fn collapse_flow_margins(
    children: Vec<Node>,
    reverse: bool,
    lead: Option<f32>,
) -> Vec<Node> {
    let mut out = children;
    let trailing: Option<f32> = lead.filter(|m| *m >= 0.0);
    collapse_flow_kids(reverse, &mut out, trailing);
    out
}

pub(super) fn collapse_flow_kids(reverse: bool, out: &mut [Node], mut trailing: Option<f32>) {
    for node in out.iter_mut() {
        let child = match node {
            Node::Element(child) => child,
            Node::Text(text) if !text.trim().is_empty() => {
                trailing = None;
                continue;
            }
            _ => continue,
        };
        // Out-of-flow boxes neither collapse nor interrupt adjacent block margins.
        if child.style.float.is_some_and(|f| f != 0)
            || matches!(
                child.style.position,
                Some(
                    crate::style::computed::Position::Absolute
                        | crate::style::computed::Position::Fixed
                )
            )
        {
            continue;
        }
        // An in-flow inline box forms a line, separating neighboring block margins.
        if inline_level(child) {
            trailing = None;
            continue;
        }
        // Ведущее поле ребёнка схлопывается с ведущим полем ЕГО первого
        // потокового ребёнка — и дальше вниз по цепочке (CSS 2.1 §8.3.1: «top
        // margin of a box and top margin of its first in-flow child»;
        // css-writing-modes-4 §7.4 подставляет block-start). Хвостовую цепочку
        // `absorb_margin(child, true, …)` ниже собирает давно, ведущую — нет:
        // `body{margin-left:100px}` → `div` с нулевым полем → `p` (UA 1em)
        // давали 100 + 0 + 16 вместо max(100, 0, 16) = 100, и поток уезжал на
        // +16 CSS (`sizing-orthog-prct-htb-in-vlr-001`: чернила и рамка
        // совпадают побайтно, сдвиг ровно 20 px снимка). Blink несёт поле
        // вниз `MarginStrut`-ом на любую глубину.
        // Свой контекст форматирования поле с детьми не схлопывает — тот же
        // список, что у `lead_margin` в `element()`; текст перед первым
        // блоком — строка, и она поля разделяет.
        let own_context = child.inline
            || child.style.display.is_some()
            || !matches!(
                child.style.overflow_x,
                None | Some(crate::style::computed::Overflow::Visible)
            )
            || !matches!(
                child.style.overflow_y,
                None | Some(crate::style::computed::Overflow::Visible)
            )
            || child.style.flow_root == Some(true)
            || child.style.align_content_block
            || child.style.contain_layout == Some(true)
            || child.style.contain_paint == Some(true);
        let text_first = child
            .children
            .iter()
            .find(|n| !is_blank(n))
            .is_some_and(|n| matches!(n, Node::Text(_)));
        if !own_context && !text_first {
            absorb_margin(child, false, reverse);
        }
        // В обратном потоке ведущая сторона — правая.
        let lead = if reverse {
            child.style.margin.right
        } else {
            child.style.margin.left
        };
        // Доли кегля разрешаются здесь же: голый разбор точек считал `1em`
        // нулём и ЗАПИСЫВАЛ ноль — поле абзаца вдоль вертикального потока
        // пропадало вовсе (wm-propagation-body-*).
        let lead_px = margin_px(lead, &child.style).unwrap_or(0.0);
        // Пустой блок пропускает поля СКВОЗЬ себя: хвост предыдущего брата,
        // его ведущее и его хвостовое поле сливаются в одно (наибольшее из
        // неотрицательных), и оно же становится хвостом для следующего.
        // Прежде ведущее урезалось на хвост брата, а хвостовое оставалось
        // целиком — у двух пустых `margin-left: 2em` в `vertical-rl`
        // выходило 4em вместо 2em (`margin-collapse-vrl-024/030`,
        // `-vlr-025/031`). Гейт «хвостовое поле > 0»: при нулевом хвосте
        // картина прежняя, а абсолютный сосед (`margin-collapse-vrl-022`,
        // `-vlr-023` — пустой `widthless-static` перед абсолютом) не должен
        // получить новый `trailing`. Отрицательные поля — прежним путём.
        let tail = if reverse {
            child.style.margin.left
        } else {
            child.style.margin.right
        };
        let tail_px = margin_px(tail, &child.style).unwrap_or(0.0);
        let prev = trailing.unwrap_or(0.0);
        if tail_px > 0.0 && lead_px >= 0.0 && prev >= 0.0 && collapses_through(child) {
            let combined = prev.max(lead_px).max(tail_px);
            let kept = combined - prev;
            if reverse {
                child.style.margin.right = Some(Len::Px(kept));
                child.style.margin.left = Some(Len::Px(0.0));
            } else {
                child.style.margin.left = Some(Len::Px(kept));
                child.style.margin.right = Some(Len::Px(0.0));
            }
            trailing = Some(combined);
            continue;
        }
        if let Some(prev) = trailing {
            let kept = (lead_px - prev).max(0.0);
            if reverse {
                child.style.margin.right = Some(Len::Px(kept));
            } else {
                child.style.margin.left = Some(Len::Px(kept));
            }
        }
        trailing = Some(absorb_margin(child, true, reverse));
    }
}
