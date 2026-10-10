//! Постраничная отрисовка документа.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_break, oof_reach, page_monolith};
use crate::layout::fragment::flex_lines::{class_a_box, inline_display};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::page::names::{
    PageMarginDeclsFn, fill_used_page, first_kid_page_name, hoist_named_wrappers, page_names,
};
use crate::layout::page::{page_boxes, page_counters};
use crate::layout::replaced::iframe::IFRAME_DEPTH;
use crate::paint::effects::mask::collect_mask_defs;
use crate::render::{RenderOpts, blocks, is_blank, out_of_flow};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// Копий ребёнка в стопке страниц — потолок числа страниц, на которые может
/// растянуться один блок верхнего уровня (в `css-page` не больше шести).
const PAGE_COPIES: usize = 12;

thread_local! {
    /// Строится стопка страниц. Абсолют корня БЕЗ заданных сторон тоже уходит
    /// в слой ICB: на месте он остаётся внутри обёртки кида и режется маской
    /// её фрагмента (`monolithic-overflow-013`: четыре пустых листа). Экрану
    /// это не нужно — там статическая позиция и есть место в потоке.
    pub(crate) static PAGED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Слой `position: fixed` текущей сборки кида — по копии на лист без
    /// сдвига (`flow::PageStack::fixed`). В слой ICB ему нельзя: тот
    /// поднимается на `p` page area и показывает фиксированный только на
    /// первом листе (`fixedpos-007..009`).
    pub(crate) static FIXED_LAYER: std::cell::RefCell<Vec<AnyElement>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Постраничная отрисовка (css-page-3): блоки верхнего уровня документа —
/// дети стопки страниц, page area каждой страницы — фрагментаинер.
///
/// Обёртки `html`/`body` снимаются здесь (сборщик оставляет их при
/// собственной коробке, `doc.rs::has_box_style`): их фон — канвас документа
/// (§painting, слой 2), боковые поля/рамки/отступы — сдвиг содержимого на
/// КАЖДОЙ странице, верхнее — только на первой: коробка тела режется по
/// страницам вместе с содержимым, а её `page` — умолчание имени для детей.
pub fn render_paged(
    nodes: &[Node],
    opts: &RenderOpts,
    geom_for: crate::layout::page::page_stack::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
) -> AnyElement {
    render_paged_select(nodes, opts, geom_for, margin_decls, None)
}

/// `render_paged`, показывающий только листы `select` (номера с нуля).
pub fn render_paged_select(
    nodes: &[Node],
    opts: &RenderOpts,
    geom_for: crate::layout::page::page_stack::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
    select: Option<Vec<usize>>,
) -> AnyElement {
    // Снятые обёртки и корень без коробки правят КАЖДЫЙ лист одинаково:
    // `none` — пустой лист без свойств `@page`, `canvas` — фон `html`/`body`.
    let mut none = false;
    let mut canvas: Option<gpui::Hsla> = None;
    let mut root = opts.root_style();
    let document_counters = page_counters::PageCounters::from_document(nodes);
    crate::interactive::frame::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let mut nodes: Vec<Node> = nodes.to_vec();
    let (mut left, mut right, mut top) = (0.0f32, 0.0f32, 0.0f32);
    let mut root_page = String::new();
    resolve_root_page(
        &mut none,
        &mut canvas,
        &mut root,
        &mut nodes,
        &mut left,
        &mut right,
        &mut top,
        &mut root_page,
    );
    fill_used_page(&mut nodes, &root_page);
    // Обёртка в ином режиме письма — ортогональный поток, монолит (css-break-3
    // §4.1): снимать её нельзя, и у вертикального корня обёртки не снимаются.
    if root.vertical != Some(true) {
        hoist_named_wrappers(&mut nodes);
    }
    let geom_for: crate::layout::page::page_stack::PageGeomFn =
        std::rc::Rc::new(move |i, name: &str| {
            let mut g = geom_for(i, name);
            if none {
                g.bg = gpui::white();
                g.border.0 = 0.0;
                g.canvas = None;
            } else if canvas.is_some() {
                g.canvas = canvas;
            }
            g
        });
    let geom = geom_for(0, &first_kid_page_name(&nodes, &root_page));
    // Мера для страниц: `contain: size` — монолит (как в `page_monolith`),
    // `vh`/`vw` — от page area (в сыром `e.style` они ещё не разрешены:
    // `resolve_viewport` работает на копии внутри `element()`).
    let shape_cx = ShapeCx {
        paged: true,
        viewport: Some(opts.viewport),
        cell_pad: None,
        unclamped: false,
    };
    // Слой ICB — по КОПИИ на страницу: каждая сборка ребёнка рождает свои
    // `LatePlace` абсолютов, копия `c` уходит на лист `c` (`PageStack.icb`).
    let mut icb_copies: Vec<Vec<AnyElement>> = (0..PAGE_COPIES).map(|_| Vec::new()).collect();
    let mut fixed_copies: Vec<Vec<AnyElement>> = (0..PAGE_COPIES).map(|_| Vec::new()).collect();
    let mut icb_reach = 0.0f32;
    let mut kids: Vec<crate::layout::page::page_stack::PageKid> = Vec::new();
    let mut prev_end: Option<String> = None;
    let mut first = true;
    PAGED.with(|p| p.set(true));
    // Слой — коробка размером с page area: `layout_as_root` кладёт корень в
    // `(0, 0)` (taffy `compute_root_layout`, `location: Point::ZERO`), и
    // `top`/`bottom` абсолюта без обёртки пропадали (эталоны `fixedpos-*`:
    // `top: 100vh` — все копии в начале первого листа). `LatePlace` своей
    // коробки не заводит, содержащим блоком становится эта обёртка.
    let (aw, ah) = geom.area;
    let layer = |els: Vec<AnyElement>| -> Vec<AnyElement> {
        if els.is_empty() {
            return els;
        }
        vec![
            div()
                .relative()
                .w(px(aw))
                .h(px(ah))
                .children(els)
                .into_any_element(),
        ]
    };
    // Строчное содержимое корня — ОДИН анонимный блок (CSS 2.1 §9.2.1.1:
    // «any inline-level content … is wrapped in an anonymous block box»):
    // соседние текст и строчные элементы вместе с пробелами между ними идут
    // одним ребёнком стопки. Прежде каждый узел был своим ребёнком, и
    // `This page should <em>not</em> have a blue box.` вставал тремя
    // строками (`fixedpos-010-print`, мера `heights=[21.6, 21.6, 21.6, …]`).
    let inline_level = |n: &Node| match n {
        Node::Text(_) => true,
        Node::Element(e) => {
            (e.inline || inline_display(e))
                && !out_of_flow(&e.style)
                && !matches!(
                    e.style.display,
                    Some(Display::Block)
                        | Some(Display::Flex)
                        | Some(Display::Grid)
                        | Some(Display::Table)
                        | Some(Display::ListItem)
                        | Some(Display::None)
                )
        }
    };
    // Флоат корня с ПОСЛЕДУЮЩИМ содержимым — тоже один ребёнок: соседний блок
    // и строки обтекают флоат или очищаются от него (CSS 2.1 §9.5), а дети
    // стопки раскладываются порознь, и флоат вставал над соседом, а не рядом
    // (`monolithic-overflow-020-print`: флоат справа и жёлтый блок — обе
    // стороны пары столбиком; `content-001-print-ref`: два флоата и
    // `clear: both`). Группа флоата закрывается первым блоком после него.
    let floated = |n: &Node| match n {
        Node::Element(e) => {
            e.style.float.is_some_and(|f| f != 0)
                && !matches!(
                    e.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
                && !matches!(e.style.display, Some(Display::None))
        }
        _ => false,
    };
    let mut groups: Vec<Vec<Node>> = Vec::new();
    let run = Run::None;
    group_runs(nodes, inline_level, floated, &mut groups, run);
    for g in groups.iter_mut() {
        while g.last().is_some_and(is_blank) {
            g.pop();
        }
    }
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
            icb_reach = icb_reach.max(oof_reach(e, shape_cx));
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
            let el = wrap.children(blocks(group, &root, opts)).into_any_element();
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
            group_break_names(&root_page, &mut prev_end, floated, group, n);
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
        let (margins, shape) = group_shape(&root, shape_cx, group, n, pad_top);
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
    PAGED.with(|p| p.set(false));
    // Марджин-боксы: наследуют от контекста страницы, а тот — от корня
    // (css-page-3 §page-properties; Blink `StyleForPage` от documentElement).
    let margin_for =
        margin_decls.map(|f| page_boxes::builder(f, root.clone(), opts.clone(), document_counters));
    crate::layout::page::page_stack::PageStack::new(
        kids,
        geom_for,
        icb_copies,
        icb_reach,
        fixed_copies,
        margin_for,
    )
    .with_select(select)
    .into_any_element()
}

#[derive(PartialEq)]
enum Run {
    None,
    Inline,
    Float,
}

#[allow(clippy::too_many_arguments)]
fn resolve_root_page(
    none: &mut bool,
    canvas: &mut Option<gpui::Hsla>,
    root: &mut Computed,
    nodes: &mut Vec<Node>,
    left: &mut f32,
    right: &mut f32,
    top: &mut f32,
    root_page: &mut String,
) {
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else {
            break;
        };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        // Корень без коробки: один пустой лист, и свойства `@page` к нему не
        // применяются (Blink `StyleForPage`: «The root is display:none. One
        // page box will still be created, but no properties should apply»;
        // `root-element-display-none-print` против `blank-print-ref`).
        if matches!(e.style.display, Some(Display::None)) {
            *none = true;
            *nodes = Vec::new();
            break;
        }
        let e = (*e).clone();
        let side = |l: &Option<Len>| match l {
            Some(Len::Px(v)) => *v,
            _ => 0.0,
        };
        let b = e.style.borders();
        // Обёртку с видимой рамкой или своим `display` (сетка, флекс) не
        // снимаем: снятая теряла рамку и раскладку (`page-box-011-print-ref`:
        // `body { border: 10px solid }` — чёрной рамки не было;
        // `page-box-000-print-ref`: `html { display: grid; border: 20px }`).
        // Она остаётся одним ребёнком стопки; фон всё равно уходит в канвас
        // (§painting: фон корня/тела красит канвас листа).
        let framed = [&b.top, &b.right, &b.bottom, &b.left]
            .iter()
            .any(|l| side(l) > 0.0);
        let boxy = matches!(
            e.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::GridLanes)
        );
        if framed || boxy {
            if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
                *canvas = Some(c.to_hsla());
            }
            break;
        }
        *left += side(&e.style.margin.left) + side(&b.left) + side(&e.style.padding.left);
        *right += side(&e.style.margin.right) + side(&b.right) + side(&e.style.padding.right);
        *top += side(&e.style.margin.top) + side(&b.top) + side(&e.style.padding.top);
        if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
            *canvas = Some(c.to_hsla());
        }
        if let Some(p) = &e.style.page {
            *root_page = p.clone();
        }
        *root = inherit(&*root, &e.style);
        *nodes = e.children;
    }
}

fn group_runs(
    nodes: Vec<Node>,
    inline_level: impl Fn(&Node) -> bool,
    floated: impl Fn(&Node) -> bool,
    groups: &mut Vec<Vec<Node>>,
    mut run: Run,
) {
    for n in nodes.iter() {
        if is_blank(n) {
            // Пробел внутри строчного пробега — его часть, вне — пропуск.
            if run != Run::None
                && let Some(g) = groups.last_mut()
            {
                g.push(n.clone());
            }
            continue;
        }
        let (fl, il) = (floated(n), inline_level(n));
        let join = match run {
            Run::None => false,
            Run::Inline => il || fl,
            Run::Float => true,
        };
        match groups.last_mut() {
            Some(g) if join => g.push(n.clone()),
            _ => groups.push(vec![n.clone()]),
        }
        run = if fl || (run == Run::Float && il) {
            Run::Float
        } else if il {
            Run::Inline
        } else {
            Run::None
        };
    }
}

fn group_break_names(
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

fn group_shape(
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

// Мера блочного поддерева для укладки по фрагментаинерам — колонкам и
// страницам: высота, поля, точки законного разреза, принудительные разрывы
// и монолиты. Раньше жила внутри `element()`; локальных переменных не
// захватывала, вынесена ради `render_paged`.
/// Флоат ВО ВСЮ ШИРИНУ содержащего блока в блочной оси неотличим от блока:
/// рядом с ним поместиться нечему — следующий флоат встаёт ПОД ним, строка
/// сдвигается ПОД него (CSS 2.1 §9.5). Значит укладка по фрагментаинерам
/// может вести его обычным ребёнком стопки, и css-break-4 §3.1 прямо этого
/// требует: «User agents should also apply these properties to floated boxes
/// whose containing block is in the normal flow of the root fragmented
/// element». Узкий флоат (`width: 60%`, `auto`) — параллельный поток, стопкой
/// его не выразить, и он по-прежнему гейт (FRAG-PARALLEL-FLOW).
/// ВНИМАНИЕ: `Len::Pct` — ДОЛЯ, а не проценты (`value.rs:175`
/// `Len::Pct(v / 100.0)`, то есть `100%` хранится как `Pct(1.0)`), сравнение
/// идёт с единицей.
/// Отрисовку это не меняет: `apply()` читает `float` ровно в одном месте
/// (`apply.rs:924`, предикат `shrink_to_fit`) и только при `width: None|Auto`,
/// а здесь ширина задана явно.
/// Коробка своё переполнение ПОКАЗЫВАЕТ. Обрезающая (`hidden`/`clip`) и
/// прокручиваемая (`scroll`) коробка параллельного потока не рождает
/// вовсе: за её низом ничего не видно, и повторить её содержимое в
/// следующей колонке значило бы нарисовать то, что браузер прячет
/// (css-overflow-3 §3; css-break-3 §4.1 — прокручиваемая коробка ещё и
/// монолит). У всех девяти приобретений корня `overflow` не задан, так что
/// ворота им ничего не стоят, а зелёные с обрезкой (`overflow-clip-*`)
/// закрывают.
pub(crate) fn visible_overflow(c: &Computed) -> bool {
    use crate::style::computed::Overflow;
    // Параллельный поток живёт по БЛОЧНОЙ оси: решает `overflow-y`. Строчная ось
    // мешает, только если делает коробку прокручиваемой — css-overflow-3
    // §overflow-control: «if the other axis specifies a scrollable value, a
    // specified value of visible computes to auto»; `clip` прокручиваемым
    // значением не является, и `visible` по y остаётся видимым (раскраска по
    // осям раздельная, `apply.rs` `overflow.x = Clip`). `overflow-clip-003/008`:
    // `overflow-x: clip` на коробке 150/200 с содержимым 200/400 — хвост обязан
    // уйти в следующие колонки, а ворота отдавали ему нулевой поток.
    matches!(c.overflow_y, None | Some(Overflow::Visible))
        && matches!(
            c.overflow_x,
            None | Some(Overflow::Visible) | Some(Overflow::Clip)
        )
}
