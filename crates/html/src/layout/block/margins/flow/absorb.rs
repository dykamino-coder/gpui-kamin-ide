//! Поглощение полей потомка и сквозное схлопывание пустой коробки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::struts::margin_px;
use crate::style::values::value::Len;

// Поле контейнера схлопывается С КРАЙНИМ flow-ребёнком через пустую
// границу (CSS 2.1 §8.3.1): у `<body>` без рамки и паддинга хвостовое
// поле — max(своё, block-end последнего ребёнка), рекурсивно. Без этого
// `html::after` за body отъезжал на сумму полей (wm-propagation-body-042:
// 16 у последнего `<p>` + 8 у body складывались вместо max).
// Поглощение: поле крайнего ребёнка ОБНУЛЯЕТСЯ и уезжает на контейнер
// (иначе оно распирало бы его коробку изнутри и зазор снаружи удваивался).
pub(crate) fn absorb_margin(
    e: &mut Element,
    tail_side: bool,
    reverse: bool,
    vcx: crate::document::state::ValueCx<'_>,
) -> f32 {
    let own = if tail_side == reverse {
        margin_px(e.style.computed().margin.left, &e.style.computed(), vcx)
    } else {
        margin_px(e.style.computed().margin.right, &e.style.computed(), vcx)
    }
    .unwrap_or(0.0);
    // Контейнер с ГОРИЗОНТАЛЬНЫМ письмом в вертикальном потоке —
    // ортогональный: его внутренний поток идёт по другой оси, и полей
    // на этой границе не отдаёт (available-size-020..023).
    if e.style.cascaded().vertical_decl() == Some(false) {
        return own;
    }
    let b = e.style.computed().borders();
    let (border, pad) = if tail_side == reverse {
        (b.left, e.style.computed().padding.left)
    } else {
        (b.right, e.style.computed().padding.right)
    };
    let sealed = margin_px(border, &e.style.computed(), vcx).unwrap_or(0.0) > 0.0
        || margin_px(pad, &e.style.computed(), vcx).unwrap_or(0.0) > 0.0;
    if sealed {
        return own;
    }
    let edge_child = {
        let mut it = e.children.iter_mut().filter_map(|n| match n {
            Node::Element(c)
                if !matches!(
                    c.style.computed().position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ) && c.style.computed().display.is_none() =>
            {
                Some(c)
            }
            _ => None,
        });
        if tail_side { it.last() } else { it.next() }
    }
    // Схлопка живёт в ОДНОМ потоке: ребёнок со своим письмом заводит другой
    // и границу запечатывает. He still is the EDGE child: skipping him made
    // an earlier sibling's end margin the parent's and dropped it from
    // between the two (`available-size-020`: an orthogonal `article` after a
    // `p` lost the `p`'s block-end margin).
    .filter(|c| {
        c.style.cascaded().writing_mode.is_none() && c.style.cascaded().flow_mode().is_none()
    });
    match edge_child {
        Some(c) => {
            let inner = absorb_margin(c, tail_side, reverse, vcx);
            // Поглощать есть что только при ненулевом внутреннем поле;
            // иначе стили НЕ переписываются: заморозка `Em` в точки
            // до разрешения кегля портила поле (`font-size: 5em` у
            // text-combine-upright-value-*).
            if inner <= 0.0 {
                return own;
            }
            if tail_side == reverse {
                c.style.used_mut().margin.left = Some(Len::Px(0.0));
            } else {
                c.style.used_mut().margin.right = Some(Len::Px(0.0));
            }
            let total = own.max(inner);
            if tail_side == reverse {
                e.style.used_mut().margin.left = Some(Len::Px(total));
            } else {
                e.style.used_mut().margin.right = Some(Len::Px(total));
            }
            total
        }
        None => own,
    }
}

// Ведущее поле ПЕРВОГО ребёнка схлопывается с полем контейнера так же,
// как поля братьев между собой (§8.3.1, первый in-flow ребёнок): в
// `prev` кладётся поле контейнера, и ребёнку остаётся разница.
// `body { margin: 8px }` + `p { margin-block: 1em }` при `html
// { writing-mode: vertical-lr }` дают 16 от края окна, а не 24 — ровно
// на эти 8 CSS px уезжала ВСЯ страница (`abs-pos-non-replaced-vlr-007`,
// `text-indent-vlr-011`, `clip-rect-vlr-011`: эталон `…-vlr-007-ref` считает
// «80px + p's margin-left (1em)» от `margin-left: 0.5em` + `body` 8).
// Отрицательное поле контейнера в схлопывание не вступает (иначе
// `kept` росло бы на его модуль).
// `available-size-022/023` — дети КОРНЯ: у `html` поля не схлопываются,
// вызов передаёт `None`.
// Пустой блок, сквозь который смыкаются его собственные поля вдоль оси
// потока (CSS 2.1 §8.3.1: «does not establish a new block formatting
// context … zero computed 'min-height', zero or 'auto' computed 'height',
// and no in-flow children … it is possible for margins to collapse through
// it»; css-writing-modes-4 §7.1 переносит правило на горизонталь
// вертикального письма, Overview.bs:1918-1926). Ось потока здесь
// горизонтальна, поэтому «высота» правила — `width`/`min-width`, а рамки
// и отступы — левые и правые. Своё письмо и собственный контекст
// форматирования поток запечатывают.
pub(crate) fn collapses_through(e: &Element) -> bool {
    let none_or_zero = |l: Option<Len>| match l {
        None | Some(Len::Auto) => true,
        Some(Len::Px(v)) => v == 0.0,
        _ => false,
    };
    let b = e.style.computed().borders();
    e.style.computed().display.is_none()
        && e.style.cascaded().writing_mode.is_none()
        && e.style.cascaded().flow_mode().is_none()
        && !matches!(
            e.style.computed().position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && matches!(
            e.style.computed().overflow_x,
            None | Some(crate::style::computed::Overflow::Visible)
        )
        && matches!(
            e.style.computed().overflow_y,
            None | Some(crate::style::computed::Overflow::Visible)
        )
        && e.style.computed().flow_root != Some(true)
        && e.style.computed().contain_layout != Some(true)
        && e.style.computed().contain_paint != Some(true)
        && none_or_zero(e.style.computed().width)
        && none_or_zero(e.style.computed().min_width)
        && none_or_zero(b.left)
        && none_or_zero(b.right)
        && none_or_zero(e.style.computed().padding.left)
        && none_or_zero(e.style.computed().padding.right)
        && e.children.iter().all(is_blank)
}
