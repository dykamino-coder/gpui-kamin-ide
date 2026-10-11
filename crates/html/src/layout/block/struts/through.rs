//! Сквозные струны: коробка, через которую схлопываются поля, и обёртка из одних флоатов.

use super::{Strut, margin_or_bail};
use crate::dom::{Element, Node};
use crate::layout::float::initial_letter::px_margin_w;
use crate::render::{holds_line_box, in_flow, inline_marked_block, own_context};
use crate::style::computed::Display;
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

pub(crate) fn strut_of(v: f32) -> Strut {
    (v.max(0.0), v.min(0.0))
}

pub(crate) fn adjoin(a: Strut, b: Strut) -> Strut {
    (a.0.max(b.0), a.1.min(b.1))
}

/// Итог струны: максимум положительных минус максимум модулей отрицательных.
pub(crate) fn solve(s: Strut) -> f32 {
    s.0 + s.1
}

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// НЕ СОДЕРЖИТ СТРОЧНОЙ КОРОБКИ, и поля всех её детей в потоке тоже
/// схлопываются. Возвращаются слитые поля — свои плюс поля всех
/// насквозь-потомков: это и есть транзитивность примыкания.
pub(crate) fn through_strut(e: &Element) -> Option<Strut> {
    through_strut_inner(e, false)
}

/// То же, но без вето по `clear`: нужно, чтобы отличить «не схлопывается
/// вовсе» от «схлопнулась бы, если бы не клиренс».
pub(crate) fn through_strut_no_clear(e: &Element) -> Option<Strut> {
    through_strut_inner(e, true)
}

pub(super) fn through_strut_inner(e: &Element, ignore_clear: bool) -> Option<Strut> {
    // Строчная пометка у блочной коробки (псевдоэлемент) — не строчный.
    if (e.inline && !inline_marked_block(e)) || !in_flow(&e.style) || own_context(e) {
        return None;
    }
    // Поля КОРНЯ ни с чем не схлопываются (§8.3.1).
    if e.tag == "html" {
        return None;
    }
    // Коробка с `clear` насквозь не схлопывается: клиренс разделяет её поля
    // (§8.3.1, «if the element's margins are collapsed … clearance»). Пустой
    // `<div class="clear-left">` между флоатом и соседом иначе пропускал бы
    // поле соседа наружу (`floats-clear/margin-collapse-033…035`).
    //
    // ПРОБОВАЛИ И ОТКАТИЛИ: делать вето инертным, когда во всём документе нет
    // ни одного флоата (§9.5.2 вводит клиренс только при флоате выше). Проба
    // по паре `margin-collapse-135` и `margin-collapse-clear-016`, на которые
    // правка и рассчитывалась: обе остались «красное видно», флипов ноль.
    // Держит их не вето, а что-то ниже по цепи. Признак документа пришлось бы
    // нести отдельным thread-local, а рамка рисует вложенный документ тем же
    // `render()` и признак бы затёрла.
    if e.style.clear.is_some() && !ignore_clear {
        return None;
    }
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let b = e.style.borders();
    // §10.5: доля высоты при НЕОПРЕДЕЛЁННОМ содержащем блоке «computes to
    // auto», а `auto` схлопыванию насквозь не мешает (§8.3.1). Прежде любая
    // ненулевая доля закрывала ветку, и три пустых блока с полями 100 давали
    // 200 вместо 100 (`margin-collapse-through-percentage-height-block`).
    // Признак блока несёт сам стиль — `cb_height_def` ставит `inline::inherit`.
    let height_is_auto = matches!(
        e.style.height,
        None | Some(Len::Auto) | Some(Len::Px(0.0)) | Some(Len::Pct(0.0))
    ) || matches!(e.style.height, Some(Len::Pct(_)) if !e.style.cb_height_def);
    let min_height_is_zero = zero(e.style.min_height)
        || matches!(e.style.min_height, Some(Len::Pct(_)) if !e.style.cb_height_def);
    if !zero(e.style.padding.top)
        || !zero(e.style.padding.bottom)
        || !zero(b.top)
        || !zero(b.bottom)
        || !min_height_is_zero
        || !height_is_auto
    {
        return None;
    }
    // Главная проверка содержимого: без неё `<div>` из четырёх `<img>`
    // считался пустым (ЗАМЕРЕНО: -30 на эталонах `bidi-box-model-*`).
    if holds_line_box(&e.children) {
        return None;
    }
    let mut s = adjoin(
        strut_of(margin_or_bail(e.style.margin.top, &e.style)?),
        strut_of(margin_or_bail(e.style.margin.bottom, &e.style)?),
    );
    // «all of its in-flow children's margins collapse» — рекурсия по блочным
    // детям в потоке. Строчных здесь уже нет (проверка выше), вне потока —
    // запрета не создают.
    for c in &e.children {
        let Node::Element(ch) = c else { continue };
        // `clear` у ребёнка в потоке закрывает схлопывание насквозь (§8.3.1
        // исключение 2) — проверять ДО отсечки строчных: псевдоэлемент
        // помечается строчным независимо от своего `display`, и клирфикс
        // `div::after { clear: both; display: block }` был отсюда не виден.
        if in_flow(&ch.style) && ch.style.clear.is_some() {
            return None;
        }
        if (ch.inline && !inline_marked_block(ch))
            || ch.style.display == Some(Display::None)
            || ch.style.display == Some(Display::Contents)
            || !in_flow(&ch.style)
        {
            continue;
        }
        let t = through_strut(ch)?;
        // Поля детей контейнера с `margin-trim` обрезаны (css-box-4
        // §margin-trim-block: «and any margins collapsed with it»): насквозь
        // схлопнутый ребёнок примыкает ОБОИМИ краями, значит его поля
        // обрезаются при любом бите. В струну родителя идут только свои поля
        // контейнера. Обрезку на своём уровне сделает `collapse_margins`, но
        // уровень выше считается РАНЬШЕ и успевал забрать поле 222
        // (`block-container-block-end-self-collapsing-block-start-margin-nested`).
        if e.style.margin_trim & 3 == 0 {
            s = adjoin(s, t);
        }
    }
    Some(s)
}

/// Обёртка, у которой в потоке нет НИЧЕГО, кроме флоатов: обычный блок без
/// своего контекста, `clear`, позиционирования, полей, отступов и рамок по
/// блочной оси, высотой `auto` (или нулём, который ей ставит §10.6.3 в
/// `collapse_margins`). Вернуть (есть левые, есть правые, нижняя оценка
/// ширины узчайшего флоата).
///
/// Такая коробка схлопывается насквозь, и её флоаты ПРИМЫКАЮТ к полю
/// следующего брата: без разделения поле брата увезло бы их вниз
/// (`new-fc-separates-from-float`, `adjoining-float-before-clearance`).
pub(crate) fn float_only_wrapper(w: &Element) -> Option<(bool, bool, f32)> {
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let b = w.style.borders();
    if (w.inline && !inline_marked_block(w))
        || !in_flow(&w.style)
        || !matches!(w.style.display, None | Some(Display::Block))
        || own_context(w)
        || w.style.clear.is_some()
        || w.style.position.is_some()
        || !matches!(w.style.height, None | Some(Len::Auto) | Some(Len::Px(0.0)))
        || !zero(w.style.min_height)
        || ![
            w.style.margin.top,
            w.style.margin.bottom,
            w.style.padding.top,
            w.style.padding.bottom,
            b.top,
            b.bottom,
        ]
        .into_iter()
        .all(zero)
    {
        return None;
    }
    let (mut left, mut right, mut min_w, mut any) = (false, false, f32::INFINITY, false);
    for n in &w.children {
        match n {
            Node::Text(t) if blank_text(t) => {}
            Node::Element(f) if f.style.float.is_some_and(|s| s != 0) => {
                any = true;
                if f.style.float.is_some_and(|s| s < 0) {
                    left = true;
                } else {
                    right = true;
                }
                min_w = min_w.min(px_margin_w(&f.style).unwrap_or(0.0));
            }
            _ => return None,
        }
    }
    any.then_some((left, right, min_w))
}
