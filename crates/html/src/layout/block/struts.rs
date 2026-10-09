//! Струны полей: сквозное схлопывание, цепочки первого и последнего ребёнка.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::containing::{CB_WIDTH, with_inner_cb};
use crate::layout::block::margins::{COLLAPSE_CB_WIDTH_PX, COLLAPSE_FONT_PX};
use crate::layout::float::clear::bfc_no_fit;
use crate::layout::float::initial_letter::px_margin_w;
use crate::render::{atomic_inline, holds_line_box, in_flow, inline_level_box, inline_marked_block, own_context, replaced_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

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

pub(super) fn strut_of(v: f32) -> Strut {
    (v.max(0.0), v.min(0.0))
}

pub(super) fn adjoin(a: Strut, b: Strut) -> Strut {
    (a.0.max(b.0), a.1.min(b.1))
}

/// Итог струны: максимум положительных минус максимум модулей отрицательных.
pub(crate) fn solve(s: Strut) -> f32 {
    s.0 + s.1
}

/// Ненулевые поля, отступы или рамки строчной коробки по СТРОЧНОЙ оси.
/// Такая коробка не даёт строке стать фантомной (css-inline-3
/// §invisible-line-boxes: «no inline boxes with non-zero inline-axis margins,
/// padding, or borders»); блочная ось (`padding-top`, `margin-bottom`) в счёт
/// не идёт, а отрицательное поле — тоже ненулевое (`phantom-line-boxes-004`).
/// Оси физические: в вертикальном письме строчная ось — `top`/`bottom`, там
/// правило пока не различает (тестов нет).
pub(crate) fn inline_axis_edges(c: &Computed) -> bool {
    let nonzero = |l: Option<Len>| matches!(l, Some(Len::Px(v) | Len::Pct(v) | Len::Em(v)) if v != 0.0);
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
pub(super) fn through_strut_no_clear(e: &Element) -> Option<Strut> {
    through_strut_inner(e, true)
}

fn through_strut_inner(e: &Element, ignore_clear: bool) -> Option<Strut> {
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

pub(super) fn leading_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    // Нижняя оценка ширины флоатов, которые цепь уже прошла: они ПРИМЫКАЮТ к
    // полю следующего в потоке (их блочное смещение ещё не решено).
    let mut adjoining_floats: Option<f32> = None;
    for (i, c) in children.iter().enumerate() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            // Непробельный текст — строчная коробка, примыкание кончилось.
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        // Блочный псевдоэлемент (`::after { display: flow-root }`) —
        // блочный ребёнок, его поле примыкает; см. `inline_marked_block`.
        if ch.inline && !inline_marked_block(ch) {
            // Пустой `<span>` прозрачен, замещаемый атом рождает строку;
            // пустой строчный с полем/отступом/рамкой по строчной оси — не
            // фантом, строка есть (css-inline-3 §invisible-line-boxes).
            if ch.children.is_empty()
                && !replaced_inline(&ch.tag)
                && !inline_axis_edges(&ch.style)
            {
                continue;
            }
            return Some(s);
        }
        // Атомарный строчный в потоке есть и рождает строчную коробку.
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        // Плавающий и абсолютный в потоке не участвуют и примыкания не рвут.
        if !in_flow(&ch.style) {
            if ch.style.float.is_some_and(|f| f != 0) {
                let w = px_margin_w(&ch.style).unwrap_or(0.0);
                adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
            }
            continue;
        }
        // Клиренс разделяет поля (§8.3.1): дальше по цепи примыкание не
        // идёт, и поле такого ребёнка наружу не уходит.
        if ch.style.clear.is_some() {
            return Some(s);
        }
        // Коробка своего контекста, которой рядом с ПРИМЫКАЮЩИМИ флоатами нет
        // места (§9.5), отделяется от них, как клиренсом: её поле флоат вниз
        // не увозит (ассерт `new-fc-separates-from-float`: «will need to
        // separate its margin from the float, so that it doesn't affect the
        // float»). Дальше по цепи примыкание не идёт.
        if adjoining_floats.is_some_and(|w| bfc_no_fit(ch, w)) {
            return Some(s);
        }
        // Поле САМОГО ребёнка примыкает к полю родителя всегда — даже когда
        // ребёнок заводит свой контекст: запрет §8.3.1 лежит на РОДИТЕЛЕ.
        s = adjoin(s, strut_of(margin_or_bail(ch.style.margin.top, &ch.style)?));
        path.push(i);
        if let Some(t) = through_strut(ch) {
            // Насквозь: поля всего поддерева уже в струне, идём к брату.
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            if let Some((_, _, w)) = float_only_wrapper(ch) {
                adjoining_floats = Some(adjoining_floats.map_or(w, |v| v.min(w)));
            }
            continue;
        }
        eat.push((path.clone(), false));
        // Спуск в ребёнка с `margin-trim: block-start` не нужен: верхнее поле
        // его первого ребёнка (и всё, что с ним схлопнулось) обрезано, наружу
        // примыкать нечему. Без этого поле 50 ребёнка уходило через контейнер
        // в `body`, и страница съезжала на 50 (`block-container-non-adjoining-item`).
        if top_edge_open(ch) && ch.style.margin_trim & 1 == 0 {
            // Содержащий блок детей `ch` — сам `ch`: его ширина точками —
            // оценка для `bfc_no_fit` на спуске.
            let cb_prev = CB_WIDTH.get();
            if let Some(Len::Px(w)) = ch.style.width
                && w > 0.0
            {
                CB_WIDTH.set(Some(w));
            }
            // Внуки меряют доли от содержимого `ch`, а не от уровня (§10.1 п.2).
            let inner = with_inner_cb(&ch.style, || leading_chain(&ch.children, path, eat));
            CB_WIDTH.set(cb_prev);
            s = adjoin(s, inner?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Хвостовая цепочка примыкающих НИЖНИХ полей — зеркало `leading_chain`.
///
/// В схлопывании она НЕ участвует: попытка поднимать её наружу замерена и
/// откачена (см. комментарий в `collapse_margins`, CSS2 4684 -> 4594) —
/// вглубь подъём уходил мимо ещё неизвестной высоты родителя. Для
/// `margin-trim: block-end` этой опасности нет: наружу ничего не поднимается,
/// поля только гасятся, и цепочка ограничена последним ребёнком в потоке.
pub(super) fn trailing_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    for (i, c) in children.iter().enumerate().rev() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        if ch.inline && !inline_marked_block(ch) {
            if ch.children.is_empty()
                && !replaced_inline(&ch.tag)
                && !inline_axis_edges(&ch.style)
            {
                continue;
            }
            return Some(s);
        }
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        if !in_flow(&ch.style) {
            continue;
        }
        s = adjoin(
            s,
            strut_of(margin_or_bail(ch.style.margin.bottom, &ch.style)?),
        );
        path.push(i);
        if let Some(t) = through_strut(ch) {
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            continue;
        }
        eat.push((path.clone(), false));
        // Заданная высота или нижняя рамка/отступ отрезают цепочку: поле
        // внука к краю контейнера уже не примыкает.
        if ch.style.height.is_none() && top_edge_open(ch) {
            s = adjoin(s, trailing_chain(&ch.children, path, eat)?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Обнулить поле по пути: наружу оно ушло одним полем родителя, и раскладка
/// сложила бы его второй раз. `deep` — коробка схлопнулась насквозь: чистится
/// она сама с обеих сторон.
pub(super) fn zero_at(children: &mut [Node], path: &[usize], top: bool, deep: bool) {
    let Some((&i, rest)) = path.split_first() else {
        return;
    };
    let Some(Node::Element(ch)) = children.get_mut(i) else {
        return;
    };
    if !rest.is_empty() {
        zero_at(&mut ch.children, rest, top, deep);
        return;
    }
    pin_inherited_margins(ch, top || deep, !top || deep);
    if deep {
        ch.style.margin.top = Some(Len::Px(0.0));
        ch.style.margin.bottom = Some(Len::Px(0.0));
    } else if top {
        ch.style.margin.top = Some(Len::Px(0.0));
    } else {
        ch.style.margin.bottom = Some(Len::Px(0.0));
    }
}

/// `margin: inherit` берёт ВЫЧИСЛЕННОЕ поле родителя (CSS 2.1 §6.2.1: «the
/// property takes the same computed value as the property for the element's
/// parent»). Схлопывание же переписывает поле родителя (обнуляет ушедшее
/// наружу, пишет остаток струны) РАНЬШЕ, чем до ребёнка доходит наследование
/// (`inline::inherit` живёт ниже по пути), — и ребёнок наследовал used-ноль
/// вместо написанного: `margin-bottom-113` (низ `#wrapper` ушёл в `body`),
/// `margin-top-113` (остаток 80 вместо 96), `margin-em-inherit-001` (верх
/// `#parent` съеден цепочкой `#grand-parent`). Поэтому до перезаписи
/// наследующие дети получают поле как есть и флаг снимается: шрифтовые
/// единицы — в точки по кеглю РОДИТЕЛЯ (наследуется вычисленная длина, «not
/// 80px 120px 40px 160px»), доля остаётся долей и решается от своего
/// содержащего блока (`margin-percentage-inherit-001`: 15% от 200, а не 60).
pub(super) fn pin_inherited_margins(e: &mut Element, top: bool, bottom: bool) {
    let own = e.style.margin;
    for n in e.children.iter_mut() {
        let Node::Element(ch) = n else { continue };
        for (i, want, src) in [(0usize, top, own.top), (2, bottom, own.bottom)] {
            if !want || !ch.style.margin_inherit[i] {
                continue;
            }
            let pinned = match src {
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                    margin_px(Some(l), &e.style).map(Len::Px).or(Some(l))
                }
                other => other,
            };
            if i == 0 {
                ch.style.margin.top = pinned;
            } else {
                ch.style.margin.bottom = pinned;
            }
            ch.style.margin_inherit[i] = false;
        }
    }
}

/// Отступ в точках для схлопывания.
///
/// Схлопывание идёт ДО каскада размеров шрифта, а разметка пишет `margin: 1em 0`
/// не реже, чем в точках: без перевода правило не срабатывало ни разу на таких
/// отступах, и блоки уезжали вниз на целый отступ. Кегль берётся свой, если
/// элемент его задал, иначе базовый — унаследованного здесь ещё нет.
/// Проценты не переводятся: они считаются от ширины родителя, а её тут никто
/// не знает, и выдуманное число было бы хуже пропуска.
pub(crate) fn margin_px(l: Option<Len>, style: &Computed) -> Option<f32> {
    // Кегль элемента: свой, если задан, иначе унаследованный от уровня
    // (см. `COLLAPSE_FONT_PX`). Прежде вместо унаследованного брались
    // постоянные 16 точек, и `table{font-size:50px} div{margin:1em 0}`
    // схлопывался по 16 вместо 50 — вся семья Hixie `margin-collapse-1xx`
    // расходилась с эталоном ровно на эту разницу.
    let parent = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
    let base = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * parent,
        Some(Len::Pct(k)) => k * parent,
        _ => parent,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * base),
        // Единицы шрифта, считающиеся по МЕТРИКЕ гарнитуры: сюда они доезжают
        // неразрешёнными, потому что `resolve_em` живёт в наследовании
        // (`inline::inherit`), а схлопывание идёт раньше. Прежде `-6ex`
        // отдавало `None`, поле пропадало целиком (`positioning/top-091`).
        l @ (Len::Ch(_) | Len::Ex(_)) => Some(crate::text::metrics::spacing_px(
            Some(l),
            &style.font_family.clone().unwrap_or_default(),
            base,
        )),
        // Процент — от ширины содержащего блока, когда она известна в точках
        // (`margin-top-103`, `margin-bottom-113`); иначе поле пропускается.
        Len::Pct(k) => COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get).map(|w| k * w),
        _ => None,
    }
}
