//! Схлопывание полей в потоке: поглощение полей детей и сквозные коробки.

use crate::dom::{Element, Node};
use crate::layout::block::struts::margin_px;
use crate::render::{inline_level, is_blank};
use crate::style::values::value::Len;

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

// Поле контейнера схлопывается С КРАЙНИМ flow-ребёнком через пустую
// границу (CSS 2.1 §8.3.1): у `<body>` без рамки и паддинга хвостовое
// поле — max(своё, block-end последнего ребёнка), рекурсивно. Без этого
// `html::after` за body отъезжал на сумму полей (wm-propagation-body-042:
// 16 у последнего `<p>` + 8 у body складывались вместо max).
// Поглощение: поле крайнего ребёнка ОБНУЛЯЕТСЯ и уезжает на контейнер
// (иначе оно распирало бы его коробку изнутри и зазор снаружи удваивался).
pub(super) fn absorb_margin(e: &mut Element, tail_side: bool, reverse: bool) -> f32 {
    let own = if tail_side == reverse {
        margin_px(e.style.margin.left, &e.style)
    } else {
        margin_px(e.style.margin.right, &e.style)
    }
    .unwrap_or(0.0);
    // Контейнер с ГОРИЗОНТАЛЬНЫМ письмом в вертикальном потоке —
    // ортогональный: его внутренний поток идёт по другой оси, и полей
    // на этой границе не отдаёт (available-size-020..023).
    if e.style.vertical == Some(false) {
        return own;
    }
    let b = e.style.borders();
    let (border, pad) = if tail_side == reverse {
        (b.left, e.style.padding.left)
    } else {
        (b.right, e.style.padding.right)
    };
    let sealed = margin_px(border, &e.style).unwrap_or(0.0) > 0.0
        || margin_px(pad, &e.style).unwrap_or(0.0) > 0.0;
    if sealed {
        return own;
    }
    let edge_child = {
        let mut it = e.children.iter_mut().filter_map(|n| match n {
            Node::Element(c)
                if !matches!(
                    c.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ) && c.style.display.is_none()
                    // Схлопка живёт в ОДНОМ потоке: ребёнок со своим
                    // письмом заводит другой и границу запечатывает.
                    && c.style.vertical.is_none()
                    && c.style.vertical_rl.is_none() =>
            {
                Some(c)
            }
            _ => None,
        });
        if tail_side { it.last() } else { it.next() }
    };
    match edge_child {
        Some(c) => {
            let inner = absorb_margin(c, tail_side, reverse);
            // Поглощать есть что только при ненулевом внутреннем поле;
            // иначе стили НЕ переписываются: заморозка `Em` в точки
            // до разрешения кегля портила поле (`font-size: 5em` у
            // text-combine-upright-value-*).
            if inner <= 0.0 {
                return own;
            }
            if tail_side == reverse {
                c.style.margin.left = Some(Len::Px(0.0));
            } else {
                c.style.margin.right = Some(Len::Px(0.0));
            }
            let total = own.max(inner);
            if tail_side == reverse {
                e.style.margin.left = Some(Len::Px(total));
            } else {
                e.style.margin.right = Some(Len::Px(total));
            }
            total
        }
        None => own,
    }
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

// Ведущее поле ПЕРВОГО ребёнка схлопывается с полем контейнера так же,
// как поля братьев между собой (§8.3.1, первый in-flow ребёнок): в
// `prev` кладётся поле контейнера, и ребёнку остаётся разница.
// `body { margin: 8px }` + `p { margin-block: 1em }` при `html
// { writing-mode: vertical-lr }` дают 16 от края окна, а не 24 — ровно
// на эти 8 CSS px уезжала ВСЯ страница (`abs-pos-non-replaced-vlr-007`
// 1.09, `text-indent-vlr-011` 1.09, `clip-rect-vlr-011` 1.00: снимок
// сдвинут на 10 px при масштабе 1.25, эталон `…-vlr-007-ref` считает
// «80px + p's margin-left (1em)» от `margin-left: 0.5em` + `body` 8).
// Отрицательное поле контейнера в схлопывание не вступает (иначе
// `kept` росло бы на его модуль).
// Прежний замер «available-size-022/023 0.00 -> 2.66» относился к детям
// КОРНЯ — у `html` поля не схлопываются, вызов передаёт `None`.
// Пустой блок, сквозь который смыкаются его собственные поля вдоль оси
// потока (CSS 2.1 §8.3.1: «does not establish a new block formatting
// context … zero computed 'min-height', zero or 'auto' computed 'height',
// and no in-flow children … it is possible for margins to collapse through
// it»; css-writing-modes-4 §7.1 переносит правило на горизонталь
// вертикального письма, Overview.bs:1918-1926). Ось потока здесь
// горизонтальна, поэтому «высота» правила — `width`/`min-width`, а рамки
// и отступы — левые и правые. Своё письмо и собственный контекст
// форматирования поток запечатывают.
pub(super) fn collapses_through(e: &Element) -> bool {
    let none_or_zero = |l: Option<Len>| match l {
        None | Some(Len::Auto) => true,
        Some(Len::Px(v)) => v == 0.0,
        _ => false,
    };
    let b = e.style.borders();
    e.style.display.is_none()
        && e.style.vertical.is_none()
        && e.style.vertical_rl.is_none()
        && !matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && matches!(
            e.style.overflow_x,
            None | Some(crate::style::computed::Overflow::Visible)
        )
        && matches!(
            e.style.overflow_y,
            None | Some(crate::style::computed::Overflow::Visible)
        )
        && e.style.flow_root != Some(true)
        && e.style.contain_layout != Some(true)
        && e.style.contain_paint != Some(true)
        && none_or_zero(e.style.width)
        && none_or_zero(e.style.min_width)
        && none_or_zero(b.left)
        && none_or_zero(b.right)
        && none_or_zero(e.style.padding.left)
        && none_or_zero(e.style.padding.right)
        && e.children.iter().all(is_blank)
}
