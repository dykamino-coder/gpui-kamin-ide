//! Постраничная отрисовка документа.
// owner: A

use crate::dom::Node;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flex_lines::inline_display;
use crate::layout::page::names::{
    PageMarginDeclsFn, fill_used_page, first_kid_page_name, hoist_named_wrappers,
};
use crate::layout::page::{page_boxes, page_counters};
use crate::layout::replaced::iframe::IFRAME_DEPTH;
use crate::paint::effects::mask::collect_mask_defs;
use crate::render::{RenderOpts, is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
mod kids;
use kids::page_kids_from_groups;
mod root;
use root::{Run, group_runs, resolve_root_page};

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
    let prev_end: Option<String> = None;
    let first = true;
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
    page_kids_from_groups(
        opts,
        &root,
        left,
        right,
        top,
        root_page,
        shape_cx,
        &mut icb_copies,
        &mut fixed_copies,
        &mut icb_reach,
        &mut kids,
        prev_end,
        first,
        ah,
        layer,
        floated,
        groups,
    );
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
