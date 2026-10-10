//! Схлопывание полей в потоке (CSS 2.1 §8.3.1).
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::containing::with_inner_cb;
use crate::layout::block::struts::{
    Strut, adjoin, first_in_flow, leading_chain, margin_px, pin_inherited_margins, solve, strut_of,
    through_strut, through_strut_no_clear, trailing_chain, zero_at,
};
use crate::layout::block::{margin_edges, margin_height, margin_inline_boxes};
use crate::layout::float::band_clearance;
use crate::layout::table::anon::wrap_anon_tables;
use crate::render::{
    in_flow, inline_level, inline_level_box, is_blank, own_context, split_block_in_inline,
};
use crate::style::computed::Display;
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

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
pub(super) fn collapse_flow_margins(
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
fn absorb_margin(e: &mut Element, tail_side: bool, reverse: bool) -> f32 {
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

fn collapse_flow_kids(reverse: bool, out: &mut [Node], mut trailing: Option<f32>) {
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
fn collapses_through(e: &Element) -> bool {
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

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
/// `abs_parent` — родитель абсолютно позиционирован: по §10.6.7 его
/// автовысота ВКЛЮЧАЕТ плавающих детей, и правило «блок из одних флоатов
/// высотой ноль» (§10.6.3) к его детям не применяется.
pub(crate) fn collapse_margins(nodes: &[Node], abs_parent: bool) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    margin_inline_boxes::prepare(&mut out);
    // CSS 2.1 §10.6.3: floats do not contribute to ordinary auto height.
    // Margin collapse proves zero in-flow height for an open empty block;
    // The contextual proof also handles borders, padding and white-space.
    // Formatting contexts retain floats (§10.6.7). Keep the absolute-parent
    // guard: its float containment currently depends on the child's height.
    for node in out.iter_mut().filter(|_| !abs_parent) {
        let Node::Element(e) = node else { continue };
        let has_float = e
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(c) if c.style.float.is_some_and(|f| f != 0)));
        if has_float && through_strut_no_clear(e).is_some() {
            e.style.height = Some(Len::Px(0.0));
        }
    }
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    collapse_kid_margins(&mut out);
    // Струна примыкающих полей соседей (§8.3.1). `emitted` — сколько точек уже
    // ЗАПИСАНО в стили этого зазора: раскладка складывает поля сама, и
    // верхнему полю следующего блока достаётся только разница.
    let mut strut: Option<Strut> = None;
    let mut emitted = 0.0f32;
    // Последняя коробка прогона с клиренсом: её остаток поля остаётся ВНУТРИ
    // родителя и наружу не уходит.
    let mut cleared_run: Option<usize> = None;
    emit_margin_struts(&mut out, &mut strut, &mut emitted, &mut cleared_run);
    // Прогон кончился на коробке с клиренсом: остаток слитого поля пишется ей
    // самой — родителя он растит, но наружу не выходит.
    if let (Some(i), Some(s)) = (cleared_run, strut) {
        let rest = solve(s) - emitted;
        if rest > 0.0
            && let Some(Node::Element(e)) = out.get_mut(i)
        {
            e.style.margin.bottom = Some(Len::Px(rest));
        }
    }
    out
}

fn collapse_kid_margins(out: &mut [Node]) {
    for node in out.iter_mut() {
        let Node::Element(e) = node else { continue };
        if inline_level_box(e) {
            continue;
        }
        // Отступ не протекает наружу и через край блока с собственным
        // контекстом: прокрутка, обрезка, гибкая раскладка, сетка,
        // позиционирование. Раньше учитывались только рамка и внутренний
        // отступ, и содержимое прокручиваемой панели вставало на 6-8 точек
        // выше браузерного.
        let own_context = own_context(e);
        // Отсечка по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и
        // `border: 0` схлопыванию не мешают (CSS 2.1 §8.3.1).
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
        // `margin-trim` (css-box-4 §margin-trim-block): поле первого/последнего
        // ребёнка у ВНУТРЕННЕГО края контейнера обнуляется вместе со всем,
        // что с ним схлопнулось. Идёт ДО гейта схлопывания: обрезка работает
        // и когда край закрыт рамкой или внутренним отступом — там поле
        // наружу не уходит, но обрезать его всё равно надо.
        // Собственное поле контейнера не трогается («but not its own»).
        // Блок внутри строчного (`<span><div>…</div></span>`) — тоже первый/
        // последний потоковый ребёнок контейнера: строчный рвётся на
        // анонимные коробки, а пустые куски коробок не дают (CSS 2.1
        // §9.2.1.1). Разрыв делает `blocks()` уже ПОСЛЕ этого шага, и цепочки
        // обрезки упирались в `<span>` как в строчную коробку
        // (`block-container-block-in-inline-001…007`). Контейнеру с обрезкой
        // рвём заранее тем же путём, что и `blocks()` (`wrap_anon_tables` →
        // `split_block_in_inline`); повторный разрыв там ничего не меняет.
        // У гибкого контейнера и сетки разрыва нет (`ordered_context`).
        if e.style.margin_trim & 3 != 0
            && !matches!(
                e.style.display,
                Some(Display::Flex)
                    | Some(Display::InlineFlex)
                    | Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::GridLanes)
            )
        {
            e.children = split_block_in_inline(&wrap_anon_tables(&e.children));
        }
        if e.style.margin_trim & 1 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if leading_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, true, *deep);
                }
            }
        }
        if e.style.margin_trim & 2 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if trailing_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, false, *deep);
                }
            }
        }
        // Root margins do not collapse with their children (CSS 2.1 §8.3.1).
        if e.tag == "html" {
            continue;
        }
        margin_edges::collapse_top(e);
        // То же СНИЗУ: отступ последнего ребёнка протекает наружу, если
        // родителя от него не отделяют ни рамка, ни внутренний отступ, ни
        // заданная высота. Иначе следующий за родителем блок отодвигался на
        // сумму двух отступов вместо большего из них
        // (`text-align-end-015`: вторая коробка стояла на 20 точек ниже).
        // ЗАМЕРЕНО И ОТКАЧЕНО: считать снизу ВСЮ хвостовую цепочку, зеркально
        // верхней (`trailing_chain` + `zero_at(.., false, ..)`). CSS2 4684 ->
        // 4594: прибавка 4 (`margin-collapse-101/105`, два
        // `inline-formatting-context`) против 94 потерь. Наверху цепочку
        // ограничивает первый ребёнок с содержимым, а внизу ограничитель —
        // ВЫСОТА родителя, которой на этом шаге ещё нет: подъём уходит вглубь
        // и снимает поля там, где родитель на деле уже кончился. Возвращать
        // вместе с настоящей проверкой итоговой высоты (тот же блокер, что у
        // `min-height` ниже).
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: снять отсюда `min-height`, потому что по
        // спеке подъём закрывает не написанное свойство, а РАСХОЖДЕНИЕ
        // итоговой высоты с высотой по содержимому (CSS 2.1 §8.3.1, так же
        // считает Blink). Замерено: oldfront 2332 -> 2328. Нашей раскладке
        // «дотянулась ли высота» на этом шаге ещё не известно, и снятие
        // условия открывало подъём там, где высота на деле выросла.
        // Возвращать вместе с настоящей проверкой итоговой высоты.
        // `min-height` закрывает подъём, только если он ДЕЙСТВИТЕЛЬНО тянет
        // коробку выше её содержимого (§8.3.1 говорит о РАСХОЖДЕНИИ итоговой
        // высоты с высотой по содержимому, а не о написанном свойстве).
        // Нижняя оценка содержимого — сумма разрешимых в точки высот блочных
        // детей в потоке; неизвестная высота хотя бы у одного оставляет
        // прежний запрет. Замерено: CSS2 5074 -> 5077, oldfront 2353 -> 2352
        // (`css-flexbox-height-animation-stretch` 0.47 -> 1.00).
        let raises = margin_height::raises(e);
        // Край закрыт СВОИМИ свойствами: поле ребёнка остаётся внутри и
        // трогать его нечем.
        // Высота, которая «behaves as auto» (css-sizing-3: прозу CSS2
        // «computes to auto» читать так), подъёму не мешает: ключевые слова
        // содержимого по блочной оси блок-контейнера и доля при
        // НЕОПРЕДЕЛЁННОЙ высоте блока (§10.5), включая `stretch` — у нас это
        // та же доля. Прежде поле ребёнка оставалось внутри, и красная
        // подложка росла на 100 (`margin-collapse-with-indefinite-block-
        // size-001…005`). Blink — `BlockLengthUnresolvable`.
        let behaves_auto = match e.style.height {
            None | Some(Len::Auto) => true,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent) => true,
            Some(Len::Pct(_)) => !COLLAPSE_CB_HEIGHT_DEF.with(std::cell::Cell::get),
            _ => false,
        };
        if !zero(e.style.padding.bottom)
            || !zero(e.style.borders().bottom)
            || !behaves_auto
            || margin_height::separate(e)
            || own_context
        {
            continue;
        }
        // Снизу плавающий ИЛИ АБСОЛЮТНЫЙ ребёнок ЗАКРЫВАЕТ подъём: float
        // заякорен в потоке после последнего блока (box-shadow-overlapping-002),
        // а статическая позиция абсолютного считается от места в потоке —
        // утёкший отступ поднимал их обоих (z-index-015: квадрат вставал
        // на отступ параграфа выше эталона).
        let child_bottom =
            first_in_flow(e.children.iter().enumerate().rev().take_while(|(_, c)| {
                !matches!(c, Node::Element(ch)
                if ch.style.float.is_some()
                    || matches!(
                        ch.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ))
            }))
            .and_then(|(i, ch)| {
                with_inner_cb(&e.style, || margin_px(ch.style.margin.bottom, &ch.style)).map(|_| i)
            });
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): запрет поглощения, когда в хвосте
        // есть коробка с клиренсом (CSS 2.1 §8.3.1, «does not collapse with a top
        // margin that has clearance»): срез CSS2 6204 пары, 5691 -> 5691 (+0/-0),
        // целевые `margin-collapse-clear-012/-013` как были «красное видно»,
        // так и остались. Значит потеря не здесь: до этого места дело либо не
        // доходит (гейты выше), либо `child_bottom` уже `None` — искать
        // надо во втором проходе (`cleared_run`).
        if let Some(i) = child_bottom {
            // Минимальная высота выше содержимого: поле последнего ребёнка
            // ПРИМЫКАЕТ к его нижнему краю (§8.3.1), но наружу не идёт и
            // родителя не растит — низ родителя решает `min-height` (§10.6.3).
            // Третьего исхода не было вовсе: поле оставалось внутри и место
            // занимало.
            if raises {
                if let Node::Element(ch) = &mut e.children[i] {
                    pin_inherited_margins(ch, false, true);
                    ch.style.margin.bottom = Some(Len::Px(0.0));
                }
                continue;
            }
            margin_edges::collapse_bottom(e);
        }
    }
}

fn emit_margin_struts(
    out: &mut [Node],
    strut: &mut Option<(f32, f32)>,
    emitted: &mut f32,
    cleared_run: &mut Option<usize>,
) {
    for (idx, node) in out.iter_mut().enumerate() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу.
            if matches!(node, Node::Text(t) if blank_text(t)) {
                continue;
            }
            *strut = None;
            continue;
        };
        // Строчный элемент С СОДЕРЖИМЫМ порождает строчную коробку, и поля
        // блоков через неё уже не примыкают.
        if inline_level_box(e) {
            if !e.children.is_empty() {
                *strut = None;
            }
            continue;
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: обрывать струну на атомарном строчном
        // (`inline-block` и родня в потоке рождают строчную коробку). CSS2
        // -38, вся потеря — семья `bidi-box-model-*`: там такой сосед стоит
        // между блоками сплошь, и разрыв струны разводит их полями врозь.
        // Возвращаться вместе с настоящей строчной коробкой в раскладке.
        if !in_flow(&e.style) {
            band_clearance::remember_float_margin(e, *strut, *emitted);
            continue;
        }
        let top = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
        let bottom = margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0);
        let through = through_strut(e);
        let mut merged = match *strut {
            Some(s) => {
                let m = adjoin(s, strut_of(top));
                // Верхний край насквозь-схлопнутой коробки встаёт там, где
                // разрешается струна до неё вместе с её верхним полем. Та же
                // строка даёт это и обычной коробке: нижнее поле соседа
                // раскладка уже поставила, верхнему достаётся разница.
                pin_inherited_margins(e, true, false);
                e.style.margin.top = Some(Len::Px(solve(m) - *emitted));
                *emitted = solve(m);
                m
            }
            None => {
                // Примыкать не к чему: верхнее поле остаётся как написано.
                *emitted = top;
                strut_of(top)
            }
        };
        // Коробка с клиренсом, которая иначе схлопнулась бы насквозь
        // (§8.3.1): её поля СЛИВАЮТСЯ между собой, но получившееся поле не
        // схлопывается с нижним полем родителя — «these margins collapse with
        // the adjoining margins of following siblings but the resulting margin
        // does not collapse with the bottom margin of the parent block».
        // Верхнее поле уже выложено рядом обтекания, поэтому наружу идёт
        // только остаток.
        if through.is_none() && e.style.clear.is_some() && through_strut_no_clear(e).is_some() {
            *emitted = top;
            pin_inherited_margins(e, false, true);
            e.style.margin.bottom = Some(Len::Px(0.0));
            *strut = Some(adjoin(strut_of(top), strut_of(bottom)));
            *cleared_run = Some(idx);
            continue;
        }
        if let Some(own) = through {
            merged = adjoin(merged, own);
            // Своё нижнее поле коробка не ставит: оно ушло в струну, и
            // раскладка сложила бы его второй раз.
            pin_inherited_margins(e, false, true);
            e.style.margin.bottom = Some(Len::Px(0.0));
            // ЗАМЕРЕНО И ОТКАЧЕНО: снимать поля и у ДЕТЕЙ насквозь-коробки
            // (`zero_margins_deep`) — CSS2 4675 -> 4672, потери
            // `floats-clear/margin-collapse-033/034/035`, прибавки нет. Поля
            // детей уже учтены струной, но раскладка ставит саму коробку не
            // по струне, а по своим полям — снятие уводит её вверх.
            *strut = Some(merged);
            continue;
        }
        *strut = Some(strut_of(bottom));
        *emitted = bottom;
        *cleared_run = None;
    }
}

thread_local! {
    /// Кегль РОДИТЕЛЯ на разбираемом уровне: единицы шрифта в отступах
    /// меряются от кегля элемента, а он к моменту схлопывания ещё не
    /// унаследован — наследование живёт ниже по пути (`inline::inherit`).
    /// Значение ставит `blocks()` вокруг вызова `collapse_margins` и
    /// возвращает на место после него.
    pub(crate) static COLLAPSE_FONT_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(16.0) };
    /// Ширина содержащего блока уровня схлопывания (для процентных полей).
    pub(crate) static COLLAPSE_CB_WIDTH_PX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
    /// Определена ли высота содержащего блока уровня схлопывания (§10.5):
    /// доля высоты ребёнка при неопределённой ведёт себя как `auto`.
    pub(crate) static COLLAPSE_CB_HEIGHT_DEF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next `blocks` call lays out a table cell's or caption's content:
    /// both are block formatting context roots (CSS 2.1 §9.4.1) and contain
    /// their floats (§10.6.7), even as a `td`/`caption` without `display`.
    pub(crate) static CELL_BFC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
