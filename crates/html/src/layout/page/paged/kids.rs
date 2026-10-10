//! Дети листов из групп корневых узлов: копии ICB/fixed, имена страниц, формы.

use super::PAGE_COPIES;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_break, oof_reach, page_monolith};
use crate::layout::fragment::flex_lines::{class_a_box, inline_display};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::page::names::page_names;
use crate::layout::page::paged::FIXED_LAYER;
use crate::render::{RenderOpts, blocks, is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn page_kids_from_groups(
    opts: &RenderOpts,
    root: &Computed,
    left: f32,
    right: f32,
    top: f32,
    root_page: String,
    shape_cx: ShapeCx,
    icb_copies: &mut [Vec<AnyElement>],
    fixed_copies: &mut [Vec<AnyElement>],
    icb_reach: &mut f32,
    kids: &mut Vec<super::super::page_stack::PageKid>,
    mut prev_end: Option<String>,
    mut first: bool,
    ah: f32,
    layer: impl Fn(Vec<AnyElement>) -> Vec<AnyElement>,
    floated: impl Fn(&Node) -> bool,
    groups: Vec<Vec<Node>>,
) {
    for group in &groups {
        let n = &group[0];
        if let Node::Element(e) = n
            && matches!(e.style.display, Some(Display::None))
        {
            continue;
        }
        if let Node::Element(e) = n
            && out_of_flow(&e.style)
        {
            *icb_reach = icb_reach.max(oof_reach(e, shape_cx));
        }
        let pad_top = if first { top } else { 0.0 };
        first = false;
        let build = |slot: &mut Vec<AnyElement>, fixed_slot: &mut Vec<AnyElement>| {
            crate::layout::positioned::containing_block::icb_open();
            FIXED_LAYER.with(|f| f.borrow_mut().clear());
            // Оставленная обёртка (`html`/`body`) с долей высоты считает её от
            // page area — содержащего блока корня (css-page-3 §page-model).
            let mut wrap = div().pl(px(left)).pr(px(right)).pt(px(pad_top));
            if let [Node::Element(r)] = group.as_slice()
                && matches!(r.tag.as_str(), "html" | "body")
                && matches!(r.style.height, Some(Len::Pct(_)))
            {
                wrap = wrap.h(px(ah));
            }
            let el = wrap.children(blocks(group, root, opts)).into_any_element();
            slot.extend(layer(
                crate::layout::positioned::containing_block::icb_close(),
            ));
            fixed_slot.extend(layer(
                FIXED_LAYER.with(|f| std::mem::take(&mut *f.borrow_mut())),
            ));
            el
        };
        let el = build(&mut icb_copies[0], &mut fixed_copies[0]);
        let frags: Vec<AnyElement> = (1..PAGE_COPIES)
            .map(|i| build(&mut icb_copies[i], &mut fixed_copies[i]))
            .collect();
        // Анонимный блок вокруг текста/строчного — коробка в потоке со
        // значением `page` родителя; флоат и внепоточный в сравнении имён
        // не участвуют (свойство к ним не применяется, §named pages п.2).
        // Коробка класса A — по `display`, не по тегу (`page-name-img-004`:
        // `<img style="display: block; page: b">` шла анонимным блоком с
        // именем корня и рвала страницу). Разрыв первого/последнего ребёнка
        // передаётся коробке (css-break-4 §break-propagation; Blink
        // `InitialBreakBefore`): `block-page-break-inside-avoid-8-ref` —
        // `<div><p style="page-break-before: always">` рвал не перед `div`.
        let (fb, fa, monolith, renamed, start_name) =
            group_break_names(&root_page, &mut prev_end, &floated, group, n);
        // Мера поддерева — те же точки разреза, что у колонок. Обёртка
        // первого ребёнка несёт отбивку корня сверху (`pad_top`): все
        // смещения меры сдвигаются на неё, а высота растёт.
        // Внепоточный корня места в стопке не занимает (CSS 2.1 §9.3.1):
        // его мера — обёртка (0); иначе абсолют `height: 4in` двигал соседей
        // на 384 (`monolithic-overflow-027`, абсолют `fixedpos-007` — 720).
        // Флоат: не влезший MARGIN box уходит на следующую страницу целиком
        // (`float-with-large-margin-bottom-cross-page-002`: эталон —
        // `break-before: page`), нижнее поле — часть его меры.
        // Флоат корня — ребёнок стопки СО СВОЕЙ мерой: css-break-4 §3.1
        // «User agents should also apply these properties to floated boxes
        // whose containing block is in the normal flow of the root fragmented
        // element». Гейт `out_of_flow` отнимал у него меру вместе с
        // абсолютами: таблица `float: left; break-inside: avoid` выше листа
        // резалась срезом по краю сквозь абзац вместо точки класса A между
        // абзацами ячейки (`float-page-break-inside-avoid-1-print` против
        // эталона с обычной таблицей), а ветка `h + mb` ниже была мёртвой.
        let (margins, shape) = group_shape(root, shape_cx, group, n, pad_top);
        // Монолит выше листа решается в `fill` (правило «сначала перенос,
        // потом разрыв внутри»): здесь мера считается и для него — точки
        // класса A нужны, когда он окажется с верха страницы.
        kids.push(crate::layout::page::page_stack::PageKid {
            el,
            frags,
            monolith,
            force_before: fb || renamed,
            force_after: fa,
            shape,
            page: start_name,
            mt: margins.0,
            mb: margins.1,
            inner_top: margins.2,
        });
    }
}

pub(super) fn group_break_names(
    root_page: &str,
    prev_end: &mut Option<String>,
    floated: impl Fn(&Node) -> bool,
    group: &[Node],
    n: &Node,
) -> (bool, bool, bool, bool, String) {
    let (monolith, fb, fa, names) = match n {
        Node::Element(e) if class_a_box(e) => (
            page_monolith(e),
            edge_break(e, false),
            edge_break(e, true),
            Some(page_names(e, root_page)),
        ),
        Node::Element(e) if !e.inline && !inline_display(e) => (
            page_monolith(e),
            edge_break(e, false),
            edge_break(e, true),
            None,
        ),
        _ => (
            false,
            false,
            false,
            Some((root_page.to_string(), root_page.to_string())),
        ),
    };
    // Группа флоата: сам флоат имени не передаёт (§named pages п. 2), но
    // поточные коробки класса A в группе — передают конец — у последней (`page-name-000-print`: флоат, `clear`-блок
    // страницы `foo` и следом блок страницы `bar` — разрыв перед `bar`).
    let names = match (&names, n) {
        (None, Node::Element(e)) if floated(n) && !class_a_box(e) => {
            let named: Vec<(String, String)> = group
                .iter()
                .filter_map(|g| match g {
                    Node::Element(k) if class_a_box(k) => Some(page_names(k, root_page)),
                    _ => None,
                })
                .collect();
            // Начало группы — продолжение предыдущей: разрыв перед флоатом
            // увёл бы и его (`page-name-float-002-print`: флоат `b` остаётся
            // на листе `a`).
            let start = prev_end.clone().unwrap_or_else(|| root_page.to_string());
            named.last().map(|l| (start, l.1.clone()))
        }
        _ => names,
    };
    // Группа из нескольких узлов монолитом не бывает: её режет край листа.
    let monolith = monolith && group.iter().filter(|g| !is_blank(g)).count() == 1;
    let renamed = match (&*prev_end, &names) {
        (Some(p), Some((start, _))) => p != start,
        _ => false,
    };
    // Имя, с которого коробка начинается: своё у коробки класса A, иначе
    // — конец предыдущей (анонимный блок и прочие продолжают страницу).
    let start_name = match &names {
        Some((start, _)) => start.clone(),
        None => prev_end.clone().unwrap_or_else(|| root_page.to_string()),
    };
    if let Some((_, end)) = &names {
        *prev_end = Some(end.clone());
    }
    (fb, fa, monolith, renamed, start_name)
}

pub(super) fn group_shape(
    root: &Computed,
    shape_cx: ShapeCx,
    group: &[Node],
    n: &Node,
    pad_top: f32,
) -> (
    (f32, f32, f32),
    Option<(f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>,
) {
    let positioned = |e: &Element| {
        matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    };
    // Поля ребёнка: мера `shape_full` — border box, а обёртка рисует его
    // со смещением на верхнее поле. Прежде поле выбрасывалось, и маска
    // фрагмента высотой в border box резала нарисованное ниже поля
    // (`page-left-right-001-print-ref`: `margin-top: 200px` у блока 100 —
    // жёлтого квадрата не было вовсе). Теперь поля уходят в `Kid.mt/mb`
    // (схлопывание соседей — в `fill`), а копия поднимается на смещение
    // border box внутри обёртки (`PageKid::inner_top`). У первого ребёнка
    // с отбивкой корня и у флоата (его поля не схлопываются, CSS 2.1
    // §8.3.1) поля — часть самой меры.
    let mut margins = (0.0f32, 0.0f32, 0.0f32);
    let shape = match n {
        _ if group.iter().filter(|g| !is_blank(g)).count() > 1 => None,
        Node::Element(e) if !e.inline && !positioned(e) => {
            shape_full(e, 4, shape_cx).map(|(h, mt, mb, mut cuts, mut forced, mut solid)| {
                let floated = e.style.float.unwrap_or(0) != 0;
                // Письмо — своё или унаследованное от корня (`html, body {
                // writing-mode }` снимаются выше, свой стиль ребёнка его не
                // несёт).
                let vertical = e.style.vertical.or(root.vertical) == Some(true);
                let fold = pad_top > 0.0 || floated;
                let lead = if vertical {
                    pad_top
                } else if fold {
                    pad_top + mt
                } else {
                    0.0
                };
                if lead != 0.0 {
                    for c in cuts.iter_mut() {
                        c.0 += lead;
                        c.1 += lead;
                    }
                    for f in forced.iter_mut() {
                        *f += lead;
                    }
                    for r in solid.iter_mut() {
                        r.0 += lead;
                        r.1 += lead;
                    }
                }
                // Вертикальное письмо: поля меры — по блочной оси письма, не
                // по высоте стопки; прежнее поведение (`block-001-wm-vlr/vrl`:
                // `margin-inline-start` сверху — 0.40 -> 0.88 с полями).
                if vertical {
                    let h = if floated { h + mb } else { h };
                    return (h + pad_top, cuts, forced, solid);
                }
                if fold {
                    let tail = if floated { mb } else { 0.0 };
                    margins = (0.0, if floated { 0.0 } else { mb }, 0.0);
                    (h + lead + tail, cuts, forced, solid)
                } else {
                    margins = (mt, mb, mt);
                    (h, cuts, forced, solid)
                }
            })
        }
        _ => None,
    };
    (margins, shape)
}
