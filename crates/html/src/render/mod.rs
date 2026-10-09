//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

pub(crate) mod outline;
pub(crate) mod fragment_size;
pub(crate) mod first_line_text;
pub(crate) mod band_clearance;
pub(crate) mod band_dimensions;
pub(crate) mod margin_inline_boxes;
pub(crate) use band_clearance::supported as band_clear_supported;
pub(crate) mod mask_geometry;
pub(crate) mod content_wrapper;
pub(crate) use content_wrapper::{content_sized, content_sized_wraps};
pub(crate) mod orthogonal_inline;
pub(crate) mod native_vertical;
pub(crate) mod containment_paint;
pub(crate) mod paint_scope;
pub(crate) use paint_scope::{DepthScope, snapshot as defer_depth, inside as inside_deferred};
pub(crate) mod page_boxes;
pub(crate) mod page_counters;
pub(crate) mod rotated_atom;
pub(crate) mod combined_text;
pub(crate) mod physical_atomic;
pub(crate) mod vertical_flow_margins;
pub(crate) mod margin_edges;
pub(crate) mod margin_height;
pub(crate) mod float_clear_scope;
pub(crate) mod inline_floats;
pub(crate) mod float_atom;
pub(crate) use float_atom::band_atom;
pub(crate) mod first_letter_descendants;
pub(crate) mod first_letter_scope;
pub(crate) mod pseudo_line_layers;
pub(crate) mod first_line_descendants;
pub(crate) mod inline_splits;
pub(crate) use inline_splits::split_block_in_inline;
pub(crate) mod native_paragraph_route;
pub(crate) mod scroll_box;
pub(crate) mod orthogonal_fixed_child;
pub(crate) mod orthogonal_children;
pub(crate) mod orthogonal_horizontal;
pub(crate) use orthogonal_children::orthogonal_children;
pub(crate) mod orthogonal_absolute;
pub(crate) mod vertical_intrinsic;
pub(crate) mod vertical_hug;
pub(crate) mod native_intrinsic;
pub(crate) mod animation_frame;
pub(crate) mod animation_live;
pub(crate) use animation_live::animated;
pub(crate) mod table_roles;
pub(crate) mod table_border_widths;
pub(crate) mod table_spanning_size;
pub(crate) mod table_clipped_content;
pub(crate) mod replaced_used_style;
pub(crate) mod inline_replaced_position;
pub(crate) mod replaced_holder_ratio;
pub(crate) mod replaced_content;
pub(crate) mod svg_percentage_size;
pub(crate) mod list_item;
pub(crate) mod list_container;
pub(crate) mod available_width;
pub(crate) use replaced_content::svg_replaced;
pub(crate) mod ratio_basis;
pub(crate) mod absolute_overflow;
pub(crate) mod absolute_overflow_math;
pub(crate) mod ruby_hiding;
pub(crate) mod ruby_transform;
pub(crate) mod text_shadows;
pub(crate) use text_shadows::with_text_shadow;
pub(crate) use fragment_size::shape_full;

pub(crate) use crate::apply::{apply, apply_hover};
pub(crate) use crate::computed::{Align, Computed, Display, FlexDir};
pub(crate) use crate::dom::{Element, Node};
pub(crate) use crate::inline::{self};
pub(crate) use crate::value::Len;
pub(crate) use gpui::{
    AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, TextStyle, div, px,
};
pub(crate) use crate::paint::stacking::*;
pub(crate) use crate::paint::decorations::*;
pub(crate) mod box_div;
pub(crate) use crate::render::box_div::*;
pub(crate) mod classify;
pub(crate) use crate::render::classify::*;
pub(crate) mod util;
pub(crate) use crate::render::util::*;
pub(crate) use crate::layout::table::*;
pub(crate) use crate::layout::table::columns::*;
pub(crate) use crate::layout::table::anon::*;
pub(crate) use crate::layout::replaced::image::*;
pub(crate) use crate::layout::replaced::limits::*;
pub(crate) use crate::layout::replaced::iframe::*;
pub(crate) use crate::layout::atom::*;
pub(crate) use crate::layout::positioned::static_position::*;
pub(crate) use crate::layout::positioned::predicates::*;
pub(crate) use crate::layout::positioned::relative::*;
pub(crate) use crate::interactive::sticky::*;
pub(crate) use crate::layout::float::*;
pub(crate) use crate::layout::float::clear::*;
pub(crate) use crate::layout::float::initial_letter::*;
pub(crate) use crate::layout::float::wrap::*;
pub(crate) use crate::layout::float::band_host::*;
pub(crate) use crate::layout::float::band_measured::*;
pub(crate) use crate::layout::float::band_flow_host::*;
pub(crate) use crate::layout::float::band_nest::*;
pub(crate) use crate::layout::float::float_flow::*;
pub(crate) use crate::layout::float::shape_flow::*;
pub(crate) use crate::layout::multicol::column_flow::*;
pub(crate) use crate::layout::multicol::gap_rules::*;
pub(crate) use crate::layout::multicol::spanner::*;

/// Настройки отрисовки: то, что задаёт приложение, а не документ.
#[derive(Clone)]
pub struct RenderOpts {
    /// Базовый стиль текста — от него считаются прогоны и наследование.
    pub text: TextStyle,
    /// Размер окна в точках — от него считаются `vh` и `vw`.
    pub viewport: (f32, f32),
    /// Множитель строки при `line-height: normal`.
    ///
    /// Браузер берёт его из метрик шрифта — у интерфейсных это около 1.31
    /// кегля. Умолчание GPUI — золотое сечение (1.618), и без своего значения
    /// КАЖДЫЙ блок текста выходил на четверть выше браузерного, а разница
    /// копилась вниз по документу.
    pub normal_line_height: f32,
    /// Соль документа для буферов проб (`Document::key`).
    ///
    /// Номера узлов считаются с нуля в каждом документе: когда в одном
    /// потоке живут два документа сразу (стенд гонит пары параллельно),
    /// полоса фона одного забирала прямоугольники ячеек другого с тем же
    /// номером узла. Ноль допустим, пока документ один.
    pub doc_salt: u64,
}

impl RenderOpts {
    /// Цвет подложки выделения.
    ///
    /// Отдельного поля в настройках нет, чтобы не ломать вызывающих: берём
    /// цвет текста и делаем из него полупрозрачную подложку — она читается
    /// и на светлой, и на тёмной теме.
    pub(crate) fn selection_color(&self) -> gpui::Hsla {
        let mut c = self.text.color;
        c.a = 0.25;
        c
    }

    pub(crate) fn base_size(&self) -> f32 {
        f32::from(self.text.font_size.to_pixels(px(16.)))
    }

    /// Корневой стиль документа.
    ///
    /// Высота строки тут НЕ задаётся: `normal` по CSS — метрика шрифта, и
    /// считает её `normal_fraction` по семейству элемента. Пока корень
    /// навязывал постоянную долю, она наследовалась ВСЕМ, и замер шрифта не
    /// работал ни разу: коробка с `line-height: normal` выходила выше коробки
    /// с `line-height: 1em` при одном и том же шрифте.
    pub(crate) fn root_style(&self) -> Computed {
        Computed::default()
    }
}

pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let stripped = without_inert_clear(nodes);
    let nodes: &[Node] = stripped.as_deref().unwrap_or(nodes);
    crate::metrics::set_doc_family(&opts.text.font_family);
    let root = opts.root_style();
    crate::interact::frame_sanitize();
    // Пойманная паника кадра внутри рамки оставляла счётчик глубины
    // навсегда — три такие паники, и рамки исчезали до перезапуска.
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    // Слой начального содержащего блока: внепоточные элементы без
    // позиционированного предка дописываются последними детьми документа —
    // их края решает область просмотра (§10.1 п.4).
    crate::interact::icb_open();
    // Корень документа получает ширину области просмотра: от неё цепочка
    // `AVAIL_W` вычитает поля/рамки/отступы `html` и `body`.
    let avail_prev = AVAIL_W.replace(Some(opts.viewport.0).filter(|w| *w > 0.0));
    // Шаг 8 приложения E: позиционированные `z-index: auto | 0` красятся
    // после потока корневого контекста в порядке разметки — собиратель
    // `gpui::PaintCollect` между парой меток. Внешняя сборка идёт вне краски
    // и сбрасывает собиратели (пойманная паника кадра оставила бы их
    // открытыми); вложенный документ (рамка) собирает своё внутри.
    let depth = RENDER_DEPTH.with(|d| {
        d.set(d.get() + 1);
        d.get()
    });
    if depth == 1 {
        gpui::paint_collect_reset();
    }
    let unkeyed_prev = UNKEYED.replace(unkeyed_positions(nodes));
    let mut out = blocks(nodes, &root, opts);
    AVAIL_W.set(avail_prev);
    out.extend(crate::interact::icb_close());
    UNKEYED.replace(unkeyed_prev);
    RENDER_DEPTH.with(|d| d.set(d.get() - 1));
    let (open, close) = gpui::PaintCollect::pair();
    out.insert(0, open.into_any_element());
    out.push(close.into_any_element());
    out
}

thread_local! {
    /// Глубина вложенных `render` (документ в рамке собирается внутри).
    pub(crate) static RENDER_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Номер элемента в порядке сборки — ключ краски шага 8 (`PaintLast`).
    pub(crate) static PAINT_KEY: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Позиции узлов в прямом обходе документа и позиция ПОСЛЕДНЕГО
    /// позиционированного, который ключа краски не получает (см.
    /// `unkeyed_positions`).
    pub(crate) static UNKEYED: std::cell::RefCell<(std::collections::HashMap<u64, usize>, Option<usize>)> =
        std::cell::RefCell::new((std::collections::HashMap::new(), None));
}

/// Следующий ключ краски: зовётся при входе в элемент, до сборки детей, —
/// предок получает ключ меньше потомков (прямой обход).
pub(crate) fn next_paint_key() -> u64 {
    PAINT_KEY.with(|k| {
        let v = k.get().wrapping_add(1);
        k.set(v);
        v
    })
}

/// Позиционированный с `z-index: auto | 0`, которого сборщик не оборачивает
/// в `PaintLast`: части таблицы (их строит табличный сборщик),
/// `relative`/`sticky` строчного уровня и строчный абсолют — те идут в
/// абзац. Такие красятся первым проходом там, где стоят.
pub(crate) fn unkeyed_positioned(e: &Element) -> bool {
    if e.style.z_index.unwrap_or(0) != 0 {
        return false;
    }
    let table_part = matches!(
        e.style.display,
        Some(Display::TableRow) | Some(Display::TableCell) | Some(Display::TableRowGroup)
    ) || (e.style.display.is_none()
        && matches!(
            e.tag.as_str(),
            "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" | "caption" | "col" | "colgroup"
        ));
    match e.style.position {
        Some(crate::computed::Position::Relative) | Some(crate::computed::Position::Sticky) => {
            table_part || !block_level_in_flow(e)
        }
        Some(crate::computed::Position::Absolute) => {
            table_part || (e.style.display.is_none() && e.inline)
        }
        _ => false,
    }
}

/// Прямой обход документа: конец поддерева каждого узла (позиция за его
/// последним потомком) и позиция последнего позиционированного без ключа
/// краски.
pub(crate) fn unkeyed_positions(nodes: &[Node]) -> (std::collections::HashMap<u64, usize>, Option<usize>) {
    fn walk(
        nodes: &[Node],
        at: &mut usize,
        map: &mut std::collections::HashMap<u64, usize>,
        last: &mut Option<usize>,
    ) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if unkeyed_positioned(e) {
                *last = Some(*at);
            }
            *at += 1;
            walk(&e.children, at, map, last);
            // Конец поддерева: свои потомки порядок не ломают — они рисуются
            // вместе с элементом.
            map.insert(e.node_id, *at);
        }
    }
    let mut map = std::collections::HashMap::new();
    let mut last = None;
    walk(nodes, &mut 0, &mut map, &mut last);
    (map, last)
}

/// Можно ли поднять краску элемента в собиратель шага 8: ПОЗЖЕ по документу
/// нет позиционированного, который останется в первом проходе (иначе
/// порядок разметки перевернётся — `position-relative-table-*`: ячейка
/// `relative` после абсолютного красного индикатора). Узел вне обхода
/// (порождённый сборщиком) — по братьям, как прежде.
pub(crate) fn paint_last_ok(e: &Element, rest: &[Node]) -> bool {
    let known = UNKEYED.with(|u| {
        let u = u.borrow();
        u.0.get(&e.node_id).map(|&end| u.1.is_none_or(|last| last < end))
    });
    match known {
        Some(ok) => ok,
        None => !positioned_later(rest),
    }
}

/// Строчный абсолют с `z-index: auto | 0` в позднем слое (`late_push`): шаг 8
/// приложения E CSS 2.1 — позиционированные рисуются ПОСЛЕ строчного
/// содержимого (шаг 7) своего контекста наложения. Строки абзаца уходят в
/// собиратель (`PaintInline`) и рисуются в его конце, а поздний слой — прямой
/// ребёнок контейнера и красился раньше них: текст ложился поверх абсолюта
/// (`ch-unit-001`, `ic-unit-001`). `PaintLast` ставит коробку в собиратель по
/// ключу в порядке разметки. Узел вне обхода (порождённый сборщиком) остаётся
/// на прежнем пути.
pub(crate) fn inline_abs_paint_last(e: &Element, el: AnyElement) -> AnyElement {
    let known = UNKEYED.with(|u| {
        let u = u.borrow();
        u.0.get(&e.node_id).map(|&end| u.1.is_none_or(|last| last < end))
    });
    if e.style.z_index.unwrap_or(0) == 0 && known == Some(true) {
        gpui::PaintLast::new(el).key(next_paint_key()).into_any_element()
    } else {
        el
    }
}

/// Копий ребёнка в стопке страниц — потолок числа страниц, на которые может
/// растянуться один блок верхнего уровня (в `css-page` не больше шести).
pub(crate) const PAGE_COPIES: usize = 12;

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
    geom_for: crate::flow::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
) -> AnyElement {
    render_paged_select(nodes, opts, geom_for, margin_decls, None)
}

/// `render_paged`, показывающий только листы `select` (номера с нуля).
pub fn render_paged_select(
    nodes: &[Node],
    opts: &RenderOpts,
    geom_for: crate::flow::PageGeomFn,
    margin_decls: Option<PageMarginDeclsFn>,
    select: Option<Vec<usize>>,
) -> AnyElement {
    // Снятые обёртки и корень без коробки правят КАЖДЫЙ лист одинаково:
    // `none` — пустой лист без свойств `@page`, `canvas` — фон `html`/`body`.
    let mut none = false;
    let mut canvas: Option<gpui::Hsla> = None;
    let mut root = opts.root_style();
    let document_counters = page_counters::PageCounters::from_document(nodes);
    crate::interact::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let mut nodes: Vec<Node> = nodes.to_vec();
    let (mut left, mut right, mut top) = (0.0f32, 0.0f32, 0.0f32);
    let mut root_page = String::new();
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else { break };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        // Корень без коробки: один пустой лист, и свойства `@page` к нему не
        // применяются (Blink `StyleForPage`: «The root is display:none. One
        // page box will still be created, but no properties should apply»;
        // `root-element-display-none-print` против `blank-print-ref`).
        if matches!(e.style.display, Some(Display::None)) {
            none = true;
            nodes = Vec::new();
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
                canvas = Some(c.to_hsla());
            }
            break;
        }
        left += side(&e.style.margin.left) + side(&b.left) + side(&e.style.padding.left);
        right += side(&e.style.margin.right) + side(&b.right) + side(&e.style.padding.right);
        top += side(&e.style.margin.top) + side(&b.top) + side(&e.style.padding.top);
        if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
            canvas = Some(c.to_hsla());
        }
        if let Some(p) = &e.style.page {
            root_page = p.clone();
        }
        root = inline::inherit(&root, &e.style);
        nodes = e.children;
    }
    fill_used_page(&mut nodes, &root_page);
    // Обёртка в ином режиме письма — ортогональный поток, монолит (css-break-3
    // §4.1): снимать её нельзя, и у вертикального корня обёртки не снимаются.
    if root.vertical != Some(true) {
        hoist_named_wrappers(&mut nodes);
    }
    let geom_for: crate::flow::PageGeomFn = std::rc::Rc::new(move |i, name: &str| {
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
    let mut fixed_copies: Vec<Vec<AnyElement>> =
        (0..PAGE_COPIES).map(|_| Vec::new()).collect();
    let mut icb_reach = 0.0f32;
    let mut kids: Vec<crate::flow::PageKid> = Vec::new();
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
    #[derive(PartialEq)]
    enum Run {
        None,
        Inline,
        Float,
    }
    let floated = |n: &Node| match n {
        Node::Element(e) => {
            e.style.float.is_some_and(|f| f != 0)
                && !matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
                )
                && !matches!(e.style.display, Some(Display::None))
        }
        _ => false,
    };
    let mut groups: Vec<Vec<Node>> = Vec::new();
    let mut run = Run::None;
    for n in nodes.iter() {
        if is_blank(n) {
            // Пробел внутри строчного пробега — его часть, вне — пропуск.
            if run != Run::None && let Some(g) = groups.last_mut() {
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
            crate::interact::icb_open();
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
            slot.extend(layer(crate::interact::icb_close()));
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
        let (monolith, fb, fa, names) = match n {
            Node::Element(e) if class_a_box(e) => (
                page_monolith(e),
                edge_break(e, false),
                edge_break(e, true),
                Some(page_names(e, &root_page)),
            ),
            Node::Element(e) if !e.inline && !inline_display(e) => (
                page_monolith(e),
                edge_break(e, false),
                edge_break(e, true),
                None,
            ),
            _ => (false, false, false, Some((root_page.clone(), root_page.clone()))),
        };
        // Группа флоата: сам флоат имени не передаёт (§named pages п. 2), но
        // поточные коробки класса A в группе — передают конец — у последней (`page-name-000-print`: флоат, `clear`-блок
        // страницы `foo` и следом блок страницы `bar` — разрыв перед `bar`).
        let names = match (&names, n) {
            (None, Node::Element(e)) if floated(n) && !class_a_box(e) => {
                let named: Vec<(String, String)> = group
                    .iter()
                    .filter_map(|g| match g {
                        Node::Element(k) if class_a_box(k) => Some(page_names(k, &root_page)),
                        _ => None,
                    })
                    .collect();
                // Начало группы — продолжение предыдущей: разрыв перед флоатом
                // увёл бы и его (`page-name-float-002-print`: флоат `b` остаётся
                // на листе `a`).
                let start = prev_end.clone().unwrap_or_else(|| root_page.clone());
                named.last().map(|l| (start, l.1.clone()))
            }
            _ => names,
        };
        // Группа из нескольких узлов монолитом не бывает: её режет край листа.
        let monolith = monolith && group.iter().filter(|g| !is_blank(g)).count() == 1;
        let renamed = match (&prev_end, &names) {
            (Some(p), Some((start, _))) => p != start,
            _ => false,
        };
        // Имя, с которого коробка начинается: своё у коробки класса A, иначе
        // — конец предыдущей (анонимный блок и прочие продолжают страницу).
        let start_name = match &names {
            Some((start, _)) => start.clone(),
            None => prev_end.clone().unwrap_or_else(|| root_page.clone()),
        };
        if let Some((_, end)) = &names {
            prev_end = Some(end.clone());
        }
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
        let positioned = |e: &Element| {
            matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
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
        // Монолит выше листа решается в `fill` (правило «сначала перенос,
        // потом разрыв внутри»): здесь мера считается и для него — точки
        // класса A нужны, когда он окажется с верха страницы.
        kids.push(crate::flow::PageKid {
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
    let margin_for = margin_decls.map(|f| {
        page_boxes::builder(f, root.clone(), opts.clone(), document_counters)
    });
    crate::flow::PageStack::new(kids, geom_for, icb_copies, icb_reach, fixed_copies, margin_for)
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
    use crate::computed::Overflow;
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

/// Коробка и всё её поддерево — ОБЫЧНЫЕ блоки: ни гибкого контейнера, ни
/// сетки, ни таблицы, ни вложенного многоколоночника. Только у такого
/// поддерева мера `shape_full` совпадает с тем, что рисует движок: у гибкого
/// контейнера с переносом она складывает элементы стопкой (`shape_full`,
/// ветка без `row_nowrap`), у вложенного многоколоночника — не знает про его
/// собственные колонки. Протяжённость параллельного потока, снятая с такого
/// приближения, была бы выдуманной, и зелёные `multicol-nested-026`,
/// `single-line-row-flex-fragmentation-039`,
/// `multi-line-row-flex-fragmentation-093`,
/// `single-line-column-flex-fragmentation-043/058` разъехались бы
/// (`target/scout-fragparallel-2026-09.md` §5). `display: flow-root` сюда
/// входит: он сводится к `Block` (`computed.rs:3057`).
pub(crate) fn plain_block_tree(c: &Element, depth: u8) -> bool {
    if c.style.column_count.is_some() || c.style.column_width.is_some() {
        return false;
    }
    if c.style.webkit_box == Some(true) {
        return false;
    }
    if !matches!(
        c.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) {
        return false;
    }
    if depth == 0 {
        return true;
    }
    c.children.iter().all(|n| match n {
        Node::Element(k) => k.inline || plain_block_tree(k, depth - 1),
        _ => true,
    })
}
/// Поддерево обычных блоков (`plain_block_tree`), где допустим и flex-ряд с
/// переносом, каждый элемент которого занимает всю строку (`flex-basis` или
/// `width` 100%, без роста и боковых полей): строка = элемент, и элементы идут
/// блочной стопкой (css-flexbox-1 §9.3 шаг 5: следующий элемент в строку не
/// помещается). Такую стопку мера `shape_full` ведёт так же точно, как блок
/// (ветка «строка = элемент»), и её переполнение заданной высоты продолжается
/// в следующем фрагментаинере параллельным потоком (css-break-3 §3).
pub(crate) fn stacked_flex_tree(c: &Element, depth: u8) -> bool {
    use crate::computed::FlexDir;
    if c.style.column_count.is_some() || c.style.column_width.is_some() || c.style.webkit_box == Some(true) {
        return false;
    }
    let s = &c.style;
    let flex_stack = s.display == Some(Display::Flex)
        && matches!(s.flex_dir, None | Some(FlexDir::Row))
        && s.flex_wrap == Some(true)
        && s.flex_wrap_reverse != Some(true)
        && s.vertical != Some(true)
        && s.gap.is_none()
        && !flex_gap_rules(s);
    if !flex_stack && !matches!(s.display, None | Some(Display::Block) | Some(Display::ListItem)) {
        return false;
    }
    if depth == 0 {
        return !flex_stack;
    }
    c.children.iter().all(|n| match n {
        Node::Element(k) if flex_stack => {
            let ks = &k.style;
            let full = |l: &Option<Len>| matches!(l, Some(Len::Pct(p)) if (*p - 1.0).abs() < 1e-4);
            let zero = |l: &Option<Len>| match l {
                None => true,
                Some(Len::Px(v)) => v.abs() < 0.01,
                _ => false,
            };
            !k.inline
                && !out_of_flow(ks)
                && ks.position.is_none()
                && ks.flex_grow.is_none_or(|g| g == 0.0)
                && ks.basis_content != Some(true)
                && match ks.flex_basis {
                    None | Some(Len::Auto) => full(&ks.width),
                    _ => full(&ks.flex_basis),
                }
                && zero(&ks.margin.left)
                && zero(&ks.margin.right)
                && zero(&ks.padding.left)
                && zero(&ks.padding.right)
                && zero(&ks.borders().left)
                && zero(&ks.borders().right)
                && stacked_flex_tree(k, depth - 1)
        }
        Node::Element(k) => k.inline || stacked_flex_tree(k, depth - 1),
        _ => true,
    })
}

/// Мера блочного ребёнка для укладки колонок: высота с
/// отбивками и рамками, поля и точки ЗАКОННОГО разреза
/// (css-break-3 §4.3, класс A) — границы вложенных
/// блочных детей, рекурсивно. Высота `auto` складывается
/// из тех же детей со схлопыванием полей (CSS 2.1
/// §8.3.1); строчное содержимое высоты не даёт — такой
/// ребёнок мерить нечем, и весь стек идёт другим путём.
pub(crate) fn shape(c: &Element, depth: u8) -> Option<(f32, f32, f32, Vec<(f32, f32)>)> {
    shape_full(c, depth, ShapeCx::COLUMNS).map(|s| (s.0, s.1, s.2, s.3))
}
/// Высота сетки по ЯВНЫМ дорожкам рядов: все дорожки в
/// точках, плюс зазоры между ними. `None` — дорожки
/// неизвестны или не все в точках.
pub(crate) fn grid_rows_px(c: &Computed) -> Option<f32> {
    use crate::computed::{Track, TrackSize};
    if !matches!(
        c.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let mut total = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => total += v,
            _ => return None,
        }
    }
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    Some(total + gap * (rows.len() as f32 - 1.0))
}

/// Зазоры между ЯВНЫМИ рядами сетки от верха содержимого: `(начало, конец)`.
/// Ряды — в точках либо доли `fr` при заданной в точках высоте коробки
/// (остаток после точечных рядов и зазоров делится по долям, css-grid-1
/// §12.7). Иначе — пусто: дорожек не знаем, точек не даём.
pub(crate) fn grid_row_gaps(c: &Computed, inner_h: f32) -> Vec<(f32, f32)> {
    use crate::computed::{Track, TrackSize};
    if !matches!(
        c.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return Vec::new();
    }
    let Some(rows) = c.grid_rows.as_ref() else {
        return Vec::new();
    };
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    if rows.len() < 2 || gap <= 0.0 {
        return Vec::new();
    }
    let mut fixed = 0.0f32;
    let mut fr = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => fixed += v,
            TrackSize::Single(Track::Fr(k)) => fr += k,
            _ => return Vec::new(),
        }
    }
    let per_fr = if fr > 0.0 {
        if !matches!(c.height, Some(Len::Px(_))) {
            return Vec::new();
        }
        (inner_h - fixed - gap * (rows.len() as f32 - 1.0)).max(0.0) / fr
    } else {
        0.0
    };
    let mut out = Vec::new();
    let mut y = 0.0f32;
    for (i, t) in rows.iter().enumerate() {
        y += match t {
            TrackSize::Single(Track::Px(v)) => *v,
            TrackSize::Single(Track::Fr(k)) => k * per_fr,
            _ => 0.0,
        };
        if i + 1 < rows.len() {
            out.push((y, y + gap));
            y += gap;
        }
    }
    out
}

/// Полосы ЯВНЫХ рядов сетки, когда все дорожки и зазор — в точках:
/// `(начало, конец)` каждого ряда от верха содержимого. Тот же путь, что
/// даёт высоту в `grid_rows_px`, только развёрнутый по рядам: границы рядов
/// — точки разреза класса A (css-grid-2 §Fragmenting Grid Layout: «Class A
/// break opportunities occur between rows or columns»). `None` — дорожек
/// нет, они не все в точках или зазор задан не в точках: границ мы не знаем
/// и точек не даём. Строже, чем `grid_rows_px` (тот считает незнакомый
/// зазор нулём) — неверная граница ряда хуже отсутствующей.
pub(crate) fn grid_px_row_bands(c: &Computed) -> Option<Vec<(f32, f32)>> {
    use crate::computed::{Track, TrackSize};
    if !matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let gap = match c.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    let mut out = Vec::with_capacity(rows.len());
    let mut y = 0.0f32;
    for t in rows {
        let TrackSize::Single(Track::Px(v)) = t else {
            return None;
        };
        out.push((y, y + v));
        y += v + gap;
    }
    Some(out)
}

/// Полосы рядов сетки, когда `grid_rows_px` бессилен: колонок больше одной,
/// ряд `auto` или ряд вовсе неявный. `(начало, конец)` каждого ряда от верха
/// содержимого; последний конец — высота сетки.
///
/// css-grid-2 §Fragmenting Grid Layout, «Sample Fragmentation Algorithm»
/// шаг 4: «If the grid height is ''auto'', the height of the grid should be
/// the sum of the final row sizes». Blink считает ровно это —
/// `grid_layout_algorithm.cc:370` `CalculateIntrinsicBlockSize`:
/// `layout_data.Rows().CalculateSetSpanSize() + border_scrollbar_padding
/// .BlockSum()`, без всякого условия «дорожки в точках».
///
/// Размер ряда: явная дорожка `Px` — как есть; `auto` и неявный ряд — по
/// НАИБОЛЬШЕМУ элементу ряда (css-grid-1 §12.5: `auto` как максимум —
/// max-content вклада), с полями: поля элементов сетки не схлопываются
/// (§6.1). Размещение — css-grid-1 §8.5: сперва элементы с ЯВНОЙ линией
/// ряда (шаг 2), затем курсор по рядам (шаг 4).
///
/// Отказ (`None`) — на всём, где догадка была бы неверной: `fr`, проценты,
/// `minmax`, `min-content`, `subgrid`, `repeat(auto-fill …)` в дорожках;
/// `grid-template-areas`; `grid-auto-flow` по колонкам или `dense`;
/// `grid-auto-rows` заданного размера; распределяющий `align-content`;
/// зазор не в точках; явная КОЛОНКА или охват рядов у ребёнка; ребёнок,
/// который сам себя измерить не даёт. Отказ = прежнее поведение, поэтому
/// ни одна пара, что мерится сегодня, этой функции не видит: она стоит
/// ПОСЛЕ `grid_rows_px` в той же ветке.
///
/// Точек разреза функция НЕ даёт нарочно. Класс A между рядами
/// (css-grid-2 §Fragmenting Grid Layout) — возможность, а не предпочтение:
/// Blink переносит ряд в следующий фрагментаинер только при принудительном
/// разрыве (`grid_layout_algorithm.cc:2161-2167`) или при отказе
/// `MovePastBreakpoint` (:2178), а обычный ряд режет по краю. Точка класса A
/// на каждой границе ряда увела бы разрез у зелёных
/// `grid-item-oof-002/003` (ряды `50px 150px`, край колонки на 100 внутри
/// второго ряда) с края на 50 и потеряла бы половину колонки.
pub(crate) fn grid_auto_row_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
) -> Option<(Vec<(f32, f32)>, Vec<GridSpot>)> {
    use crate::computed::{AutoFlow, Placement, Track, TrackSize};
    let s = &c.style;
    if depth == 0 || !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    // Именованные области размещаются ниже (область в один ряд); прочее —
    // отказ, как прежде.
    if s.align_content.is_some()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || !matches!(s.grid_auto_rows, None | Some(TrackSize::Single(Track::Auto)))
        || !s.grid_auto_rows_list.is_empty()
    {
        return None;
    }
    let gap = match s.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    // Явные дорожки рядов: `Some(px)` — размер известен, `None` — ряд `auto`
    // и меряется содержимым. Всё прочее — отказ.
    let mut track: Vec<Option<f32>> = Vec::new();
    if let Some(rows) = s.grid_rows.as_ref() {
        for t in rows {
            match t {
                TrackSize::Single(Track::Px(v)) => track.push(Some(*v)),
                TrackSize::Single(Track::Auto) => track.push(None),
                _ => return None,
            }
        }
    }
    // Колонки: `grid-template-columns` перечислимым списком либо его нет —
    // тогда неявная колонка ровно одна.
    let cols = match (s.grid_cols, s.grid_tracks.as_ref()) {
        (Some(n), _) => n.max(1) as usize,
        (None, Some(t)) => {
            if t.iter().any(|x| matches!(x, TrackSize::AutoRepeat { .. })) {
                return None;
            }
            t.len().max(1)
        }
        (None, None) => 1,
    };
    // Области задают и неявные колонки (css-grid-1 §7.3): `'a b' 'c c'` без
    // `grid-template-columns` — две колонки.
    let cols = cols.max(
        s.grid_areas
            .as_ref()
            .map_or(0, |a| a.iter().map(|r| r.len()).max().unwrap_or(0)),
    );
    // `used[ряд][колонка]` — занятость, `fill[ряд]` — содержимое ряда.
    let mut used: Vec<Vec<bool>> = Vec::new();
    // Элементы с их рядом и мерой — для внутренних точек (`shape_full`) и
    // спуска распорки роста (`pushed_box_at`).
    let mut spots: Vec<GridSpot> = Vec::new();
    let mut fill: Vec<f32> = Vec::new();
    for pass in 0..2u8 {
        let mut cur_row = 0usize;
        let mut cur_col = 0usize;
        for (ix, n) in c.children.iter().enumerate().filter(|(_, n)| !is_blank(n)) {
            let Node::Element(k) = n else {
                return None;
            };
            if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
                continue;
            }
            // `(ряд, начальная колонка, охват колонок)`. Именованная область —
            // её прямоугольник (как `place_named_areas`), только в ОДИН ряд:
            // охват рядов сам меняет размер дорожек (css-grid-1 §12.5).
            // Колонка — css-grid-1 §8.3: `N`, `span M`, `N / span M`, `N / M`;
            // неявные колонки за краем и отрицательные линии — отказ.
            let (line, cline, cspan) = if let Some(name) = &k.style.grid_area_name {
                let areas = s.grid_areas.as_ref()?;
                let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (r, cells) in areas.iter().enumerate() {
                    for (cc, cell) in cells.iter().enumerate() {
                        if cell == name {
                            r0 = r0.min(r);
                            r1 = r1.max(r + 1);
                            c0 = c0.min(cc);
                            c1 = c1.max(cc + 1);
                        }
                    }
                }
                if r0 == usize::MAX || r1 != r0 + 1 {
                    return None;
                }
                (Some(r0), Some(c0), c1 - c0)
            } else {
                let line = match k.style.grid_row {
                    None | Some((Placement::Auto, Placement::Auto)) => None,
                    Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some((a - 1) as usize),
                    _ => return None,
                };
                let (cline, cspan) = match k.style.grid_col {
                    None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                    Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Auto)) if a >= 1 => {
                        (Some((a - 1) as usize), 1usize)
                    }
                    Some((Placement::Line(a), Placement::Span(m))) if a >= 1 => {
                        (Some((a - 1) as usize), m.max(1) as usize)
                    }
                    Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b > a => {
                        (Some((a - 1) as usize), (b - a) as usize)
                    }
                    _ => return None,
                };
                (line, cline, cspan)
            };
            if cspan > cols || cline.is_some_and(|c0| c0 + cspan > cols) {
                return None;
            }
            // Проход 0 — «locked to a given row» (§8.5 шаг 2), проход 1 —
            // курсор (§8.5 шаг 4). Мера ребёнка берётся ровно один раз.
            if (pass == 0) != line.is_some() {
                continue;
            }
            let ks = shape_full(k, depth - 1, cx)?;
            let h = ks.0 + ks.1 + ks.2;
            let busy = |used: &Vec<Vec<bool>>, r: usize, c0: usize| {
                used.get(r).is_some_and(|v| v[c0..c0 + cspan].iter().any(|x| *x))
            };
            let (row, col) = match (line, cline) {
                // Ряд и колонка заданы (область): ячейки как есть — §8.5
                // шаг 1, перекрытие законно.
                (Some(r), Some(c0)) => (r, c0),
                (Some(r), None) => {
                    // «the earliest line index that ensures this item's grid
                    // area will not overlap any occupied grid cells».
                    let mut cc = 0usize;
                    while cc + cspan < cols && busy(&used, r, cc) {
                        cc += 1;
                    }
                    (r, cc)
                }
                // §8.5 шаг 4 «sparse», заданная колонка: курсор-ряд растёт,
                // если колонка левее курсора, и дальше — до свободных ячеек.
                (None, Some(c0)) => {
                    let mut rr = cur_row + usize::from(c0 < cur_col);
                    while busy(&used, rr, c0) {
                        rr += 1;
                    }
                    (rr, c0)
                }
                // Авто-колонка с охватом: первое место от курсора, где
                // свободны все `cspan` ячеек подряд.
                (None, None) => {
                    let mut rr = cur_row;
                    let mut cc = cur_col;
                    loop {
                        if cc + cspan > cols {
                            cc = 0;
                            rr += 1;
                            continue;
                        }
                        if !busy(&used, rr, cc) {
                            break;
                        }
                        cc += 1;
                    }
                    (rr, cc)
                }
            };
            while used.len() <= row {
                used.push(vec![false; cols]);
                fill.push(0.0);
            }
            for x in &mut used[row][col..col + cspan] {
                *x = true;
            }
            fill[row] = fill[row].max(h);
            // Внутренние точки элемента годны сетке, только если его коробка
            // стоит в начале ряда: монолит (`solid_box`) целиком не режется,
            // выравнивание center/end/baseline и `margin-top: auto` сдвигают
            // коробку на величину, которой мера не знает
            // (`grid-item-fragmentation-024`, `-008`).
            let aligned = |a: &Option<crate::computed::Align>| {
                matches!(
                    a,
                    Some(crate::computed::Align::Center)
                        | Some(crate::computed::Align::End)
                        | Some(crate::computed::Align::Baseline)
                        | Some(crate::computed::Align::AnchorCenter)
                )
            };
            let plain = !solid_box(k)
                && !aligned(&k.style.align_self)
                && !(k.style.align_self.is_none() && aligned(&c.style.align_items))
                && !matches!(k.style.margin.top, Some(Len::Auto));
            spots.push((ix, row, ks.1, ks, plain));
            if line.is_none() {
                cur_row = row;
                cur_col = col + cspan;
                if cur_col >= cols {
                    cur_col = 0;
                    cur_row += 1;
                }
            }
        }
    }
    let rows_n = track.len().max(used.len());
    if rows_n == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(rows_n);
    let mut y = 0.0f32;
    for i in 0..rows_n {
        let h = match track.get(i) {
            Some(Some(v)) => *v,
            _ => fill.get(i).copied().unwrap_or(0.0),
        };
        out.push((y, y + h));
        y += h + gap;
    }
    Some((out, spots))
}

/// Элемент сетки для `grid_auto_row_bands`: номер среди `c.children`, ряд,
/// верхнее поле, мера `shape_full`, годны ли его внутренние точки сетке.
pub(crate) type GridSpot = (usize, usize, f32, Shape, bool);

/// Сетка, которую мерит `grid_auto_row_bands`, — та же цепочка, что в
/// `shape_full`: не стопка, высота `auto`, ряды не все в точках.
pub(crate) fn grid_items_spotted(c: &Element) -> bool {
    matches!(c.style.display, Some(Display::Grid) | Some(Display::InlineGrid))
        && !grid_stack(c)
        && c.style.height.is_none()
        && grid_rows_px(&c.style).is_none()
}

/// Принудительные разрывы, перенесённые с ЭЛЕМЕНТОВ сетки на границы РЯДОВ
/// (css-grid-2 §Fragmenting Grid Layout: «The 'break-before' and
/// 'break-after' properties on grid items are propagated to their grid
/// row»). Blink `grid_layout_algorithm.cc:1846-1857`:
/// `row_break_between[set_indices.begin] |= item_break_before`,
/// `[set_indices.end] |= item_break_after`, причём оба значения берутся
/// через `InitialBreakBefore`/`FinalBreakAfter` — то есть С ПОТОМКОВ, что у
/// нас делает `edge_break`. Смещения — от верха СОДЕРЖИМОГО коробки и всегда
/// на НАЧАЛЕ ряда, с которого продолжится следующий фрагмент: зазор перед
/// ним съедается разрывом (css-gaps-1 §fragmentation).
///
/// Разрыв перед ПЕРВЫМ рядом и после ПОСЛЕДНЕГО сюда не попадает: спека
/// отдаёт его контейнеру, и его переносит `edge_break`.
///
/// Размещение элементов по рядам считается только там, где оно однозначно
/// (css-grid-1 §8.5, поток `row` без `dense`): именованных областей нет,
/// `grid-row`/`grid-area` у детей нет, число колонок известно. Курсор идёт
/// по колонкам, `grid-column: N / span M` занимает M колонок и при
/// необходимости пинает курсор вперёд; не влезающий в остаток ряда элемент
/// начинает новый ряд. Любая непонятная форма — пустой список, а не догадка.
pub(crate) fn grid_row_forced(c: &Element) -> (Vec<f32>, Vec<(f32, f32)>) {
    use crate::computed::{AutoFlow, Placement};
    let s = &c.style;
    if matches!(
        s.grid_auto_flow,
        Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
    ) {
        return (Vec::new(), Vec::new());
    }
    let Some(bands) = grid_px_row_bands(s) else {
        return (Vec::new(), Vec::new());
    };
    let cols = s
        .grid_cols
        .map(|n| n.max(1) as usize)
        .or_else(|| s.grid_tracks.as_ref().map(|t| t.len().max(1)))
        .unwrap_or(1);
    // Явный ОДИН ряд элемента: `grid-row: N`, `N / N+1` или именованная
    // область в один ряд. `Some(None)` — ряд по курсору, `None` — форма,
    // которой мы не знаем (отказ целиком, как прежде).
    let own_row = |k: &Element| -> Option<Option<usize>> {
        if let Some(name) = &k.style.grid_area_name {
            let areas = s.grid_areas.as_ref()?;
            let hit: Vec<usize> = areas
                .iter()
                .enumerate()
                .filter(|(_, r)| r.iter().any(|x| x == name))
                .map(|(i, _)| i)
                .collect();
            return match hit.as_slice() {
                [r] => Some(Some(*r)),
                _ => None,
            };
        }
        match k.style.grid_row {
            None | Some((Placement::Auto, Placement::Auto)) => Some(None),
            Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some(Some((a - 1) as usize)),
            Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b == a + 1 => {
                Some(Some((a - 1) as usize))
            }
            _ => None,
        }
    };
    let mut items: Vec<(&Element, Option<usize>)> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else {
            return (Vec::new(), Vec::new());
        };
        if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
            continue;
        }
        let Some(r) = own_row(k) else {
            return (Vec::new(), Vec::new());
        };
        items.push((k, r));
    }
    // Курсор ниже занятых ячеек не знает: смесь явных рядов с курсорными
    // (css-grid-1 §8.5 шаги 2 и 4 зависят друг от друга) и курсорный элемент
    // при областях — отказ.
    let explicit = items.iter().filter(|(_, r)| r.is_some()).count();
    if (explicit > 0 && explicit < items.len()) || (explicit == 0 && s.grid_areas.is_some()) {
        return (Vec::new(), Vec::new());
    }
    let mut out: Vec<f32> = Vec::new();
    // Ряды с монолитным элементом (css-break-4 §4.1; Blink: элемент, не
    // влезший в остаток, — разрыв ПЕРЕД рядом, `MovePastBreakpoint`,
    // grid_layout_algorithm.cc:2161-2178). Первый ряд — как прежде: разрыв
    // перед ним принадлежит контейнеру.
    let mut mono: Vec<(f32, f32)> = Vec::new();
    let mut row = 0usize;
    let mut col = 0usize;
    for (k, fixed) in items {
        let r = match fixed {
            Some(r) => r,
            None => {
                let (line, span) = match k.style.grid_col {
                    None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                    Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Span(m))) => (Some(a), m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Auto)) => (Some(a), 1usize),
                    Some((Placement::Line(a), Placement::Line(b))) => {
                        (Some(a.min(b)), (b - a).unsigned_abs().max(1) as usize)
                    }
                    _ => return (Vec::new(), Vec::new()),
                };
                if span > cols {
                    return (Vec::new(), Vec::new());
                }
                if let Some(a) = line {
                    if a < 1 {
                        return (Vec::new(), Vec::new());
                    }
                    let want = (a - 1) as usize;
                    if want + span > cols {
                        return (Vec::new(), Vec::new());
                    }
                    if want < col {
                        row += 1;
                    }
                    col = want;
                } else if col + span > cols {
                    row += 1;
                    col = 0;
                }
                let here = row;
                col += span;
                if col >= cols {
                    row += 1;
                    col = 0;
                }
                here
            }
        };
        if r >= bands.len() {
            if fixed.is_some() {
                continue;
            }
            break;
        }
        if r > 0 && edge_break(k, false) {
            out.push(bands[r].0);
        }
        if r + 1 < bands.len() && edge_break(k, true) {
            out.push(bands[r + 1].0);
        }
        if r > 0 && (k.style.break_inside_avoid || size_monolith(k)) {
            mono.push(bands[r]);
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    (out, mono)
}

/// Сетка, которую фрагментация вправе спускать СТОПКОЙ: одна колонка, ряды
/// по содержимому, дети без явного размещения. Тогда ряд — ровно один
/// ребёнок, высота ряда равна мере ребёнка (css-grid-1 §11.8: дорожка
/// `auto` — по max-content), а порядок рядов равен порядку детей (§8.5
/// auto-placement при `grid-auto-flow: row`). Поля рядов НЕ схлопываются
/// (§6.1: «margins of grid items do not collapse»), между рядами стоит
/// `row-gap`.
///
/// Отказ (прежний путь — `grid_rows_px`, чаще всего `None`): явные дорожки
/// рядов не все `auto` (px/`fr`/`minmax` — размер ряда не равен мере
/// ребёнка), колонок больше одной (дети параллельны, а не стопкой),
/// именованные области, поток по колонкам или `dense`, неявные ряды
/// заданного размера, распределяющий `align-content` (двигает ряды внутри
/// заданной высоты), зазор не в точках, явное размещение у любого ребёнка
/// (`grid-row`/`grid-column`/`grid-area`). `display: grid-lanes` не
/// проходит никогда — у полос своя укладка.
pub(crate) fn grid_stack(c: &Element) -> bool {
    use crate::computed::{AutoFlow, Track, TrackSize};
    let s = &c.style;
    if !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return false;
    }
    if s.grid_cols.unwrap_or(1) > 1
        || s.grid_tracks.as_ref().is_some_and(|t| t.len() > 1)
        || s.grid_areas.is_some()
    {
        return false;
    }
    if let Some(rows) = s.grid_rows.as_ref() {
        // Ряд, чей размер при `height: auto` равен вкладу ЕДИНСТВЕННОГО
        // элемента ряда: `auto`/`min-content`/`max-content` (css-grid-1
        // §12.4-12.6); `minmax(<0 | по содержимому>, <auto | max-content |
        // fr>)` — база не больше вклада, предел = вклад; `fr` при
        // неопределённом свободном месте — §12.7.1 «max-content contribution»
        // (ровно вклад, пока гибкая дорожка ОДНА); `minmax(<по содержимому>,
        // <px>)` — база = min-content = вклад (расходится лишь для элемента
        // ниже предела; копия кладётся `Definite(h)` и берёт ту же высоту).
        // css-grid-2 §12.1 шаг 3 растит именно такие ряды. С заданной
        // высотой `fr`/`minmax` делят ЕЁ, а не вклад: `grid-item-
        // fragmentation-014/016` (`height:200px`) держатся на прежнем пути.
        let content = |t: &Track| matches!(t, Track::Auto | Track::MinContent | Track::MaxContent);
        let lo_ok = |t: &Track| content(t) || matches!(t, Track::Px(v) if *v <= 0.0);
        let by_item = |t: &TrackSize| match t {
            TrackSize::Single(t) => content(t) || matches!(t, Track::Fr(_)),
            TrackSize::MinMax(lo, hi) => {
                lo_ok(lo)
                    && (content(hi)
                        || matches!(hi, Track::Fr(_))
                        || (content(lo) && matches!(hi, Track::Px(_))))
            }
            TrackSize::AutoRepeat { .. } => false,
        };
        let all_auto = rows
            .iter()
            .all(|t| matches!(t, TrackSize::Single(Track::Auto)));
        let flexible = rows
            .iter()
            .filter(|t| {
                matches!(t, TrackSize::Single(Track::Fr(_)) | TrackSize::MinMax(_, Track::Fr(_)))
            })
            .count();
        if !all_auto
            && (!matches!(s.height, None | Some(Len::Auto))
                || s.max_height.is_some()
                || flexible > 1
                || !rows.iter().all(by_item))
        {
            return false;
        }
    }
    if !matches!(
        s.grid_auto_rows,
        None | Some(TrackSize::Single(Track::Auto))
    ) || !s.grid_auto_rows_list.is_empty()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || s.align_content.is_some()
    {
        return false;
    }
    if !matches!(s.gap, None | Some((None, _)) | Some((Some(Len::Px(_)), _))) {
        return false;
    }
    !c.children.iter().any(|n| match n {
        Node::Element(k) => {
            k.style.grid_row.is_some()
                || k.style.grid_col.is_some()
                || k.style.grid_area_name.is_some()
        }
        _ => false,
    })
}

/// `contain: size` — монолит везде, где есть фрагментация (Blink
/// `LayoutBox::IsMonolithic`, `layout_box.cc`: `ShouldApplySizeContainment()`
/// = `StyleRef().ContainsSize() && IsEligibleForSizeContainment()`; обе оси —
/// `size`/`strict`, не `inline-size`; `contain-intrinsic-size` не участвует;
/// таблица, группа, ряд и ячейка не годны — `layout_table*.h`
/// `IsEligibleForSizeContainment() … return false`). `content-visibility:
/// hidden` ставит тот же `contain_size`.
pub(crate) fn size_monolith(k: &Element) -> bool {
    k.style.contains_block_size()
        && !matches!(
            k.style.display,
            Some(Display::TableCell) | Some(Display::TableRow) | Some(Display::TableRowGroup)
        )
        && !matches!(
            k.tag.as_str(),
            "td" | "th" | "tr" | "thead" | "tbody" | "tfoot"
        )
}

/// Коробка, принудительные разрывы ВНУТРИ которой фрагментации не видны:
/// `contain: size` (`size_monolith`) и прокручиваемая коробка — css-break-4
/// §possible-breaks: «Any forced breaks within such boxes therefore cannot split
/// the box, and must therefore also be ignored by the box’s own fragmentation
/// context». `break-inside: avoid` сюда НЕ входит: там принудительный разрыв
/// сильнее запрета (§forced-breaks). Ряд и группа рядов `overflow` не берут
/// (css-overflow-3, «Applies to»), ячейка — берёт.
/// Есть ли в поддереве принудительный разрыв (`break-before/after` любого
/// потомка, кроме уходящих внутрь монолита, `forced_opaque`).
pub(crate) fn forced_inside(e: &Element, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    e.children.iter().any(|n| match n {
        // Вне потока — свой поток (`edge_break`).
        Node::Element(k)
            if matches!(
                k.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) =>
        {
            false
        }
        Node::Element(k) => {
            k.style.break_before_force
                || k.style.break_after_force
                || (!forced_opaque(k) && forced_inside(k, depth - 1))
        }
        _ => false,
    })
}

pub(crate) fn forced_opaque(k: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    size_monolith(k)
        || ((scrolls(k.style.overflow_x) || scrolls(k.style.overflow_y))
            && !matches!(
                k.style.display,
                Some(Display::TableRow) | Some(Display::TableRowGroup)
            )
            && !matches!(k.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot"))
}

/// Гибкий контейнер или сетка БЕЗ своей коробки (ни рамок, ни отбивок, ни фона,
/// ни заданной высоты, ни позиционирования) с единственным элементом, у которого
/// `box-decoration-break: clone`. По блочной оси элемент такой обёртки стоит
/// там же и той же высоты, что блок-ребёнок: строка flex одна, её высота —
/// высота элемента; сетка без своих дорожек — один ряд `auto`; поперёк элемент
/// растянут (колонка flex, сетка) или имеет свою ширину (ряд flex). Вернуть
/// элемент, поднятый на место обёртки (с её полями), — тогда клонированное
/// украшение (css-break-4 §break-decoration) фрагментирует сам элемент
/// (`box-decoration-break-clone-018/019/028/029`). Иначе `None`.
pub(crate) fn clone_wrapper_item(w: &Element) -> Option<Element> {
    use crate::computed::FlexDir;
    let s = &w.style;
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let b = s.borders();
    let row = match s.display {
        Some(Display::Flex) => matches!(s.flex_dir, None | Some(FlexDir::Row)),
        Some(Display::Grid) => false,
        _ => return None,
    };
    if w.inline
        || s.webkit_box == Some(true)
        || s.vertical == Some(true)
        || s.position.is_some()
        || s.float.is_some_and(|f| f != 0)
        || s.background.is_some()
        || s.bg_image.is_some()
        || s.transform.is_some()
        || s.filter.is_some()
        || s.opacity.is_some()
        || s.flex_wrap == Some(true)
        || s.grid_tracks.is_some()
        || s.grid_cols.is_some()
        || s.grid_areas.is_some()
        || s.align_items.is_some()
        || s.justify_content.is_some()
        || !matches!(s.height, None | Some(Len::Auto))
        || s.min_height.is_some()
        || s.max_height.is_some()
        || ![&s.padding.top, &s.padding.bottom, &s.padding.left, &s.padding.right, &b.top, &b.bottom, &b.left, &b.right]
            .into_iter()
            .all(zero)
        || multicol_container(s)
    {
        return None;
    }
    let mut kids = w.children.iter().filter(|n| !is_blank(n));
    let Some(Node::Element(item)) = kids.next() else {
        return None;
    };
    if kids.next().is_some()
        || item.inline
        || out_of_flow(&item.style)
        || clone_dec(item).is_none()
        || item.style.align_self.is_some()
        || item.style.order.is_some()
        || !zero(&item.style.margin.top)
        || !zero(&item.style.margin.bottom)
        || (row && !matches!(item.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))))
    {
        return None;
    }
    let mut item = item.clone();
    item.style.display = Some(Display::Block);
    item.style.margin.top = s.margin.top;
    item.style.margin.bottom = s.margin.bottom;
    Some(item)
}

/// Монолит по css-break-4 §4.1 (Blink `IsMonolithic`): замещаемый,
/// атомарный строчный, прокручиваемый, `break-inside: avoid`,
/// строчное содержимое (строк укладка не видит) — пустая
/// коробка монолитом НЕ является.
pub(crate) fn solid_box(k: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    // `contain: size` — монолит и у страниц, и в колонках (`size_monolith`).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): то же БЕЗ роста коробки от
    // вытолкнутого монолита (`705fd58`) и без правил параллельного потока
    // в `shape_full` (диапазоны за заданной высотой; `max-height` у
    // обрезающей коробки). Срез css-break + css-multicol 1495 пар:
    // 472 -> 467, потеряно 5 (`single-line-{column,row}-flex-fragmentation-
    // 010/011/051/063`, `overflow-clip-012` 0.00 -> 0.52): монолит в
    // переполняющем ребёнке выталкивал коробку с ЗАДАННОЙ высотой целиком,
    // а лишняя мера обрезающей коробки рожала колонку. С тремя правилами
    // вместе — замер `scout-break-2026-09e.md` §6.
    size_monolith(k)
        || k.style.break_inside_avoid
        || scrolls(k.style.overflow_x)
        || scrolls(k.style.overflow_y)
        || matches!(
            k.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        // Таблица и ячейка монолитами НЕ являются
        // (css-break-4 §4.1: монолитен замещаемый,
        // прокручиваемый и `break-inside: avoid`);
        // строка таблицы — да, но её не режет и укладка.
        || matches!(
            k.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        || (k.children.iter().any(|n| !is_blank(n))
            && !k.children.iter().any(block_kid))
}
/// `box-decoration-break: clone` (css-break-4 §break-decoration): блочное
/// украшение `(верх, низ)` — рамка с отбивкой, повторяемые в КАЖДОМ
/// фрагменте. Поле не входит: §break-margins «Cloned margins are always
/// truncated to zero». Длины не в точках — ноль, как в `shape_full`
/// (нестрогий `px_or`). `None` — прежний путь `slice` до последней строки.
/// Ворота — всё, что пара «украшение + поднятое тело» выразить не может:
/// строчная и монолит (свой путь), таблица (своя копия), `border-box`
/// (Blink делит заданную высоту на N·украшение — `clone-007`),
/// многоколоночник с детьми (вложенная стопка), графические эффекты (тело —
/// вложенный клон, эффект лёг бы дважды), позиционированная коробка и
/// внепоточный потомок (`clone-005.tentative`: содержащим блоком стало бы
/// поднятое тело без отбивки).
pub(crate) fn clone_dec(c: &Element) -> Option<(f32, f32)> {
    use crate::computed::Position;
    if !c.style.bdb_clone {
        return None;
    }
    fn oof_inside(e: &Element) -> bool {
        e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style) || oof_inside(k)))
    }
    if c.inline
        || solid_box(c)
        || table_box(c)
        || c.style.border_box == Some(true)
        || (multicol_container(&c.style) && c.children.iter().any(|n| !is_blank(n)))
        || c.style.transform.is_some()
        || c.style.filter.is_some()
        || c.style.mask_image.is_some()
        || c.style.opacity.is_some_and(|o| o < 1.0)
        || matches!(
            c.style.position,
            Some(Position::Sticky) | Some(Position::Absolute) | Some(Position::Fixed)
        )
        || oof_inside(c)
    {
        return None;
    }
    let px_of = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    let dt = px_of(&c.style.padding.top) + px_of(&b.top);
    let db = px_of(&c.style.padding.bottom) + px_of(&b.bottom);
    // Нулевое украшение тоже клонируется, если фрагменту есть что рисовать
    // своё: «'box-shadow' … applied to each fragment independently», «A
    // no-repeat background image will thus be rendered once in each
    // fragment» (§break-decoration). `clone-009`: у каждого квадрата свои тень
    // и контур; маска `slice` срезала бы их у обоих.
    let paints = !c.style.shadows.is_empty()
        || c.style.outline.is_some()
        || c.style.bg_image.is_some()
        || c.style.gradient.is_some();
    (dt + db > 0.01 || paints).then_some((dt, db))
}

/// Копия ОДНОГО фрагмента коробки с `box-decoration-break: clone`
/// (css-break-4 §break-decoration: «Each box fragment is independently
/// wrapped with the border, padding … The background is drawn independently
/// in each fragment»). Три коробки:
/// * `deco` — исходная коробка блоком высоты ЭТОГО фрагмента `fh`: рамка,
///   отбивка, фон, тень, скругление — свои у фрагмента;
/// * обёртка `overflow-y: clip` высотой `clip` — «сколько содержимого съел
///   этот фрагмент» (у последнего — до конца видимого переполнения:
///   Blink `fragmentation_utils.cc:435` «child content may overflow it»,
///   `clone-002`); по строчной оси не режет (`clone-012` кладёт содержимое
///   второй колонки в первую отрицательным полем);
/// * `body` — исходная коробка без краски и без рамки/отбивки/полей/ширины,
///   поднятая на `from` СДВИГОМ (`position: relative`), а не полем: у обёртки
///   без рамки поле тела схлопнулось бы сквозь неё и увезло обрезку
///   (taffy `block.rs:186`, `Clip` — не скролл-контейнер).
/// Blink устроен так же: фрагмент — свой `PhysicalBoxFragment` со всеми
/// сторонами, краска общим путём (`box_fragment_painter.cc:2322` разводит
/// только `slice`).
pub(crate) fn clone_fragment(c: &Element, dt: f32, db: f32, from: f32, fh: f32, clip: f32) -> Element {
    use crate::computed::{Overflow, Position, Sides};
    let mut body = c.clone();
    body.style = c.style.paint_off();
    body.style.padding = Sides::default();
    body.style.border_width = Sides::default();
    body.style.border_visible = [Some(false); 4];
    body.style.margin = Sides::default();
    body.style.width = None;
    body.style.min_width = None;
    body.style.max_width = None;
    body.style.position = Some(Position::Relative);
    body.style.inset = Sides::default();
    body.style.inset.top = Some(Len::Px(-from));
    body.style.bdb_clone = false;
    body.hover = None;
    body.anim = None;
    body.list_item = None;
    let clip_box = Element {
        list_item: None,
        node_id: c.node_id ^ 0x0BDB_C10E_0000_0001,
        anim: None,
        tag: "div".to_string(),
        style: Computed {
            display: Some(Display::Block),
            height: Some(Len::Px(clip.max(0.0))),
            overflow_y: Some(Overflow::Clip),
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![Node::Element(body)],
        attrs: Vec::new(),
        inline: false,
    };
    let mut deco = c.clone();
    deco.style.display = Some(Display::Block);
    deco.style.height = Some(Len::Px((fh - dt - db).max(0.0)));
    deco.style.min_height = None;
    deco.style.max_height = None;
    deco.style.column_count = None;
    deco.style.column_width = None;
    deco.style.column_height = None;
    deco.first_letter = None;
    deco.first_line = None;
    deco.children = vec![Node::Element(clip_box)];
    deco
}

/// То же плюс смещения принудительных разрывов и диапазоны
/// монолитов внутри.
pub(crate) type Shape = (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>);

/// Условия меры: `paged` — стопка страниц (монолитом считается и
/// `contain: size`, см. `solid_box`); `viewport` — размер области просмотра
/// для `vh`/`vw`: у страниц это page area, у колонок единицы окна остаются
/// неразрешёнными (`None` → отказ от меры, прежнее поведение).
#[derive(Clone, Copy)]
pub(crate) struct ShapeCx {
    pub(crate) paged: bool,
    pub(crate) viewport: Option<(f32, f32)>,
    /// Презентационный `cellpadding` таблицы — отступ ЭТОЙ ячейки поверх
    /// умолчания `td { padding: 1px }` (как в `table()`); потомкам не
    /// передаётся.
    pub(crate) cell_pad: Option<f32>,
    /// Мера ПОТОКА, а не коробки: заданная высота не обрезает ни высоту, ни
    /// точки разреза, ни монолитные диапазоны. Переполнение коробки с
    /// заданной высотой — параллельный поток (css-break-3 §3), и укладке
    /// колонок нужна его протяжённость ОТДЕЛЬНО от высоты коробки.
    /// Бюджет ОДИН на путь: флаг гаснет на первой же коробке с заданной
    /// высотой (`shape_full`, перепривязка `cx`). Вложенная ограниченная
    /// коробка заводит СВОЙ параллельный поток, в поток предка он не
    /// входит, а модель несёт один `over` на ребёнка стопки — второго
    /// потока ей выразить нечем. Через коробки с высотой `auto` флаг идёт
    /// насквозь: там своей ограниченности нет
    /// (`overflowed-block-with-room-after-003` — обёртка `auto` над
    /// коробкой 70).
    pub(crate) unclamped: bool,
}

impl ShapeCx {
    pub(crate) const COLUMNS: ShapeCx = ShapeCx {
        paged: false,
        viewport: None,
        cell_pad: None,
        unclamped: false,
    };
}

/// Кадр меры строк: наследованный стиль коробки и ширина её содержимого
/// (`None` — неизвестна, строки не меряются).
pub(crate) struct LineFrame {
    pub(crate) inh: Computed,
    pub(crate) w: Option<f32>,
    /// Анонимный блок строк хоста (`group_inline_runs`): его срез — срез хоста.
    pub(crate) anon: bool,
    /// Ширина детей-элементов этой коробки (`items_kind`): 0 — блочный поток,
    /// 1 — растянутые на всю ширину (колонка flex и сетка-стопка при
    /// `stretch`), 2 — ширина по раскладке (ряд flex, прочая сетка): известна
    /// лишь заданная в точках.
    pub(crate) items: u8,
}

/// Контекст меры строк для `shape_full`: включается только вокруг меры детей
/// стопки колонок (`with_lines`), где ширина колонки известна. `shape_full` о
/// наследовании и ширине ничего не знает (ей дают голый элемент), поэтому
/// кадры ведёт `LineScope` на входе в неё.
pub(crate) struct LineCx {
    pub(crate) opts: RenderOpts,
    pub(crate) frames: Vec<LineFrame>,
}

thread_local! {
    pub(crate) static LINE_CX: std::cell::RefCell<Option<LineCx>> = const { std::cell::RefCell::new(None) };
}

/// Выполнить `f` с контекстом меры строк: `base` — стиль многоколоночника,
/// `w` — строчный размер колонки.
pub(crate) fn with_lines<T>(base: &Computed, w: Option<f32>, opts: &RenderOpts, f: impl FnOnce() -> T) -> T {
    let prev = LINE_CX.with(|l| {
        l.borrow_mut().replace(LineCx {
            opts: opts.clone(),
            frames: vec![LineFrame { inh: base.clone(), w, anon: false, items: 0 }],
        })
    });
    let out = f();
    LINE_CX.with(|l| *l.borrow_mut() = prev);
    out
}

/// Ширина содержимого блока в потоке родителя шириной `pw` (CSS 2.1 §10.3.3:
/// `margin-left + border + padding + width + … = containing block width`).
/// Только обычный блок потока — у прочих ширину решает своя раскладка.
pub(crate) fn line_content_w(c: &Element, pw: f32) -> Option<f32> {
    let s = &c.style;
    // Блочный flex-контейнер и сетка в потоке занимают ширину как блок
    // (css-flexbox-1 §9.2 / css-grid-2 §6.1: «block-level … sized as a
    // block»); ширину ИХ детей решает `items_kind`.
    if c.inline
        || !matches!(
            s.display,
            None | Some(Display::Block) | Some(Display::ListItem) | Some(Display::Flex) | Some(Display::Grid)
        )
        || s.webkit_box == Some(true)
        || s.float.unwrap_or(0) != 0
        || !matches!(s.position, None | Some(crate::computed::Position::Relative))
        || table_box(c)
        || multicol_container(s)
    {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    // Боковые поля копии фрагмента кладёт обёртка (`side_margin_wrap`): корень
    // `layout_as_root` своих полей не читает, а под обёрткой копия — обычный
    // ребёнок. ★ Прежде (03.10) замер обёртки дал `multicol-nested-002` 0.00 ->
    // 2.67 из-за концевого поля в балансе — теперь оно в `balance_line`.
    let b = s.borders();
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    match s.width {
        Some(Len::Px(w)) => Some(if s.border_box == Some(true) { (w - edges).max(0.0) } else { w }),
        None | Some(Len::Auto) => {
            Some((pw - px(&s.margin.left)? - px(&s.margin.right)? - edges).max(0.0))
        }
        _ => None,
    }
}

/// Элемент КОЛОНКИ flex без переноса с главным размером по `flex-basis`
/// (css-flexbox-1 §9.2 шаг 3): `flex-basis: content` — по содержимому, и
/// `height` при этом не действует. Контейнер `height: auto` свободного места не
/// даёт, и гибкость базу не меняет (§9.7). `None` — мера по `height`
/// элемента, как прежде.
pub(crate) fn basis_sized(c: &Element, k: &Element) -> Option<Element> {
    use crate::computed::FlexDir;
    let s = &c.style;
    if k.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.vertical == Some(true)
        || s.flex_wrap == Some(true)
        || !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
        || !matches!(s.height, None | Some(Len::Auto))
    {
        return None;
    }
    let mut kk = k.clone();
    // База в точках здесь не ставится: при `min-height: auto` элемент не
    // меньше своего содержимого (§4.5), а этой меры у нас нет.
    if k.style.basis_content != Some(true) {
        return None;
    }
    kk.style.height = None;
    Some(kk)
}

/// Хвост непоследнего фрагмента обычной коробки (`flow::StackChild::slack`):
/// фрагмент, разорванный внутри коробки, занимает остаток фрагментаинера
/// (css-break-3 §box-splitting «the box … continues to the end of the
/// fragmentainer»; Blink `fragmentation_utils.cc` «Consumed block-size … is
/// always stretched to the fragmentainers»). Художник хвоста красит его одним
/// цветом по ширине копии — это точно, лишь когда у коробки сплошной фон без
/// картинки и скруглений, а видимые боковые рамки того же цвета. Иначе `None`.
pub(crate) fn slack_fill(c: &Element) -> Option<gpui::Hsla> {
    let s = &c.style;
    let bg = s.background?;
    if s.bg_image.is_some()
        || s.webkit_box == Some(true)
        || [&s.radius.tl, &s.radius.tr, &s.radius.br, &s.radius.bl]
            .into_iter()
            .any(|r| !matches!(r, None | Some(Len::Px(0.0))))
        || !visible_overflow(s)
    {
        return None;
    }
    let b = s.borders();
    for (i, w) in [(1usize, &b.right), (3usize, &b.left)] {
        let wide = match w {
            None => false,
            Some(Len::Px(v)) => *v > 0.0,
            Some(_) => true,
        };
        if wide && s.border_colors[i].or(s.border_color).or(s.color) != Some(bg) {
            return None;
        }
    }
    Some(bg.to_hsla())
}

/// Как ширина детей коробки `c` известна мере строк (`LineFrame::items`).
pub(crate) fn items_kind(c: &Element) -> u8 {
    use crate::computed::FlexDir;
    let s = &c.style;
    match s.display {
        Some(Display::Flex) => {
            let col = matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            let stretch = matches!(s.align_items, None | Some(Align::Stretch));
            if col && stretch && s.vertical != Some(true) { 1 } else { 2 }
        }
        Some(Display::Grid) => {
            if grid_stack(c) && matches!(s.justify_items, None | Some(Align::Stretch)) { 1 } else { 2 }
        }
        _ => 0,
    }
}

/// Кадр меры строк на время `shape_full(c)`.
pub(crate) struct LineScope(pub(crate) bool);

impl LineScope {
    pub(crate) fn enter(c: &Element) -> Self {
        LINE_CX.with(|l| {
            let mut g = l.borrow_mut();
            let Some(cx) = g.as_mut() else {
                return LineScope(false);
            };
            let Some(top) = cx.frames.last() else {
                return LineScope(false);
            };
            let inh = crate::inline::inherit(&top.inh, &c.style);
            let w = top.w.and_then(|pw| match top.items {
                // Элемент flex/сетки шириной по раскладке: известна лишь
                // заданная в точках.
                2 if !matches!(c.style.width, Some(Len::Px(_))) => None,
                // Растянутый элемент (css-flexbox-1 §9.4 шаг 11 / css-grid-2
                // §11.3 `stretch`): ширина как у блока в потоке, если сам
                // элемент выравнивание не переопределил.
                1 if c.style.align_self.is_some() || c.style.justify_self.is_some() => None,
                _ => line_content_w(c, pw),
            });
            cx.frames.push(LineFrame { inh, w, anon: c.tag == "anon-block", items: items_kind(c) });
            LineScope(true)
        })
    }
}

impl Drop for LineScope {
    fn drop(&mut self) {
        if self.0 {
            LINE_CX.with(|l| {
                if let Some(cx) = l.borrow_mut().as_mut() {
                    cx.frames.pop();
                }
            });
        }
    }
}

/// Текст строчного содержимого для меры строк: `<br>` — `\n`, пробелы
/// схлопнуты (css-text-3 §4.1.1). `None` — среди детей есть то, что строку
/// меняет сверх голого текста (атом, свой шрифт, отбивка, внепоточный).
pub(crate) fn line_text(nodes: &[Node]) -> Option<String> {
    fn gather(nodes: &[Node], out: &mut String) -> bool {
        for n in nodes {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) if e.tag == "br" => out.push('\u{2028}'),
                // Абсолют в строке места не занимает (CSS 2.1 §9.6): строку
                // не меняет, рисуется копией фрагмента от своего содержащего
                // блока (`css-position/multicol/*-in-multicols`).
                Node::Element(e)
                    if matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
                    ) => {}
                Node::Element(e) => {
                    let s = &e.style;
                    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                    let b = s.borders();
                    if !e.inline
                        || s.display.is_some()
                        || out_of_flow(&s)
                        // Относительный сдвиг куска строку не меняет (CSS 2.1
                        // §9.4.3: «after laying out … shifted»), рисует его копия
                        // (`text-box-trim-multicol-002-ref`: `<span
                        // style="position: relative">`).
                        || !matches!(s.position, None | Some(crate::computed::Position::Relative))
                        || s.font_size.is_some()
                        || s.font_family.is_some()
                        || s.font_weight.is_some()
                        || s.italic.is_some()
                        || s.line_height.is_some()
                        || s.vertical_align.is_some()
                        || s.letter_spacing.is_some()
                        || !zero(&s.padding.left)
                        || !zero(&s.padding.right)
                        || !zero(&s.margin.left)
                        || !zero(&s.margin.right)
                        || !zero(&b.left)
                        || !zero(&b.right)
                        || matches!(e.tag.as_str(), "img" | "svg" | "input" | "button" | "select" | "textarea" | "ruby" | "canvas" | "video" | "iframe" | "object" | "embed")
                        || !gather(&e.children, out)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
    let mut raw = String::new();
    if !gather(nodes, &mut raw) {
        return None;
    }
    let mut out = String::new();
    let mut prev_space = false;
    for ch in raw.chars() {
        if ch == '\u{2028}' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
            prev_space = true;
            continue;
        }
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    let mut out = out.trim_matches(' ').to_string();
    // Последний `<br>` строки не открывает (CSS 2.1 §9.4.2: перевод строки
    // завершает текущую строчную коробку; пустой хвостовой коробки нет).
    if out.ends_with('\n') {
        out.pop();
    }
    (!out.trim().is_empty()).then_some(out)
}

/// Мера блока со СТРОЧНЫМ содержимым по строкам (css-break-3 §4.3: разрыв
/// «between line boxes» — законная точка класса B; §4.4 `orphans`/`widows`).
/// Высота — строки × высота строки; точки разреза — границы строк, кроме
/// первых `orphans` и последних `widows`. Blink: `inline_layout_algorithm.cc`
/// + `BreakBeforeChildIfNeeded` для строк (`block_layout_algorithm.cc`
/// `HandleInflow` → `IsBreakInside` по строкам). Только при включённом
/// контексте (`with_lines`) и известной ширине колонки; иначе `None`, и мера
/// идёт прежним путём (сплошной строчный набор — монолит).
pub(crate) fn line_run_shape(c: &Element, top: f32, bot: f32, mt: f32, mb: f32) -> Option<Shape> {
    // Высота в точках — коробка своей высоты, строки внутри неё режутся так
    // же (css-break-3 §4.3); строки ниже её низа — переполнение, точек там нет.
    let fixed_h = match c.style.height {
        None | Some(Len::Auto) => None,
        Some(Len::Px(v)) if v >= 0.0 => Some(if c.style.border_box == Some(true) {
            (v - top - bot).max(0.0)
        } else {
            v
        }),
        _ => return None,
    };
    if c.style.min_height.is_some()
        || c.style.max_height.is_some()
        || c.children.iter().all(is_blank)
    {
        return None;
    }
    // Срез строк на РАЗРЫВАХ колонок: `text-box-trim` САМОГО многоколоночника
    // (первый кадр `with_lines`) режет строки у верха и низа каждой колонки
    // (csswg-drafts#5335, comment-2380160677; `text-box-trim-multicol-001`:
    // в первой колонке 4 строки вместо 3, вторая — от самого верха). Срез у
    // блока-ребёнка на разрывах повторяется только при `box-decoration-break:
    // clone` — каждый его фрагмент целая коробка со своей первой и последней
    // строкой (css-break-4 §break-decoration; `-003`); при `slice` — лишь у
    // первой и последней строки (`-002-ref`: вторая колонка с полулидингом).
    // Сторона решается БЛИЖАЙШЕЙ коробкой со срезом этой стороны
    // (`brk_trim`).
    let (inh, w, opts, brk_start, brk_end) = LINE_CX.with(|l| {
        let g = l.borrow();
        let cx = g.as_ref()?;
        let f = cx.frames.last()?;
        Some((
            f.inh.clone(),
            f.w?,
            cx.opts.clone(),
            brk_trim(&cx.frames, |s| s.text_box_trim_start),
            brk_trim(&cx.frames, |s| s.text_box_trim_end),
        ))
    })?;
    if inh.nowrap == Some(true)
        || inh.keep_spaces == Some(true)
        || inh.preserve_newlines == Some(true)
        || inh.letter_spacing.is_some()
        || !matches!(inh.text_indent, None | Some(Len::Px(0.0)))
        || c.first_line.is_some()
        || c.first_letter.is_some()
    {
        return None;
    }
    let text = line_text(&c.children)?;
    let size = match inh.font_size {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => return None,
    };
    let lh = match inh.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        None => size * normal_fraction(&inh, &opts),
        _ => return None,
    };
    if lh <= 0.0 {
        return None;
    }
    let font = measure_font(&inh, &opts);
    let lines = crate::metrics::line_count(&font, size, &text, w)?.max(1);
    let orphans = inh.orphans.unwrap_or(2).max(1) as usize;
    let widows = inh.widows.unwrap_or(2).max(1) as usize;
    // Свой срез первой/последней строки (`blocks()` кладёт его отрицательным
    // полем у первого и последнего ребёнка — коробка ужимается) и срез у
    // разрыва: строка до разрыва кончается на своей метрике, строка после —
    // с неё начинается (`text-box-trim-multicol-001…012`).
    let tt = trim_amount(&inh, size, lh, true);
    let tb = trim_amount(&inh, size, lh, false);
    let shift = if c.style.text_box_trim_start || c.attr("kamin-host-trim-start").is_some() {
        tt
    } else {
        0.0
    };
    let tail = if c.style.text_box_trim_end || c.attr("kamin-host-trim-end").is_some() {
        tb
    } else {
        0.0
    };
    let content = fixed_h.unwrap_or((lines as f32 * lh - shift - tail).max(0.0));
    let h = top + content + bot;
    let cut_need = if brk_end { tb } else { 0.0 };
    let cut_from = if brk_start { tt } else { 0.0 };
    let mut cuts: Vec<(f32, f32)> = (orphans..=lines.saturating_sub(widows))
        .filter(|k| *k >= 1 && *k < lines && (*k as f32) * lh - shift < content - 0.01)
        .map(|k| {
            let at = top + k as f32 * lh - shift;
            ((at - cut_need).max(top), at + cut_from)
        })
        .collect();
    // Разрыв ПЕРЕД первой строкой (блок целиком уходит в следующую колонку):
    // и там строка у верха колонки срезается (`text-box-trim-multicol-012`:
    // `orphans: 2` уводит все строки во вторую колонку, первая — от её верха).
    // Точка в нуле: кусок нулевой высоты в текущей колонке, продолжение — с
    // метрики первой строки.
    if cut_from > 0.0 && shift <= 0.0 && top <= 0.0 && fixed_h.is_none() {
        cuts.insert(0, (0.0, cut_from));
    }
    // Строка неразрывна (css-break-3 §4.3: разрыв только МЕЖДУ строками), а
    // первые `orphans` и последние `widows` строк — одним куском (§4.4).
    // Монолитные диапазоны — промежутки между законными точками: край колонки
    // внутри диапазона уводит разрыв к его началу (`flow.rs` `fill_at`), а не
    // режет строку пополам (балансу было всё равно, где резать: `multicol-
    // margin-001`, кусок 13.33 из строки 20).
    let mut solid = Vec::new();
    let mut from = 0.0f32;
    for &(need, next) in &cuts {
        solid.push((from, need));
        // Срезанная полоса у разрыва — тоже без разрыва внутри: край колонки в
        // ней уводит разрыв к её началу, то есть ровно в точку (`need`), а не
        // режет по краю (`fill_at`: край вне диапазонов — срез по краю).
        if next > need + 0.01 {
            solid.push((need, next));
        }
        from = next;
    }
    solid.push((from, h));
    Some((h, mt, mb, cuts, Vec::new(), solid))
}

/// Срез строк у разрыва колонки с одной стороны: ближайшая к строке коробка
/// со срезом этой стороны решает — многоколоночник (первый кадр: каждая
/// колонка — его фрагментаинер) или коробка с `box-decoration-break: clone`
/// режут у каждого разрыва, коробка `slice` — только у своих первой/последней
/// строки, и срез многоколоночника под ней не действует
/// (`text-box-trim-multicol-004`: блок `trim-start` под `trim-both` — низ
/// первой колонки срезан, верх второй нет; `-005` — наоборот).
pub(crate) fn brk_trim(frames: &[LineFrame], side: impl Fn(&Computed) -> bool) -> bool {
    // Анонимный блок строк — строки самого хоста, его флаг — копия хостового.
    match frames.iter().rposition(|f| side(&f.inh) && !f.anon) {
        Some(0) => true,
        Some(i) => frames[i].inh.bdb_clone,
        None => false,
    }
}

/// Срез `text-box-trim` с одной стороны строки (css-inline-3 §4.2): полулидинг
/// плюс расстояние от подъёма/спуска до метрики края — та же арифметика, что у
/// `blocks()` (`trim_for`).
pub(crate) fn trim_amount(s: &Computed, size: f32, lh: f32, start: bool) -> f32 {
    let family = s.font_family.clone().unwrap_or_default();
    let (ascent, descent, cap) = crate::metrics::vmetrics_px(&family, size);
    let half = (lh - (ascent + descent)) / 2.0;
    let edge = if start {
        match s.text_box_over {
            crate::computed::TextEdge::Cap => ascent - cap,
            crate::computed::TextEdge::Ex => ascent - crate::metrics::ch_ex_px(&family, size).1,
            _ => 0.0,
        }
    } else {
        match s.text_box_under {
            crate::computed::TextEdge::Alphabetic => {
                descent + crate::fonts::alphabetic_em(&family) * size
            }
            _ => 0.0,
        }
    };
    (half + edge).max(0.0)
}

/// Сплошной строчный набор (без блочных детей) — монолит в стопке, ПОКА его
/// строки не измерены (`line_run_shape` дала точки разреза).
pub(crate) fn inline_content(k: &Element) -> bool {
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    k.children.iter().any(|n| !is_blank(n)) && !k.children.iter().any(block_kid)
}

/// Клон поддерева с длинами коробки в ТОЧКАХ: `em`/`ch`/`ex`/`rem` в
/// размерах, полях, отбивках, рамках и вставках разрешены по кеглю своего
/// элемента (тем же `inline::inherit` → `Computed::resolve_em`, что и при
/// отрисовке). `shape_full` читает голый стиль, и `margin-top: 2em` мерился
/// нулём (нестрогий `px_or`), а `height: 4em` — отказом всей стопки, хотя
/// рисунок кладёт их в точках. Наследуемые (`font-size`, `line-height`) не
/// трогаются: число в `line-height` наследуется множителем.
pub(crate) fn resolved_lengths(c: &Element, parent: &Computed) -> Element {
    let m = crate::inline::inherit(parent, &c.style);
    let mut t = c.clone();
    // Подменяются ТОЛЬКО шрифтовые единицы, разрешённые в точки: прочее
    // (`None`, `auto`, доли, точки) остаётся как было — `inherit` дописывает
    // и умолчания, а на `min-height.is_some()` стоят гейты строк flex
    // (`multi-line-row-flex-fragmentation-018/037`).
    fn fix(own: &mut Option<Len>, res: Option<Len>) {
        if own.is_some_and(|l| !matches!(l, Len::Px(_) | Len::Auto | Len::Pct(_)))
            && matches!(res, Some(Len::Px(_)))
        {
            *own = res;
        }
    }
    fn fix_sides(own: &mut crate::computed::Sides, res: &crate::computed::Sides) {
        fix(&mut own.top, res.top);
        fix(&mut own.right, res.right);
        fix(&mut own.bottom, res.bottom);
        fix(&mut own.left, res.left);
    }
    fix(&mut t.style.width, m.width);
    fix(&mut t.style.height, m.height);
    fix(&mut t.style.min_width, m.min_width);
    fix(&mut t.style.min_height, m.min_height);
    fix(&mut t.style.max_width, m.max_width);
    fix(&mut t.style.max_height, m.max_height);
    fix_sides(&mut t.style.margin, &m.margin);
    fix_sides(&mut t.style.padding, &m.padding);
    fix_sides(&mut t.style.border_width, &m.border_width);
    fix_sides(&mut t.style.inset, &m.inset);
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => Node::Element(resolved_lengths(k, &m)),
            other => other.clone(),
        })
        .collect();
    t
}

/// Вложенный многоколоночник, который внешняя стопка ведёт РЯДАМИ (`nest_row`):
/// обычный блок с колонками, высотой в точках, без своих рядов, спаннеров,
/// внепоточных и вертикального письма.
pub(crate) fn nested_rows_box(c: &Element) -> bool {
    let s = &c.style;
    multicol_container(s)
        && (s.column_count.is_some_and(|n| n > 1) || s.column_width.is_some())
        && s.column_height.is_none()
        && s.column_wrap.is_none()
        && (matches!(s.height, Some(Len::Px(h)) if h > 0.0)
            || (matches!(s.height, None | Some(Len::Auto))
                && s.min_height.is_none()
                && s.max_height.is_none()))
        && matches!(s.display, None | Some(Display::Block))
        && s.vertical != Some(true)
        && s.float.unwrap_or(0) == 0
        && !has_deep_spanner(c)
        && !carries_abspos(c, 4)
        // Ряды считаются от верха СОДЕРЖИМОГО: блочные рамка и отбивка (и их
        // повтор у `box-decoration-break: clone`) сдвинули бы границы рядов с
        // границ внешних колонок (`box-decoration-break-clone-010`).
        && !s.bdb_clone
        && {
            // Нижние рамка и отбивка у `height: auto` допустимы: они встают
            // после последнего ряда (`multicol-breaking-006`), а у заданной
            // высоты сдвинули бы последний ряд.
            let b = s.borders();
            let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
            let auto = matches!(s.height, None | Some(Len::Auto));
            zero(&s.padding.top)
                && zero(&b.top)
                && (auto || (zero(&s.padding.bottom) && zero(&b.bottom)))
        }
        && c.children.iter().any(|n| !is_blank(n))
        // Внепоточный потомок — содержащий блок и дотяг рядами не выражены
        // (`out-of-flow-in-multicolumn-019`).
        && !oof_descendant(c)
}

pub(crate) fn oof_descendant(e: &Element) -> bool {
    e.children
        .iter()
        .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style) || oof_descendant(k)))
}

/// Мера вложенного многоколоночника с `height: auto`, который внешняя стопка
/// ведёт рядами во внешний фрагментаинер `hh` (`nest_row`): блочный размер —
/// полные ряды по `hh` и сбалансированный последний (css-multicol-1 §7.1
/// «only the last fragment is balanced»; Blink `column_layout_algorithm.cc`
/// `LayoutRow` с `ConstrainColumnBlockSize`), плюс нижние рамка и отбивка.
/// Та же укладка (`ColumnStack::measure_rows`) и те же меры детей
/// (`resolved_lengths` + `with_lines`), что у копии через `element()`.
/// Точек разреза нет: внешняя стопка режет коробку краем колонки — по рядам.
pub(crate) fn nested_rows_shape(c: &Element, parent: &Computed, hh: f32, cw: f32, opts: &RenderOpts) -> Option<Shape> {
    let m = inline::inherit(parent, &c.style);
    let w = nested_box_w(c, cw)?;
    let n = match m.column_count {
        Some(n) if n > 1 => n as usize,
        _ => return None,
    };
    if m.column_width.is_some() {
        return None;
    }
    let gap = match m.column_gap {
        Some(Len::Px(v)) => v,
        _ => match m.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        },
    };
    let col_w = ((w + gap) / n as f32 - gap).max(0.0);
    let mut mc = c.clone();
    mc.style.width = Some(Len::Px(w));
    let g = group_inline_runs(&mc).unwrap_or(mc);
    let kids: Vec<crate::flow::Kid> = with_lines(&m, Some(col_w), opts, || {
        g.children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                // Только строчное содержимое (анонимные блоки строк): ряды по
                // строкам у нас сходятся с Blink, а блочные дети с
                // переполнением своей коробки и монолиты выше ряда ведут себя
                // иначе (`multicol-fill-balance-003/030`, `multicol-nested-026/
                // 031` при блочных детях уходили 0.00 → «красное видно»).
                Node::Element(k)
                    if !k.inline
                        && inline_content(k)
                        && !out_of_flow(&k.style)
                        && matches!(k.style.position, None | Some(crate::computed::Position::Relative))
                        && k.style.float.unwrap_or(0) == 0 =>
                {
                    let k = resolved_lengths(k, &m);
                    let sh = shape_full(&k, 4, ShapeCx::COLUMNS)?;
                    Some(crate::flow::Kid {
                        h: sh.0,
                        mt: sh.1,
                        mb: sh.2,
                        monolith: solid_box(&k) && !(inline_content(&k) && !sh.3.is_empty()),
                        cuts: sh.3,
                        force_before: edge_break(&k, false),
                        force_after: edge_break(&k, true),
                        avoid_before: edge_avoid(&k, false),
                        avoid_after: edge_avoid(&k, true),
                        forced: sh.4,
                        solid: sh.5,
                        span: false,
                        over: sh.0,
                        clone_dec: None,
                        overflow_top: false,
                        repeat: Default::default(),
                        par: Default::default(),
                    })
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    })?;
    if kids.is_empty() {
        return None;
    }
    let fixed = (m.column_fill_auto == Some(true)).then_some(hh);
    let rows = crate::flow::Rows {
        h: Some(hh),
        gap: 0.0,
        wrap: true,
        cap: false,
    };
    let content = crate::flow::ColumnStack::measure_rows(&kids, n, gap, fixed, rows);
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = c.style.borders();
    let bot = px(&c.style.padding.bottom)? + px(&b.bottom)?;
    let h = content + bot;
    let solid = if bot > 0.0 { vec![(content, h)] } else { Vec::new() };
    Some((h, 0.0, px(&c.style.margin.bottom)?, Vec::new(), Vec::new(), solid))
}

/// Ширина коробки (`width` по её `box-sizing`) ребёнка в колонке `cw`.
pub(crate) fn nested_box_w(c: &Element, cw: f32) -> Option<f32> {
    let s = &c.style;
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = s.borders();
    let outer = cw - px(&s.margin.left)? - px(&s.margin.right)?;
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    Some(if s.border_box == Some(true) { outer } else { outer - edges }.max(0.0))
}

/// Копия ребёнка стопки встаёт КОРНЕМ (`flow.rs` `layout_as_root` во всю
/// колонку), а корень taffy своих полей не кладёт: боковое поле `margin: 0 1em`
/// пропадало, текст ложился от края колонки (`multicol-nested-002`). Обёртка-
/// колонка делает копию обычным ребёнком: её поля и растяжение решает
/// раскладка (CSS 2.1 §10.3.3). Только горизонтальная стопка и только при
/// ненулевых полях в точках — иначе копия прежняя.
pub(crate) fn side_margin_wrap(el: AnyElement, copy: &Element, vertical: bool) -> AnyElement {
    let nz = |l: &Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
    if vertical || !(nz(&copy.style.margin.left) || nz(&copy.style.margin.right)) {
        return el;
    }
    div().flex().flex_col().w_full().child(el).into_any_element()
}

/// Строчные прогоны среди блочных детей многоколоночника — в анонимные блоки
/// (CSS 2.1 §9.2.1.1: «If a block container box has a block-level box inside
/// it, then we force it to have only block-level boxes inside it» — строчное
/// содержимое оборачивается анонимной блочной коробкой). Тогда стопка колонок
/// видит их обычными детьми и режет по строкам. `None` — заворачивать нечего.
pub(crate) fn group_inline_runs(e: &Element) -> Option<Element> {
    let inline_level = |n: &Node| match n {
        Node::Text(_) => true,
        Node::Element(k) => k.inline && !out_of_flow(&k.style) && k.style.display.is_none(),
    };
    if !e.children.iter().any(|n| inline_level(n) && !is_blank(n)) {
        return None;
    }
    let mut out: Vec<Node> = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.iter().all(is_blank) {
            out.append(run);
        } else {
            out.push(Node::Element(anon_element("anon-block", std::mem::take(run))));
        }
    };
    for n in &e.children {
        if inline_level(n) {
            run.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    // `text-box-trim` хоста режет его ПЕРВУЮ/ПОСЛЕДНЮЮ отформатированную
    // строку (css-inline-3 §4.2). Строки ушли в анонимные блоки — флаг едет
    // туда, где строка: первому анонимному, если он первый ребёнок, и
    // последнему, если последний (`text-box-trim-multicol-001`).
    if e.style.text_box_trim_start || e.style.text_box_trim_end {
        let flow: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n))
            .map(|(i, _)| i)
            .collect();
        let anon = |n: &Node| matches!(n, Node::Element(k) if k.tag == "anon-block");
        if e.style.text_box_trim_start
            && let Some(&i) = flow.first()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_start = true;
        }
        if e.style.text_box_trim_end
            && let Some(&i) = flow.last()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_end = true;
        }
    }
    let mut g = e.clone();
    g.children = out;
    Some(g)
}

/// Клон поддерева, у которого ФИЗИЧЕСКИЕ поля коробки повёрнуты так, что
/// БЛОЧНАЯ ось вертикального письма встаёт на место вертикальной: `width` ↔
/// `height`, стороны — по логическим ролям (css-writing-modes-4 §3.1, §6.4
/// «abstract-to-physical mappings»): новый верх — block-start (левый край у
/// `vertical-lr`, правый у `vertical-rl`), новый низ — block-end, новые лево/право
/// — inline-start/-end (верх/низ при `direction: ltr`).
///
/// Нужен ТОЛЬКО мере стопки колонок: `shape_full` написана в терминах блочного
/// потока (`h` — размер по оси потока, `cuts`/`solid` — смещения от его начала),
/// но читает физические поля. На повёрнутом клоне её `h` — блочный размер, а
/// `flex-direction: row` остаётся строчной осью (в вертикали она вертикальна) —
/// дети ряда стоят рядом, точек разреза между ними нет, как и должно быть.
/// Рисуется по-прежнему ИСХОДНЫЙ элемент: повернуть отрисовку нельзя, вместе с
/// коробкой повернулись бы текст, рамки и фон. Blink делает то же логическими
/// величинами (`BoxStrut`/`LogicalSize` в `block_layout_algorithm.cc`).
///
/// `None` — в поддереве потомок с ДРУГИМ письмом (ортогональный поток,
/// css-writing-modes-4 §7.3, или обратная блочная ось) либо `direction: rtl`:
/// поворотом его мера не выражается, и многоколоночник остаётся на прежнем
/// пути.
pub(crate) fn transpose_tree(c: &Element, rl: bool) -> Option<Element> {
    if c.style.vertical == Some(false)
        || c.style.vertical_rl.is_some_and(|v| v != rl)
        || c.style.rtl == Some(true)
    {
        return None;
    }
    let turn = |s: &crate::computed::Sides| crate::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: s.top,
        right: s.bottom,
    };
    let mut t = c.clone();
    std::mem::swap(&mut t.style.width, &mut t.style.height);
    std::mem::swap(&mut t.style.min_width, &mut t.style.min_height);
    std::mem::swap(&mut t.style.max_width, &mut t.style.max_height);
    t.style.padding = turn(&c.style.padding);
    t.style.margin = turn(&c.style.margin);
    t.style.border_width = turn(&c.style.border_width);
    t.style.inset = turn(&c.style.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = c.style.border_visible;
    t.style.border_visible = if rl {
        [v[1], v[2], v[3], v[0]]
    } else {
        [v[3], v[2], v[1], v[0]]
    };
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    t.style.overflow_y = c.style.overflow_x;
    t.style.overflow_x = c.style.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = c.style.border_spacing {
        t.style.border_spacing = Some((y, x));
    }
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => transpose_tree(k, rl).map(Node::Element),
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<Node>>>()?;
    Some(t)
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164, `scout-breakcore-2026-09.md` FRAG-FLEX-WRAP,
/// 11 хунков): сбор строк гибкого контейнера с `flex-wrap` при фрагментации
/// (`flex_lines`/`flex_item_main_w`, `ShapeCx::col_w`, `wrap_end`, ветка
/// «колонка = параллельные потоки, ряд = стопка строк», `max(высота, низ
/// содержимого)`). Обещание +2…+9. Полный свод против v36: +4
/// (`multi-line-row-flex-fragmentation-083a…d`) / −15 (`multi-line-column-
/// flex-fragmentation-009/012/014/038`, `multi-line-row-flex-fragmentation-
/// 007/011/018/020/022/023/029` → «красное видно», `-035/-039/-040/-059`).
/// Строки собираются, но контейнер с переносом теряет высоту фрагмента: пары,
/// которые держались стопкой детей, разваливаются. Половинить нельзя (это и
/// есть откат 04.09); брать заново только с мерой по строкам (FRAG-LINES).
pub(crate) fn shape_contents(c: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    // Кадр меры строк (наследование и ширина) — только при `with_lines`.
    let _line_frame = LineScope::enter(c);
    let px_or = |l: &Option<Len>, strict: bool| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Vw(k)) if cx.viewport.is_some() => Some(*k * cx.viewport.unwrap().0),
        Some(Len::Vh(k)) if cx.viewport.is_some() => Some(*k * cx.viewport.unwrap().1),
        Some(_) if !strict => Some(0.0),
        _ => None,
    };
    // Коробка из одних флоатов (`float_only_box`): мера — высота их ряда,
    // точек разреза внутри нет (флоаты пустые), рамка сверху — монолит, как
    // у общей ветки ниже.
    if matches!(c.style.height, None | Some(Len::Auto))
        && c.style.min_height.is_none()
        && let Some(tall) = float_only_box(c)
    {
        let b = c.style.borders();
        let top = px_or(&c.style.padding.top, false)? + px_or(&b.top, false)?;
        let bot = px_or(&c.style.padding.bottom, false)? + px_or(&b.bottom, false)?;
        let mt = px_or(&c.style.margin.top, false)?;
        let mb = px_or(&c.style.margin.bottom, false)?;
        let solid = if top > 0.0 { vec![(0.0, top)] } else { Vec::new() };
        return Some((top + tall + bot, mt, mb, Vec::new(), Vec::new(), solid));
    }
    if has_float(c, 3) {
        return None;
    }
    // Таблица — своя мера: ряды стопкой, зазоры `border-spacing`, точки
    // класса A между рядами (css-break-4 §possible-breaks). Неизмеримая
    // (сросшиеся рамки, `rowspan`, подпись, заданная высота) идёт прежним
    // путём — стопкой блоков по тегу: `return None` здесь отнимал у КОЛОНОК
    // точки внутри такой таблицы (`border-collapse-001`, флаг выключен:
    // 0.20 → 2.25 между базой v27 и v79).
    // У колонок неизмеримая таблица по-прежнему `None`: сквозной путь по тегу
    // хранит перенос принудительного разрыва ячейки на таблицу
    // (`break-after-table-cell`, `-child`: 0.00 → 2.08 без гейта, срез
    // `L-brk` 2874 пар, +0/−2). Страницы — стопкой блоков.
    if table_box(c) {
        match table_shape(c, depth, cx) {
            Some(s) => return Some(s),
            None if !cx.paged => return None,
            None => {}
        }
    }
    let b = c.style.borders();
    let mt = px_or(&c.style.margin.top, false)?;
    let mb = px_or(&c.style.margin.bottom, false)?;
    // `cellpadding` — только этой коробке (её кладёт `table_shape`).
    let cell_pad = cx.cell_pad;
    // Бюджет разжатия — ОДИН на путь. Мера потока снимает обрезку заданной
    // высотой только у ВЕРХНЕЙ ограниченной коробки: по css-break-3 §3
    // вложенная ограниченная коробка заводит СВОЙ параллельный поток, и её
    // переполнение в поток предка не входит. Модель несёт один `over` на
    // ребёнка стопки — второй поток ей выразить нечем, значит и мерить его
    // нельзя.
    // ЗАМЕРЕНО (10.09, `target/scout-fragparallel-2026-09b.md`): без бюджета
    // мера ЭТАЛОНА `flex-item-content-overflow-001-ref` (коробка 70 >
    // элемент 50 > внук 140) росла 70 -> 170, эталон разъезжался по двум
    // колонкам, и четыре пары `flex-item-content-overflow-001a/001b/002a/
    // 002b` уходили 0.00 -> 0.81. С бюджетом та же мера даёт ровно 70:
    // `over == h`, ветка потока не включается, `StackChild` байт-в-байт
    // прежний.
    // Приобретения целы: у них ограниченная коробка на пути ОДНА, а её
    // ребёнок задаёт высоту сам и в неё умещается
    // (`overflowed-block-with-room-after-000`: 70 > 200 > 70+60+70).
    let unclamp = cx.unclamped;
    let cx = ShapeCx {
        cell_pad: None,
        unclamped: cx.unclamped && c.style.height.is_none(),
        ..cx
    };
    let pad = |l: &Option<Len>| match cell_pad {
        Some(v) if matches!(l, Some(Len::Px(p)) if *p == 1.0) => Some(v),
        _ => px_or(l, false),
    };
    let top = pad(&c.style.padding.top)? + px_or(&b.top, false)?;
    let bot = pad(&c.style.padding.bottom)? + px_or(&b.bottom, false)?;
    // Строчное содержимое — по строкам, если ширина колонки известна.
    if !cx.paged
        && inline_content(c)
        && matches!(c.style.display, None | Some(Display::Block) | Some(Display::ListItem))
        && let Some(s) = line_run_shape(c, top, bot, mt, mb)
    {
        return Some(s);
    }
    let mut kids: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    // Гибкий контейнер, чьи элементы идут СТОПКОЙ (колонка; перенос по
    // строкам — пока «строка = элемент»). css-flexbox-1 §4.2: «The margins of
    // adjacent flex items do not collapse», и сквозь край контейнера поле
    // элемента не уходит (контейнер — свой контекст); между элементами —
    // `row-gap` (css-align-3 §8.1: главная ось колонки и ось строк переноса —
    // обе блочные). Порядок — визуальный: копия раскладывается уже
    // переставленной (`reorder` в `blocks()`).
    let flex_items = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.webkit_box != Some(true)
        && !(matches!(
            c.style.flex_dir,
            None | Some(crate::computed::FlexDir::Row) | Some(crate::computed::FlexDir::RowReverse)
        ) && c.style.flex_wrap != Some(true));
    let flex_col = flex_items
        && c.style.vertical != Some(true)
        && c.style.flex_wrap != Some(true)
        && matches!(
            c.style.flex_dir,
            Some(crate::computed::FlexDir::Col) | Some(crate::computed::FlexDir::ColReverse)
        );
    let flex_gap = match c.style.gap {
        Some((Some(Len::Px(v)), _)) if flex_items && c.style.vertical != Some(true) => v.max(0.0),
        _ => 0.0,
    };
    if flex_items {
        kids.sort_by_key(|n| match n {
            Node::Element(e) => e.style.order.unwrap_or(0),
            Node::Text(_) => 0,
        });
    }
    let oof_kid: Vec<bool> = kids
        .iter()
        .map(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style)))
        .collect();
    // Абсолют с заданным `top` стоит от верха содержащего блока, а не на
    // статическом месте (CSS 2.1 §10.6.4): его дотяг у страниц отсчитывается
    // от верха коробки. Прежде — от курсора потока, и `top: 0` после блока
    // 250vh тянул лист на 250vh дальше (`fixedpos-008-print`: девять листов
    // вместо шести).
    let abs_top: Vec<bool> = kids
        .iter()
        .map(|n| {
            matches!(n, Node::Element(k)
                if k.style.position == Some(crate::computed::Position::Absolute)
                    && matches!(k.style.inset.top, Some(l) if !matches!(l, Len::Auto)))
        })
        .collect();
    // Запреты `break-before/after: avoid*` элементов гибкой стопки — с
    // переносом с крайних потомков (`edge_avoid`). Сцепки из них (`flex_run`
    // ниже) — только у РЯДА С ПЕРЕНОСОМ: там стопка «строка = элемент» идёт по
    // блочной оси в порядке строк. ★ Потери P6 (свод v219): у колонки сцепка
    // уводила `single-line-column-flex-fragmentation-016/017`,
    // `multi-line-column-flex-fragmentation-027/028` — диапазон сцепки для
    // `fill_at` монолит, и в колонке с заданной высотой ветка `overflow_to`
    // держала в одной колонке элемент в 300px, хотя `avoid` запрещает только
    // ТОЧКУ между элементами (css-break-3 §4.4 правило 1; Blink
    // `fragmentation_utils.cc:266-269` лишь снижает её привлекательность).
    // `wrap-reverse` кладёт строки с другого края, а стопка — в порядке DOM
    // (`multi-line-row-flex-fragmentation-050`).
    let avoid_chains = flex_items
        && c.style.vertical != Some(true)
        && matches!(
            c.style.flex_dir,
            None | Some(crate::computed::FlexDir::Row) | Some(crate::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap == Some(true)
        && c.style.flex_wrap_reverse != Some(true);
    let avoid_kid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if avoid_chains => (edge_avoid(k, false), edge_avoid(k, true)),
            _ => (false, false),
        })
        .collect();
    // Те же запреты на границах БЛОЧНЫХ детей (css-break-4 §4.3 правило 1):
    // граница, закрытая `break-after: avoid*` предыдущего или `break-before:
    // avoid*` следующего, точкой разрыва не служит. Разрыв уходит к последней
    // законной точке ВНУТРИ предыдущего ребёнка (Blink `early_break_`,
    // `block_layout_algorithm.cc:1086`; `break-between-avoid-007`: c с
    // `break-before: avoid` после обёрток над a и b — разрыв между a и b).
    // Начальное/конечное имя страницы поточных детей класса A (css-page-3
    // §using-named-pages п. 4): несовпадение конца предыдущего с началом
    // следующего — принудительный разрыв на их границе, и на ЛЮБОЙ глубине
    // (Blink `fragmentation_utils.cc` `CalculateBreakBetweenValue`: имя
    // ребёнка против имени текущего фрагмента контейнера). Только у страниц;
    // `style.page` здесь уже несёт используемое значение (`fill_used_page`).
    let page_kid: Vec<Option<(String, String)>> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if cx.paged && !item_container(c) && class_a_box(k) => {
                Some(page_names(k, ""))
            }
            _ => None,
        })
        .collect();
    let mut page_prev: Option<String> = None;
    let blk_avoid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if !flex_items && !out_of_flow(&k.style) => {
                (edge_avoid(k, false), edge_avoid(k, true))
            }
            _ => (false, false),
        })
        .collect();
    // Элемент в ОДНОЙ строке с предыдущим — там, где это видно без раскладки:
    // ширины в процентах без полей, отступов, рамок по главной оси, без
    // `flex-basis`, `min/max-width` и `column-gap` (css-flexbox-1 §9.3: строка
    // набирается, пока следующий элемент помещается). Граница внутри строки —
    // не точка класса A (§12: «Class A break opportunities occur between
    // sibling flex lines»; Blink кладёт `break-*` на СТРОКУ,
    // `flex_layout_algorithm.cc:1892-1906`): сцепка от неё начаться не может
    // (`multi-line-row-flex-fragmentation-040`: 50% + 50%).
    let main_gap0 = match c.style.gap {
        None | Some((_, None)) => true,
        Some((_, Some(Len::Px(v)))) => v.abs() < 0.01,
        _ => false,
    };
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let pct_of = |k: &Element| -> Option<f32> {
        let kb = k.style.borders();
        match k.style.width {
            Some(Len::Pct(p))
                if main_gap0
                    && k.style.flex_basis.is_none()
                    && k.style.min_width.is_none()
                    && k.style.max_width.is_none()
                    && zero(&k.style.margin.left)
                    && zero(&k.style.margin.right)
                    && zero(&k.style.padding.left)
                    && zero(&k.style.padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right) =>
            {
                Some(p)
            }
            _ => None,
        }
    };
    let mut line_acc: Option<f32> = None;
    let mut line_fa = false;
    let line_inner: Vec<bool> = kids
        .iter()
        .map(|n| match n {
            // Внепоточный — не элемент (§4.1): строку не рвёт и в неё не входит.
            Node::Element(k) if avoid_chains && out_of_flow(&k.style) => false,
            Node::Element(k) if avoid_chains => {
                let w = pct_of(k);
                let forced = line_fa || edge_break(k, false);
                line_fa = edge_break(k, true);
                let inner = !forced
                    && matches!((line_acc, w), (Some(s), Some(p)) if s + p <= 1.0 + 1e-3);
                line_acc = match w {
                    Some(p) if inner => line_acc.map(|s| s + p),
                    w => w,
                };
                inner
            }
            _ => {
                line_acc = None;
                line_fa = false;
                false
            }
        })
        .collect();
    // Спуск — по физике контейнера. ★ ЗАМЕРЕНО (04.09,
    // срез 1498): без гейта 384, гейт «только блочный
    // поток» 376 (+15/−23) — flex-колонки, flex с
    // переносом и ВЛОЖЕННЫЙ многоколоночник без спуска
    // теряют высоту и вылетают из укладки целиком.
    // Ряд flex без переноса: дети рядом — высота ряда
    // равна наибольшему, точек разреза между ними нет.
    // Сетка и таблица: дети не стопкой, спуска нет.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None
                | Some(crate::computed::FlexDir::Row)
                | Some(crate::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // Сетка в одну колонку с рядами по содержимому — стопка (`grid_stack`):
    // ряд равен одному ребёнку. Без спуска её мера уходила в `grid_rows_px`
    // и почти всегда возвращала `None`, а `None` означает отказ от укладки:
    // многоколоночник с такой сеткой внутри не фрагментировался ВОВСЕ —
    // колонка 1 переполнена, остальные пусты (`scout-break-2026-09f.md` §1;
    // пробы `target/probe-9f/p-grid-item-fragmentation-043.html` и
    // `p-grid-container-fragmentation-009.html` = 0.00 при подмене на блок).
    let grid_rows_stack = grid_stack(c);
    // Зазор рядов такой сетки: `grid_stack` ручается, что он в точках.
    let row_gap = if grid_rows_stack {
        match c.style.gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    // Ряд/группа рядов ВНЕ таблицы (тегом или `display`) — не стопка блоков.
    let no_descent = (matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    ) && !grid_rows_stack)
        || matches!(c.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot");
    // (высота, поля, точки, forced, монолиты, force_before,
    //  force_after, ДОТЯГ внепоточного)
    // Дотяг — насколько ниже собственного верха ребёнка
    // уходит низ его внепоточного потомка. В поток он не
    // добавляется (абсолют соседей не двигает), но
    // фрагментация обязана его видеть: css-position-3
    // §abspos-breaking — «The box may subsequently be
    // broken over several fragmentation containers».
    type KidShape = (
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        f32,
    );
    let inner: Option<Vec<KidShape>> = if depth == 0 || no_descent {
        None
    } else {
        kids.iter()
            .map(|n| match n {
                // Абсолют высоты стопке не даёт и разреза
                // не мешает: нулевая запись, а не отказ
                // от всей укладки (`out-of-flow-in-
                // multicolumn-*`, корень A2). Но НУЛЬ в
                // девятом поле означал бы, что его вовсе
                // нет во фрагментации, а css-position-3
                // §abspos-breaking требует обратного: «an
                // absolutely positioned box is positioned
                // relative to its containing block ignoring
                // any fragmentation breaks (as if the flow
                // were continuous). The box may
                // subsequently be broken over several
                // fragmentation containers». Значит
                // содержащий блок обязан ДОТЯНУТЬСЯ до его
                // низа — иначе колонок под него не
                // родится (Blink
                // `column_layout_algorithm.cc:1125`:
                // `actual_column_count +=
                // column_balancing_info.num_new_columns`).
                // Плавающий сюда не входит: он не
                // позиционированный, и содержащего блока
                // собой не задаёт.
                Node::Element(k) if out_of_flow(&k.style) => {
                    let abs = matches!(
                        k.style.position,
                        Some(crate::computed::Position::Absolute)
                    );
                    let reach = if abs {
                        // `top: 100vh` у страниц — от page area (эталоны
                        // `fixedpos-*` ставят копии `top: N00vh`; без этого
                        // досягаемость нулевая, лист один).
                        let top = px_or(&k.style.inset.top, false).unwrap_or(0.0);
                        // Собственная высота абсолюта — той
                        // же мерой: она уже включает дотяг
                        // ЕГО внепоточных потомков, и
                        // цепочка `abs > abs` складывается
                        // сама (`out-of-flow-in-multicolumn-
                        // 022/025`).
                        let own =
                            shape_full(k, depth - 1, cx).map(|s| s.0).unwrap_or(0.0);
                        (top + own).max(0.0)
                    } else {
                        0.0
                    };
                    Some((
                        0.0,
                        0.0,
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                        false,
                        reach,
                    ))
                }
                Node::Element(k)
                    if !k.inline
                        && (k.style.position.is_none()
                            || k.style.position
                                == Some(crate::computed::Position::Relative))
                        && (k.style.float.unwrap_or(0) == 0
                            || block_like_float(&k.style)) =>
                {
                    // Главный размер элемента КОЛОНКИ flex — его `flex-basis`
                    // (css-flexbox-1 §9.2 шаг 3): `content` — по содержимому,
                    // а `height` при этом не действует; в точках — сама база.
                    // Контейнер `height: auto` свободного места не даёт, и
                    // гибкость базу не меняет (§9.7).
                    let based = if flex_col { basis_sized(c, k) } else { None };
                    let k = based.as_ref().unwrap_or(k);
                    shape_full(k, depth - 1, cx).map(
                        |(h, mt, mb, cuts, forced, solid)| {
                            // Монолит-потомок — весь диапазон
                            // его высоты; иначе — его собственные
                            // монолиты.
                            let solid = if solid_box(k)
                                && !(inline_content(k) && !cuts.is_empty())
                            {
                                vec![(0.0, h)]
                            } else {
                                solid
                            };
                            (
                                h,
                                mt,
                                mb,
                                cuts,
                                forced,
                                solid,
                                // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                                // передаётся коробке (css-break-4
                                // §break-propagation) — у страниц; колонки
                                // не трогаются (отдельный замер).
                                // Разрыв ПЕРВОГО/ПОСЛЕДНЕГО поточного ребёнка
                                // передаётся коробке (css-break-3 §5.1
                                // break-propagation; Blink `InitialBreakBefore`)
                                // одинаково у страниц и у колонок: правило не
                                // про вид фрагментаинера. Гейт `cx.paged` был
                                // «пока не замерено» — `single-line-row-flex-
                                // fragmentation-016` с разрывом на внуке стоит
                                // красной ровно из-за него (проба
                                // `target/probe-9g/p-…-016.html` = 0.00).
                                edge_break(k, false),
                                edge_break(k, true),
                                // Дотяг внепоточных ЭТОГО потомка в
                                // поток родителя не переходит: у
                                // него свой содержащий блок.
                                0.0,
                            )
                        },
                    )
                }
                _ => None,
            })
            .collect()
    };
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    // Рамка и отбивка самой коробки — без разрывов (Blink:
    // «Avoid breaking inside block-start border»).
    if top > 0.0 {
        solid.push((0.0, top));
    }
    // Стек вложенных: конец, поле первого, схлопнувшееся
    // сквозь верх без отбивки, поле последнего.
    let mut stacked: Option<(f32, f32, f32)> = None;
    // Самый нижний край внепоточных потомков, отсчитанный
    // от верха ЭТОЙ коробки. В поток не входит, высоту
    // соседей не двигает — нужен только фрагментации.
    let mut oof_reach = 0.0f32;
    if let Some(kids) = inner.filter(|k| !k.is_empty()) {
        let inner_h: Vec<f32> = kids.iter().map(|k| k.0).collect();
        // Ряд flex БЕЗ переноса: дети стоят бок о бок, и
        // каждый фрагментируется СВОИМИ точками (Blink
        // `flex_layout_algorithm.cc`: элементу строки
        // выдаётся своя доля фрагментаинера). Значит точки
        // ряда — объединение точек детей, а запрет разрыва
        // — объединение их монолитных диапазонов: рвать
        // нельзя там, где не даёт хоть один. Прежде ряд
        // объявлялся монолитом целиком, и разреза не было
        // никогда (`single-line-row-flex-fragmentation-*`).
        if row_nowrap {
            let tallest = inner_h.iter().copied().fold(0.0f32, f32::max);
            for k in &kids {
                let start = top;
                for (need, nf) in &k.3 {
                    cuts.push((start + need, start + nf));
                }
                for f in &k.4 {
                    forced.push(start + f);
                }
                for (a, b) in &k.5 {
                    solid.push((start + a, start + b));
                }
            }
            stacked = Some((top + tallest, 0.0, 0.0));
        } else {
        let mut y = top;
        let mut prev_mb = 0.0f32;
        let mut through = 0.0f32;
        let mut first = true;
        let mut force_next = false;
        // Сцепка элементов, скованных `avoid*`, — как `avoid_run` в
        // `table_shape`: ОДИН сплошной диапазон от разрешённой границы перед
        // сцепкой до конца её последнего элемента.
        let mut flex_run: Option<f32> = None;
        let mut flex_open = 0.0f32;
        let mut flex_prev_aa = false;
        // Состояние запретов на границах блочных детей (`blk_avoid`).
        let mut blk_prev_aa = false;
        let mut blk_prev_start = top;
        let mut blk_prev_cut: Option<f32> = None;
        // Можно ли начать сцепку от `flex_open`. Нельзя от верха контейнера
        // (css-flexbox-1 §12; Blink `fragmentation_utils.cc:244-253`: без
        // `has_container_separation` — `kBreakAppealLastResort`), от
        // принудительного разрыва (`multi-line-row-flex-fragmentation-023`: рост
        // в `growths` уходил из распорки-коробки в поле) и изнутри строки
        // (`line_inner`).
        let mut flex_open_ok = false;
        // Идёт сцепка (открыта и без диапазона — чтобы её хвост не начал новую
        // с середины).
        let mut flex_chain = false;
        for (ki, (h, kmt, kmb, kcuts, kforced, ksolid, fb, fa, kreach)) in
            kids.into_iter().enumerate()
        {
            // Внепоточный ребёнок гибкого контейнера элементом не является
            // (css-flexbox-1 §4.1): ни зазора, ни границы элементов. Дотяг
            // его низа фрагментации по-прежнему нужен.
            if flex_items && oof_kid.get(ki).copied().unwrap_or(false) {
                let origin = if cx.paged && abs_top.get(ki).copied().unwrap_or(false) {
                    0.0
                } else {
                    y
                };
                oof_reach = oof_reach.max(origin + kreach);
                continue;
            }
            // Сетка: поля рядов не схлопываются ни между собой, ни сквозь
            // верх контейнера (css-grid-1 §6.1), между рядами — `row-gap`.
            // Точка класса A ставится там же, где у блочной стопки, а
            // `cuts` с `nf` за концом зазора продолжает копию с начала
            // следующего ряда — зазор на разрыве пропадает
            // (css-gaps-1 §fragmentation, как в `grid_row_gaps`).
            let lead = if grid_rows_stack {
                if first {
                    kmt
                } else {
                    prev_mb + row_gap + kmt
                }
            } else if flex_items {
                // css-flexbox-1 §4.2: поля соседних элементов НЕ схлопываются
                // и сквозь край контейнера не уходят; между ними — `row-gap`.
                if first { kmt } else { prev_mb + flex_gap + kmt }
            } else if first {
                if top == 0.0 {
                    through = kmt;
                    0.0
                } else {
                    kmt
                }
            } else {
                prev_mb.max(kmt)
            };
            if !first && flex_items {
                // Граница элементов — конец ПОЛЯ предыдущего. Поля элементов
                // режутся как содержимое и не усекаются (Blink держит
                // `margin-top` элемента и после разрыва: `single-line-column-
                // flex-fragmentation-033/034`, эталон `-060-print`); усекается
                // только зазор: край внутри зазора уводит разрез к его началу,
                // копия продолжается с его конца — тот же приём, что
                // `grid_row_gaps` ниже (css-gaps-1 §fragmentation).
                // Диапазон зазора начинается РОВНО на границе: с допуском
                // вверх (`b - 0.05`) он перекрывал монолит предыдущего
                // элемента, и `fill_at` шёл по цепочке перекрытий к началу
                // ЭТОГО монолита — разрыв уходил выше целого элемента
                // (`single-line-column-flex-fragmentation-061`: строка Ahem
                // уезжала в следующую колонку вместе с рамкой). Край ровно на
                // `b` по-прежнему режет здесь (`cuts` с тем же `need`).
                let b = y + prev_mb;
                if flex_gap > 0.0 {
                    solid.push((b, b + flex_gap + 0.05));
                }
                cuts.push((b, b + flex_gap));
                if fb || force_next {
                    forced.push(b);
                }
                // css-break-4 §4.3 правило 1: запрет с ЛЮБОЙ стороны границу
                // закрывает, принудительный разрыв открывает обратно.
                let joined = (flex_prev_aa || avoid_kid.get(ki).is_some_and(|a| a.0))
                    && !(fb || force_next);
                if joined {
                    if !flex_chain {
                        flex_chain = true;
                        flex_run = flex_open_ok.then_some(flex_open);
                    }
                } else {
                    if let Some(s) = flex_run.take() {
                        solid.push((s, y));
                    }
                    flex_chain = false;
                    flex_open = b;
                    flex_open_ok =
                        !(fb || force_next) && !line_inner.get(ki).copied().unwrap_or(false);
                }
            } else if !first {
                cuts.push((y, y + lead));
                // Принудительный разрыв на границе детей.
                if fb || force_next {
                    forced.push(y);
                }
                // Закрытая запретом граница (`blk_avoid`): сплошной диапазон от
                // последней точки внутри предыдущего ребёнка (без неё — от его
                // начала) до начала этого: край колонки в нём уводит разрыв к
                // его началу (`fill_at`, ветка `holds`). От верха коробки
                // диапазон не начинается — там разрыв был бы разрывом ПЕРЕД
                // коробкой, и это решает уровень выше.
                if !grid_rows_stack
                    && (blk_prev_aa || blk_avoid.get(ki).is_some_and(|a| a.0))
                    && !(fb || force_next)
                {
                    let open = blk_prev_cut.unwrap_or(blk_prev_start);
                    if open > top + 0.01 {
                        solid.push((open, y + lead + 0.05));
                    }
                }
            }
            // Смена имени страницы между соседями — принудительный разрыв.
            let renamed = match (&page_prev, page_kid.get(ki).and_then(|p| p.as_ref())) {
                (Some(prev), Some((start, _))) => prev != start,
                _ => false,
            };
            if renamed && !first && !(fb || force_next) {
                forced.push(if flex_items { y + prev_mb } else { y });
            }
            if let Some(Some((_, end))) = page_kid.get(ki) {
                page_prev = Some(end.clone());
            }
            force_next = fa;
            let start = y + lead;
            // Последняя законная точка ВНУТРИ этого ребёнка (для `blk_avoid`).
            blk_prev_start = if first { start } else { y };
            blk_prev_cut = kcuts
                .iter()
                .map(|&(need, _)| need)
                .filter(|&n| n > 0.01 && n < h - 0.01)
                .fold(None::<f32>, |m, n| Some(m.map_or(n, |x| x.max(n))))
                .map(|n| start + n);
            blk_prev_aa = blk_avoid.get(ki).is_some_and(|a| a.1);
            for (need, nf) in kcuts {
                cuts.push((start + need, start + nf));
            }
            for f in kforced {
                forced.push(start + f);
            }
            for (a, b) in ksolid {
                solid.push((start + a, start + b));
            }
            // Дотяг ребёнка — от ЕГО верха; переводим в
            // координаты этой коробки. `y` он не двигает:
            // внепоточный соседей не сдвигает
            // (CSS 2.1 §9.3.1).
            let origin = if cx.paged && abs_top.get(ki).copied().unwrap_or(false) {
                0.0
            } else {
                start
            };
            oof_reach = oof_reach.max(origin + kreach);
            y = start + h;
            prev_mb = kmb;
            first = false;
            flex_prev_aa = avoid_kid.get(ki).is_some_and(|a| a.1);
        }
        // Сцепка, дожившая до последнего элемента, закрывается его низом.
        if let Some(s) = flex_run {
            solid.push((s, y));
        }
        // Нижнее поле последнего ряда наружу не схлопывается и входит в
        // высоту сетки (css-grid-1 §6.1).
        // Нижнее поле последнего ЭЛЕМЕНТА гибкого контейнера тоже входит в
        // его высоту (внешний размер элемента, css-flexbox-1 §9.4).
        if grid_rows_stack || flex_items {
            y += prev_mb;
            prev_mb = 0.0;
        }
        stacked = Some((y, through, prev_mb));
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): ряд flex С ПЕРЕНОСОМ
        // как строки — жадная сборка по ширинам детей в точках
        // (css-flexbox-1 §9.3), разрез между строками, высота —
        // сумма строк: срез фрагментации 469 -> 469 (0/0) —
        // ширины элементов в тестах не в точках (`flex: 1`,
        // проценты), ветка не срабатывает. Нужна ширина из
        // раскладки, а не из стиля (корень R4 scout-flexfrag).
        }
    }
    // Заданная высота — в точках или (для страниц) в единицах окна.
    let (h, mt, mb) = match c.style.height.as_ref().map(|_| px_or(&c.style.height, true)) {
        // Мера потока (`unclamp`) заданной высотой не обрезается:
        // css-break-3 §3 «parallel flows» — переполнение продолжается в
        // следующем фрагментаинере само по себе, и его протяжённость нужна
        // укладке колонок. Высота КОРОБКИ при этом не меняется: её даёт
        // обычная мера (`unclamped: false`), и именно она остаётся шагом для
        // соседа. Ниже по функции тем же `h` усекаются `cuts`/`forced`/
        // `solid` — в мере потока они остаются полными, и это ровно то, что
        // нужно: точки разреза и монолиты хвоста.
        Some(Some(v)) => (
            if unclamp {
                // Отрицательное поле первого ребёнка, схлопнутое сквозь верх
                // коробки (`through`), поднимает всё содержимое: его низ —
                // `end + through` от верха коробки (`css-break/float-001`:
                // коробка `height: 0` с ребёнком 40px и `margin-top: -40px`
                // — содержимое кончается на её верху, а мера давала поток
                // 40, и коробка с края колонки уезжала в следующую).
                fragment_size::border_size(v, &c.style, top + bot)
                    .max(stacked.map_or(0.0, |s| s.0 + s.1.min(0.0)) + bot)
            } else {
                fragment_size::border_size(v, &c.style, top + bot)
            },
            mt,
            mb,
        ),
        Some(None) => return None,
        None => match stacked {
            Some((end, through, last_mb)) => (
                // Нижнее поле последнего ребёнка при собственном нижнем
                // отступе/рамке коробки НЕ схлопывается наружу и входит в
                // высоту (CSS 2.1 §10.6.3, §8.3.1): `table-fragmentation-
                // 001c-ref` — `.table { padding }` + `.td { margin: .25in 0 }`,
                // мера 360 при рисунке 384, нижняя рамка не влезала в копию.
                // У страниц; колонки не трогаются (отдельный замер).
                end + if bot > 0.0 && cx.paged { last_mb } else { 0.0 } + bot,
                mt.max(through),
                if bot == 0.0 { mb.max(last_mb) } else { mb },
            ),
            // Сетка без заданной высоты: её высоту знают
            // ЯВНЫЕ дорожки рядов (`grid-template-rows:
            // 200px`) с зазорами между ними. Без этой
            // оценки укладка колонок отказывалась от всей
            // коробки, и многоколоночник с сеткой внутри
            // уходил в запасную сетку целиком
            // (`scout-break-2026-09b.md`, корень C1).
            // Дорожки идут ПЕРЕД пустотой: у сетки БЕЗ детей высота всё
            // равно есть — её задают дорожки (css-grid-1 §7.1: дорожка
            // существует независимо от того, занята ли она). Прежде
            // `kids.is_empty()` заслонял эту ветку, и
            // `grid-container-fragmentation-002` (`grid-template-rows:
            // 200px`, детей нет) мерился нулём: квадрат 100×100 красен
            // ЦЕЛИКОМ (снимок `target/wpt-shots/_fg-grid-container-
            // fragmentation-002.png`, `x 10..134, y 67..191`). Проба
            // `target/probe-fg/p-fg-002.html` (та же сетка блоком 200px)
            // = 0.00.
            None => match grid_rows_px(&c.style) {
                Some(v) => (v + top + bot, mt, mb),
                // Сетка, у которой дорожки рядов НЕ все в точках (несколько
                // колонок, ряд `auto`, неявный ряд), до сих пор отдавала
                // `return None`. `None` тут стоит дорого: в `blocks()`
                // (:13391) `stackable` собирается через
                // `collect::<Option<Vec<_>>>()`, и одна неизмеримая сетка
                // отменяет укладку колонок ВСЕГО многоколоночника вместе с
                // её соседями — колонка 1 переполнена, остальные пусты
                // (`grid-item-fragmentation-003`: сетка `auto auto` с
                // элементом 200px, ряд 200, ничего не фрагментируется).
                // Высота сетки с `height: auto` — сумма размеров дорожек
                // рядов (css-grid-2 §Fragmenting Grid Layout, шаг 4
                // «Sample Fragmentation Algorithm»; Blink
                // `grid_layout_algorithm.cc:370`
                // `Rows().CalculateSetSpanSize()`). Точек разреза эта ветка
                // не добавляет — см. `grid_auto_row_bands`.
                None => match grid_auto_row_bands(c, depth, cx) {
                    Some((b, spots)) => {
                        // Элементы ряда — параллельные потоки (css-break-3 §3;
                        // Blink `PlaceGridItems`: у каждого свой break token):
                        // точки и монолиты — ОБЪЕДИНЕНИЕМ, со смещением ряда,
                        // как у ряда flex без переноса выше. Рост элемента,
                        // чья строка ушла в следующую колонку, ставит
                        // `grow_pushed` через спуск `pushed_box_at`; после него
                        // строка стоит на краю, и общий срез совпадает с
                        // раздельными (css-grid-2 §12.1 шаг 3: ряд растёт).
                        // Границ РЯДОВ по-прежнему нет — см. выше.
                        for (ix, row, kmt, s, plain) in &spots {
                            // Страницы: монолитный элемент (`contain: size`,
                            // замещаемый…) — сплошной диапазон во всю его
                            // высоту, и край листа внутри него уводит разрыв
                            // к началу ряда (css-grid-2 §fragmenting: «a grid
                            // container may break between rows»; Blink
                            // `IsMonolithic` → разрыв перед рядом;
                            // `grid-fragmentation-between-rows-001-print`:
                            // второй ряд `contain: size` резался краем листа).
                            if cx.paged
                                && !*plain
                                && let (Some(&(r0, _)), Some(Node::Element(k))) =
                                    (b.get(*row), c.children.get(*ix))
                                && solid_box(k)
                            {
                                let start = top + r0 + kmt;
                                solid.push((start, start + s.0));
                                continue;
                            }
                            let (true, Some(&(r0, _))) = (*plain, b.get(*row)) else {
                                continue;
                            };
                            let start = top + r0 + kmt;
                            for (need, nf) in &s.3 {
                                cuts.push((start + need, start + nf));
                            }
                            for f in &s.4 {
                                forced.push(start + f);
                            }
                            for (a, e) in &s.5 {
                                solid.push((start + a, start + e));
                            }
                        }
                        (b.last().map_or(0.0, |r| r.1) + top + bot, mt, mb)
                    }
                    None if kids.is_empty() => (top + bot, mt, mb),
                    None => return None,
                },
            },
        },
    };
    // Содержащий блок обязан дотянуться до низа своих
    // внепоточных потомков — только тогда фрагментация
    // родит под них колонки, а балансировка их посчитает
    // (css-position-3 §abspos-breaking; Blink
    // `column_layout_algorithm.cc:1092-1131` прогоняет
    // `OutOfFlowLayoutPart` внутри цикла балансировки
    // именно ради этого). Если коробка содержащим блоком
    // НЕ является, дотяг принадлежит кому-то выше и здесь
    // не учитывается — он всплывёт там.
    let own_h = h;
    let h = if crate::inline::establishes_cb(&c.style) {
        h.max(oof_reach + bot)
    } else {
        h
    };
    let h = fragment_size::constrain(h, &c.style, top + bot, cx.viewport, unclamp);
    // Дотяг меняет меру фрагментации, но не размер коробки: сосед в потоке
    // встаёт под КОНЦОМ коробки, а абсолют продолжается параллельным потоком
    // (css-position-3 §abspos-breaking; Blink ведёт OOF во фрагментаинере
    // отдельно от потока). Стопка берёт собственный размер из `OOF_OWN`.
    {
        let own = fragment_size::constrain(own_h, &c.style, top + bot, cx.viewport, unclamp);
        OOF_OWN.with(|m| {
            let mut m = m.borrow_mut();
            if own < h - 0.01 {
                m.insert(c.node_id, (own, h));
            } else {
                m.remove(&c.node_id);
            }
        });
    }
    // css-gaps-1 §fragmentation / css-align-3 §column-row-gap: «the gap
    // disappears when it coincides with a fragmentation break»; Blink
    // `GridLayoutAlgorithm` `MaybeSuppressLastGap`: зазор рядов, в который
    // попал край фрагментаинера (внутрь, на начало или на конец), снимается —
    // следующий ряд начинается с верха следующего фрагмента, линейка в таком
    // зазоре не рисуется (`grid-gap-decorations-fragmentation-001…010`).
    // В стопке колонок это точка класса A с усечением: край внутри
    // `solid`-диапазона зазора уводит разрез к его началу (`fill`, `at(a)`),
    // а `cuts` с тем же `need` продолжает копию с КОНЦА зазора.
    // Keep the cut at the actual gutter start: fill_at's at(edge) handles
    // an exact start boundary. Moving it by a tolerance shortens the painted
    // fragment and can discard a device row. Only extend the end interval
    // for Blink's inclusive last_gap_end_offset >= fragmentainer_space check.
    for (a, b) in grid_row_gaps(&c.style, h - top - bot) {
        cuts.push((top + a, top + b));
        solid.push((top + a, top + b + 0.05));
    }
    // Принудительный разрыв элемента сетки — на границу его РЯДА
    // (css-grid-2 §Fragmenting Grid Layout; Blink `grid_layout_algorithm.cc`
    // `row_break_between`). Сетка без `grid_stack` спуска не знает, и до
    // этого места её `forced` был пуст ВСЕГДА: разрез шёл по краю колонки
    // (`flow.rs: fill_at`, ветка `Some(at(edge))`) —
    // `grid-item-fragmentation-044` резался на 100 при границе ряда 50
    // (снимок `target/wpt-shots/_fg-grid-item-fragmentation-044.png`:
    // красный прямоугольник `x 73..134, y 129..191`). Проба
    // `target/probe-fg/p-fg-044.html` (та же геометрия блоками, разрыв на
    // границе ряда) = 0.00.
    let (row_forced, row_mono) = grid_row_forced(c);
    for f in row_forced {
        forced.push(top + f);
    }
    // Монолитный ряд — целиком (`grid_row_forced`); разрез у его начала
    // растит предыдущую дорожку (`grow_grid_track`), и переполнение
    // элементов предыдущего ряда остаётся в колонке, как у Blink.
    for (a, e) in row_mono {
        solid.push((top + a, top + e));
    }
    // Внутренние принудительные разрывы монолита фрагментации не видны
    // (`forced_opaque`): `fill_at` проверяет `forced` РАНЬШЕ монолитности и
    // разрезал бы `contain: size`-коробку по разрыву её потомка.
    if forced_opaque(c) {
        forced.clear();
    }
    cuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    forced.retain(|&f| f > 0.01 && f < h - 0.01);
    // Монолит ребёнка за ЗАДАННОЙ высотой коробки — параллельный поток, а
    // не запрет разреза в её потоке (css-break-4 §parallel-flows; Blink
    // `FinishFragmentation`, `fragmentation_utils.cc`: «If the block-size is
    // constrained / fixed … we know that we're at the end» — сосед
    // продолжает в той же колонке; `BoxFragmentBuilder::
    // MustStayInCurrentFragmentainer`: «any first piece of child content
    // also needs to stay in the current fragmentainer, even if this causes
    // fragmentainer overflow»). Начатый ниже низа — вычёркивается, начатый
    // выше — бережётся лишь до низа. Без этого `contain: size` в
    // переполняющем ребёнке выталкивал коробку целиком (`single-line-
    // column-flex-fragmentation-051`, `tall-content-inside-constrained-
    // block-*`). У коробки с высотой auto диапазоны и так внутри `h`.
    solid.retain(|&(a, _)| a < h - 0.01);
    for r in solid.iter_mut() {
        r.1 = r.1.min(h);
    }
    if bot > 0.0 {
        // Между последним поточным ребёнком и нижней отбивкой/рамкой БЕЗ зазора
        // точки разрыва нет: класс C css-break-4 §possible-breaks существует лишь
        // «if there is a (non-zero) gap between them», а Blink держит там только
        // «a last-resort breakpoint before trailing border and padding»
        // (`fragmentation_utils.cc`, `FinishFragmentation`). Край колонки, упавший
        // на начало или внутрь нижней отбивки, обязан увести разрыв к НАЧАЛУ
        // монолита, который к ней примыкает (строка, `inline-block`,
        // `break-inside: avoid`), а не оставить отбивку одну в следующей колонке;
        // рост до низа колонки делает `grow_pushed` (срез на начале монолита).
        // `break-at-end-container-edge-000/001/004`: строки `inline-block` и
        // `padding-bottom` 50/70/60 — последняя строка уходит вместе с отбивкой;
        // `fieldset-005`: две коробки `break-inside: avoid` по 60 и `border-bottom:
        // 40px`. Диапазон из нуля — верхняя рамка самой коробки, его не клеим
        // (пустая коробка с отбивками стала бы монолитом). Нижнее поле последнего
        // ребёнка у колонок в `h` не входит (выше, `cx.paged`), при нём зазор есть
        // — тогда не клеим.
        let end_edge = h - bot;
        let gapless = stacked.map_or(true, |s| s.2.abs() < 0.01);
        let glue = if gapless {
            solid
                .iter()
                .filter(|&&(a, b)| a > 0.01 && (b - end_edge).abs() < 0.01)
                .map(|&(a, _)| a)
                .fold(end_edge, f32::min)
        } else {
            end_edge
        };
        solid.push((glue, h));
    }
    Some((h, mt, mb, cuts, forced, solid))
}

/// Поле, которое мера схлопнула СКВОЗЬ верх коробки (`shape_full`: у первого
/// поточного блока без верхней отбивки и рамки `through = kmt`, и при высоте
/// `auto` оно уходит в `mt` коробки). Стопка кладёт это поле сама — `lead` в
/// `fill_at`, с усечением на разрыве (css-break-4 `margin-break: auto`: «any
/// margins adjoining the break … are truncated to zero after the break»). Копия
/// же ставится КОРНЕМ
/// (`layout_as_root`), а у корня taffy поля с детьми не схлопывает
/// (`vendor/taffy/src/compute/block.rs:186-192`, `vertical_margins_are_collapsible`
/// у корня ложно): поле внука вставало ВТОРОЙ раз внутри копии. На разрыве это
/// сдвигало содержимое на всё поле вниз: `margin-at-break-001/002` — `margin-top:
/// 60px` у первого внука, в колонке 2 зелёное 110..160 вместо 50..100 (снимок
/// `target/wpt-shots/margin-at-break-001.png`). Снимается ровно цепочка меры:
/// первый непустой ребёнок — поточный блок, у коробки нет верхней отбивки/рамки и
/// высота `auto`. Сетка, гибкий контейнер, таблица, вложенный многоколоночник
/// идут в мере иначе (`grid_rows_stack`, `row_nowrap`, `table_shape`) — не
/// трогаем. Внепоточный первым — у меры нулевая запись, `through` не рождается.
pub(crate) fn strip_through_top(c: &mut Element, depth: u8) {
    if depth == 0
        || c.style.height.is_some()
        || table_box(c)
        || multicol_container(&c.style)
        || c.style.webkit_box == Some(true)
        || !matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
    {
        return;
    }
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    if px(&c.style.padding.top) + px(&b.top) != 0.0 {
        return;
    }
    let Some(Node::Element(k)) = c.children.iter_mut().find(|n| !is_blank(n)) else {
        return;
    };
    if k.inline
        || out_of_flow(&k.style)
        || !matches!(
            k.style.position,
            None | Some(crate::computed::Position::Relative)
        )
    {
        return;
    }
    k.style.margin.top = None;
    strip_through_top(k, depth - 1);
}

/// Самая внешняя коробка, начинающаяся ровно в `a` от верха `c`, — перед
/// ней встаёт распорка роста (`grow_pushed`). Смещения детей — той же
/// арифметикой, что в `shape_full`: `lead` = схлопнутое поле, у первого
/// без отбивки поле уходит сквозь верх, внепоточный — нулевая запись, ряд
/// flex без переноса — все дети с верха. Сетка, таблица, ряды — без
/// спуска, как там; таблица по тегу — рядами `table_shape`
/// (`pushed_cell_at`). Текст или строчный среди детей — у `shape_full`
/// отказ от спуска, и здесь тоже.
thread_local! {
    /// Щуп `pushed_box_at` ищет коробку под ПРИНУДИТЕЛЬНЫЙ разрыв (`grow_pushed`).
    pub(crate) static PUSH_FORCED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn pushed_box_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
    if depth == 0 {
        return None;
    }
    if table_box(c) {
        return pushed_cell_at(c, a, depth);
    }
    // Сетка-стопка спускается ровно как в `shape_full` — иначе распорка
    // роста (`705fd58`) до ряда сетки не добирается, отдаёт `None`, и фон
    // коробки не дотягивается до низа колонки.
    let grid_rows_stack = grid_stack(c);
    let row_gap = if grid_rows_stack {
        match c.style.gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    if (matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    ) && !grid_rows_stack
        && !grid_items_spotted(c))
        || matches!(c.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
    {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    let top = px(&c.style.padding.top) + px(&b.top);
    // Сетка `grid_auto_row_bands`: коробка ищется в элементах ряда,
    // накрывающего `a`, — теми же смещениями, что в мере (`shape_full`).
    // Точка ровно на верху элемента уходит в его первого ребёнка.
    if grid_items_spotted(c) {
        let (bands, spots) = grid_auto_row_bands(c, depth, ShapeCx::COLUMNS)?;
        for (ix, row, kmt, s, plain) in spots {
            let (true, Some(&(r0, _)), Some(Node::Element(k))) =
                (plain, bands.get(row), c.children.get(ix))
            else {
                continue;
            };
            let start = top + r0 + kmt;
            if a > start - 0.01 && a < start + s.0 - 0.01 {
                if let Some(id) = pushed_box_at(k, a - start, depth - 1) {
                    return Some(id);
                }
            }
        }
        return None;
    }
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None
                | Some(crate::computed::FlexDir::Row)
                | Some(crate::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // МНОГОСТРОЧНЫЙ flex: элемент — не коробка стопки, а член СТРОКИ. Распорка
    // перед самим элементом сдвигала его внутри строки и уводила за собой все
    // следующие строки (`flex-gap-decorations-fragmentation-009/010`: из шести
    // элементов в колонках оставался один — v225 0.03 → v226 0.72); Blink растит
    // строку целиком (`flex_layout_algorithm.cc:2536-2560`
    // `item_offset_adjustment`). Спуск ВНУТРЬ элемента остаётся: распорка в его
    // потомке растит только его (`multi-line-row-flex-fragmentation-007`).
    // Только у контейнера ЗАДАННОЙ высоты: у `height: auto` в узкой колонке
    // строка обычно из одного элемента, и распорка перед ним — это распорка
    // перед строкой (`multi-line-row-flex-fragmentation-018/024/037`: без неё
    // «красное видно»).
    // И не для принудительного разрыва: распорка `break-before` перед
    // элементом — это и есть начало новой строки во фрагменте
    // (`multi-line-row-flex-fragmentation-029`).
    let wrap_flex = is_flex
        && c.style.flex_wrap == Some(true)
        && matches!(c.style.height, Some(Len::Px(_)))
        && !PUSH_FORCED.with(|f| f.get());
    // Та же гибкая стопка, что в `shape_full` (`flex_items`): поля не
    // схлопываются, между элементами `row-gap`, порядок визуальный,
    // внепоточные — не элементы.
    let flex_items = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.webkit_box != Some(true)
        && !row_nowrap;
    let flex_gap = match c.style.gap {
        Some((Some(Len::Px(v)), _)) if flex_items && c.style.vertical != Some(true) => v.max(0.0),
        _ => 0.0,
    };
    let mut items: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    if flex_items {
        items.sort_by_key(|n| match n {
            Node::Element(e) => e.style.order.unwrap_or(0),
            Node::Text(_) => 0,
        });
    }
    let mut y = top;
    let mut prev_mb = 0.0f32;
    let mut first = true;
    for n in items {
        let Node::Element(k) = n else {
            return None;
        };
        let oof = out_of_flow(&k.style);
        if flex_items && oof {
            continue;
        }
        let (h, kmt, kmb) = if oof {
            (0.0, 0.0, 0.0)
        } else if !k.inline
            && (k.style.position.is_none()
                || k.style.position == Some(crate::computed::Position::Relative))
            && k.style.float.unwrap_or(0) == 0
        {
            // Та же база элемента колонки flex, что у `shape_full`.
            let based = basis_sized(c, k);
            let (h, mt, mb, ..) = shape_full(based.as_ref().unwrap_or(k), depth - 1, ShapeCx::COLUMNS)?;
            (h, mt, mb)
        } else {
            return None;
        };
        if row_nowrap {
            // Элементы ряда стоят бок о бок с верха; точка внутри одного
            // из них — его собственная (объединение точек, `shape_full`).
            if !oof && a > top + 0.01 && a < top + h - 0.01 {
                if let Some(id) = pushed_box_at(k, a - top, depth - 1) {
                    return Some(id);
                }
            }
            continue;
        }
        // Та же арифметика, что в `shape_full`: у сетки поля не
        // схлопываются, между рядами — `row-gap`.
        let lead = if grid_rows_stack {
            if first {
                kmt
            } else {
                prev_mb + row_gap + kmt
            }
        } else if flex_items {
            if first { kmt } else { prev_mb + flex_gap + kmt }
        } else if first {
            if top == 0.0 {
                0.0
            } else {
                kmt
            }
        } else {
            prev_mb.max(kmt)
        };
        let start = y + lead;
        if !oof && (start - a).abs() < 0.01 {
            return (!wrap_flex).then_some(k.node_id);
        }
        // Принудительный разрыв гибкой стопки: `growths` отдаёт `nf` пары
        // `cuts` — начало ПОЛЯ элемента (конец зазора), а не верх коробки.
        if flex_items && !first && (start - kmt - a).abs() < 0.01 {
            return (!wrap_flex).then_some(k.node_id);
        }
        if !oof && start < a && a < start + h - 0.01 {
            return pushed_box_at(k, a - start, depth - 1);
        }
        y = start + h;
        prev_mb = kmb;
        first = false;
    }
    None
}

/// Распорка в таблице по тегу: та же арифметика рядов, что у `table_shape`
/// (порядок групп `thead`/…/`tfoot`, `border-spacing`, презентационные
/// `cellspacing`/`cellpadding`, высота ряда — наибольшая мера ячейки), без
/// `avoid` и разрывов; у ряда, накрывающего `a`, — первая ячейка, в которой
/// нашлась коробка. Точка ровно на верху ряда — первый ребёнок ячейки
/// (ячейка — свой контекст, поле сквозь её верх не уходит; в мере
/// `shape_full(cell)` распорка ложится в `lead`). Что `table_shape` не
/// меряет (`rowspan`, подпись, сросшиеся рамки, заданная высота), здесь
/// тоже `None` — распорки нет, поведение прежнее.
pub(crate) fn pushed_cell_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
    if depth == 0
        || c.style.vertical == Some(true)
        || c.style.border_collapse == Some(true)
        || c.style.height.is_some()
        || c.style.min_height.is_some()
    {
        return None;
    }
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let top = px_of(&c.style.padding.top)? + px_of(&c.style.borders().top)?;
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..ShapeCx::COLUMNS
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Щуп разреза ходит по ТОМУ ЖЕ дереву, что мера (Х2) и рисование: иначе
    // `table_shape` посчитает точки по анонимным рядам, а `pushed_cell_at` на
    // тех же детях вернёт `None`, и распорка роста (`grow_pushed`) не найдёт
    // коробку, которую надо дотянуть до низа колонки.
    let fixed = fixup_table_children(&c.children);
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    let mut rows: Vec<&Element> = Vec::new();
    for (_, e) in &parts {
        if is_row(e) {
            rows.push(e);
            continue;
        }
        for n in e.children.iter().filter(|n| !is_blank(n)) {
            match n {
                Node::Element(r) if is_row(r) => rows.push(r),
                _ => return None,
            }
        }
    }
    let mut y = top;
    for r in rows {
        let start = y + spacing;
        let mut h = px_of(&r.style.height)?;
        let mut cells: Vec<&Element> = Vec::new();
        for n in r.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) || cell.attr("rowspan").is_some_and(|v| v.trim() != "1") {
                return None;
            }
            h = h.max(shape_full(cell, depth - 1, cell_cx)?.0);
            cells.push(cell);
        }
        if a > start - 0.01 && a < start + h - 0.01 {
            return cells
                .iter()
                .find_map(|cell| pushed_box_at(cell, a - start, depth - 1));
        }
        y = start + h;
    }
    None
}

/// Распорка роста: `margin-top += grow` у потомка `id`. Поле ложится в
/// `lead` меры (`shape_full`) и в раскладку копии одинаково; недобор от
/// схлопывания с большим нижним полем соседа добирает следующий заход
/// `grow_pushed`.
pub(crate) fn grow_before(c: &mut Element, id: u64, grow: f32) -> bool {
    for n in c.children.iter_mut() {
        let Node::Element(k) = n else {
            continue;
        };
        if k.node_id == id {
            let old = match &k.style.margin.top {
                Some(Len::Px(v)) => *v,
                _ => 0.0,
            };
            k.style.margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}

/// Распорка-КОРОБКА перед коробкой `id`: пустой блок высотой `grow`.
/// Нужна принудительному разрыву. Точка `forced` в мере стоит ПЕРЕД
/// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, потом
/// `forced.push(y)`), поэтому `margin-top` её не сдвигает — сдвигает только
/// новый поточный сосед.
pub(crate) fn spacer_before(c: &mut Element, id: u64, grow: f32) -> bool {
    // Сетка-стопка: между рядами стоит `row-gap` (`shape_full`:
    // `lead = prev_mb + row_gap + kmt`), и вставка ряда добавляет ЛИШНИЙ
    // зазор. Ряд flex без переноса: высота ряда — `tallest` по детям, а
    // распорка встала бы соседом БОК О БОК и подняла бы весь ряд. Оба
    // случая забирает подъём разрыва (Х5-Х8), а не рост.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None | Some(crate::computed::FlexDir::Row) | Some(crate::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // Зазор гибкой стопки (`row-gap`) встал бы и перед распоркой — лишний
    // зазор, как у сетки-стопки; такой разрыв остаётся без роста.
    let flex_gapped = is_flex
        && c.style.webkit_box != Some(true)
        && matches!(c.style.gap, Some((Some(Len::Px(v)), _)) if v > 0.0);
    let at = if grid_stack(c) || row_nowrap || flex_gapped {
        None
    } else {
        c.children
            .iter()
            .position(|n| matches!(n, Node::Element(k) if k.node_id == id))
    };
    if let Some(i) = at {
        // Распорка двигает только разрыв ПЕРЕД коробкой. Разрыв ПОСЛЕ
        // предыдущего соседа несёт `force_next`, и `forced.push(y)`
        // сработает на границе самой распорки: точка не сдвинется, а у
        // коробки исчезнет вовсе — распорка уедет в следующую колонку
        // вместе с ней. 11 пар из 51 в корзине D держатся только на
        // `break-after`; их забирает подъём (Х5-Х8).
        if !matches!(&c.children[i], Node::Element(k) if edge_break(k, false)) {
            return false;
        }
        // Нижнее поле предыдущего соседа переносится на распорку. Иначе
        // `lead` схлопывается ДВАЖДЫ — перед распоркой (`prev_mb.max(0)`) и
        // перед коробкой (`0.max(kmt)`), — и точка разрыва уезжает на
        // `prev_mb` НИЖЕ края колонки (`trailing-child-margin-000`, `-002`:
        // `margin-bottom: 50px`, обе зелёные).
        let pi = c.children[..i].iter().rposition(|n| !is_blank(n));
        let prev_mb = match pi.map(|j| &c.children[j]) {
            Some(Node::Element(k)) => k.style.margin.bottom.clone(),
            _ => None,
        };
        if prev_mb.is_some() {
            if let Some(Node::Element(k)) = pi.map(|j| &mut c.children[j]) {
                k.style.margin.bottom = None;
            }
        }
        let mut style = crate::computed::Computed::default();
        style.height = Some(Len::Px(grow));
        style.margin.bottom = prev_mb;
        // Распорка в гибком хозяине — сама элемент: при переносе по строкам
        // она обязана занять СВОЮ строку (иначе встаёт рядом с предыдущим
        // элементом и строку не растит), не сжиматься в контейнере с заданной
        // высотой и стоять в визуальном порядке рядом со своей коробкой
        // (`order`, `reorder` в `blocks()`).
        if matches!(c.style.display, Some(Display::Flex) | Some(Display::InlineFlex)) {
            style.width = Some(Len::Pct(1.0));
            style.flex_shrink = Some(0.0);
            if let Node::Element(k) = &c.children[i] {
                style.order = k.style.order;
            }
        }
        c.children.insert(
            i,
            Node::Element(Element {
                list_item: None,
                // Свой устойчивый номер: анимации у распорки нет, но номер
                // обязан быть уникальным — иначе GPUI склеит её состояние с
                // коробкой, перед которой она стоит.
                node_id: id ^ 0x5350_4143_4552_0001,
                anim: None,
                tag: "div".to_string(),
                style,
                hover: None,
                first_letter: None,
                first_line: None,
                children: Vec::new(),
                attrs: Vec::new(),
                inline: false,
            }),
        );
        // Заданная высота хозяина СТАРШЕ содержимого (`shape_full`: ветка
        // `c.style.height` возвращает `v + top + bot`), и распорка внутри неё
        // меры не меняет — `changed` не взводится, цикл `grow_pushed` встаёт
        // на первом заходе. Проба `p2-single-line-column-flex-fragmentation-
        // 037` осталась красной именно поэтому, а `p3-…` с высотой 100 → 150
        // сняла 4/5 площади (3906 → 756 точек).
        if let Some(Len::Px(h)) = c.style.height {
            c.style.height = Some(Len::Px(h + grow));
        }
        return true;
    }
    for n in c.children.iter_mut() {
        if let Node::Element(k) = n {
            if spacer_before(k, id, grow) {
                return true;
            }
        }
    }
    false
}

/// Рост коробки от вытолкнутого монолита (Blink `FinishFragmentation`,
/// `fragmentation_utils.cc:641-656`: у НЕпоследнего фрагмента
/// `final_block_size = space_left`; css-flexbox-1 §fragmentation: «A forced
/// break inside a flex item effectively increases the size of its
/// contents»). Укладка режет ребёнка стопки в НАЧАЛЕ монолитного диапазона
/// `a` (`flow.rs` `fill_at`: `holds` → `at(a)`): в колонке остаётся
/// `a − from` содержимого при остатке `room`, и коробки, где лежит монолит,
/// обязаны дотянуться до низа, а всё после него — сдвинуться на
/// `grow = room − (a − from)`. Копия одна на все колонки, и сдвиг в ней
/// делает распорка — `margin-top += grow` у самой внешней коробки,
/// начинающейся ровно в `a` (`pushed_box_at`). После неё монолит стоит на
/// краю колонки, мера выросла на `grow`, и срез по краю совпадает с Blink:
/// колонка 1 — `room`, следующая копия — с `from + room`. План зависит
/// только от мер (`ColumnStack::growths`), поэтому распорки ставятся ДО
/// сборки копий, до неподвижной точки (≤ 6 заходов; обычно один). Проба
/// устройства руками — `target/probe-9d/*.html`: SLC-007/009/010/011 и
/// `table-cell-expansion-001` 0.00 (`scout-break-2026-09d.md` §3.3).
pub(crate) fn grow_pushed(
    mut kids: Vec<(Element, Shape)>,
    count: usize,
    fixed: Option<f32>,
    rows: Option<crate::flow::Rows>,
    copies: usize,
    par: &[crate::flow::Par],
) -> Vec<(Element, Shape)> {
    for _ in 0..6 {
        let probe: Vec<crate::flow::Kid> = kids
            .iter()
            .enumerate()
            .map(|(i, (c, s))| crate::flow::Kid {
                h: s.0,
                mt: s.1,
                mb: s.2,
                // Тот же предикат, что у `StackChild` в сборке стопки.
                monolith: solid_box(c),
                cuts: s.3.clone(),
                // Щуп обязан видеть ровно то же, что стопка (Х6), иначе
                // распорки лягут по другому плану, чем укладка.
                force_before: edge_break(c, false),
                force_after: edge_break(c, true),
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // запретов `growths` считал бы распорки по ДРУГОМУ плану,
                // чем укладка после отступа.
                avoid_before: edge_avoid(c, false),
                avoid_after: edge_avoid(c, true),
                forced: s.4.clone(),
                solid: s.5.clone(),
                span: c.style.column_span == Some(true) && !c.inline,
                // Щуп роста параллельного потока не знает: распорка
                // (`spacer_before`) — про в-поточную высоту, а поток высоты
                // не даёт (css-break-3 §3). Раздвинуть коробку им значило бы
                // вернуть переполнение в поток. `over == 0.0` при `h >= 0`
                // выключает поток в `fill_at` тождественно.
                over: 0.0,
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // `clone` соседи после такой коробки получили бы распорки по
                // ДРУГОМУ плану.
                clone_dec: clone_dec(c),
                // Тот же предикат, что у `StackChild` в сборке стопки: иначе
                // распорки легли бы по другому плану, чем укладка.
                overflow_top: fixed.is_some() && rows.is_none() && !parallel_items_inside(c, 4),
                repeat: repeat_leads(c, fixed, rows),
                par: par.get(i).copied().unwrap_or_default(),
            })
            .collect();
        let mut grows = crate::flow::ColumnStack::growths(&probe, count, fixed, rows, copies);
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает точки
        // выше, и `at` из одного плана остаётся верным для всех записей
        // прохода (рост дорожки иначе находил ряд по устаревшим полосам:
        // `grid-item-fragmentation-048`, два разрыва в одном проходе).
        grows.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal))
        });
        let mut changed = false;
        for (kid, at, grow, forced) in grows {
            let c = &mut kids[kid].0;
            // Ряд в точках, вытолкнутый целиком: растёт ПРЕДЫДУЩАЯ дорожка.
            if grow_grid_track(c, at, grow) {
                if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
                    kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                    changed = true;
                }
                continue;
            }
            PUSH_FORCED.with(|f| f.set(forced));
            let found = pushed_box_at(c, at, 4);
            PUSH_FORCED.with(|f| f.set(false));
            let Some(id) = found else {
                continue;
            };
            // Монолит двигает поле, принудительный разрыв — коробка (Х3).
            let moved = if forced {
                spacer_before(c, id, grow)
            } else {
                grow_before(c, id, grow)
            };
            if !moved {
                continue;
            }
            if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
                kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    kids
}

/// Монолит ребёнка колонок держится ТОЛЬКО на `break-inside: avoid`: ни
/// `contain: size`, ни прокрутки, ни замещаемого, ни атомарной строчной, ни
/// сплошного строчного набора (тот же список, что у `monolith` в сборке
/// стопки, без `break_inside_avoid`).
pub(crate) fn avoid_only_monolith(c: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| matches!(o, Some(crate::computed::Overflow::Scroll));
    let block_kid = |n: &Node| matches!(n, Node::Element(k) if !k.inline || k.style.display == Some(Display::Block));
    !(size_monolith(c)
        || scrolls(c.style.overflow_x)
        || scrolls(c.style.overflow_y)
        || matches!(
            c.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            c.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
        || (c.children.iter().any(|n| !is_blank(n)) && !c.children.iter().any(block_kid)))
}

/// Перемера после распорки — с прежними полями у элемента строки flex
/// (`split_flex_lines` кладёт в поле ещё и `row-gap`).
pub(crate) fn keep_par_margins(s: Shape, old: &Shape, par: Option<&crate::flow::Par>) -> Shape {
    if par.is_some_and(|p| p.group != 0) {
        (s.0, old.1, old.2, s.3, s.4, s.5)
    } else {
        s
    }
}

/// Многострочный КОЛОНОЧНЫЙ flex-контейнер — ребёнок стопки колонок —
/// раскрывается в свои элементы, разложенные по строкам (`flow::Par`): строки
/// такого контейнера — параллельные потоки (Blink `flex_layout_algorithm.cc`
/// :2108-2112 — `FlexColumnBreakInfo` на каждую строку; :2504-2515 — разрыв
/// элемента переходит к следующей СТРОКЕ, а не обрывает контейнер; :2536-2560
/// — рост элемента от фрагментации двигает только его строку,
/// `item_offset_adjustment`). Прежде контейнер мерился стопкой ВСЕХ
/// элементов подряд (`shape_full`, ветка `flex_items`), и строки ложились одна
/// под другой (`multi-line-column-flex-fragmentation-*`: «красное видно»).
///
/// Строки — по css-flexbox-1 §9.3 (шаг 5, «collect consecutive items one by one
/// until the first time that the next collected item would not fit into the
/// flex container's inner main size»): главный размер — внешняя высота меры
/// элемента, между элементами `row-gap`. Поперечный — наибольшая внешняя
/// ширина в строке; `align-content: normal` = `stretch` раздаёт свободное место
/// строкам поровну (§9.4 шаг 15 / css-align-3 §5.4), элемент `width: auto` при
/// `align-items: normal` тянется на строку (§9.4 шаг 11). Коробка контейнера —
/// первая «строка» группы без детей: рисует его фон под элементами и занимает
/// его высоту и тогда, когда строки короче.
///
/// Гейт узкий — ровно то, что выражается без раскладки: колонка с переносом
/// (не `reverse`), высота в точках, ни полей, ни рамок, ни отбивок у
/// контейнера, `justify-content`/`align-content`/`align-items` по умолчанию,
/// дети — блочные элементы в потоке без `flex-grow`, `flex-basis`,
/// `align-self`, боковых полей и отбивок, ширина в точках либо пустой
/// `auto`. Иначе контейнер идёт прежним путём.
pub(crate) fn split_flex_lines(
    kids: Vec<(Element, Shape)>,
    col_w: Option<f32>,
    merged: &Computed,
) -> (Vec<(Element, Shape)>, Vec<crate::flow::Par>, Vec<Option<Computed>>, Vec<usize>) {
    let mut out: Vec<(Element, Shape)> = Vec::with_capacity(kids.len());
    let mut par: Vec<crate::flow::Par> = Vec::with_capacity(kids.len());
    let mut parent: Vec<Option<Computed>> = Vec::with_capacity(kids.len());
    let mut group = 0u32;
    let mut starts: Vec<usize> = Vec::with_capacity(kids.len() + 1);
    for (c, s) in kids {
        starts.push(out.len());
        match flex_lines_of(&c, col_w) {
            Some(lines) => {
                group += 1;
                let pm = inline::inherit(merged, &c.style);
                let h = match c.style.height {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                // Коробка контейнера — ПЕРВОЙ «строкой» группы: рисуется под
                // элементами (фон контейнера, `multi-line-column-flex-
                // fragmentation-033`) и занимает свою высоту, даже когда строки
                // короче. `break-before` первых элементов ВСЕХ строк — на неё,
                // `break-after` последних — на последний элемент группы (Blink
                // `flex_layout_algorithm.cc:1907-1918`: колонки строк —
                // «ряд», значения сливаются и уходят контейнеру; «avoid» +
                // принудительный = принудительный, `JoinFragmentainerBreakValues`).
                let heads: Vec<&Element> = lines.iter().filter_map(|l| l.1.first().map(|x| &x.0)).collect();
                let tails: Vec<&Element> = lines.iter().filter_map(|l| l.1.last().map(|x| &x.0)).collect();
                let (bf, ba) = (
                    heads.iter().any(|e| edge_break(e, false)),
                    heads.iter().any(|e| edge_avoid(e, false)),
                );
                let (af, aa) = (
                    tails.iter().any(|e| edge_break(e, true)),
                    tails.iter().any(|e| edge_avoid(e, true)),
                );
                let mut boxc = c.clone();
                boxc.node_id = c.node_id ^ 0x0F1E_5BAC_E000_0001;
                boxc.children = Vec::new();
                boxc.style.display = Some(Display::Block);
                boxc.style.flex_dir = None;
                boxc.style.flex_wrap = None;
                boxc.style.gap = None;
                boxc.style.break_before_force = bf;
                boxc.style.break_before_avoid = ba && !bf;
                boxc.style.break_after_force = false;
                boxc.style.break_after_avoid = false;
                out.push((boxc, (h, 0.0, 0.0, Vec::new(), Vec::new(), Vec::new())));
                par.push(crate::flow::Par {
                    group,
                    group_start: true,
                    line_start: true,
                    group_end: false,
                    dx: 0.0,
                    avoid_only: false,
                    float: false,
                    clears: false,
                });
                parent.push(None);
                let n_lines = lines.len();
                for (li, (dx, items)) in lines.into_iter().enumerate() {
                    let m = items.len();
                    for (ii, (mut e, sh)) in items.into_iter().enumerate() {
                        if li + 1 == n_lines && ii + 1 == m {
                            e.style.break_after_force |= af;
                            e.style.break_after_avoid |= aa && !af;
                        }
                        // `break-inside: avoid` без настоящего монолита
                        // (`flow::Par::avoid_only`).
                        let avoid_only = e.style.break_inside_avoid
                            && !size_monolith(&e)
                            && visible_overflow(&e.style);
                        out.push((e, sh));
                        par.push(crate::flow::Par {
                            group,
                            group_start: false,
                            line_start: ii == 0,
                            group_end: false,
                            dx,
                            avoid_only,
                            float: false,
                    clears: false,
                        });
                        parent.push(Some(pm.clone()));
                    }
                }
                if let Some(p) = par.last_mut() {
                    p.group_end = true;
                }
            }
            None => match flex_row_lines_of(&c, col_w) {
                // Многострочный РЯД: строки идут одна за другой, а элементы
                // строки — параллельные потоки своей группы (Blink
                // `flex_layout_algorithm.cc:2167-2213`: элемент ряда — свой
                // поток, конец ряда — самый дальний конец его элементов).
                Some(lines) => {
                    let pm = inline::inherit(merged, &c.style);
                    for items in lines {
                        group += 1;
                        let m = items.len();
                        for (ii, (dx, e, sh)) in items.into_iter().enumerate() {
                            let avoid_only = e.style.break_inside_avoid
                                && !size_monolith(&e)
                                && visible_overflow(&e.style);
                            out.push((e, sh));
                            par.push(crate::flow::Par {
                                group,
                                group_start: ii == 0,
                                line_start: true,
                                group_end: ii + 1 == m,
                                dx,
                                avoid_only,
                                float: false,
                    clears: false,
                            });
                            parent.push(Some(pm.clone()));
                        }
                    }
                }
                None => {
                    out.push((c, s));
                    par.push(crate::flow::Par::default());
                    parent.push(None);
                }
            },
        }
    }
    starts.push(out.len());
    (out, par, parent, starts)
}

/// Строки контейнера для `split_flex_lines`: `(сдвиг строки по x, элементы с
/// мерой)`. `None` — контейнер вне гейта.
#[allow(clippy::type_complexity)]
pub(crate) fn flex_lines_of(c: &Element, col_w: Option<f32>) -> Option<Vec<(f32, Vec<(Element, Shape)>)>> {
    use crate::computed::FlexDir;
    if flex_gap_rules(&c.style) {
        return None;
    }
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let s = &c.style;
    let b = s.borders();
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.flex_dir != Some(FlexDir::Col)
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.vertical == Some(true)
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        // `position: relative` без сдвигов ничего не двигает, а содержащим
        // блоком ему служить некому (внепоточных детей гейт не пускает).
        || !(s.position.is_none()
            || (s.position == Some(crate::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || s.transform.is_some()
        || s.min_height.is_some()
        || s.max_height.is_some()
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return None;
    }
    let Some(Len::Px(main)) = s.height else {
        return None;
    };
    let cross = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = match s.gap {
        None => (0.0, 0.0),
        Some((r, g)) => {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v.max(0.0)),
                _ => None,
            };
            (px(&r)?, px(&g)?)
        }
    };
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = &k.style;
        if k.inline
            || out_of_flow(ks)
            || !matches!(ks.position, None | Some(crate::computed::Position::Relative))
            || ks.float.unwrap_or(0) != 0
            || ks.flex_grow.is_some_and(|g| g > 0.0)
            || ks.flex_basis.is_some()
            || ks.align_self.is_some()
            || ks.align_self_normal
            || !zero(&ks.margin.left)
            || !zero(&ks.margin.right)
        {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    // Визуальный порядок (`order`, стабильно), как в `blocks()`.
    items.sort_by_key(|k| k.style.order.unwrap_or(0));
    // Мера и внешний поперечный размер элемента; `None` у ширины — `auto`.
    let mut measured: Vec<(Element, Shape, Option<f32>)> = Vec::with_capacity(items.len());
    for k in items {
        let sh = shape_full(k, 4, ShapeCx::COLUMNS)?;
        let kb = k.style.borders();
        let side = |l: &Option<Len>| match l {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(*v),
            _ => None,
        };
        let w = match k.style.width {
            Some(Len::Px(w)) => {
                let extra = if k.style.border_box == Some(true) {
                    0.0
                } else {
                    side(&k.style.padding.left)?
                        + side(&k.style.padding.right)?
                        + side(&kb.left)?
                        + side(&kb.right)?
                };
                Some(w + extra)
            }
            None | Some(Len::Auto)
                if zero(&k.style.padding.left)
                    && zero(&k.style.padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right)
                    && k.children.iter().all(is_blank) =>
            {
                None
            }
            _ => return None,
        };
        measured.push((k.clone(), sh, w));
    }
    // Строки: §9.3 шаг 5.
    let mut lines: Vec<Vec<(Element, Shape, Option<f32>)>> = Vec::new();
    let mut used = 0.0f32;
    for (k, sh, w) in measured {
        let outer = sh.0 + sh.1 + sh.2;
        match lines.last_mut() {
            Some(line) if used + row_gap + outer <= main + 0.01 => {
                used += row_gap + outer;
                line.push((k, sh, w));
            }
            _ => {
                used = outer;
                lines.push(vec![(k, sh, w)]);
            }
        }
    }
    // Одна строка — однострочный по сути контейнер: прежний путь его знает.
    if lines.len() < 2 {
        return None;
    }
    let n = lines.len() as f32;
    let crosses: Vec<f32> = lines
        .iter()
        .map(|l| l.iter().filter_map(|x| x.2).fold(0.0f32, f32::max))
        .collect();
    let free = cross - crosses.iter().sum::<f32>() - col_gap * (n - 1.0);
    let extra = if free > 0.0 { free / n } else { 0.0 };
    let mut out = Vec::with_capacity(lines.len());
    let mut dx = 0.0f32;
    for (line, lc) in lines.into_iter().zip(crosses) {
        let lc = lc + extra;
        let mut items = Vec::with_capacity(line.len());
        for (i, (mut k, mut sh, w)) in line.into_iter().enumerate() {
            // `auto` тянется на строку (§9.4 шаг 11, `align-self: stretch`).
            if w.is_none() {
                k.style.width = Some(Len::Px(lc));
            }
            // Зазор между элементами строки — к полю следующего: на разрыве
            // он пропадает вместе с полем (Blink
            // `UpdateOffsetAdjustmentForSuppressedRowGap`, :2486-2500).
            if i > 0 {
                sh.1 += row_gap;
            }
            items.push((k, sh));
        }
        out.push((dx, items));
        dx += lc + col_gap;
    }
    Some(out)
}

/// Линейки промежутков у flex-контейнера (css-gaps-1 `column-rule`/`row-rule`).
/// Строки, раскрытые в параллельные потоки (`split_flex_lines`), — отдельные
/// копии без контейнера, и художник линеек (`GapRulePainter`) их не видит:
/// линейки пропадали целиком (`flex-gap-decorations-fragmentation-025/028/029/
/// 030`, v225 0.04…0.28 → v226 0.82…2.07). Такой контейнер идёт прежним путём —
/// одной копией со своими линейками.
pub(crate) fn flex_gap_rules(s: &Computed) -> bool {
    s.column_rule_visible == Some(true)
        || s.row_rule_visible == Some(true)
        || s.column_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
        || s.row_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
}

/// Строки многострочного РЯДА flex для `split_flex_lines`: по строке —
/// `(сдвиг по x, элемент, мера)` каждого элемента. Строки — по css-flexbox-1
/// §9.3 шаг 5 (главная ось — ширина), поперечный размер строки — наибольшая
/// внешняя высота её элементов; элемент `height: auto` при `align-items:
/// normal` тянется на строку (§9.4 шаг 11) — полом `min-height`, чтобы рост
/// от фрагментации (`grow_pushed`) коробку не обрезал (Blink: «expansion past
/// the block-end of each row», `flex_layout_algorithm.cc:2560-2575`).
/// `break-before` любого элемента строки — разрыв перед строкой, `break-after`
/// — после неё (`:1898-1906`): на первого и последнего элемента строки. Гейт —
/// как у колонки, плюс высота контейнера `auto` и хотя бы одна строка из
/// нескольких элементов: ряд «элемент на строку» прежний путь уже знает.
#[allow(clippy::type_complexity)]
pub(crate) fn flex_row_lines_of(c: &Element, col_w: Option<f32>) -> Option<Vec<Vec<(f32, Element, Shape)>>> {
    use crate::computed::FlexDir;
    if flex_gap_rules(&c.style) {
        return None;
    }
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let s = &c.style;
    let b = s.borders();
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || !matches!(s.flex_dir, None | Some(FlexDir::Row))
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.vertical == Some(true)
        || s.rtl == Some(true)
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        || !matches!(s.height, None | Some(Len::Auto))
        || s.min_height.is_some()
        || s.max_height.is_some()
        || s.transform.is_some()
        || s.background.is_some()
        || s.bg_image.is_some()
        || !(s.position.is_none()
            || (s.position == Some(crate::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return None;
    }
    let main = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = match s.gap {
        None => (0.0, 0.0),
        Some((r, g)) => {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v.max(0.0)),
                _ => None,
            };
            (px(&r)?, px(&g)?)
        }
    };
    let side = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = &k.style;
        if k.inline
            || out_of_flow(ks)
            || !matches!(ks.position, None | Some(crate::computed::Position::Relative))
            || ks.float.unwrap_or(0) != 0
            || ks.flex_grow.is_some_and(|g| g > 0.0)
            || ks.align_self.is_some()
            || ks.align_self_normal
            || ks.min_height.is_some()
            || !zero(&ks.margin.left)
            || !zero(&ks.margin.right)
            || row_item_width(ks, main).is_none()
        {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by_key(|k| k.style.order.unwrap_or(0));
    // Строки по внешней ширине элементов (главная ось).
    let mut lines: Vec<Vec<(f32, Element, Shape)>> = Vec::new();
    let mut used = 0.0f32;
    // Строка из одного элемента и размер в процентах/`flex-basis` — шире
    // прежнего гейта: такой ряд прежде шёл целым контейнером, и его мера
    // (`shape_full` контейнера) уже знала рост строки от разрыва внутри
    // элемента, растяжение соседей на выросшую строку, статическое место
    // абсолютного потомка и вложенный параллельный поток. Раскрытые элементы
    // этого не выражают (`grow_pushed` растит лишь сам элемент): замерено
    // −3 (`multi-line-row-flex-fragmentation-053/060/062`). Такие элементы —
    // прежним путём.
    let widened = items.iter().any(|k| !matches!(k.style.width, Some(Len::Px(_))) || k.style.flex_basis.is_some());
    let mut single = true;
    let mut risky = false;
    for k in &items {
        risky |= carries_abspos(k, 4) || constrained_inside(k, 4);
    }
    for k in items {
        let kb = k.style.borders();
        // Гипотетический главный размер (css-flexbox-1 §9.2 шаг 3):
        // `flex-basis` в точках/процентах, иначе `width`; проценты — от
        // главного размера контейнера (§9.2 «percentage … against the flex
        // container's inner main size»). Копия элемента несёт его в точках:
        // в стопке он рисуется блоком в колонке.
        let w = row_item_width(&k.style, main)?;
        let mut k = k.clone();
        k.style.width = Some(Len::Px(w));
        k.style.flex_basis = None;
        let k = &k;
        let w = if k.style.border_box == Some(true) {
            w
        } else {
            w + side(&k.style.padding.left)?
                + side(&k.style.padding.right)?
                + side(&kb.left)?
                + side(&kb.right)?
        };
        let sh = shape_full(k, 4, ShapeCx::COLUMNS)?;
        match lines.last_mut() {
            Some(line) if used + col_gap + w <= main + 0.01 => {
                single = false;
                line.push((used + col_gap, k.clone(), sh));
                used += col_gap + w;
            }
            _ => {
                lines.push(vec![(0.0, k.clone(), sh)]);
                used = w;
            }
        }
    }
    if (single || widened) && (risky || lines.iter().flatten().any(|x| !x.2.4.is_empty())) {
        return None;
    }
    for (li, line) in lines.iter_mut().enumerate() {
        let cross = line.iter().map(|x| x.2.0 + x.2.1 + x.2.2).fold(0.0f32, f32::max);
        let (bf, ba) = (
            line.iter().any(|x| edge_break(&x.1, false)),
            line.iter().any(|x| edge_avoid(&x.1, false)),
        );
        let (af, aa) = (
            line.iter().any(|x| edge_break(&x.1, true)),
            line.iter().any(|x| edge_avoid(&x.1, true)),
        );
        let m = line.len();
        for (ii, (_, e, sh)) in line.iter_mut().enumerate() {
            // `height: auto` тянется на строку — полом.
            if matches!(e.style.height, None | Some(Len::Auto)) && sh.0 + sh.1 + sh.2 < cross - 0.01 {
                let eb = e.style.borders();
                let edges = side(&e.style.padding.top).unwrap_or(0.0)
                    + side(&e.style.padding.bottom).unwrap_or(0.0)
                    + side(&eb.top).unwrap_or(0.0)
                    + side(&eb.bottom).unwrap_or(0.0);
                let content = (cross - sh.1 - sh.2 - edges).max(0.0);
                e.style.min_height = Some(Len::Px(content));
                *sh = shape_full(e, 4, ShapeCx::COLUMNS)?;
            }
            // Зазор между строками — к полю элементов следующей строки.
            if li > 0 {
                sh.1 += row_gap;
            }
            if ii == 0 {
                e.style.break_before_force |= bf;
                e.style.break_before_avoid |= ba && !bf;
            }
            if ii + 1 == m {
                e.style.break_after_force |= af;
                e.style.break_after_avoid |= aa && !af;
            }
        }
    }
    Some(lines)
}

/// В поддереве (до `depth`) — коробка с заданной высотой и содержимым: свой
/// параллельный поток (css-break-3 §3), которого раскрытый элемент ряда не
/// выражает (`flex_row_lines_of`).
pub(crate) fn constrained_inside(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                (matches!(k.style.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
                    && k.children.iter().any(|n| !is_blank(n)))
                    || constrained_inside(k, depth - 1)
            }
            _ => false,
        })
}

/// Главный размер элемента многострочного ряда для `flex_row_lines_of`:
/// `flex-basis` (точки/проценты) при `flex-grow: 0`, иначе `width`; проценты —
/// от главного размера контейнера `main`. `None` — размер по содержимому
/// (`auto`/`content`), который гейт не выражает.
pub(crate) fn row_item_width(ks: &Computed, main: f32) -> Option<f32> {
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Pct(p)) => Some(p * main),
        _ => None,
    };
    if ks.basis_content == Some(true) {
        return None;
    }
    match ks.flex_basis {
        Some(Len::Auto) | None => px(&ks.width),
        _ => px(&ks.flex_basis),
    }
}

/// «Сдвиг ряда» сетки с рядами в точках (Blink `row_offset_adjustments`,
/// grid_layout_algorithm.cc:2304-2337: ряд, начатый в следующем
/// фрагментаинере, сдвигается на остаток предыдущего): разрез ровно на начале
/// ряда `i ≥ 1` (монолитный ряд или перенесённый `break-*`) растит дорожку
/// `i−1` на `grow`. Высота `auto` вырастает на то же (`grid_rows_px`),
/// заданная — нет, и ряд `i` всё равно встаёт на край колонки
/// (`grid-item-oof-004`: абсолют `align-self: end` в ряду 2 — в колонке 2).
/// Только без `row-gap`: при зазоре эталоны css-gaps держат ряд прежней
/// высоты (`grid-gap-decorations-fragmentation-011`). Прямой ребёнок стопки;
/// вложенная сетка — как прежде, без роста.
pub(crate) fn grow_grid_track(c: &mut Element, at: f32, grow: f32) -> bool {
    use crate::computed::{Track, TrackSize};
    if grid_stack(c) || !matches!(c.style.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return false;
    }
    let gap0 = match c.style.gap {
        None | Some((None, _)) => true,
        Some((Some(Len::Px(v)), _)) => v.abs() < 0.01,
        _ => false,
    };
    if !gap0 {
        return false;
    }
    let Some(bands) = grid_px_row_bands(&c.style) else {
        return false;
    };
    let px_of = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let top = px_of(&c.style.padding.top) + px_of(&c.style.borders().top);
    let Some(i) = bands.iter().position(|b| (top + b.0 - at).abs() < 0.01) else {
        return false;
    };
    if i == 0 {
        return false;
    }
    let Some(TrackSize::Single(Track::Px(v))) =
        c.style.grid_rows.as_mut().and_then(|r| r.get_mut(i - 1))
    else {
        return false;
    };
    *v += grow;
    true
}

/// Коробка, дающая точку разрыва класса A (css-break-4 §possible-breaks):
/// блочная, в потоке, не плавающая. `Element.inline` ставится по ТЕГУ
/// (`dom.rs` `INLINE_TAGS`), поэтому `<img style="display: block; page: b">`
/// блочным тут признаётся по `display` (`page-name-img-004`: иначе картинка
/// шла анонимным блоком с именем корня и рвала страницу).
pub(crate) fn class_a_box(e: &Element) -> bool {
    let blocky = (!e.inline && !inline_display(e))
        || matches!(
            e.style.display,
            Some(Display::Block)
                | Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::Table)
                | Some(Display::ListItem)
        );
    blocky
        && !out_of_flow(&e.style)
        && e.style.float.unwrap_or(0) == 0
        && !matches!(
            e.style.display,
            Some(Display::None) | Some(Display::Contents)
        )
}

/// Флекс- и грид-контейнер: его дети — элементы раскладки, не блоки потока.
/// 'page' применяется только к коробкам с точками разрыва класса A
/// (css-page-3 §page-prop «Applies to: boxes that create class A break
/// points»), и имя элемента флекса/грида контейнеру не передаётся и
/// разрыва между элементами не ставит (`page-name-flex-001/002-print`:
/// эталон без разрывов). Внутри элемента — обычный блочный поток
/// (`page-name-flex-004-print`).
pub(crate) fn item_container(e: &Element) -> bool {
    matches!(
        e.style.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    )
}

/// Строчный уровень по `display` у элемента с блочным тегом: `<div
/// style="display: inline-block">` стоит в строке и точки класса A не даёт
/// (css-display-3 §inner-outer; `page-name-inline-block-003-print`: два
/// таких `div` с разными `page` — одна строка, без разрыва).
pub(crate) fn inline_display(e: &Element) -> bool {
    // `display: inline` у блочного тега хранится как `InlineBlock` с меткой
    // `inline_display`: блоки внутри такого строчного разрывают его
    // (block-in-inline), и их разрывы — точки класса A
    // (`css-break/block-in-inline-015-print`). Его не трогаем.
    e.style.inline_display != Some(true)
        && matches!(
        e.style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    )
}

/// Табличная коробка — по тегу или по `display`.
pub(crate) fn table_box(c: &Element) -> bool {
    c.tag == "table"
        || matches!(
            c.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        )
}

/// Мера таблицы для укладки по фрагментаинерам (css-break-4
/// §possible-breaks, класс A: «table row group boxes, table row boxes»;
/// css-tables-3 §fragmentation). Ряды — стопка: высота ряда — наибольшая
/// из мер его ячеек (ячейка — обычная блочная мера с рамкой и отбивкой),
/// между рядами и вокруг них — `border-spacing`, снаружи — отступ и рамка
/// таблицы. `break-inside: avoid` ряда или группы — монолитный диапазон
/// (css-break-4 §breaking-rules, Rule 2), `break-before/after` ряда или
/// группы — принудительный разрыв на границе ряда. `thead` встаёт первым,
/// `tfoot` — последним, как в `table()`. Заданная высота — ПОЛ коробки рядов
/// (CSS 2.1 §17.5.3, css-tables-3 §terminology: `height` относится к table
/// grid box, обёртка лишь несёт подписи); растянутая коробка раздаёт остаток
/// рядам и потому идёт сплошным блоком без внутренних точек. `rowspan`,
/// сросшиеся рамки, вертикальное письмо, монолит внутри при заданной высоте
/// и неизмеримая ячейка — `None`: таблица идёт цельным куском измеренной
/// высоты без точек, как прежде.
pub(crate) fn table_shape(c: &Element, depth: u8, cx: ShapeCx) -> Option<Shape> {
    table_shape_bands(c, depth, cx, &mut TableBands::default())
}

/// Полосы первой шапки и первого подвала таблицы в координатах её меры:
/// `(верх секции, высота секции)` и `break-inside: avoid*` секции, плюс
/// вертикальный `border-spacing`. Нужны повтору секций во фрагментах
/// (`repeat_bands`).
#[derive(Default, Clone, Copy)]
pub(crate) struct TableBands {
    pub(crate) head: Option<(f32, f32)>,
    pub(crate) foot: Option<(f32, f32)>,
    pub(crate) head_avoid: bool,
    pub(crate) foot_avoid: bool,
    pub(crate) spacing: f32,
    /// Коробка рядов `[верх, низ)` — без подписей обёртки.
    pub(crate) box_top: f32,
    pub(crate) box_end: f32,
}

/// Повтор секций для стопки: полосы `flow::Repeat` и геометрия укладки.
pub(crate) type RepeatSpec = (Option<(f32, f32)>, Option<(f32, f32)>, crate::flow::RepeatGeom);

/// Повтор шапки/подвала таблицы-ребёнка стопки колонок (css-tables-3
/// §repeated-headers; Blink `table_layout_algorithm.cc:1082-1150`): секция
/// повторяется, если у неё `break-inside: avoid*` и блочный размер не больше
/// четверти фрагментаинера («block-size of the section is one quarter or less
/// than that of the fragmentainer»). Размер фрагментаинера Blink знает только
/// вне первого прохода балансировки (`HasKnownFragmentainerBlockSize`), поэтому
/// здесь — только `column-fill: auto` с заданной высотой и без рядов. Ответ —
/// полосы для `flow::Repeat`: шапка `(верх секции, секция + зазор под ней)`,
/// подвал `(верх секции − зазор, зазор + секция)`.
pub(crate) fn repeat_bands(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::flow::Rows>,
) -> Option<RepeatSpec> {
    let per = fixed.filter(|_| rows.is_none() && table_box(c))?;
    let mut b = TableBands::default();
    table_shape_bands(c, 4, ShapeCx::COLUMNS, &mut b)?;
    let max = per / 4.0;
    let head = b
        .head
        .filter(|&(_, h)| b.head_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at, h + b.spacing));
    let foot = b
        .foot
        .filter(|&(_, h)| b.foot_avoid && h > 0.01 && h <= max + 0.01)
        .map(|(at, h)| (at - b.spacing, h + b.spacing));
    if head.is_none() && foot.is_none() {
        return None;
    }
    let geom = crate::flow::RepeatGeom {
        head: head.map_or(0.0, |h| h.1),
        foot: foot.map_or(0.0, |f| f.1),
        head_end: head.map_or(0.0, |(at, h)| at + h),
        foot_at: foot.map_or(f32::MAX, |(at, _)| at),
        foot_end: foot.map_or(f32::MAX, |(at, h)| at + h),
        box_top: b.box_top,
        box_end: b.box_end,
    };
    Some((head, foot, geom))
}

/// `RepeatGeom` для щупов укладки (`grow_pushed`, план `clone`): та же мера,
/// что у `StackChild` в сборке стопки.
pub(crate) fn repeat_leads(
    c: &Element,
    fixed: Option<f32>,
    rows: Option<crate::flow::Rows>,
) -> crate::flow::RepeatGeom {
    repeat_bands(c, fixed, rows).map_or_else(Default::default, |r| r.2)
}

pub(crate) fn table_shape_bands(c: &Element, depth: u8, cx: ShapeCx, bands: &mut TableBands) -> Option<Shape> {
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    if depth == 0
        || c.style.vertical == Some(true)
        || c.style.border_collapse == Some(true)
    {
        return None;
    }
    // Заданная высота таблицы БОЛЬШЕ не повод отказаться от меры: она просто
    // ПОЛ коробки рядов (CSS 2.1 §17.5.3 — используемая высота есть большая
    // из заданной и суммы рядов; css-tables-3 §terminology кладёт `height` на
    // table grid box, а §style-overrides отдаёт обёртке только `position`,
    // `float`, `margin`-* и края — `height` среди них НЕТ). Пока отказ стоял,
    // многоколоночник с такой таблицей не фрагментировался ВОВСЕ: снимок
    // `specified-block-size-002` — зелёное (10,67)..(134,566), то есть 403 css
    // высоты во всю ширину при эталоне (10,67)..(134,191); снимок
    // `specified-block-size-003` — колонки 2-4 пусты, 11750 точек красного
    // фона многоколоночника.
    // Проценты и прочие единицы, как и прежде, — отказ от меры целиком:
    // разрешать их некому, а недомер увёл бы разрез не туда.
    let spec_of = |l: &Option<Len>| match l {
        None => Some(None),
        Some(Len::Px(v)) => Some(Some(*v)),
        _ => None,
    };
    let spec_h = spec_of(&c.style.height)?;
    let spec_min_h = spec_of(&c.style.min_height)?;
    let b = c.style.borders();
    let mt = px_of(&c.style.margin.top).unwrap_or(0.0);
    let mb = px_of(&c.style.margin.bottom).unwrap_or(0.0);
    let top = px_of(&c.style.padding.top)? + px_of(&b.top)?;
    let bot = px_of(&c.style.padding.bottom)? + px_of(&b.bottom)?;
    // Умолчание тега `<table>` (2px) приходит каскадом; у `display: table`
    // зазора нет. Презентационные `cellspacing`/`cellpadding` — как в
    // `table()`: ниже авторского, выше умолчания браузера
    // (`block-page-break-inside-avoid-11`: мера 198 при рисунке 192 —
    // ложный разрыв внутри `avoid`).
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..cx
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Дети чинятся ТЕМ ЖЕ `fixup_table_children`, которым их чинит рисователь
    // `table()` (render.rs:13551; css-tables-3 §3 fixup): бесхозная ячейка,
    // блок или текст прямо в таблице получают анонимный ряд, не-ячейка внутри
    // ряда — анонимную ячейку, `display: contents` растворяется, а ряды внутри
    // групп чинит рекурсивный заход. Прежде мера шла по СЫРОМУ дереву, и любой
    // ребёнок, которому нужна анонимная коробка, отдавал `None`; у колонок
    // `None` — отказ от укладки целиком, и многоколоночник не фрагментировался
    // вовсе: первая колонка переполнялась, остальные пустовали
    // (`table-border-000` 6.25 — зелёное до y=566 при коробке до y=191,
    // `table-cell-border-001` 2.08, `overflow-scroll-row`).
    // `fixed` объявлен ДО `parts`: `parts` держит ссылки внутрь него.
    let fixed = fixup_table_children(&c.children);
    // Подпись — НЕ ряд и не группа: она живёт в анонимной ОБЁРТКЕ таблицы, а
    // не в коробке рядов (css-tables-3 §terminology: table wrapper box —
    // «A block container box generated around table grid boxes to account for
    // any space occupied by each table-caption it owns»; table grid box —
    // «A block-level box containing the table-internal boxes, EXCLUDING its
    // captions»). `fixup_table_children` её не чинит (ветка `is_cap` →
    // `out.push(child.clone())`, render.rs:16726), и в разборе ниже она
    // уходила в `_ => return None`: таблица с подписью в колонках не
    // фрагментировалась ВОВСЕ — первая колонка переполнялась, остальные
    // пустовали. Снимок `table-border-004`: красное 41,67..134,191 — три
    // колонки из четырёх пусты (11750 точек фона многоколоночника).
    // Сторона — с самой подписи, при пустоте — с таблицы (наследование
    // `caption-side`), ровно как в рисователе `table()`; порядок внутри
    // стороны — разметки (`sections-and-captions-mixed-order`: сверху 1 и 2,
    // снизу 14 и 15…20).
    let is_cap_kid = |e: &Element| e.tag == "caption" || e.style.is_caption == Some(true);
    let mut caps_top: Vec<&Element> = Vec::new();
    let mut caps_bot: Vec<&Element> = Vec::new();
    // Части в порядке отрисовки: первая заголовочная группа — вперёд,
    // первая подвальная — назад, остальное как в разметке (`table()`).
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        if is_cap_kid(e) {
            if e.style.caption_bottom.or(c.style.caption_bottom) == Some(true) {
                caps_bot.push(e);
            } else {
                caps_top.push(e);
            }
            continue;
        }
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    // `break-before`/`break-after: avoid*` РЯДА — не только своё значение.
    // css-break-4 §break-propagation переносит `break-before` ПЕРВОГО поточного
    // ребёнка на контейнер («a 'break-before' value on a first in-flow child box
    // is propagated to its container. Likewise a 'break-after' value on a last
    // in-flow child box»), а для «parallel layout» разрешает более частное
    // правило — ячейки ряда как раз параллельные потоки. Частное правило берём у
    // эталона: Blink СЛИВАЕТ значения ВСЕХ ячеек ряда в значение ряда,
    // `table_row_layout_algorithm.cc:169-177` (`row_break_before =
    // JoinFragmentainerBreakValues(row_break_before, cell_break_before)` и та же
    // строка для `break-after`), и отдаёт результат наружу на `:255-257`.
    // `edge_avoid` уже делает перенос с КРАЙНЕГО ребёнка ячейки — это Blink'овы
    // `InitialBreakBefore`/`FinalBreakAfter`; остаётся объединение по всем ячейкам.
    fn row_avoid(row: &Element, last: bool) -> bool {
        edge_avoid(row, last)
            || row.children.iter().filter(|n| !is_blank(n)).any(|n| {
                matches!(n, Node::Element(cell) if is_cell(cell) && edge_avoid(cell, last))
            })
    }
    // Принудительные `break-before`/`break-after` ячеек — тем же слиянием на
    // ряд (Blink `table_row_layout_algorithm.cc:169-177`,
    // `JoinFragmentainerBreakValues`): `break-before-expansion-001` — ячейка
    // второго ряда с `break-before: column`.
    fn row_force(row: &Element, last: bool) -> bool {
        (if last { row.style.break_after_force } else { row.style.break_before_force })
            || row.children.iter().filter(|n| !is_blank(n)).any(|n| {
                matches!(n, Node::Element(cell) if is_cell(cell) && edge_break(cell, last))
            })
    }
    // Плоский список рядов: ряд, № группы, avoid группы, разрывы (свои и
    // группы — на первом/последнем её ряду), запреты разрыва на КРАЯХ ряда
    // (`ab`/`aa` — свои, ячеек и краёв группы).
    struct RowRef<'a> {
        row: &'a Element,
        group: usize,
        avoid: bool,
        fb: bool,
        fa: bool,
        ab: bool,
        aa: bool,
    }
    let mut rows: Vec<RowRef> = Vec::new();
    for (gi, (_, e)) in parts.iter().enumerate() {
        if is_row(e) {
            rows.push(RowRef {
                row: e,
                group: gi,
                avoid: false,
                fb: row_force(e, false),
                fa: row_force(e, true),
                ab: row_avoid(e, false),
                aa: row_avoid(e, true),
            });
            continue;
        }
        let inner: Vec<&Element> = e
            .children
            .iter()
            .filter(|n| !is_blank(n))
            .map(|n| match n {
                Node::Element(r) if is_row(r) => Some(r),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let last = inner.len().saturating_sub(1);
        for (i, r) in inner.iter().enumerate() {
            rows.push(RowRef {
                row: r,
                group: gi,
                avoid: e.style.break_inside_avoid,
                fb: row_force(r, false) || (i == 0 && e.style.break_before_force),
                fa: row_force(r, true) || (i == last && e.style.break_after_force),
                // Край ГРУППЫ — тот же перенос, что у принудительных выше:
                // `break-before: avoid` группы действует на её ПЕРВОМ ряду,
                // `break-after: avoid` — на ПОСЛЕДНЕМ (css-break-4 §break-between,
                // «Applies to: … table row groups, table rows»).
                ab: row_avoid(r, false) || (i == 0 && e.style.break_before_avoid),
                aa: row_avoid(r, true) || (i == last && e.style.break_after_avoid),
            });
        }
    }
    // Таблица ИЗ ОДНИХ ПОДПИСЕЙ мерится: коробка рядов у неё пуста (только
    // рамка и отбивка), но обёртка несёт подписи и их точки разреза.
    // `table-border-004`: подпись 110 + пустая коробка `border-width:20px 0`
    // (20 + 20) + подпись 250 = 400 = ровно четыре колонки по 100.
    // Таблица ИЗ ОДНОЙ ЗАДАННОЙ ВЫСОТЫ мерится так же, как из одних подписей:
    // рядов нет, но коробка есть и её высоту знает стиль
    // (`specified-block-size-002`: пустая `display: table; height: 400px` =
    // ровно 4 колонки по 100; `table-border-007`: рамка 10 + 180 + 10 = 200 =
    // две колонки по 100 — проба `p-tb-007` = 0.00).
    if rows.is_empty()
        && caps_top.is_empty()
        && caps_bot.is_empty()
        && spec_h.is_none()
        && spec_min_h.is_none()
    {
        return None;
    }
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    if top > 0.0 {
        solid.push((0.0, top));
    }
    let mut y = top;
    let mut group_open: Option<(usize, f32)> = None;
    // Сцепка рядов, скованных `break-before/after: avoid*`: начало открытого
    // диапазона, `open` предыдущего ряда и его `break-after: avoid*`.
    let mut avoid_run: Option<f32> = None;
    let mut prev_open = 0.0f32;
    let mut prev_aa = false;
    let mut force_next = false;
    // Ячейки с `rowspan`: (первый ряд, охват, высота содержимого). Их высота
    // НЕ растит свой ряд — она ложится на все охваченные (css-tables-3
    // §height-distribution); мера принимается, только если охват и так
    // вмещает ячейку (сверка после цикла), иначе — прежний отказ. Точки
    // разреза внутри такой ячейки не берутся: где именно внутри охвата
    // стоит её содержимое, мера не знает. Прежде любой `rowspan` отменял
    // меру таблицы целиком, и стол не фрагментировался вовсе
    // (`table-rowspan-001`: пустая ячейка `rowspan=2`, снимок — вторая
    // колонка пуста, стол переполняет первую).
    let mut spans: Vec<(usize, usize, f32)> = Vec::new();
    let mut row_box: Vec<(f32, f32)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let start = y + spacing;
        let mut h = px_of(&r.row.style.height)?;
        for n in r.row.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) {
                return None;
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (02.10): урезать охват до оставшихся
            // рядов (охват в один ряд — обычная ячейка) и брать точки и
            // монолиты содержимого охватывающей ячейки в меру (от начала её
            // ряда). Срез css-break/table 136 пар: 64 → 63, ядро 670: 413 →
            // 412 — потеряна `table-cell-expansion-005` (0.00 → «красное
            // видно»), приобретений ноль: монолиты ячейки, которой раскладка
            // отдаёт высоту охвата, закрывали разрез там, где эталон режет.
            let rs = match cell.attr("rowspan").map(str::trim) {
                None => 1,
                Some(v) => match v.parse::<usize>() {
                    Ok(0) => rows.len().saturating_sub(i).max(1),
                    Ok(n) => n.max(1),
                    Err(_) => return None,
                },
            };
            let (ch, _, _, kcuts, kforced, ksolid) = shape_full(cell, depth - 1, cell_cx)?;
            if rs > 1 {
                spans.push((i, rs, ch));
                continue;
            }
            h = h.max(ch);
            // Точки и монолиты ячеек — объединением, как у ряда flex без
            // переноса: рвать нельзя там, где не даёт хоть одна ячейка.
            cuts.extend(kcuts.into_iter().map(|(a, b)| (start + a, start + b)));
            forced.extend(kforced.into_iter().map(|f| start + f));
            solid.extend(ksolid.into_iter().map(|(a, b)| (start + a, start + b)));
        }
        // css-break-4 §unforced-breaks Rule 1: «may break at a class A break point
        // only if all the 'break-after' and 'break-before' values applicable to
        // this break point allow it, which is when at least one of them forces a
        // break or when none of them forbid it». Значит запрет с ЛЮБОЙ из двух
        // сторон границу закрывает, а принудительный разрыв её открывает обратно
        // (§forced-breaks: «a forced break value effectively overrides any avoid
        // break value that also applies at that break point»). До сих пор мера не
        // читала запреты вовсе: точка класса A ставилась между любой парой рядов,
        // и разрез садился между вторым и третьим рядом `break-avoidance-001…006`
        // вместо первого и второго.
        let joined = i > 0 && (prev_aa || r.ab) && !(r.fb || force_next);
        if i > 0 {
            // Класс A между рядами: разрез по концу предыдущего ряда,
            // продолжение с начала этого (зазор остаётся на новой странице —
            // копия разложена целиком, геометрия сходится). На запрещённой
            // границе точки нет.
            if !joined {
                cuts.push((y, start));
            }
            if r.fb || force_next {
                forced.push(y);
            }
        }
        force_next = r.fa;
        // Диапазон `avoid` начинается в точке класса A ПЕРЕД зазором (`y`),
        // а у первого ряда — в нуле: тогда край листа внутри него читается
        // как «разрыв перед таблицей», а не «внутри её верхней рамки» (Blink
        // `FinishFragmentation`: «Avoid breaking inside block-start border …
        // No valid breakpoints there»; `rowgroup-page-break-inside-avoid-1`).
        let open = if i == 0 { 0.0 } else { y };
        // Одного отсутствия точки класса A мало: `fill_at` (flow.rs) режет ПО
        // КРАЮ КОЛОНКИ (`else { Some(at(edge)) }`), а `cuts` читает только на
        // точное совпадение с краем — они дают усечение поля, а не запрет.
        // Разрез останавливает единственная вещь — сплошной диапазон: ветка
        // `k.solid.iter().find(|&&(a, b)| holds(a, b))` уводит срез к НАЧАЛУ
        // диапазона, накрывшего край. Поэтому сцепка скованных рядов идёт ОДНИМ
        // диапазоном, и открывается он на `open` ПРЕДЫДУЩЕГО ряда: разрыв обязан
        // уйти к разрешённой границе ПЕРЕД ним (`break-avoidance-001`: сцепка
        // (50, 150), срез уходит на 50 — css-tables-3 §breaking-rules «insert some
        // vertical gap between the rows located before and at the overflow point»).
        // Вложенность с `break-inside: avoid` ряда и группы законна: `fill_at` на
        // страницах берёт САМЫЙ ВНЕШНИЙ из накрывших край диапазонов.
        if joined {
            if avoid_run.is_none() {
                avoid_run = Some(prev_open);
            }
        } else if let Some(s) = avoid_run.take() {
            solid.push((s, y));
        }
        if r.row.style.break_inside_avoid {
            solid.push((open, start + h));
        }
        if let Some((g, gs)) = group_open
            && g != r.group
        {
            solid.push((gs, y));
            group_open = None;
        }
        if r.avoid && group_open.is_none() {
            group_open = Some((r.group, open));
        }
        prev_open = open;
        prev_aa = r.aa;
        // Полосы первой шапки/подвала (`TableBands`): от верха их первого ряда
        // до низа последнего. Секция-ряд (`is_row` прямо в таблице) — тоже
        // секция своей роли.
        let (kind, sec) = parts[r.group];
        let band = match kind {
            0 => Some((&mut bands.head, &mut bands.head_avoid)),
            2 => Some((&mut bands.foot, &mut bands.foot_avoid)),
            _ => None,
        };
        if let Some((slot, avoid)) = band {
            *slot = Some(match *slot {
                Some((a, _)) => (a, start + h - a),
                None => (start, h),
            });
            *avoid = sec.style.break_inside_avoid;
        }
        row_box.push((start, h));
        y = start + h;
    }
    bands.spacing = spacing;
    // Сверка охватов (см. `spans`): ячейка выше суммы своих рядов с зазорами
    // раздала бы им высоту — этого мера не умеет, отказ как прежде.
    for (i, rs, ch) in spans {
        let last = (i + rs).min(row_box.len()).saturating_sub(1);
        let span_h = row_box[last].0 + row_box[last].1 - row_box[i].0;
        if ch > span_h + 0.01 {
            return None;
        }
    }
    // Сцепка, дожившая до конца коробки рядов, закрывается её низом.
    if let Some(s) = avoid_run {
        solid.push((s, y));
    }
    if let Some((_, gs)) = group_open {
        solid.push((gs, y));
    }
    // Монолит ВНУТРИ коробки рядов (ячейка с `contain: size`, `break-inside:
    // avoid` ряда или группы) вместе с заданной высотой — прежний отказ.
    // Устройства «монолит переполняет колонку» у стопки колонок нет:
    // `flow.rs:826` при `holds(a, b)` и `a <= from` разреза не берёт, а
    // следующая ветка режет по краю колонки прямо сквозь монолит. Проба
    // `target/probe-ftb/p-mo-003.html` — та же геометрия ОДНИМИ блоками
    // (коробка 200 с двумя `contain: size` по 100 в колонках по 60) — даёт
    // «красное видно», то есть даже верная мера рисунка не спасает, а
    // `monolithic-overflow-003` сегодня 2.08: мера сделала бы ЕЙ ХУЖЕ.
    // Единственный диапазон, которого гейт не считает, — верхняя рамка
    // (0, top): её положили ДО цикла рядов.
    // Страницам этот отказ не нужен: у стопки листов переполнение монолитом
    // своё (`ColumnStack::fill_at`, ветка `paged && placed && cur > target`,
    // crbug 1402540), а «монолитом» здесь оказывается уже верхняя рамка любой
    // ячейки (`shape_full` кладёт её сплошным диапазоном — Blink
    // `FinishFragmentation`: «Avoid breaking inside block-start border»).
    // Отказ уводил таблицу с `block-size` в меру стопки блоков по тегу, где
    // высота считается content-box: `table-fragmentation-001b-print` —
    // 336 + отбивка + рамка = 432 вместо 336 по border-box (UA-лист
    // css-tables-3 `table { box-sizing: border-box }`), третий пустой лист.
    let mono_inside = solid.iter().any(|&(a, b)| a > 0.01 || b > top + 0.01);
    if mono_inside && !cx.paged && (spec_h.is_some() || spec_min_h.is_some()) {
        return None;
    }
    // Заданная высота — ПОЛ коробки рядов. `box-sizing` — та же мерка, что в
    // рисователе (`table()`, `table_border_box`): у ТЕГА `<table>` высота по
    // border-box (UA-правило css-tables-3 `table { box-sizing: border-box }`),
    // у `display: table` на прочих тегах — контентная. `min-height` мерится
    // ПОЛНОЙ коробкой ВСЕГДА — так его кладёт `min_fix` в `table()`
    // (css-tables-3 §computing-the-table-height, CSSWG #5336).
    let content_h = y + spacing + bot;
    let edges = top + bot;
    let border_box =
        c.style.border_box == Some(true) || (c.tag == "table" && c.style.border_box.is_none());
    let floor_h = spec_h
        .map(|v| if border_box { v.max(edges) } else { v + edges })
        .into_iter()
        .chain(spec_min_h.map(|v| v.max(edges)))
        .fold(0.0f32, f32::max);
    let h_box = content_h.max(floor_h);
    // Растянутая коробка раздаёт остаток РЯДАМ (CSS 2.1 §17.5.3; css-tables-3
    // §height-distribution-algorithm), и границы рядов уезжают с измеренных
    // мест: точки класса A между ними больше не верны. Такая коробка идёт
    // сплошным блоком — срез по краю колонки есть правило, а не исключение
    // (css-break-4 §4 «slice»). Снимок `specified-block-size-007`: жёлтый ряд
    // (10,67)..(172,316), голубой (10,317)..(172,566) — раздача у нас РОВНАЯ
    // (200/200) при содержимом 1 и 3, и точки на 1 и 4 были бы ложью.
    if h_box > content_h + 0.01 {
        // Ряды растянуты — измеренные полосы секций тоже неверны.
        bands.head = None;
        bands.foot = None;
        cuts.clear();
        forced.clear();
        solid.clear();
        if top > 0.0 {
            solid.push((0.0, top));
        }
    }
    if bot > 0.0 {
        // Нижняя рамка/отбивка таблицы приклеена к монолиту последнего ряда —
        // то же правило, что у блока (`shape_full`, Р4 break-rest): точки
        // разрыва перед block-end рамкой нет (css-break-4 §possible-breaks, класс
        // C — только при ненулевом зазоре; Blink `FinishFragmentation` держит там
        // лишь «last-resort breakpoint»). `table-border-006`: ряды `avoid` 100 и
        // 70, `border-bottom: 30px` в колонке 170 — рамка уходит вместе с
        // последним рядом, а не одна во вторую колонку. Зазор `border-spacing`
        // между рядом и рамкой — та же «без промежутка» граница: рамка таблицы
        // от ряда отделена именно им, а не полем.
        let end_edge = h_box - bot;
        let glue = solid
            .iter()
            .filter(|&&(a, b)| a > 0.01 && (b - (end_edge - spacing)).abs() < 0.01)
            .map(|&(a, _)| a)
            .fold(end_edge, f32::min);
        solid.push((glue, h_box));
    }
    bands.box_top = 0.0;
    bands.box_end = h_box;
    // Подписей нет — коробка рядов и есть вся мера, как прежде.
    if caps_top.is_empty() && caps_bot.is_empty() {
        cuts.retain(|&(need, _)| need > 0.01 && need < h_box - 0.01);
        forced.retain(|&f| f > 0.01 && f < h_box - 0.01);
        return Some((h_box, mt, mb, cuts, forced, solid));
    }
    // Обёртка таблицы — обычная блочная стопка: верхние подписи, коробка
    // рядов, нижние подписи (css-tables-3 §terminology; Blink
    // `table_layout_algorithm.cc:988` «Add all the top captions» и `:1584`
    // «Add all the bottom captions» — обе петли по ВСЕМ подписям своей
    // стороны, секции между ними). Между соседями обёртки — точка класса A
    // (css-break-4 §possible-breaks: «Between sibling boxes of the following
    // types: … in-flow block-level boxes»), поля соседей схлопываются, как в
    // блочной стопке `shape_full`, и подпись поля ИМЕЕТ (Blink `:1195`
    // «Captions allow margins»). Рамка и отбивка самой таблицы подпись не
    // трогают: она вне коробки рядов.
    // `None` в списке — сама коробка рядов; поля у неё НУЛЕВЫЕ, поля таблицы
    // носит обёртка (они уже в `mt`/`mb` и возвращаются наружу).
    let mut cap_slots: Vec<Option<&Element>> =
        Vec::with_capacity(caps_top.len() + caps_bot.len() + 1);
    cap_slots.extend(caps_top.iter().copied().map(Some));
    cap_slots.push(None);
    cap_slots.extend(caps_bot.iter().copied().map(Some));
    let mut wy = 0.0f32;
    let mut wcuts: Vec<(f32, f32)> = Vec::new();
    let mut wforced: Vec<f32> = Vec::new();
    let mut wsolid: Vec<(f32, f32)> = Vec::new();
    let mut prev_mb = 0.0f32;
    let mut through = 0.0f32;
    let mut last_mb = 0.0f32;
    let mut wfirst = true;
    let mut wforce_next = false;
    for slot in cap_slots {
        let (ih, imt, imb, icuts, iforced, isolid, ifb, ifa) = match slot {
            None => (
                h_box,
                0.0,
                0.0,
                std::mem::take(&mut cuts),
                std::mem::take(&mut forced),
                std::mem::take(&mut solid),
                false,
                false,
            ),
            Some(cap) => {
                let (ch, cmt, cmb, ccuts, cforced, csolid) = shape_full(cap, depth - 1, cx)?;
                // Монолитная подпись (`contain: size`, `break-inside: avoid`,
                // прокрутка, замещаемая) — сплошной диапазон во всю высоту,
                // как у любого ребёнка блочной стопки (`shape_full`, ветка
                // `solid_box`).
                let csolid = if solid_box(cap) { vec![(0.0, ch)] } else { csolid };
                (
                    ch,
                    cmt,
                    cmb,
                    ccuts,
                    cforced,
                    csolid,
                    cap.style.break_before_force,
                    cap.style.break_after_force,
                )
            }
        };
        // У первого поле уходит СКВОЗЬ верх обёртки: своих рамки и отбивки у
        // неё нет (CSS 2.1 §8.3.1).
        let lead = if wfirst {
            through = imt;
            0.0
        } else {
            prev_mb.max(imt)
        };
        if !wfirst {
            wcuts.push((wy, wy + lead));
            if ifb || wforce_next {
                wforced.push(wy);
            }
        }
        wforce_next = ifa;
        let start = wy + lead;
        // Полосы секций — в координаты обёртки: коробка рядов стоит под
        // верхними подписями.
        if slot.is_none() {
            for s in [&mut bands.head, &mut bands.foot].into_iter().flatten() {
                s.0 += start;
            }
            bands.box_top += start;
            bands.box_end += start;
        }
        wcuts.extend(icuts.into_iter().map(|(need, nf)| (start + need, start + nf)));
        wforced.extend(iforced.into_iter().map(|f| start + f));
        wsolid.extend(isolid.into_iter().map(|(a, b)| (start + a, start + b)));
        wy = start + ih;
        prev_mb = imb;
        last_mb = imb;
        wfirst = false;
    }
    let h = wy;
    wcuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    wforced.retain(|&f| f > 0.01 && f < h - 0.01);
    Some((h, mt.max(through), mb.max(last_mb), wcuts, wforced, wsolid))
}

/// Досягаемость внепоточного корня стопки страниц: низ его коробки, а при
/// видимом переполнении — низ стопки его блочных детей (Blink копит
/// переполнение монолита в `BlockBreakToken::monolithic_overflow_` и
/// добавляет страницы, пока оно не кончится, crbug 1402540;
/// `monolithic-overflow-027`: абсолют `contain:size` 4in с ребёнком 8in —
/// «four green pages»). Обрезка `overflow-y` переполнение гасит (`-028`).
pub(crate) fn oof_reach(e: &Element, cx: ShapeCx) -> f32 {
    // `vh`/`vw` — от page area; `bottom: -200vh` тянет низ коробки на две
    // area ниже листа (эталоны `fixedpos-001..009`: копии `bottom: -N00vh` и
    // `top: N00vh` — досягаемость была нулевой, лист один).
    let len = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Vh(k)) => cx.viewport.map(|v| *k * v.1),
        Some(Len::Vw(k)) => cx.viewport.map(|v| *k * v.0),
        _ => None,
    };
    let top = len(&e.style.inset.top).unwrap_or(0.0);
    let from_bottom = len(&e.style.inset.bottom)
        .and_then(|b| cx.viewport.map(|v| v.1 - b))
        .unwrap_or(0.0);
    // Мера `None` (строчное содержимое) — не «пусто»: хотя бы точка высоты,
    // чтобы лист под самой строкой родился (`fixedpos-005-print`: `top:
    // 300vh` внутри `top: 100vh` — текст ровно на краю четвёртого листа).
    let own = shape_full(e, 4, cx).map(|s| s.0).unwrap_or_else(|| {
        if e.children.iter().all(is_blank) { 0.0 } else { 1.0 }
    });
    let clipped = matches!(
        e.style.overflow_y,
        Some(crate::computed::Overflow::Hidden)
            | Some(crate::computed::Overflow::Clip)
            | Some(crate::computed::Overflow::Scroll)
    );
    let inner: f32 = if clipped {
        0.0
    } else {
        e.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(k) if !k.inline && !out_of_flow(&k.style) => {
                    shape_full(k, 3, cx).map(|s| s.0 + s.1 + s.2)
                }
                _ => None,
            })
            .sum()
    };
    // Абсолютные потомки-абсолюты: содержащий блок — эта коробка, их `top`
    // — от её верха (CSS 2.1 §10.6.4). Мера `shape_full` у коробки со
    // строчным содержимым `None`, и дотяг вложенного `top: 300vh` терялся
    // (`fixedpos-005-print`: три листа вместо пяти).
    let nested = if clipped {
        0.0
    } else {
        e.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(k)
                    if k.style.position == Some(crate::computed::Position::Absolute) =>
                {
                    Some(oof_reach(k, cx))
                }
                _ => None,
            })
            .fold(0.0f32, f32::max)
    };
    (top + own.max(inner).max(nested)).max(from_bottom)
}

/// Монолит стопки страниц (css-break-4 §4.1; Blink `IsMonolithic`):
/// замещаемый, прокручиваемый, `break-inside: avoid`, `contain: size`,
/// атомарный строчный. Сплошной строчный набор монолитом НЕ считается:
/// страница режет его по краю, а обе стороны пары режутся одинаково.
pub(crate) fn page_monolith(e: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    e.style.break_inside_avoid
        || e.style.contain_size == Some(true)
        || scrolls(e.style.overflow_x)
        || scrolls(e.style.overflow_y)
        || matches!(
            e.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            e.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}

/// Запрет разрыва на краю коробки: свой `break-before: avoid*` /
/// `break-after: avoid*` либо такой же у ПЕРВОГО/ПОСЛЕДНЕГО поточного
/// блочного ребёнка, рекурсивно (css-break-4 §break-propagation — та же
/// строка спеки, что и у `edge_break` ниже). Зеркало `edge_break`, только
/// для запрещающих значений.
///
/// Ветки «бок о бок» (ряд `flex` без переноса, ячейки одного ряда, один ряд
/// сетки) сюда НЕ перенесены НАМЕРЕННО. Они переносят значение с ЛЮБОГО
/// ребёнка, а не только с крайнего, и на `avoid` это сразу ломает зелёные:
/// в `grid-item-fragmentation-032` запрет стоит на ВТОРОМ элементе сетки
/// рядом с `break-before: column` на третьем, в
/// `single-line-column-flex-fragmentation-016` — на ТРЕТЬЕМ элементе
/// колоночного флекса. Через крайнего ребёнка ни тот, ни другой не проходит,
/// и обе пары остаются нетронутыми. Перенос «бок о бок» для запретов —
/// отдельный шаг с отдельным замером.
pub(crate) fn edge_avoid(e: &Element, last: bool) -> bool {
    let own = if last {
        e.style.break_after_avoid
    } else {
        e.style.break_before_avoid
    };
    if own {
        return true;
    }
    let mut live = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))));
    let edge = if last { live.next_back() } else { live.next() };
    // Ряд `flex` без переноса — одна строка: `break-before/after` ЛЮБОГО
    // элемента переносится на строку (css-flexbox-1 §12: «In a row flex
    // container, the break-before and break-after values on flex items are
    // propagated to the flex line»), а единственная строка — первая и
    // последняя, значит значение уходит на контейнер. Сетка и колонка сюда
    // не попадают: там «бок о бок» неверен (см. выше).
    let row_line = matches!(e.style.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && e.style.webkit_box != Some(true)
        && matches!(
            e.style.flex_dir,
            None | Some(crate::computed::FlexDir::Row) | Some(crate::computed::FlexDir::RowReverse)
        )
        && e.style.flex_wrap != Some(true);
    if row_line {
        return e.children.iter().filter(|n| !is_blank(n)).any(|n| {
            matches!(n, Node::Element(k) if class_a_box(k) && edge_avoid(k, last))
        });
    }
    matches!(edge, Some(Node::Element(k)) if class_a_box(k) && edge_avoid(k, last))
}

/// Принудительный разрыв на краю коробки: свой `break-before`/`break-after`
/// либо такой же у ПЕРВОГО/ПОСЛЕДНЕГО поточного блочного ребёнка, рекурсивно
/// (css-break-4 §break-propagation: «a 'break-before' value on a first
/// in-flow child box is propagated to its container. Likewise a
/// 'break-after' value on a last in-flow child box»; Blink
/// `BoxFragmentBuilder::SetInitialBreakBefore`). Текст или строчный на краю
/// — анонимная коробка без разрыва, пропагация останавливается.
pub(crate) fn edge_break(e: &Element, last: bool) -> bool {
    // Абсолютная коробка вне потока: `break-*` применяется к блочным коробкам
    // ПОТОКА (css-break-3 §3.1 «Applies to: block-level boxes …»), а внутри
    // своего потока абсолют фрагментируется отдельно (Blink: OOF ложится во
    // фрагментаинер после потока, `out_of_flow_layout_part.cc`) — разрыв его
    // и его потомков потоку родителя не передаётся
    // (`out-of-flow-in-multicolumn-005`).
    if matches!(
        e.style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        return false;
    }
    let own = if last {
        e.style.break_after_force
    } else {
        e.style.break_before_force
    };
    if own {
        return true;
    }
    // Свой разрыв монолита стоит (выше), разрыв его ПОТОМКА наружу не идёт
    // (`forced_opaque`). `monolithic-content-with-forced-break-001`: `break-after:
    // column` у ребёнка `contain: size` уводил соседа в новую колонку, баланс
    // шёл 50 | 150 вместо 100 | 100. Зелёные `-002/-003` держатся на разрыве
    // САМОЙ монолитной коробки — он до этой строки.
    if forced_opaque(e) {
        return false;
    }
    let mut live = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))));
    // Ряд flex БЕЗ переноса: элементы стоят бок о бок и НАЧИНАЮТСЯ с верха
    // ряда — в блочном направлении «первый» и «последний» это каждый из них
    // (css-flexbox-1 §pagination: принудительный разрыв элемента поднимается
    // на контейнер; Blink `flex_layout_algorithm.cc` — разрыв элемента рвёт
    // весь ряд). Мерка ряда — та же, что в `shape_full` и `pushed_box_at`.
    // Проба `target/probe-9g/p-single-line-row-flex-fragmentation-018.html`
    // (`break-after: column` со ВТОРОГО элемента, поднят на `#flex`) = 0.00.
    // ВНИМАНИЕ: `edge_break` зовут и СТРАНИЦЫ (`render_paged`, строки
    // 956-963) — контроль обязан включать `single-line-row-flex-
    // fragmentation-046-print`.
    let row_nowrap = (matches!(
        e.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || e.style.webkit_box == Some(true))
        && matches!(
            e.style.flex_dir,
            None | Some(crate::computed::FlexDir::Row) | Some(crate::computed::FlexDir::RowReverse)
        )
        && e.style.flex_wrap != Some(true)
        && e.style.webkit_box_vertical != Some(true);
    // Ячейки одного ряда стоят бок о бок ровно как элементы ряда `flex` без
    // переноса: в блочном направлении «первой» и «последней» служит КАЖДАЯ
    // (css-break-4 §break-propagation вместе с css-tables-3 §fragmentation;
    // Blink рвёт весь ряд, если разрыв стоит в любой его ячейке). Пока мерка
    // молчала, `break-after` ПЕРВОЙ из двух ячеек терялся, и пары спасал
    // только отказ меры на таблице с голыми ячейками — это записано в
    // комментарии `shape_full` перед `table_box(c)`: «сквозной путь по тегу
    // хранит перенос принудительного разрыва ячейки на таблицу
    // (`break-after-table-cell`, `-child`: 0.00 → 2.08 без гейта)». Х2 этот
    // отказ снимает, значит перенос обязан жить здесь.
    let cells_abreast = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .any(|n| matches!(n, Node::Element(k) if is_cell(k)));
    // Элементы ОДНОГО ряда сетки стоят бок о бок ровно как ячейки: в
    // блочном направлении «первым» и «последним» служит КАЖДЫЙ из них
    // (css-grid-2 §Fragmenting Grid Layout: «The 'break-before' property on
    // the first row and the 'break-after' property on the last row are
    // propagated to the grid container»; Blink `grid_layout_algorithm.cc`
    // берёт `InitialBreakBefore`/`FinalBreakAfter` у КАЖДОГО элемента ряда).
    // Берётся только однозначный случай — сетка, у которой ряд ровно ОДИН:
    // колонок больше одной, явных дорожек рядов нет, областей нет, у детей
    // нет `grid-row`/`grid-area`, а поточных детей не больше, чем колонок.
    // Одноколоночная сетка сюда не попадает НАМЕРЕННО: там у каждого
    // ребёнка свой ряд, и первым/последним остаётся ровно первый/последний,
    // как и было (`grid-item-fragmentation-042` — зелёная, гейт её не
    // пускает: `grid-template-columns: 25px`, одна колонка).
    let grid_one_row = matches!(
        e.style.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) && e.style.grid_rows.is_none()
        && e.style.grid_areas.is_none()
        && !matches!(
            e.style.grid_auto_flow,
            Some(crate::computed::AutoFlow::Col) | Some(crate::computed::AutoFlow::ColDense)
        )
        && !e.children.iter().any(|n| matches!(n, Node::Element(k)
            if k.style.grid_row.is_some() || k.style.grid_area_name.is_some()))
        && e.style
            .grid_cols
            .map(|n| n as usize)
            .or_else(|| e.style.grid_tracks.as_ref().map(|t| t.len()))
            .is_some_and(|cols| {
                cols > 1
                    && e.children
                        .iter()
                        .filter(|n| !is_blank(n))
                        .filter(|n| matches!(n, Node::Element(k)
                            if !out_of_flow(&k.style)
                                && !matches!(k.style.display, Some(Display::None))))
                        .count()
                        <= cols
            });
    if row_nowrap || cells_abreast || grid_one_row {
        return live.any(|n| matches!(n, Node::Element(k) if class_a_box(k) && edge_break(k, last)));
    }
    let edge = if last { live.next_back() } else { live.next() };
    matches!(edge, Some(Node::Element(k)) if class_a_box(k) && edge_break(k, last))
}

/// Начальное и конечное значения 'page' коробки (css-page-3 §"Using named
/// pages", п. 1-2): `auto` берёт имя ближайшего предка; начальное — от
/// ПЕРВОЙ дочерней коробки, конечное — от ПОСЛЕДНЕЙ, рекурсивно, но
/// передаёт значение только коробка, к которой свойство применяется
/// (класс A); текст, строчный, флоат, абсолют — не передают, и тогда
/// берётся используемое значение самой коробки.
pub(crate) fn page_names(e: &Element, inherited: &str) -> (String, String) {
    let used = e.style.page.clone().unwrap_or_else(|| inherited.to_string());
    // Крайняя дочерняя коробка — крайняя ПОТОЧНАЯ: абсолют и флоат в
    // точках класса A не участвуют (Blink берёт имя первого уложенного
    // поточного ребёнка, `SetPageNameIfNeeded`; `page-name-propagated-005`:
    // абсолют последним ребёнком не возвращал имя самой коробки).
    let boxes: Vec<&Node> = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))
            || out_of_flow(&k.style) || k.style.float.unwrap_or(0) != 0))
        .collect();
    let via = |n: Option<&&Node>| match n {
        Some(Node::Element(k)) if !item_container(e) && class_a_box(k) => {
            Some(page_names(k, &used))
        }
        _ => None,
    };
    let start = via(boxes.first()).map(|p| p.0).unwrap_or_else(|| used.clone());
    let end = via(boxes.last()).map(|p| p.1).unwrap_or_else(|| used.clone());
    (start, end)
}

/// Объявления листа для марджин-боксов: контекст страницы (наследуемое
/// идёт в коробки, css-page-3 §page-properties) и коробки по именам.
pub type PageMarginDecls = (Vec<(String, String)>, Vec<(String, Vec<(String, String)>)>);
pub type PageMarginDeclsFn = std::rc::Rc<dyn Fn(usize, &str) -> PageMarginDecls>;

/// Используемое значение 'page' (css-page-3 §using-named-pages: `auto` —
/// значение ближайшего предка с не-`auto`) — в `style.page` каждого
/// элемента, чтобы мера фрагментации сравнивала имена на любой глубине.
pub(crate) fn fill_used_page(nodes: &mut [Node], inherited: &str) {
    for n in nodes.iter_mut() {
        if let Node::Element(e) = n {
            if e.style.page.is_none() && !inherited.is_empty() {
                e.style.page = Some(inherited.to_string());
            }
            let used = e.style.page.clone().unwrap_or_default();
            fill_used_page(&mut e.children, &used);
        }
    }
}

/// Есть ли внутри коробки смена имени страницы между соседями класса A
/// (css-page-3 §using-named-pages п. 4) — на любой глубине.
pub(crate) fn renames_inside(e: &Element) -> bool {
    let kids: Vec<&Element> = e
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(k) if class_a_box(k) => Some(k),
            _ => None,
        })
        .collect();
    (!item_container(e)
        && kids
            .windows(2)
            .any(|w| page_names(w[0], "").1 != page_names(w[1], "").0))
        || kids.iter().any(|k| renames_inside(k))
}

/// Есть ли внутри коробки принудительный разрыв МЕЖДУ соседями класса A
/// (css-break-4 §3.1 `break-before`/`break-after` не у крайнего ребёнка;
/// крайний передаёт разрыв самой коробке, `edge_break`) — на любой глубине
/// блочного потока. Мера коробки с текстом неизвестна (`shape_full` —
/// `None`), и разрыв внутри такого ребёнка стопки иначе терялся
/// (`page-name-propagated-002-print-ref`: `break-before: page` у второго
/// ребёнка обёртки).
pub(crate) fn breaks_inside(e: &Element) -> bool {
    // Только блочный поток: внутри таблицы разрыв режет ряды и группы
    // (`rowgroup-page-break-inside-avoid-5-print-ref`: `thead { break-after }`
    // — таблица не обёртка, снимать её нельзя).
    let table_part = matches!(
        e.tag.as_str(),
        "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "colgroup"
    );
    if table_part || table_box(e) || item_container(e) || forced_opaque(e) {
        return false;
    }
    let kids: Vec<&Element> = e
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(k) if class_a_box(k) => Some(k),
            _ => None,
        })
        .collect();
    let n = kids.len();
    kids.iter().enumerate().any(|(i, k)| {
        (i > 0 && k.style.break_before_force)
            || (i + 1 < n && k.style.break_after_force)
            || breaks_inside(k)
    })
}

/// Обёртка без собственной коробки на листе: блок без полей, рамок,
/// отбивок, фона, размеров, разрывов и прочего, что видно или влияет на
/// раскладку детей. Снятие такой обёртки раскладку не меняет.
pub(crate) fn plain_wrapper(e: &Element) -> bool {
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let st = &e.style;
    let b = st.borders();
    !e.inline
        && matches!(st.display, None | Some(Display::Block))
        && st.position.is_none()
        && st.float.unwrap_or(0) == 0
        && [&st.margin.top, &st.margin.right, &st.margin.bottom, &st.margin.left]
            .iter()
            .all(|l| zero(l))
        && [&st.padding.top, &st.padding.right, &st.padding.bottom, &st.padding.left]
            .iter()
            .all(|l| zero(l))
        && [&b.top, &b.right, &b.bottom, &b.left].iter().all(|l| zero(l))
        && st.background.is_none_or(|c| c.a == 0.0)
        && st.bg_image.is_none()
        && st.width.is_none()
        && st.height.is_none()
        && st.min_width.is_none()
        && st.min_height.is_none()
        && st.max_width.is_none()
        && st.max_height.is_none()
        && st.overflow_x.is_none()
        && st.overflow_y.is_none()
        && st.opacity.is_none()
        && st.transform.is_none()
        && st.filter.is_none()
        && st.outline.is_none()
        && st.column_count.is_none()
        && st.z_index.is_none()
        && st.vertical != Some(true)
        && !st.break_before_force
        && !st.break_after_force
        && e.children.iter().filter(|n| !is_blank(n)).all(|n| matches!(n, Node::Element(_)))
}

/// Снимает простые обёртки, внутри которых меняется имя страницы: их дети
/// становятся детьми стопки, и разрыв по смене имени (css-page-3
/// §using-named-pages п. 4) ставится между ними, как между детьми корня.
/// Мера фрагментации (`shape_full`) у коробок с текстом неизвестна, и
/// разрыв внутри такого ребёнка стопки иначе не ставится вовсе.
pub(crate) fn hoist_named_wrappers(nodes: &mut Vec<Node>) {
    loop {
        let mut changed = false;
        let mut out = Vec::with_capacity(nodes.len());
        for n in std::mem::take(nodes) {
            match n {
                Node::Element(e) if plain_wrapper(&e) && (renames_inside(&e) || breaks_inside(&e)) => {
                    changed = true;
                    out.extend(e.children);
                }
                n => out.push(n),
            }
        }
        *nodes = out;
        if !changed {
            break;
        }
    }
}

/// Имя ПЕРВОЙ страницы (css-page-3 §using-named-pages, п. 3): start value
/// первой поточной коробки класса A детей корня, иначе имя самого корня.
pub(crate) fn first_kid_page_name(nodes: &[Node], root_page: &str) -> String {
    for n in nodes.iter().filter(|n| !is_blank(n)) {
        match n {
            Node::Element(e) if matches!(e.style.display, Some(Display::None)) => continue,
            Node::Element(e) if class_a_box(e) => return page_names(e, root_page).0,
            Node::Element(e) if !e.inline => continue,
            _ => return root_page.to_string(),
        }
    }
    root_page.to_string()
}

/// Имя первой страницы документа — для стенда: геометрия первого листа даёт
/// начальный содержащий блок (css-page-3 §page-model).
pub fn first_page_name(nodes: &[Node]) -> String {
    let mut nodes: Vec<Node> = nodes.to_vec();
    let mut root_page = String::new();
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else { break };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        if let Some(p) = &e.style.page {
            root_page = p.clone();
        }
        nodes = (*e).clone().children;
    }
    first_kid_page_name(&nodes, &root_page)
}

thread_local! {
    /// Определения `<mask id>` / `<clipPath id>` документа: id — разметка
    /// содержимого. Ссылки `url(#id)` из `mask-image`/`clip-path` резолвятся
    /// при отрисовке (см. `interact::Grouped`).
    pub(crate) static MASK_DEFS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Чей документ собран: адрес среза узлов. Виртуализация рисует ПО
    /// БЛОКАМ (`render_block`) — сбор на каждый блок каждого кадра был бы
    /// расточительным, а документ между кадрами один и тот же.
    pub(crate) static MASK_DEFS_FOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Определения `<mask>` с `mask-type: alpha`: их снимок помечается, и
    /// `match-source` маскирует альфой, а не светимостью.
    pub(crate) static MASK_ALPHA_IDS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

thread_local! {
    /// Размер окна документа в css-точках — для единиц `vw`/`vh` там, куда
    /// `RenderOpts` не доходит (`grouped`: вершины `polygon()`). Ставится в
    /// `element()` рядом с `resolve_viewport` — тем же значением, каким
    /// разрешаются `width: 50vw` эталонов.
    pub(crate) static PAINT_VIEWPORT: std::cell::Cell<(f32, f32)> =
        const { std::cell::Cell::new((0.0, 0.0)) };
}

/// Содержимое определения маски по имени (`#id` без решётки).
pub(crate) fn mask_def(id: &str) -> Option<String> {
    MASK_DEFS.with(|m| m.borrow().get(id).cloned())
}

/// SVG `<filter>` для `backdrop-filter: url(#id)` → матрица 4×5 над
/// НЕумноженным RGBA (строки R, G, B, A: четыре множителя и сдвиг — как
/// `Filter::color_matrix`). Только ОДИН примитив с аффинной формулой:
/// `feColorMatrix type="matrix"` (20 чисел; filter-effects-1 Overview.bs:950)
/// или `feComponentTransfer` с `identity`/`linear`/`table` из двух значений
/// (Overview.bs:1159-1168: C' = v0 + C·(v1 − v0); пустой список — тождество,
/// Overview.bs:1197). И только при `color-interpolation-filters="sRGB"`:
/// начальное `linearRGB` (Overview.bs:614) делает формулу нелинейной в sRGB
/// кадра. Остальное — None: подложка не рисуется, как прежде.
pub(crate) fn svg_filter_matrix(def: &str) -> Option<[f32; 20]> {
    fn attr(tag: &str, name: &str) -> Option<String> {
        let head = &tag[..tag.find('>')?];
        let key = format!(" {name}=\"");
        let at = head.find(&key)? + key.len();
        let rest = &head[at..];
        Some(rest[..rest.find('"')?].trim().to_string())
    }
    fn nums(s: &str) -> Option<Vec<f32>> {
        s.split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f32>().ok())
            .collect()
    }
    // Разметка `svg::write_element`: атрибуты ` имя="значение"`; регистр
    // имён тегов и атрибутов сводится к нижнему.
    let d = def.to_ascii_lowercase();
    if !d.contains("color-interpolation-filters=\"srgb\"") {
        return None;
    }
    let prims: Vec<&str> = d
        .match_indices("<fe")
        .map(|(i, _)| &d[i..])
        .filter(|s| !s.starts_with("<fefunc"))
        .collect();
    let [p] = prims.as_slice() else {
        return None;
    };
    let p: &str = p;
    let mut m = [
        1.0f32, 0.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    if p.starts_with("<fecolormatrix") {
        if attr(p, "type").is_some_and(|t| t != "matrix") {
            return None;
        }
        let v = nums(&attr(p, "values")?)?;
        if v.len() != 20 {
            return None;
        }
        m.copy_from_slice(&v);
    } else if p.starts_with("<fecomponenttransfer") {
        let body = &p[..p.find("</fecomponenttransfer").unwrap_or(p.len())];
        for (row, ch) in ['r', 'g', 'b', 'a'].into_iter().enumerate() {
            let Some(at) = body.find(&format!("<fefunc{ch}")) else {
                continue;
            };
            let f = &body[at..];
            let (slope, intercept) = match attr(f, "type").as_deref() {
                Some("identity") => continue,
                Some("table") => {
                    let v = nums(attr(f, "tablevalues").as_deref().unwrap_or(""))?;
                    match v.as_slice() {
                        [] => continue,
                        [v0, v1] => (v1 - v0, *v0),
                        _ => return None,
                    }
                }
                Some("linear") => (
                    attr(f, "slope")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(1.0),
                    attr(f, "intercept")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0),
                ),
                _ => return None,
            };
            m[row * 5 + row] = slope;
            m[row * 5 + 4] = intercept;
        }
    } else {
        return None;
    }
    Some(m)
}

thread_local! {
    /// Снимки определений на момент СБОРКИ дерева: отрисовка идёт позже, а
    /// документов в кадре может быть два (тест и эталон стенда) — реестр
    /// определений к моменту отрисовки уже перезаписан другим документом.
    /// Снимки копятся под уникальными ключами и не чистятся.
    pub(crate) static MASK_SNAPS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    pub(crate) static MASK_SNAP_N: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Снять снимок определения; ключ живёт до конца кадра и дольше.
pub(crate) fn snapshot_mask_def(id: &str) -> Option<String> {
    let markup = mask_def(id)?;
    let alpha = MASK_ALPHA_IDS.with(|s| s.borrow().contains(id));
    let key = MASK_SNAP_N.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        // Хвост `A` — `mask-type: alpha` определения (см. `interact`).
        if alpha { format!("k{n}A") } else { format!("k{n}") }
    });
    MASK_SNAPS.with(|m| {
        let mut map = m.borrow_mut();
        // Кадры идут бесконечно — тысяча снимков означает утечку, чистим.
        if map.len() > 1000 {
            map.clear();
        }
        map.insert(key.clone(), markup);
    });
    Some(key)
}

/// Разметка по ключу снимка (для отрисовки).
pub(crate) fn mask_snapshot(key: &str) -> Option<String> {
    MASK_SNAPS.with(|m| m.borrow().get(key).cloned())
}

/// Заменить ссылки `url(#id)` / `clipref:id` в строке маски снимками
/// определений: к отрисовке реестр может смениться другим документом.
pub(crate) fn resolve_mask_refs(raw: &str) -> String {
    if let Some(id) = raw.strip_prefix("clipref:") {
        return match snapshot_mask_def(id) {
            Some(key) => format!("clipsnap:{key}"),
            None => raw.to_string(),
        };
    }
    // Ссылка на определение в документе — `url(#id)` в любом виде записи:
    // и в кавычках (`url("#id")`, `url('#id')`). Прежде узнавалась только
    // голая форма, и маска в кавычках не применялась вовсе
    // (`mask-mode-to-mask-type`: все шесть квадратов сплошные).
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find("url(") {
        let tail = &rest[at + 4..];
        let Some(end) = tail.find(')') else {
            break;
        };
        let inner = tail[..end].trim().trim_matches(|c| c == '"' || c == '\'');
        out.push_str(&rest[..at]);
        match inner.strip_prefix('#').and_then(snapshot_mask_def) {
            Some(key) => out.push_str(&format!("url(svgsnap:{key})")),
            None => out.push_str(&rest[at..at + 4 + end + 1]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Собрать определения масок ДО отрисовки: ссылка может стоять раньше
/// определения по тексту.
pub(crate) fn collect_mask_defs(nodes: &[Node]) {
    let key = nodes.as_ptr() as usize;
    if MASK_DEFS_FOR.with(|c| c.get()) == key {
        return;
    }
    MASK_DEFS_FOR.with(|c| c.set(key));
    fn walk(nodes: &[Node], out: &mut std::collections::HashMap<String, String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            let tag = e.tag.to_ascii_lowercase();
            if tag == "mask"
                && e.style.mask_type_alpha == Some(true)
                && let Some(id) = e.attr("id")
            {
                MASK_ALPHA_IDS.with(|s| s.borrow_mut().insert(id.to_string()));
            }
            if (tag == "mask" || tag == "clippath")
                && let Some(id) = e.attr("id")
            {
                let mut markup = String::new();
                for c in &e.children {
                    if let Node::Element(el) = c {
                        crate::svg::write_element(el, &mut markup);
                    }
                }
                out.insert(id.to_string(), markup);
            }
            // `<filter id>` — целиком, с атрибутами области (x/y/width/height,
            // filterUnits): ключ с префиксом, чтобы не спутать с маской.
            if tag == "filter" && let Some(id) = e.attr("id") {
                let mut markup = String::new();
                crate::svg::write_element(e, &mut markup);
                out.insert(format!("filter:{id}"), markup);
            }
            walk(&e.children, out);
        }
    }
    MASK_ALPHA_IDS.with(|s| s.borrow_mut().clear());
    MASK_DEFS.with(|m| {
        let mut map = m.borrow_mut();
        map.clear();
        walk(nodes, &mut map);
    });
}

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let node = nodes.get(index)?;
    crate::interact::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let root = opts.root_style();
    // Слой ICB закрывается на блок ленты: дальше своего блока внепоточный
    // элемент всё равно не уедет, а без слоя он остался бы на месте.
    crate::interact::icb_open();
    let avail_prev = AVAIL_W.replace(Some(opts.viewport.0).filter(|w| *w > 0.0));
    let out = blocks(std::slice::from_ref(node), &root, opts);
    AVAIL_W.set(avail_prev);
    let layer = crate::interact::icb_close();
    let first = out.into_iter().next()?;
    if layer.is_empty() {
        return Some(first);
    }
    // Лента отдаёт РОВНО ОДИН элемент на блок, поэтому слой уходит внутрь
    // обёртки. Содержащим блоком становится она, а не окно: в ленте окна
    // всё равно нет — блок живёт в прокрутке. Без обёртки вынесенный
    // элемент просто пропадал бы с экрана.
    Some(
        div()
            .relative()
            .child(first)
            .children(layer)
            .into_any_element(),
    )
}

/// Разбор списка детей на блоки: инлайн-подряд склеивается в абзац.
/// Абзац с пробой бюджета строк: если строится внутри clamp-контейнера,
/// рядом с абзацем едет проба его границ и высоты строки.
/// Строчное содержимое блочного контейнера рисуется на шаге 7 приложения E
/// CSS 2.1 — после фонов и рамок ВСЕХ блоков потока своего контекста
/// наложения (шаг 4) и флоатов (шаг 5), в порядке дерева, но до
/// позиционированных (шаг 8). Обёртка раскладку не меняет: при открытом
/// собирателе краски (`gpui::PaintCollect`) абзац уходит в него, иначе
/// рисуется на месте (`gpui::PaintInline`).
pub(crate) fn paint_inline_step7(para: AnyElement) -> AnyElement {
    gpui::PaintInline::new(para).into_any_element()
}

pub(crate) fn paragraph_probed(taken: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Знак обрыва АВТО-режима: бюджет строк ИМЕННО ЭТОГО абзаца посчитал
    // `ClampCut` прошлого кадра. Кладём его ДО сборки абзаца — многоточие
    // нарисует строчный слой (`lines::clamp_lines` → `paint_line`), тот
    // самый, что уже зелен на `webkit-line-clamp-014` (bidi) 0.00,
    // `block-ellipsis-bidi-001/002` 0.00, `block-ellipsis-028/031` 0.19.
    // Номер абзаца выдаётся в порядке ПОСТРОЕНИЯ и уезжает в пробу,
    // поэтому сопоставление кадров не зависит от порядка обхода на
    // отрисовке.
    let ctx = crate::interact::clamp_context();
    let seq = ctx.map(|(key, _)| crate::interact::clamp_next_seq(key));
    let budget = match (ctx, seq) {
        (Some((key, _)), Some(s)) => match crate::interact::clamp_para(key) {
            Some((ps, k)) if ps == s => Some(k),
            _ => None,
        },
        _ => None,
    };
    crate::interact::set_para_budget(budget);
    crate::interact::set_para_tag(ctx.zip(seq).map(|((key, _), s)| (key, s)));
    let para = paragraph(taken, inherited, opts);
    crate::interact::set_para_tag(None);
    // Ячейку обязательно опустошить и когда абзац её не забрал
    // (вертикальное письмо уходит из `paragraph` раньше): иначе бюджет
    // достался бы СЛЕДУЮЩЕМУ абзацу.
    crate::interact::set_para_budget(None);
    let para = with_text_shadow(para, inherited, taken, opts);
    if let Some((key, skip)) = ctx {
        div()
            .relative()
            .child(para)
            .child(crate::interact::clamp_probe(
                crate::interact::clamp_lines_for(key),
                line_height_px(inherited, opts),
                skip,
                false,
                0.0,
                seq,
                budget,
            ))
            .into_any_element()
    } else {
        para
    }
}

thread_local! {
    /// Ширина содержащего блока в точках, когда её видно из стиля родителя.
    /// Нужна замещаемому элементу БЕЗ собственного размера, но С соотношением:
    /// §10.3.2 (последний пункт) берёт его ширину из уравнения для блочных
    /// коробок, то есть из содержащего блока, а резерв 300×150 применяется
    /// только когда ширину взять неоткуда.
    pub(crate) static CB_WIDTH: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Вернуть прежнюю ширину содержащего блока по выходе из `blocks()`.
pub(crate) struct CbWidthGuard(pub(crate) Option<f32>);
impl Drop for CbWidthGuard {
    fn drop(&mut self) {
        CB_WIDTH.set(self.0);
    }
}
pub(crate) fn scopeguard_cb(prev: Option<f32>) -> CbWidthGuard {
    CbWidthGuard(prev)
}

thread_local! {
    /// Ширина, которую получит БЛОЧНЫЙ ребёнок с `width: auto` в текущем
    /// `blocks()`: stretch-fit содержащего блока (CSS 2.1 §10.3.3, css-sizing-3
    /// «definite»: inline-размер block-level коробки в потоке ОПРЕДЕЛЁН, даже когда
    /// `width` не задан). Нужна счёту `repeat(auto-fill | auto-fit, …)` в лунках
    /// (css-grid-1 §7.2.3.2): прежде у `width: auto` размер считался неизвестным, и
    /// повтор не разворачивался вовсе (`column-auto-repeat-016`: три лунки вместо
    /// семи по 100 в 784). `None` — ширину честно взять неоткуда (флекс, сетка,
    /// таблица, строчный блок, флоат, абсолют, многоколоночник, вертикальное письмо).
    pub(crate) static AVAIL_W: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Доступная ширина текущего уровня `blocks()` (см. `AVAIL_W`) — для долей
/// у строчных коробок абзаца: их содержащий блок — блок абзаца.
pub(crate) fn avail_width() -> Option<f32> {
    AVAIL_W.get()
}

/// Вернуть прежнюю доступную ширину по выходе из `blocks()`.
pub(crate) struct AvailWGuard(pub(crate) Option<f32>);
impl Drop for AvailWGuard {
    fn drop(&mut self) {
        AVAIL_W.set(self.0);
    }
}


// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): разворачивать `text-emphasis` в поштучные
// руби (по знаку-аннотации над каждой буквой базы, кроме пробелов и
// пунктуации — css-text-decor-3 §5.3). Срез руби и акцентов, 167 пар:
// 125 -> 125, приобретено 6 (`text-emphasis-line-height-001a/002a/002b`,
// `-position-over-left-002`, `-position-under-left-002`, `-punctuation-3`),
// потеряно 6 — `-line-height-004a..d` 0.07 -> 0.7 и `-punctuation-1/2`
// 0.00 -> 6.31/3.12: поштучный атом меняет разбивку строки и подъём базовой
// линии, а эталоны семьи считают её по-своему. Возвращать вместе с
// настоящей надстрочной аннотацией (сдвиг базовой линии без атома).
pub(crate) fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    // Only the cell's own content list is the BFC root's (see `CELL_BFC`).
    let cell_bfc = CELL_BFC.with(|c| c.replace(false));
    let cb_prev = CB_WIDTH.get();
    if let Some(Len::Px(w)) = inherited.width
        && w > 0.0
    {
        CB_WIDTH.set(Some(w));
    }
    let _cb_guard = scopeguard_cb(cb_prev);
    // Доступная ширина блочных детей этого уровня (см. `AVAIL_W`).
    let avail_prev = AVAIL_W.get();
    AVAIL_W.set(available_width::inner(inherited, avail_prev));
    let _avail_guard = AvailWGuard(avail_prev);
    // `content-visibility: hidden`: содержимое пропускается целиком
    // (css-contain-2 §4) — коробка остаётся, детей нет.
    let stripped: Vec<Node>;
    let nodes = if nodes.iter().any(
        |n| matches!(n, Node::Element(e) if e.style.skip_content == Some(true) && !e.children.is_empty()),
    ) {
        stripped = nodes
            .iter()
            .map(|n| match n {
                Node::Element(e) if e.style.skip_content == Some(true) => {
                    let mut copy = e.clone();
                    copy.children.clear();
                    Node::Element(copy)
                }
                other => other.clone(),
            })
            .collect();
        &stripped
    } else {
        nodes
    };
    // `order` в CSS работает ТОЛЬКО внутри гибкого контейнера и сетки; в
    // обычном потоке он не значит ничего. Раньше сортировались дети любого
    // родителя — блоки меняли порядок там, где браузер их не трогает.
    // Барьер `fixed` — не только СВОЙ трансформ родителя, но и его
    // `contain: layout|paint` (css-contain-2 §3.2 п.5, §3.3: «establishes …
    // a fixed positioning containing block»). `transform_ancestor` родителя
    // несёт лишь ЕГО предков (`inline::inherit`), поэтому прямой ребёнок
    // обособленной коробки уходил в слой окна и садился в угол экрана
    // (`contain-layout-007`, `contain-paint-010`), а внук — нет
    // (`contain-*-containing-block-fixed-001` = 0.00). Список совпадает с
    // `fixed_cb_box` — расхождение он прямо запрещает.
    // Обещанное свойство, дающее блок для `fixed` (css-will-change-1 §2.1), —
    // тот же барьер, что `transform` самого родителя (`will-change-fixpos-cb-*`).
    let under_tf = inherited.transform_ancestor
        || inherited.transform.is_some()
        || inherited.contain_layout == Some(true)
        || inherited.contain_paint == Some(true)
        || inherited.will_change & crate::computed::wc::CB_FIXED != 0;
    let ordered_context = matches!(
        inherited.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            // Поток лунок — сеточный контекст: схлопывания отступов нет
            // (css-grid-3), и `order` действует.
            | Some(Display::GridLanes)
    );
    // Схлопывание вертикальных отступов есть ТОЛЬКО в обычном потоке: в
    // гибком контейнере и сетке CSS его запрещает, а мы схлопывали везде —
    // элементы ряда съезжали друг к другу против браузера.
    // Блок внутри строчного разрывает его на анонимные коробки (CSS 2.1
    // §9.2.1.1) — разбиение идёт ДО схлопывания полей: вынесенный блок
    // обязан схлопнуть свои поля с новыми соседями. В гибком контейнере и
    // сетке разрыва нет вовсе: там дети блокифицируются, и куски уехали бы
    // по чужим дорожкам.
    let split = if ordered_context {
        nodes.to_vec()
    } else {
        // Анонимная таблица вокруг ПРОГОНА табличных братьев (§17.2.1 шаг 3)
        // — до разбиения блока в строчном и до схлопывания полей, как это
        // делает и сборщик дерева в браузере.
        split_block_in_inline(&hoist_inset_abs(&wrap_anon_tables(nodes)))
    };
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v158, `scout-flex-2026-09e.md` патч №1):
    // снятие АВТОРСКОГО `align-self` у блока в обычном потоке здесь, в начале
    // `blocks()` — «до внутренних постановщиков». Полный свод против v35:
    // +3 (`align-self-013`, `flexbox-align-self-vert-001`, `-horiz-002`) /
    // −5 (`absolute-replaced-width-020` 0.00 → 3.84, `left-offset-003`
    // 0.00 → 0.96, `left-offset-percentage-001` 0.00 → 1.05,
    // `anchor-position-inline-005/-006`). Первая тройка — ровно та, что
    // названа в записи `inline.rs:1001`: место в конвейере не спасло,
    // замещаемому абсолюту `align_self` нужен ещё ДО `blocks()`. Возвращать
    // только с явным признаком «значение авторское» в `Computed`.
    // §10.3.3: у блока в потоке с `width: auto` боковое `auto`-поле
    // используется НУЛЁМ, а коробка занимает всю ширину. У нас блок — гибкая
    // колонка, и любое auto-поле на поперечной оси отменяет растяжение до
    // дорожки: абзац сжимался по содержимому и уезжал к краю.
    let split = if ordered_context {
        split
    } else {
        split
            .into_iter()
            .map(|n| match n {
                // `justify-self` блока в потоке (css-align-3 §6.1 «Block-Level
                // Boxes»): не-`normal`/`stretch` значение меряет коробку с
                // `width: auto` по содержимому (fit-content — обёртка
                // `content_sized`), auto-поля имеют приоритет над
                // выравниванием; без auto-полей выравнивание выражается ими же
                // (флекс-колонка блока их исполняет). `left`/`right` разбор
                // уже свёл к `start`/`end`; сторона — по письму родителя
                // (`justify-self-auto-margins-2`: `margin: auto` центрирует
                // 100 в 200). Таблица и замещаемый размер по содержимому уже
                // имеют — им только поля.
                Node::Element(mut e)
                    if in_flow(&e.style)
                        && !inline_level_box(&e)
                        && inherited.vertical != Some(true)
                        && matches!(
                            e.style.justify_self,
                            Some(Align::Start) | Some(Align::Center) | Some(Align::End)
                        ) =>
                {
                    let auto = |l: Option<Len>| l == Some(Len::Auto);
                    let table = e.tag == "table" || e.style.display == Some(Display::Table);
                    if matches!(e.style.width, None | Some(Len::Auto)) && !table && !replaced_tag(&e) {
                        e.style.width = Some(Len::FitContent);
                    }
                    if !auto(e.style.margin.left) && !auto(e.style.margin.right) {
                        let rtl = inherited.rtl == Some(true);
                        match (e.style.justify_self, rtl) {
                            (Some(Align::Center), _) => {
                                e.style.margin.left = Some(Len::Auto);
                                e.style.margin.right = Some(Len::Auto);
                            }
                            (Some(Align::End), false) | (Some(Align::Start), true) => {
                                e.style.margin.left = Some(Len::Auto);
                            }
                            _ => e.style.margin.right = Some(Len::Auto),
                        }
                    }
                    Node::Element(e)
                }
                Node::Element(mut e)
                    if in_flow(&e.style)
                        && matches!(e.style.width, None | Some(Len::Auto))
                        && (e.style.margin.left == Some(Len::Auto)
                            || e.style.margin.right == Some(Len::Auto)) =>
                {
                    if e.style.margin.left == Some(Len::Auto) {
                        e.style.margin.left = Some(Len::Px(0.0));
                    }
                    if e.style.margin.right == Some(Len::Auto) {
                        e.style.margin.right = Some(Len::Px(0.0));
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    let nodes: &[Node] = &split;
    let collapsed = if ordered_context {
        reorder(nodes.to_vec())
    } else {
        // Схлопывание идёт ДО наследования стилей, поэтому кегль уровня
        // передаётся отдельно: `margin: 1em` без своего `font-size` меряется
        // от родительского.
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let prev = COLLAPSE_FONT_PX.with(|c| c.replace(base));
        // Ширина содержащего блока для ПРОЦЕНТНЫХ полей (§8.3: проценты полей
        // считаются от ширины содержащего блока, схлопывание — по уже
        // разрешённым значениям). Известна только заданная в точках.
        let cb_w = match inherited.width {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let prev_w = COLLAPSE_CB_WIDTH_PX.with(|c| c.replace(cb_w));
        // Определённость высоты блока для ДОЛЕЙ высоты детей — тем же
        // предикатом, что у слитого стиля (`inline::inherit`): он зависит
        // только от родителя, а сырой стиль ребёнка признака ещё не несёт.
        let cb_h_def = inline::inherit(inherited, &Computed::default()).cb_height_def;
        let prev_h = COLLAPSE_CB_HEIGHT_DEF.with(|c| c.replace(cb_h_def));
        let mut out = collapse_margins(
            nodes,
            matches!(
                inherited.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ),
        );
        margin_height::zero_float_blocks(&mut out, inherited);
        COLLAPSE_FONT_PX.with(|c| c.set(prev));
        COLLAPSE_CB_WIDTH_PX.with(|c| c.set(prev_w));
        COLLAPSE_CB_HEIGHT_DEF.with(|c| c.set(prev_h));
        out
    };
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed: Vec<Node> = if ordered_context {
        collapsed
            .into_iter()
            // `visibility: collapse` на элементе гибкого контейнера убирает
            // его из строки, НО оставляет РАСПОРКУ (strut, css-flexbox §4.4):
            // поперечный размер и базовая линия ряда меряются как при нём
            // (flexbox-collapsed-item-baseline-001). Распорка — тот же
            // элемент с нулевой ГЛАВНОЙ осью и невидимой краской.
            .map(|n| match n {
                Node::Element(mut e) if e.style.collapsed == Some(true) => {
                    match inherited.flex_dir {
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                            e.style.height = Some(Len::Px(0.0));
                            e.style.max_height = Some(Len::Px(0.0));
                            e.style.min_height = Some(Len::Px(0.0));
                            e.style.margin.top = Some(Len::Px(0.0));
                            e.style.margin.bottom = Some(Len::Px(0.0));
                            e.style.padding.top = Some(Len::Px(0.0));
                            e.style.padding.bottom = Some(Len::Px(0.0));
                            e.style.border_width.top = Some(Len::Px(0.0));
                            e.style.border_width.bottom = Some(Len::Px(0.0));
                        }
                        // Распорка в главной оси — ноль ЦЕЛИКОМ: элемент «as
                        // if display:none» (css-flexbox-1 §4.4), значит и его
                        // поля, отбивки и рамки по главной оси соседей не
                        // раздвигают (`flexbox_visibility-collapse`: между
                        // соседями только их собственные поля).
                        _ => {
                            e.style.width = Some(Len::Px(0.0));
                            e.style.max_width = Some(Len::Px(0.0));
                            e.style.min_width = Some(Len::Px(0.0));
                            e.style.margin.left = Some(Len::Px(0.0));
                            e.style.margin.right = Some(Len::Px(0.0));
                            e.style.padding.left = Some(Len::Px(0.0));
                            e.style.padding.right = Some(Len::Px(0.0));
                            e.style.border_width.left = Some(Len::Px(0.0));
                            e.style.border_width.right = Some(Len::Px(0.0));
                        }
                    }
                    e.style.hidden = Some(true);
                    Node::Element(e)
                }
                other => other,
            })
            // ПРОБЕЛЬНЫЙ текст между детьми ряда/сетки не рождает анонимный
            // элемент (css-flexbox §4): переводы строк разметки давали
            // лишние 2-3px между коробками
            // (flexbox-baseline-align-self-baseline-horiz-001: тест дышит
            // щелями, эталон написан слитно).
            .filter(|n| !matches!(n, Node::Text(_)) || !is_blank(n))
            .map(|n| match n {
                Node::Element(mut e) => {
                    e.style.float = None;
                    e.style.clear = None;
                    e.style.vertical_align = None;
                    e.style.flex_item = matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    );
                    // Элемент КОЛОНКИ: определён ли главный размер
                    // контейнера (css-flexbox-1 §9.8 п.1). От этого зависит,
                    // определён ли блок у ЕГО детей — доля высоты внутри
                    // элемента колонки без высоты решается как `auto`
                    // (Blink `flex_layout_algorithm.cc`:
                    // `is_initial_block_size_indefinite`).
                    if matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    ) && matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    ) {
                        // Абсолют с ОБОИМИ вертикальными отступами и авто-высотой
                        // тоже определён: высота выходит из уравнения
                        // css-position-3 §4.1 (`top + height + bottom` = блок
                        // содержащего, а он у абсолюта всегда определён,
                        // css-sizing-3 §4.1 «definite»). Без этого колонка
                        // `position: absolute; top: 0; bottom: 0` считалась
                        // неопределённой, и основа-доля ребёнка снималась
                        // (`percentage-heights-002`: синяя полоса по содержимому,
                        // красный фон контейнера под ней).
                        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
                        let abs_both_insets = matches!(
                            inherited.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        ) && matches!(inherited.height, None | Some(Len::Auto))
                            && edge(inherited.inset.top)
                            && edge(inherited.inset.bottom);
                        let definite = matches!(inherited.height, Some(Len::Px(_)))
                            || (matches!(inherited.height, Some(Len::Pct(_)))
                                && inherited.cb_height_def)
                            || abs_both_insets
                            || inherited.stretched
                            || inherited.root_box;
                        e.style.flex_main_def = Some(definite);
                    }
                    // Доля высоты элемента РЯДА при неопределённой высоте
                    // контейнера ведёт себя как `auto` (CSS 2.1 §10.5;
                    // css-flexbox-1 §9.8: определённой поперечную ось делает
                    // лишь определённый размер контейнера), но вычисленное
                    // значение — не `auto`, поэтому `stretch` к ней не
                    // применяется и работает как `flex-start` (§9.4 п.11,
                    // css-align-3 §6.1). Прежде доля решалась от высоты
                    // строки (`stretch-requires-computed-auto-size`: красная
                    // коробка в полвысоты соседа).
                    if matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    ) && matches!(
                        inherited.flex_dir,
                        None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
                    ) && inherited.vertical != Some(true)
                        && e.style.vertical != Some(true)
                        && matches!(e.style.height, Some(Len::Pct(_)))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
                        let cross_definite = matches!(inherited.height, Some(Len::Px(_)))
                            || (matches!(inherited.height, Some(Len::Pct(_)))
                                && inherited.cb_height_def)
                            || (matches!(
                                inherited.position,
                                Some(crate::computed::Position::Absolute)
                                    | Some(crate::computed::Position::Fixed)
                            ) && edge(inherited.inset.top)
                                && edge(inherited.inset.bottom))
                            || inherited.stretched
                            || inherited.root_box
                            || inherited.aspect_ratio.is_some();
                        if !cross_definite {
                            e.style.height = None;
                            let stretch = match e.style.align_self {
                                Some(a) => a == crate::computed::Align::Stretch,
                                None => matches!(
                                    inherited.align_items,
                                    None | Some(crate::computed::Align::Stretch)
                                ),
                            };
                            if stretch {
                                e.style.align_self = Some(crate::computed::Align::Start);
                            }
                        }
                    }
                    // ★ Эти три правила жили в ветке ОБЫЧНОГО потока (`else`
                    // ниже) с проверками на Flex/Grid-родителя — и были
                    // недостижимы по построению (скаут flexbox: пробы
                    // `flex2-colbasis-*` показали, что компенсация основы не
                    // действует). Их место — здесь, среди детей ряда/сетки.
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    // `<canvas>` в сетке: атрибуты `width/height` — природный
                    // размер и соотношение сторон, а не CSS-размер (HTML
                    // §4.12.5, §15.3.10). Ось, растянутая выравниванием
                    // (`stretch`, css-align-3 §6.1) или переносимая из
                    // заданной автором другой оси (css-sizing-4 «transferred
                    // size»), становится `auto`, соотношение — на коробку
                    // (`replaced-element-011`, `grid-item-inline-contribution-*`,
                    // `replaced-alignment-with-aspect-ratio-001`).
                    if e.tag == "canvas"
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && (e.style.attr_sized.0 || e.style.attr_sized.1)
                    {
                        let stretch = |own: Option<Align>, items: Option<Align>| {
                            own == Some(Align::Stretch)
                                || (own.is_none() && items == Some(Align::Stretch))
                        };
                        let sx = stretch(e.style.justify_self, inherited.justify_items);
                        let sy = stretch(e.style.align_self, inherited.align_items);
                        // Обе оси пришли из атрибутов, а явный `stretch` — ровно
                        // у одной: вторая ось тоже `auto` и выводится из
                        // растянутой через соотношение (css-sizing-4 «transferred
                        // size»; Blink `length_utils.cc` — `kStretchExplicit` по
                        // блочной оси включает соотношение для строчной). Раньше
                        // она оставалась атрибутом, и taffy выводил из неё
                        // растянутую: 10×10 вместо 100×100
                        // (`replaced-alignment-with-aspect-ratio-001/002`).
                        // Вертикальную сетку не трогаем: оси там переставлены.
                        let both_attrs = e.style.attr_sized.0 && e.style.attr_sized.1;
                        let transfer = both_attrs && sx != sy && inherited.vertical != Some(true);
                        let free_x = e.style.attr_sized.0
                            && (sx || !e.style.attr_sized.1 || transfer);
                        let free_y = e.style.attr_sized.1
                            && (sy || !e.style.attr_sized.0 || transfer);
                        if free_x || free_y {
                            // Явный `stretch` по ОБЕИМ осям задаёт обе стороны
                            // растяжением — соотношение не действует
                            // (`-003.tentative`: 10×20 в области 100×100 давал
                            // 100×200; то же правило — `grid-aspect-ratio-032/033`).
                            if let (Some(Len::Px(w)), Some(Len::Px(h))) =
                                (e.style.attr_width, e.style.attr_height)
                                && h > 0.0
                                && e.style.aspect_ratio.is_none()
                                && !(both_attrs && sx && sy)
                            {
                                e.style.aspect_ratio = Some(w / h);
                            }
                            // Нерастянутая ось при `normal` у коробки с
                            // соотношением — `start` (css-grid-2 §6.6.1), иначе
                            // taffy растянет её сам (`alignment.rs:122-128`: без
                            // заданной ширины умолчание — `Stretch`) и выведет
                            // растянутую из неё. Авторское значение не трогаем.
                            if transfer {
                                if sy
                                    && e.style.justify_self.is_none()
                                    && inherited.justify_items.is_none()
                                {
                                    e.style.justify_self = Some(Align::Start);
                                }
                                if sx
                                    && e.style.align_self.is_none()
                                    && inherited.align_items.is_none()
                                {
                                    e.style.align_self = Some(Align::Start);
                                }
                            }
                            if free_x {
                                e.style.width = None;
                            }
                            if free_y {
                                e.style.height = None;
                            }
                        }
                    }
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    e.style.flex_item_ratio = ratio_ok
                        && !positioned_out
                        && matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex));
                    // `flex-basis` задаёт размер СОДЕРЖИМОГО (css-flexbox-1 §7.2.3:
                    // «flex-basis determines the size of the content box, unless
                    // otherwise specified such as by box-sizing»), а в раскладку
                    // уходит внешний размер — как `width`/`height` в `apply`, основа
                    // получает отбивку и рамку по ГЛАВНОЙ оси родителя
                    // (`flexbox-mbp-horiz-*`, `flexbox-justify-content-horiz-002`).
                    if matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex))
                        && inherited.vertical.is_none()
                        && e.style.border_box != Some(true)
                        && let Some(Len::Px(b)) = e.style.flex_basis
                    {
                        let px_of = |l: Option<Len>| match l {
                            Some(Len::Px(v)) => v,
                            _ => 0.0,
                        };
                        let bd = e.style.borders();
                        let row = matches!(
                            inherited.flex_dir,
                            None
                                | Some(crate::computed::FlexDir::Row)
                                | Some(crate::computed::FlexDir::RowReverse)
                        );
                        let extra = if row {
                            px_of(e.style.padding.left)
                                + px_of(e.style.padding.right)
                                + px_of(bd.left)
                                + px_of(bd.right)
                        } else {
                            px_of(e.style.padding.top)
                                + px_of(e.style.padding.bottom)
                                + px_of(bd.top)
                                + px_of(bd.bottom)
                        };
                        e.style.flex_basis = Some(Len::Px(b + extra));
                    }
                    // Элемент сетки с `aspect-ratio` при `normal` (css-grid-2
                    // §6.6.1): «sized consistent with the size calculation
                    // rules for block-level elements» — строчная ось заполняет
                    // область (как stretch), а БЛОЧНАЯ идёт из соотношения, не
                    // растягиваясь на ряд: там `start`
                    // (`grid-aspect-ratio-001/007/010/038`). ★ ЗАМЕРЕНО: `start`
                    // и по строчной оси — `grid-aspect-ratio-018/038` в красное.
                    if ratio_ok
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && inherited.vertical.is_none()
                    {
                        let auto_w = matches!(e.style.width, None | Some(Len::Auto));
                        let auto_h = matches!(e.style.height, None | Some(Len::Auto));
                        // Обе оси auto: строчная заполняет область, блочная — из
                        // соотношения. Блочная определена: строчная — из
                        // соотношения (CSS2 §10.3.2 для замещаемого с
                        // соотношением; css-sizing-4 §5.1).
                        if auto_w
                            && !auto_h
                            && e.style.justify_self.is_none()
                            && inherited.justify_items != Some(Align::Stretch)
                        {
                            e.style.justify_self = Some(Align::Start);
                        }
                        if auto_h
                            && e.style.align_self.is_none()
                            && inherited.align_items != Some(Align::Stretch)
                        {
                            e.style.align_self = Some(Align::Start);
                        }
                    }
                    // `flex-basis: content` — основа по содержимому, и
                    // заданный ГЛАВНЫЙ размер при ней не действует. Какая ось
                    // главная, знает только родитель, поэтому размер снимается
                    // здесь, а не в стиле самого элемента.
                    // У ЗАМЕЩАЕМОГО элемента содержимое — он сам, и его
                    // размер задаёт собственный пиксель или атрибут: снимать
                    // его нельзя, иначе `<canvas width=20>` схлопывается в
                    // ноль (`flexbox-flex-basis-content-001a`).
                    let own = matches!(
                        e.tag.as_str(),
                        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "svg"
                    );
                    if e.style.basis_content == Some(true) {
                        match inherited.flex_dir {
                            Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                                e.style.height = own.then_some(e.style.attr_height).flatten();
                            }
                            _ => e.style.width = own.then_some(e.style.attr_width).flatten(),
                        }
                    }
                    // Основа-ДОЛЯ у элемента колонки, чей контейнер не определён
                    // по главной оси, — это `content` (css-flexbox-1 §7.2.3: «if
                    // that containing block's size is indefinite, the used value
                    // for flex-basis is content»; Blink `IsItemFlexBasisDefinite`).
                    // Taffy не решает долю и падает на заданную высоту
                    // (`flexbox.rs` `flex_basis.or(main_size)`): `flex: 0 0 0%;
                    // height: 500px` давал 500 вместо содержимого 100
                    // (`flex-basis-010`), а `flex: 1 1; height: 100px` делал
                    // блок детей определённым, и `height: 100%` ребёнка
                    // закрашивал красное (`percentage-heights-017/018`).
                    if e.style.flex_main_def == Some(false)
                        && matches!(e.style.flex_basis, Some(Len::Pct(_)))
                    {
                        e.style.flex_basis = None;
                        e.style.height = own.then_some(e.style.attr_height).flatten();
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
    };
    let flex_ctx = matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex));
    let mut letter_scope = first_letter_scope::Scope::new(&collapsed, inherited);
    // Буквица `initial-letter` расшивается в плавающий узел ДО обтекания —
    // дальше её ведёт `wrap_floats` наравне с авторскими флоатами. В гибком
    // контейнере и сетке `::first-letter` не действует — там не трогаем.
    let collapsed = if ordered_context {
        collapsed
    } else {
        initial_letter_float(
            first_letter_descendants::route(
                first_line_descendants::route(collapsed, inherited),
                inherited,
            ),
            inherited,
            opts,
        )
    };
    // §8.3.1: поле первого ребёнка примыкает к верхнему полю содержащего
    // блока, только если того не отделяют ни рамка, ни отбивка и он не
    // заводит своего контекста форматирования.
    let cb_top_open = !own_context_style(inherited)
        && zero_len(inherited.padding.top)
        && zero_len(inherited.borders().top);
    // Измеряемый бандовый хост (`band_flow.rs`) — только в БЛОЧНОМ контейнере
    // горизонтального письма: в гибком и сетке `float` не
    // действует (css-flexbox-1 §3, css-grid-1 §6.1).
    // Хост работает и в вертикальном письме (шаг F10): план в
    // логических осях, перевод в физику при сборке (`band_flow::VERT`).
    // Horizontal float sides are physical (CSS 2.1 §9.5.1); paragraphs
    // handle RTL within those bands. Vertical RTL still needs axis conversion.
    let vert_host = inherited.vertical == Some(true)
        && inherited.sideways != Some(true);
    // Вне хоста и там, где у раскладки свой счёт строк и разрывов: под
    // `line-clamp` (точка среза считает строки и флоаты за ней —
    // `line-clamp-with-floats-003/004`, `webkit-line-clamp-025`; отложенный
    // ряд `float_flow` для этого и заведён) и на печатных листах (монолитная
    // коробка хоста не режется между страницами —
    // `monolithic-overflow-020-print`).
    let measured_ok = !flex_ctx
        && inherited.line_clamp.is_none()
        && crate::interact::clamp_context().is_none()
        && !PAGED.with(std::cell::Cell::get)
        && !matches!(
            inherited.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && (inherited.vertical != Some(true) || vert_host)
        && (inherited.vertical_rl != Some(true) || vert_host)
        && (inherited.rtl != Some(true) || inherited.vertical != Some(true));
    let _fl_guard = BandFlGuard(BAND_FL.with(|f| f.replace(inherited.first_line.as_deref().cloned())));
    let _cbh_guard = BandCbhGuard(BAND_CBH.with(|h| {
        h.replace(match inherited.height {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        })
    }));
    let _cbw_guard = BandCbwGuard(BAND_CBW.with(|w| {
        w.replace(match inherited.width {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        })
    }));
    let _wm_guard = BandWmGuard(BAND_WM.with(|w| {
        w.replace(match (inherited.vertical, inherited.vertical_rl) {
            (Some(true), Some(true)) => 1,
            (Some(true), _) => 2,
            _ => 0,
        })
    }));
    let collapsed = replaced_used_style::inline_nodes(collapsed, AVAIL_W.get());
    let collapsed = by_layer(
        wrap_floats(
            collapsed,
            inherited,
            cb_top_open,
            match inherited.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            },
            measured_ok,
            own_context_style(inherited) || inherited.flex_item,
            cell_bfc,
        ),
        flex_ctx,
    );
    // Блок мы изображаем гибкой колонкой, а её дети по умолчанию сжимаются —
    // в обычном потоке этого нет: ребёнок выше родителя обязан вылезти, а не
    // ужаться. Поэтому в потоке сжатие детям выключается, если разметка не
    // просила обратного.
    let flex_context = matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let collapsed: Vec<Node> = if ordered_context {
        // Элемент гибкого контейнера сжимается по умолчанию — это его
        // начальное значение в CSS. Проставляем его явно, потому что
        // `display: inline-block` в другом месте выключает сжатие: строчная
        // коробка В СТРОКЕ и правда не жмётся, а тот же элемент В РЯДУ —
        // обязан. Без этого ряд из `<span>`-ов держал свою ширину и не
        // ужимался до минимального размера содержимого.
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) if flex_context => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(1.0);
                    }
                    // `vertical-align` на элементе гибкого контейнера не
                    // действует (css-flexbox-1 §4): он выравнивается своими
                    // свойствами, а не как кусок строки.
                    e.style.vertical_shift = None;
                    e.style.vertical_shift_px = None;
                    // Элемент ряда под обособлением строчной оси: главный
                    // размер берётся из `contain-intrinsic-size`, а не от
                    // содержимого. В колонке главная ось блочная — её уже
                    // держит подмена высоты.
                    let row = !matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    );
                    if row
                        && e.style.contains_width()
                        && matches!(e.style.width, None | Some(Len::Auto))
                    {
                        e.style.width = Some(Len::Px(e.style.contain_intrinsic.0.unwrap_or(0.0)));
                    }
                    // Поперечный размер элемента ряда с `height: auto` при
                    // растяжке даёт строка (css-flexbox-1 §9.4 п.11), и
                    // обособление высоты обязано ей уступить (`apply_box`).
                    // `auto`-поле по поперечной оси растяжку отменяет.
                    let stretches = e.style.align_self_normal
                        || match e.style.align_self {
                            Some(Align::Stretch) => true,
                            None => matches!(inherited.align_items, None | Some(Align::Stretch)),
                            _ => false,
                        };
                    if row
                        && inherited.vertical != Some(true)
                        && stretches
                        && e.style.contains_height()
                        && matches!(e.style.height, None | Some(Len::Auto))
                        && e.style.margin.top != Some(Len::Auto)
                        && e.style.margin.bottom != Some(Len::Auto)
                    {
                        e.style.cross_stretched = true;
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(0.0);
                    }
                    // rtl: переполняющий блок с ЗАДАННОЙ шириной прижат к
                    // правому краю и вылезает влево (csswg-drafts#5572);
                    // только горизонтальное письмо — в вертикали cross-ось
                    // иная (abs-pos-border-offset-001/002).
                    if inherited.rtl == Some(true)
                        && inherited.vertical_rl.is_none()
                        && e.style.width.is_some()
                        && e.style.align_self.is_none()
                        // Блочный по ВЫЧИСЛЕННОМУ `display`, а не по тегу:
                        // `span { display: block; width: … }` в rtl-блоке —
                        // тоже блок (замер 1393 пар с rtl/картинками: +0/−0,
                        // `block-in-inline-margins-002a/b` 0.12 -> 0.00).
                        // Эталон `flexbox-writing-mode-013-ref` держит
                        // слева другое (не этот путь).
                        && (!e.inline
                            || matches!(
                                e.style.display,
                                Some(Display::Block)
                                    | Some(Display::ListItem)
                                    | Some(Display::Flex)
                                    | Some(Display::Grid)
                                    | Some(Display::Table)
                            ))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `sideways-lr` — единственное письмо, где строчная ось
                    // идёт СНИЗУ ВВЕРХ: таблица Abstract-Physical Mapping
                    // (css-writing-modes-4, Overview.bs:1795-1830) даёт ему
                    // `line-left` = НИЗ, всем прочим вертикальным — верх.
                    // Значит начало строчной оси содержащего блока — его
                    // нижний край, и ребёнок с ОПРЕДЕЛЁННЫМ поперечным
                    // (физически вертикальным) размером стоит там:
                    // `body { height: 9em }` под корнем `sideways-lr` прижат
                    // к низу окна (`block-flow-direction-043-ref`: стол
                    // y 412…591 из 600), квадрат `height: 100px` — в НИЖНЕМ
                    // левом углу (`wm-propagation-body-035-ref`: y 492…592).
                    // Растянутого ребёнка правило не касается: `align-self`
                    // без определённого поперечного размера снимает растяжку.
                    // ★ ЗАМЕРЕНО: срез всего вертикального письма
                    // (`target/L-wm-wide.txt`, 1798 пар) 1335 -> 1354,
                    // +20/−1. Единственная потеря — `abs-pos-border-
                    // offset-002` 0.45 -> 2.22: там 68 коробок всех
                    // сочетаний письма и направления, и порог она
                    // держала не правотой, а усреднением; статическое
                    // место абсолюта при `sideways-lr` остаётся долгом
                    // корня WM-OVERCONSTRAINED-AXIS.
                    if inherited.vertical == Some(true)
                        && inherited.sideways == Some(true)
                        && inherited.vertical_rl != Some(true)
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Pct(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `vertical-lr`/`vertical-rl`/`sideways-rl` при
                    // `direction: rtl`: строчная ось идёт СНИЗУ вверх —
                    // inline-start содержащего блока у НИЖНЕГО края
                    // (css-writing-modes-4 §6.4: line-right = низ, rtl
                    // ставит start на line-right). Переполненная по строчной
                    // оси коробка стоит у inline-start и вылезает к inline-end
                    // (CSS 2.2 §10.3.3 в логических осях, §7.1) — то есть
                    // низом к низу и ВВЕРХ. Без правила тело `height: 100vh`
                    // с рамками под корнем `vertical-lr; direction: rtl`
                    // лежало от верха, и красная верхняя рамка оставалась в
                    // окне (`contain-{body,html}-t-o-*` — ломался и эталон).
                    // `sideways-lr` исключён: у него line-left = низ, и при rtl
                    // start — ВЕРХ (правило выше его не касается rtl).
                    if inherited.vertical == Some(true)
                        && inherited.rtl == Some(true)
                        && !(inherited.sideways == Some(true) && inherited.vertical_rl != Some(true))
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_))
                                | Some(Len::Em(_))
                                | Some(Len::Pct(_))
                                | Some(Len::Vh(_))
                                | Some(Len::Vw(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // Коробка с `aspect-ratio` при auto-ширине и определённой
                    // высоте — fit-content, а не растяжка (css-sizing-4 §5.1:
                    // «automatic sizes are calculated the same as for a replaced
                    // element with a natural aspect ratio»; Blink length_utils
                    // `may_apply_aspect_ratio` → FitContent). Блок у нас —
                    // колонка flex, и `stretch` тянул ширину на всю строку
                    // (`block-aspect-ratio-002/006/…`).
                    // Нулевое и бесконечное отношение — как `auto`
                    // (css-sizing-4 §5.1; `zero-or-infinity-002`).
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    if ratio_ok
                        && !ordered_context
                        && matches!(e.style.width, None | Some(Len::Auto))
                        // Доля высоты от блока с высотой в точках — тоже
                        // определённая высота (CSS 2.1 §10.5), и ширина
                        // идёт из соотношения, а не растяжкой
                        // (`percentage-resolution-005`: 50×100 вместо 100×100).
                        && (matches!(e.style.height, Some(Len::Px(_)))
                            || (matches!(e.style.height, Some(Len::Pct(_)))
                                && matches!(inherited.height, Some(Len::Px(_)))))
                        && e.style.align_self.is_none()
                        && inherited.vertical.is_none()
                        && !e.inline
                        && !positioned_out
                    {
                        e.style.align_self = Some(Align::Start);
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    // §9.9 шаг 8: позиционированная коробка рисуется ПОВЕРХ содержимого
    // потока, а порядок краски у нас — порядок детей. В обычном потоке это
    // делает верхний слой, но в сетке и гибком контейнере он выключен
    // (`!ordered_context` у гейтов ниже), и абсолютный ребёнок оказывался под
    // соседями. Здесь его достаточно переставить в конец: место он берёт не
    // из потока (в раскладку сетки такой ребёнок не входит), поэтому
    // перестановка меняет только краску.
    // Вынесенный абсолют всё же РВЁТ прогон текста: каждая непрерывная
    // последовательность текстовых детей — свой анонимный элемент
    // (css-flexbox-1 §4, css-grid-2 §6.1), а абсолютный ребёнок в неё не
    // входит. После перестановки куски «Two » и «lines» оказывались соседями
    // и склеивались в один абзац (`anonymous-flex-item-004`,
    // `anonymous-grid-item-001`). `run_breaks` — индексы в новом списке, перед
    // которыми накопленный абзац закрывается.
    let mut run_breaks: Vec<usize> = vec![];
    let collapsed: Vec<Node> = if ordered_context {
        let in_flow = |n: &Node| match n {
            Node::Element(e) => {
                e.style.position != Some(crate::computed::Position::Absolute)
                    || e.style.z_index.is_some_and(|z| z < 0)
            }
            Node::Text(_) => true,
        };
        let mut flow: Vec<Node> = vec![];
        let mut over: Vec<Node> = vec![];
        for n in collapsed {
            if in_flow(&n) {
                flow.push(n);
            } else {
                if run_breaks.last() != Some(&flow.len()) {
                    run_breaks.push(flow.len());
                }
                over.push(n);
            }
        }
        flow.into_iter().chain(over).collect()
    } else {
        collapsed
    };
    let mut out = vec![];
    // Порядок краски подслоя (§9.9 шаг 3): соседние распорки отрицательного
    // `z-index` стоят в порядке разметки, а рисоваться обязаны по z. Высота у
    // них нулевая и y общий, поэтому перестановка СОСЕДЕЙ раскладку не меняет
    // — в отличие от перестановки в общем списке детей, замеренной в минус
    // (см. `movable`). Прогон рвётся сам, как только между распорками встаёт
    // что-то ещё.
    let mut below_run_start = 0usize;
    let mut below_run_end = usize::MAX;
    let mut below_zs: Vec<i32> = vec![];
    // Липкому ребёнку нужны две вещи, которых он сам не видит: коробка
    // родителя и видимая часть ленты. Их снимает распорка — она идёт первой,
    // потому что готовит замер до отрисовки детей.
    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => e.style.position == Some(crate::computed::Position::Sticky),
        _ => false,
    });
    let frame: crate::interact::StickyCell = Default::default();
    if sticky {
        out.push(sticky_probe(frame.clone()));
    }
    let mut pending: Vec<Node> = vec![];
    // Слой верхней отрисовки этого контейнера: позиционированные элементы
    // складывают сюда содержимое, а забирается оно последними детьми.
    crate::interact::late_open();
    let nodes = collapsed.as_slice();
    for (idx, n) in nodes.iter().enumerate() {
        if run_breaks.contains(&idx) && !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(letter_scope.paragraph(&taken, inherited, opts)));
        }
        let is_inline = match n {
            // Пробельный узел между инлайн-соседями — часть строки, а не
            // разрыв: `<button>A</button> <button>B</button>` в разметке с
            // переносами давал два абзаца, и кнопки вставали столбиком.
            // Под `white-space: pre*` пробельный узел — содержимое: узел из
            // одного перевода строки это ПУСТАЯ СТРОКА перед `</pre>`
            // (block-plaintext-006), отбрасывание съедало её высоту.
            Node::Text(t) => {
                inherited.preserve_newlines == Some(true)
                    || !blank_text(t)
                    || (!pending.is_empty() && t.contains(' '))
            }
            // Элемент с ЗАДАННЫМИ краями строчным не бывает: края он считает
            // от позиционированного предка, а не от строки. Куском абзаца он
            // получал содержащим блоком сам абзац — и `inset: 0` растягивал
            // его на одну строку вместо всей коробки родителя. На этом стоит
            // приём эталонов WPT: `::after` с `content: ""` и `inset: 0`
            // накрывает красное зелёным (`overflow-wrap-anywhere-001`).
            // Только когда заданы ОБЕ оси: у коробки с одним краем свободная
            // ось остаётся статической, а статическая позиция строчного — в
            // строке, не в блочном потоке. Такую коробку ведёт щуп в
            // `atom_element` (`x_set != y_set`).
            Node::Element(e)
                if matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) && !at_static_position(&e.style)
                    && {
                        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                        (edge(e.style.inset.left) || edge(e.style.inset.right))
                            && (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                    }
                    // Поле формы и заменяемый элемент строит СВОЙ путь
                    // (`forms::element`, картинка), и краями он распоряжается
                    // сам. Выведенный из строки, он терял свою коробку —
                    // `<button>` с четырьмя краями переставал растягиваться
                    // (`position-absolute-semi-replaced-stretch-button`).
                    // Исключение снимается ровно там, где оно даёт НЕ ТОТ
                    // прямоугольник: содержащий блок абсолюта — внутренний
                    // край рамки родителя (§10.1 п.4.2), а куском строки
                    // замещаемый считает край от содержимого. Признак —
                    // `cb_padding_shifts_replaced`.
                    && (!matches!(
                        e.tag.as_str(),
                        "input" | "textarea" | "select" | "button" | "img" | "svg" | "canvas"
                    ) || cb_padding_shifts_replaced(e, inherited)) =>
            {
                false
            }
            // Абсолют строчного уровня (до блокификации — `inline-block` и
            // родня) с РОВНО ОДНОЙ заданной осью: свободная ось берётся от
            // гипотетической коробки при `position: static` (CSS 2.1 §10.3.7,
            // §10.6.4), а та стоит в строке, не под ней. Блокифицированный, он
            // уходил блочным ребёнком ниже абзаца, и `left: 0; top: auto`
            // вставал на следующую строку (`border-left-width-thin`: белая
            // заплатка под красным вместо поверх). Щуп строки ведёт такую
            // коробку в `atom_element` (`x_set != y_set`). Без строчного
            // содержимого ДО коробки строка пуста, и гипотетическая коробка
            // стоит в её начале — там же, где блочная статическая позиция;
            // такой абсолют остаётся прежним блочным путём
            // (`left-applies-to-012/014`: абсолют — единственный ребёнок).
            Node::Element(e)
                if e.style.abs_inline_level
                    && !ordered_context
                    && pending.iter().any(|p| match p {
                        Node::Text(t) => !t.trim().is_empty(),
                        Node::Element(x) => !matches!(
                            x.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        ),
                    })
                    && {
                        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                        (edge(e.style.inset.left) || edge(e.style.inset.right))
                            != (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                    } =>
            {
                true
            }
            Node::Element(e) => match e.style.display {
                // Явно заявленная инлайновая коробка остаётся в строке даже у
                // блочного по природе тега — но НЕ внутри гибкого контейнера
                // или сетки: там каждый ребёнок сам себе элемент раскладки
                // («блокирование» из CSS). Иначе колонка из таких коробок
                // выкладывалась рядом: они склеивались в один абзац.
                Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable) => !ordered_context,
                // `display: inline grid-lanes` — такая же строчная коробка:
                // разбор держит её как `GridLanes` с пометкой `lanes_inline`,
                // и по css-display-3 внешний вид у неё `inline`. Эталоны
                // семьи `grid-lanes-intrinsic-sizing-*` написаны на
                // `display: inline-grid`, и без этой строки девять сеток
                // вставали столбиком вместо ряда.
                Some(Display::GridLanes) if e.style.lanes_inline => !ordered_context,
                // `display: contents` without block-level descendants: its
                // children are inline-level boxes and text runs of THIS
                // container (css-display-3 §2.5 «as if they replaced the
                // element»), so they join the surrounding inline run — the
                // inline collector dissolves the element (`inline.rs`,
                // `Display::Contents`). Flushing the run here split one line
                // `<div contents>abc</div><br>` into an anonymous block plus a
                // run starting with `<br>` — an extra empty line
                // (`text-autospace-elements-002`).
                Some(Display::Contents) => !ordered_context && !contains_block(&e.children),
                // Прежний откат этой строки СНЯТ (03.09). Он мерился, когда
                // строчный атом строил лунки голым `blocks()` и терял их
                // целиком — оттого вся восьмёрка `flow-tolerance-*` и уходила
                // в красное (0.00 -> 5.66 и родня). Теперь `atom_element`
                // отдаёт лунки блочному пути (`element()`), и обе правки
                // вместе дают по всему CSS3 2420 -> 2442: приобретено 27,
                // потеряно 5 (`row-line-names-007/008/010/012`,
                // `row-subgrid-abs-pos-002` — рядные лунки, они ждут обтяжку
                // по РЯДАМ, корень R4 из `target/scout-subgrid-orthogonal-
                // 2026-09.md`).
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строчный путь для абсолюта
                // с объявленным `display: inline` на статической позиции
                // (css-position-3 §staticpos-rect). Срез 12086 пар вместе с
                // патчем барьера `contain`: 9343 -> 9346 (+9/-6), причём вся
                // шестёрка потерь — этого рукава:
                // `inline-level-absolute-in-block-level-context-002`
                // (0.26->0.52), `-007` (0.00->0.54), `-010` (0.00->1.04),
                // `position-absolute-dynamic-static-position-inline`
                // (0.00->2.10), `abs-pos-border-offset-003` (0.46->1.75),
                // `css-flexbox-height-animation-stretch` (0.10->1.90), против
                // всего двух приобретений (`-009`, `-012`). Строчная ветка
                // теряет полосу обтекания и рамочные смещения — рукав нужен
                // не здесь, а в `atom_element`.
                Some(_) => false,
                // Дети гибкого контейнера и сетки блокируются по CSS: каждый
                // сам себе элемент раскладки. Без оговорки `<span>` без
                // объявленного `display` оставался строчным, склеивался с
                // соседями в ОДИН абзац, и четыре элемента раскладки
                // превращались в один.
                //
                // Плавающий кусок строчным не бывает: `float` вынимает элемент
                // из строки и делает блоком (CSS 2.1 §9.7). Пока картинка с
                // `float: right` оставалась куском абзаца, до неё не доходило
                // поле родителя, и она вылезала за край страницы.
                // Перевод строки коробки не создаёт: в гибком контейнере и
                // сетке он остаётся ВНУТРИ безымянного элемента раскладки
                // вместе с соседним текстом, а не становится своим элементом
                // (`position-absolute-root-element-flex`: два предложения,
                // разделённые `<br><br>`, вставали бок о бок и переносились
                // раньше времени).
                None => {
                    e.inline
                        && (!ordered_context || e.tag == "br")
                        && !e.style.float.is_some_and(|f| f != 0)
                }
            },
        };
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(letter_scope.paragraph(&taken, inherited, opts)));
        }
        // Позиционированные с `z-index: auto` красятся В ПОРЯДКЕ ДЕРЕВА
        // (CSS 2.1 прил. E, шаг 8; Blink `paint_layer_paint_order_iterator.h`
        // — один список). Абсолют на статической позиции живёт в верхнем слое
        // (`late_push`), и тот выпускался только в конце контейнера — ПОВЕРХ
        // позиционированных соседей, идущих в дереве позже
        // (`position-sticky-stacking-context-002`: `#overlapped-red` накрывал
        // липкий и `relative`-брата). Перед таким соседом слой выпускается:
        // пустые заместители нулевой высоты раскладку не трогают, а щупы
        // накопленных абсолютов стоят раньше по списку — дырки к подготовке
        // их заместителей уже известны. Отрицательный `z-index` не трогаем:
        // у него своя сортировка прогона (`below_run_*`).
        if let Node::Element(e) = n
            && crate::interact::late_pending()
            && !e.style.z_index.is_some_and(|z| z < 0)
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Relative) | Some(crate::computed::Position::Sticky)
            )
        {
            out.extend(crate::interact::late_close());
            crate::interact::late_open();
        }
        if let Node::Element(e) = n {
            // Ключ краски шага 8 — до сборки детей (см. `next_paint_key`).
            let paint_key = next_paint_key();
            // CSS2 Appendix E: descendants paint within their nearest stacking context.
            let layer_ok = !inside_deferred();
            let geometry_layer_ok = !paint_scope::deferred();
            let _deferred_guard = paint_scope::Guard::enter(
                defers(&e.style, inherited, under_tf), stacking_context(&e.style),
            );
            // Ряд обтекания: текст рядом с плавающим блоком и остаток под ним.
            if e.tag == "kamin-float" {
                out.push(letter_scope.flow(&e.children, inherited, |s| float_flow(e, s, opts)));
                continue;
            }
            if let Some(el) = scrollable(e, inherited, opts) {
                out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
                continue;
            }
            if let Some(el) = resizable(e, inherited, opts) {
                out.push(el);
                continue;
            }
            if let Some(el) = transitioned(e, inherited, opts) {
                // Наложение считается и для узла с переходом: раньше ветка
                // уходила мимо, и `z-index` у него пропадал.
                out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
                continue;
            }
            // `display: contents` — своей коробки у элемента нет: дети
            // становятся детьми родителя, и стиль самого элемента исчезает.
            if e.style.display == Some(Display::Contents) {
                let mut merged = inline::inherit(inherited, &e.style);
                // Своей коробки нет — значит и объёмный контекст она не
                // обрывает: дети берут ячейки ДЕДА (css-display-3
                // §box-generation; transform3d-preserve3d-014 — `rotateX(90)`
                // над `display: contents` над `rotateX(90) scale(2)`).
                if merged.frame_3d.is_none() {
                    merged.frame_3d = inherited.frame_3d.clone();
                }
                if merged.perspective_frame.is_none() {
                    merged.perspective_frame = inherited.perspective_frame.clone();
                }
                out.extend(blocks(&e.children, &merged, opts));
                continue;
            }
            // Ключевое слово содержимого в `min-width`/`max-width` при
            // ширине в точках (css-sizing-3 §4.1, зажим §5.1): used =
            // max(W, kw) либо min(W, kw) — то же самое, что `width: kw` с
            // пределом W. Перестановка отдаёт ключевое слово обёртке-сетке
            // (`content_sized`), а точки — пределу в раскладке (`apply`);
            // блочная ось решается в `apply` (`min-height: max-content`).
            let swapped;
            let e = if let Some(copy) = content_limit_swapped(e) {
                swapped = copy;
                &swapped
            } else {
                e
            };
            // Обёртка `content_sized` — сетка, а дорожка сетки НЕ считает
            // боковые поля ребёнка: коробка `width: max-content` с полем
            // теряла его и уезжала (`pre-wrap-017`: зелёный блок пропадал
            // вовсе). Поэтому элемент строится БЕЗ боковых полей, а поля
            // берёт на себя обёртка.
            // Переносится только ОТРИЦАТЕЛЬНОЕ поле: положительное внутри
            // дорожки работает как надо, а отрицательное дорожка съедает —
            // коробка `width: max-content` с `margin-left: -1em` пропадала
            // вовсе (`pre-wrap-017`).
            let negative = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::Px(v) | Len::Em(v) | Len::Ch(v) | Len::Ex(v)) if v < 0.0
                )
            };
            let hoist_margins = content_sized_wraps(e)
                && !replaced_tag(e)
                && (negative(e.style.margin.left) || negative(e.style.margin.right));
            // Размещение в сетке тоже уезжает на обёртку (см.
            // `content_sized`): в дорожках родителя стоит она. Внутри обёртки
            // (своя сетка в одну дорожку) элемент с прежним `grid-row: 2`
            // уходил бы в её неявный ряд. Прежде обёртка без размещения
            // ставилась авто-размещением (`row-fill-reverse-align-self-001`:
            // `width: min-content; grid-row: 2` в лунках вставал в ряд 1).
            let placement = crate::apply::grid_item_placement(&e.style);
            let hoist_place = content_sized_wraps(e)
                && !replaced_tag(e)
                && (placement.0.is_some() || placement.1.is_some());
            let stripped;
            let e = if hoist_margins || hoist_place {
                let mut copy = e.clone();
                if hoist_margins {
                    copy.style.margin.left = None;
                    copy.style.margin.right = None;
                }
                if hoist_place {
                    copy.style.grid_row = None;
                    copy.style.grid_col = None;
                    copy.style.grid_row_named = [None, None];
                    copy.style.grid_col_named = [None, None];
                }
                stripped = copy;
                &stripped
            } else {
                e
            };
            let placement = if hoist_place { placement } else { (None, None) };
            // Коробка по содержимому (`width: min-content | max-content |
            // fit-content`) стоит в обёртке-сетке `content_sized`, и её доли
            // решались бы ОТ ОБЁРТКИ, а не от содержащего блока: `height: 100%`
            // — от неявного ряда (по содержимому: коробка схлопывалась в ноль),
            // `padding-top: 100%` — от области сетки (заливала всё окно).
            // Блочный родитель с известными сторонами решает их сразу
            // (CSS 2.1 §10.5 высота — от высоты содержащего блока, §8.4
            // отступы и §8.3 поля — от его ширины;
            // `intrinsic-percent-replaced-012/013`).
            let resolved_pct;
            let e = match pct_resolved_for_wrapper(e, inherited) {
                Some(copy) => {
                    resolved_pct = copy;
                    &resolved_pct
                }
                None => e,
            };
            // Анимация оборачивает ЛЮБОЙ элемент: таблицу, список, картинку —
            // раньше она доставалась только простому блоку.
            // Фон КАНВАСА (CSS 2.2 §14.2): фон корневого html — а без него
            // фон body — красит всю область просмотра, включая место за
            // полями. Слой absolute от родителя-корня растягивается на всё
            // окно, с самой коробки краска снимается (иначе двойная альфа).
            let canvas_paint = e.style.canvas_bg;
            // Тело под корнем-донором фона холста при `vertical-rl`: его
            // margin-box (плюс рамка/отбивка корня) — коробка корня по
            // содержимому; её левый край пишется на подготовке тела.
            let record_root = e.tag == "body"
                && inherited.canvas_bg
                && inherited.vertical_rl == Some(true)
                && !matches!(inherited.width, Some(Len::Px(_)));
            let canvas_stripped;
            let e = if canvas_paint {
                // Фон холста — часть ГРУППЫ КОРНЯ (css-compositing-1
                // §pagebackdrop): фильтр корня красит и его. Слой лежит
                // СОСЕДОМ коробки корня, поэтому единственная точка окраски
                // фильтром (`inline::inherit`) до него не доходит — красим
                // здесь, от СОБСТВЕННОГО фильтра корня.
                let root_filter = e.style.filter;
                let mut layer = div().absolute().top_0().left_0().right_0().bottom_0();
                // Слоёв несколько — их рисуют плитки (`bg_layers` ниже), а
                // заливка всего холста верхним градиентом их закрыла бы
                // (`background-position-right-in-body`: 97.92).
                let canvas_layers = e.style.bg_layers();
                if let Some(g) = e.style.gradient.as_ref().filter(|_| canvas_layers.is_none()) {
                    let mut g = g.clone();
                    if let Some(f) = root_filter {
                        g.from = f.apply(g.from);
                        g.to = f.apply(g.to);
                        for stop in g.stops.iter_mut() {
                            stop.0 = f.apply(stop.0);
                        }
                        for stop in g.stops_px.iter_mut() {
                            stop.0 = f.apply(stop.0);
                        }
                        for stop in g.stops_raw.iter_mut() {
                            stop.0 = f.apply(stop.0);
                        }
                    }
                    layer = layer.bg(crate::apply::fill(&g));
                } else if let Some(bg) = e.style.background {
                    let bg = root_filter.map_or(bg, |f| f.apply(bg));
                    layer = layer.bg(bg.to_hsla());
                }
                // Фон-КАРТИНКА канваса красит всю область просмотра тем же
                // слоем (CSS 2.2 §14.2: painting area корневого фона —
                // канвас): на коробке корня она начиналась с его сдвинутого
                // схлопкой верха, и над краской проступала полоса
                // (background-size-document-root-vrl-*).
                let mut layer = layer.into_any_element();
                // Донор — САМ корень: область ОТСЧЁТА плитки это его коробка
                // (§14.2 «sized and positioned relative to the root element's
                // box»), а красит она весь холст. Донор-тело сюда не входит:
                // его слой лежит в детях корня, и отсчёт от padding-box корня
                // получается сам (см. записи о двух откатах ниже).
                if e.tag == "html"
                    && (e.style.bg_image.is_some() || canvas_layers.is_some())
                    && let Some(tiles) = {
                        // Единицы шрифта тоже длина: `html { margin-top: 1em }`
                        // роняло отсчёт в ноль, и плитка начиналась с края
                        // холста (`margin-collapse-020`).
                        let em = match e.style.font_size {
                            Some(Len::Px(v)) => v,
                            _ => opts.base_size(),
                        };
                        let fam = e.style.font_family.clone().unwrap_or_default();
                        let side = |l: Option<Len>| match l {
                            Some(Len::Px(v)) => v,
                            Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                                crate::metrics::spacing_px(Some(l), &fam, em)
                            }
                            _ => 0.0,
                        };
                        let b = e.style.borders();
                        let area = crate::background::RootArea {
                            left: side(e.style.margin.left) + side(b.left),
                            top: side(e.style.margin.top) + side(b.top),
                            right: side(e.style.margin.right) + side(b.right),
                            bottom: side(e.style.margin.bottom) + side(b.bottom),
                            width: match e.style.width {
                                Some(Len::Px(w)) => Some(
                                    w + side(e.style.padding.left) + side(e.style.padding.right),
                                ),
                                _ => None,
                            },
                            height: match e.style.height {
                                Some(Len::Px(h)) => Some(
                                    h + side(e.style.padding.top) + side(e.style.padding.bottom),
                                ),
                                _ => None,
                            },
                            from_right: e.style.vertical_rl == Some(true),
                            // Корень `vertical-rl` без заданной ширины — по
                            // содержимому у правого края (css-writing-modes-4
                            // §7, auto block-size): левый край его коробки
                            // пишет обёртка тела ниже при подготовке.
                            left_key: (e.style.vertical_rl == Some(true)).then_some(opts.doc_salt),
                        };
                        match &canvas_layers {
                            // Снизу вверх, каждый слой — своей плиткой от
                            // коробки корня (§14.2).
                            Some(layers) => {
                                let mut stack = div().absolute().top_0().left_0().right_0().bottom_0();
                                for l in layers.iter().rev() {
                                    if let Some(t) = crate::background::canvas_layer(l, area) {
                                        stack = stack.child(t);
                                    }
                                }
                                Some(stack.into_any_element())
                            }
                            None => crate::background::canvas_layer(&e.style, area),
                        }
                    }
                {
                    layer = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(layer)
                        .child(tiles)
                        .into_any_element();
                } else if e.style.bg_image.is_some()
                    && let Some(tiles) = crate::background::layer(&e.style)
                {
                    // Область ПОЗИЦИОНИРОВАНИЯ краски — PADDING-BOX корня:
                    // ширина + горизонтальные отступы; полоса прижата по
                    // письму с учётом поля и рамки с той стороны.
                    //
                    // ЗАМЕРЕНО ВТОРОЙ РАЗ (перенос фона тела на корень по §14.2
                    // ВМЕСТЕ с `RootArea` + `canvas_layer`): приобретено 1,
                    // потеряно 2 — `background-position-001` 0.36 -> 2.39 и
                    // `background-root-024` 0.17 -> 5.74. Перенос сам по себе
                    // даёт 0 и −2. Значит дело не в кегле тела: расходится
                    // геометрия коробки корня, и её надо чинить первой.
                    //
                    // ЗАМЕРЕНО: считать область от коробки корня целиком
                    // (`RootArea` + `canvas_layer`, плитка красит весь холст)
                    // — CSS2 +2 в `background-root-001/002`, но -3 в
                    // `margin-collapse-020/021` и `block-formatting-contexts-003`:
                    // слой на весь холст перекрывает то, что рисуется выше по
                    // потоку. Возвращаться вместе с переносом фона тела на
                    // корень, когда кегль тела будет разрешаться до переноса.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let b = e.style.borders();
                    let mut band = div().absolute().top_0().bottom_0();
                    band = match e.style.width {
                        Some(Len::Px(w)) => {
                            let pad_w =
                                w + side(e.style.padding.left) + side(e.style.padding.right);
                            let band = band.w(px(pad_w));
                            if e.style.vertical_rl == Some(true) {
                                band.right(px(side(e.style.margin.right) + side(b.right)))
                            } else {
                                band.left(px(side(e.style.margin.left) + side(b.left)))
                            }
                        }
                        _ => band.left_0().right_0(),
                    };
                    layer = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(layer)
                        .child(band.child(tiles))
                        .into_any_element();
                }
                // Прозрачность корня — на ГОТОВЫЙ слой холста целиком, вместе
                // с плиткой фона-картинки: погаси их порознь, и цвет с плиткой
                // сложились бы с двойной альфой. Коробка корня свою
                // прозрачность получает отдельно (`apply::style`), но краска с
                // неё уже снята, так что перекрытия групп нет.
                if let Some(o) = e.style.opacity.filter(|o| *o < 1.0) {
                    layer = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .opacity(o)
                        .child(layer)
                        .into_any_element();
                }
                // `clip-path` корня режет и холст (css-masking-1 §the-clip-path +
                // compositing-1 §rootgroup: фон корня — часть корневой группы):
                // слой холста получает ту же обрезку, что и коробка корня.
                // Начало координат у них общее — левый верхний угол окна.
                if e.style.clip_polygon.is_some()
                    || e.style.clip_shape.is_some()
                    || e.style.clip_inset.is_some()
                    || e.style.clip_xywh.is_some()
                {
                    let mut clip = Computed::default();
                    clip.clip_polygon = e.style.clip_polygon.clone();
                    clip.clip_shape = e.style.clip_shape.clone();
                    clip.clip_inset = e.style.clip_inset.clone();
                    clip.clip_xywh = e.style.clip_xywh.clone();
                    layer = grouped(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(layer)
                            .into_any_element(),
                        &clip,
                    );
                }
                out.push(layer);
                let mut copy = e.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                copy.style.bg_image = None;
                canvas_stripped = copy;
                &canvas_stripped
            } else {
                e
            };
            // Абсолют с КЛЮЧЕВЫМ СЛОВОМ содержимого по оси и краями с обеих
            // сторон этой оси (css-position-3 §3.7-3.8): растяжение краями —
            // только для автоматического размера; заданный ключевым словом
            // размер — по содержимому, а остаток делят auto-поля. Держатель =
            // inset-modified containing block (абсолют с краями элемента,
            // гибкий контейнер вдоль оси), внутри — та же коробка статической,
            // без краёв и полей (`div-{min,max,fit}-content-block-size`,
            // `div-*-auto-margin-*`).
            let kw_len = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                )
            };
            let positioned_out = matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            );
            // Предел ключевым словом содержимого при АВТОМАТИЧЕСКОМ размере —
            // тот же держатель: css-sizing-3 §fit-content в блочной оси даёт
            // высоту содержимого, и used = min(растяжение краями, содержимое)
            // (`position-absolute-fit-content`: `top: 0; bottom: 0;
            // max-height: fit-content` — 100, а не 200). `apply.rs` такой
            // предел умеет только при `height` в точках и иначе пропускает.
            // Содержимое выше растяжения держатель не зажмёт — приближение.
            let auto_len = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let holder_axis = if positioned_out
                && e.style.vertical.is_none()
                && auto_len(e.style.height) && kw_len(e.style.max_height)
                && edge_set(e.style.inset.top)
                && edge_set(e.style.inset.bottom)
            {
                Some(true)
            } else if positioned_out
                // Письмо коробки держателю не мешает: после `280d0d4` размеры
                // ортогонального узла ложатся по СВОЕМУ письму, и у
                // `vertical-rl` `block-size: min-content` — физическая ширина
                // (css-writing-modes-4, Abstract-Physical Mapping). Гейт
                // `vertical.is_none()` ставился, пока размеры
                // транспонировались; без держателя ширина тянулась краями
                // `left/right` во всё окно (`div-{min,max,fit}-content-
                // orthogonal-*`: 13.5 %). Ветка высоты выше гейт сохраняет: в
                // вертикальном письме это строчная ось, её пары не разбирались.
                && auto_len(e.style.width) && kw_len(e.style.max_width)
                && edge_set(e.style.inset.left)
                && edge_set(e.style.inset.right)
            {
                Some(false)
            } else {
                None
            };
            let built = if let Some(block_axis) = holder_axis {
                let auto = |l: Option<Len>| l == Some(Len::Auto);
                let mut holder = Computed::default();
                holder.position = e.style.position;
                holder.inset = e.style.inset;
                holder.z_index = e.style.z_index;
                holder.display = Some(Display::Flex);
                holder.flex_dir = Some(if block_axis {
                    crate::computed::FlexDir::Col
                } else {
                    crate::computed::FlexDir::Row
                });
                // Поля вдоль оси остаются у ВНУТРЕННЕЙ коробки: auto-поля
                // элемента гибкого контейнера забирают остаток, а при нехватке
                // места обнуляются (css-flexbox-1 §8.1) — ровно как auto-поля
                // абсолюта (§3.8; `fit-content-block-size-abspos` с
                // переполнением). Поперечные не-auto поля — у держателя.
                // Поперечные поля — у держателя целиком, включая auto: с
                // заданным размером и краями с обеих сторон они центрируют
                // сам абсолют (css-position-3 §3.8; `inline-size: 100px;
                // margin: auto; inset: 0`).
                if block_axis {
                    holder.margin.left = e.style.margin.left;
                    holder.margin.right = e.style.margin.right;
                } else {
                    holder.margin.top = e.style.margin.top;
                    holder.margin.bottom = e.style.margin.bottom;
                }
                // Поперечный размер держателя — border-box внутренней коробки:
                // её рамка и отбивка прибавляются (у держателя своих нет).
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let bd = e.style.borders();
                if block_axis {
                    let extra = px_of(e.style.padding.left)
                        + px_of(e.style.padding.right)
                        + px_of(bd.left)
                        + px_of(bd.right);
                    holder.width = match e.style.width {
                        Some(Len::Px(w)) => Some(Len::Px(w + extra)),
                        other => other.filter(|l| !kw_len(Some(*l))),
                    };
                } else {
                    let extra = px_of(e.style.padding.top)
                        + px_of(e.style.padding.bottom)
                        + px_of(bd.top)
                        + px_of(bd.bottom);
                    holder.height = match e.style.height {
                        Some(Len::Px(h)) => Some(Len::Px(h + extra)),
                        other => other.filter(|l| !kw_len(Some(*l))),
                    };
                }
                let mut inner = e.clone();
                inner.style.position = None;
                inner.style.inset = Default::default();
                if block_axis {
                    inner.style.margin.left = None;
                    inner.style.margin.right = None;
                } else {
                    inner.style.margin.top = None;
                    inner.style.margin.bottom = None;
                }
                inner.style.z_index = None;
                crate::apply::apply(div(), &holder)
                    .child(element(&inner, inherited, opts))
                    .into_any_element()
            } else if stacking_context(&e.style)
                && e.style.isolate != Some(true)
                && blends_inside(&e.children, 0)
            {
                // css-compositing-1 §mix-blend-mode: смешиваемый потомок
                // смешивается только с содержимым СВОЕГО контекста наложения.
                // Контекст обязан сложиться отдельной группой, иначе подложкой
                // становится весь кадр: белая страница вокруг родителя давала
                // красное кольцо (`-blended-element-with-transparent-pixels`),
                // lime вместо fuchsia (`-blended-with-3D-transform`). Blink —
                // `PaintLayer::HasNonIsolatedDescendantWithBlendMode`.
                let mut iso = e.style.clone();
                iso.isolate = Some(true);
                grouped(
                    transformed(animated(e, inherited, opts), &e.style, inherited),
                    &iso,
                )
            } else {
                grouped(
                    transformed(animated(e, inherited, opts), &e.style, inherited),
                    &e.style,
                )
            };
            let built = vertical_hug(built, e, inherited);
            let built = sticky_wrap(built, &e.style, &frame, layer_ok);
            // Таблица сжимается по содержимому (§17.5.2.2), и выражено это у
            // нас гибким рядом. В контейнере с БЛОЧНОЙ раскладкой гибкого
            // ряда нет, `align_self` мёртв, и таблица растягивалась на всю
            // ширину родителя — видно на `<span style="display:block">` с
            // табличными детьми.
            let table_child = e.tag == "table"
                || matches!(
                    e.style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            let block_parent = matches!(
                inherited.display,
                Some(Display::Block) | Some(Display::ListItem) | Some(Display::TableCell)
            );
            let built = if table_child && block_parent && e.style.width.is_none() {
                div().flex().flex_row().child(built).into_any_element()
            } else {
                built
            };
            // Абсолютный блок без заданных краёв стоит на СТАТИЧЕСКОЙ позиции —
            // там, где он оказался бы в потоке, а не в углу содержащего блока.
            // Пустышка нулевой высоты держит это место в потоке, элемент висит
            // от её угла. Без неё такой блок уезжал к началу родителя и
            // накрывал собой всё, что стояло выше.
            // Только в обычном потоке: в сетке и гибком контейнере пустышка
            // стала бы ЯЧЕЙКОЙ и сдвинула соседей, а по CSS абсолютный
            // ребёнок из раскладки родителя выключен.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ — наследование,
            // шрифт, письмо и маски остаются верными, — а готовый уходит
            // последним ребёнком документа, где края решит уже вьюпорт.
            //
            // Пока только при заданных ОБЕИХ осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ (наследование,
            // шрифт, письмо и маски остаются верными), а готовый уходит
            // последним ребёнком документа, где края решает уже вьюпорт.
            //
            // Предок считается ВКЛЮЧАЯ непосредственного родителя:
            // `inherited.cb_ancestor` отвечает за предков строго выше него.
            // ЗАМЕРЕНО без этого слагаемого: CSS2 4636 -> 4626, все двенадцать
            // потерь — абсолют внутри `position: relative`-РОДИТЕЛЯ.
            //
            // Пока только при заданных обеих осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Достаточно ОДНОЙ заданной оси: по ней край считает раскладка от
            // области просмотра, по пустой элемент стоит на СТАТИЧЕСКОЙ
            // позиции (§10.3.7, §10.6.4), и её сообщает щуп, оставшийся на
            // месте элемента. Ось задана, если задана хотя бы одна сторона.
            //
            // Внутри отложенного поддерева щуп готовится ПОЗЖЕ слоя, и дырка
            // была бы пуста — такие остаются на месте.
            let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
            let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
            // `fixed` считается ОТ ОКНА всегда (§10.1 п.3): позиционированный
            // предок ему не содержащий блок, и заданной оси от него не
            // требуется — незаданная сторона держит статическое место. Пока он
            // шёл общим путём, коробка висела от края родителя.
            let fixed = e.style.position == Some(crate::computed::Position::Fixed) && !under_tf;
            // `fixed` под трансформом — абсолют относительно этого предка.
            let abs_like = e.style.position == Some(crate::computed::Position::Absolute)
                || (e.style.position == Some(crate::computed::Position::Fixed) && under_tf);
            // В стопке страниц абсолют корня уходит в слой и без заданных
            // сторон: на месте его резала бы маска фрагмента кида
            // (`monolithic-overflow-013`); статическую позицию копии 0 даёт
            // щуп, копии ≥ 1 идут непрерывным потоком от верха листа.
            let orphan_abs = abs_like
                && !(inherited.cb_ancestor || crate::inline::establishes_cb(inherited))
                && (x_set || y_set || PAGED.with(|p| p.get()));
            // Позиционированный предок ЕСТЬ, но это не родитель: коробку
            // забирает слой ближайшего содержащего блока (§10.1).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09, шесть заходов): пускать в слой
            // РОДИТЕЛЯ коробку, у которой родитель сам образует содержащий
            // блок (снять вето `!establishes_cb`). Замысел верный —
            // `Spot::fixed_axes` писался под смешанный случай «одна ось от
            // края, другая статическая», и без выноса такой коробке
            // статическую позицию не считает никто (`probe/svpc.html`: y = 30
            // вместо 90). Целевой срез 650 пар (305 зелёных): 258 при ЛЮБОМ
            // гейте — по позиции родителя, по флагу «слой открыт», без
            // табличных видов, только для одной оси. Приобретено 9, и это
            // ровно те пары, которые ждал прежний откат: `abspos-009`,
            // `position-absolute-007`, `right-offset-003`, `abs-pos-non-
            // replaced-vlr-087/089`, `-vrl-086/088/158/164`. Потеряно 46 —
            // `table-anonymous-objects-011..091`: родитель там обычный
            // `position: relative` div (`display: None`, слой открыт), и
            // коробка с ОДНОЙ заданной осью в его слое встаёт не туда, где
            // стояла в потоке. Значит неверно не условие входа, а сама
            // статическая позиция, которую слой считает горизонтальной
            // одноосной коробке. Возвращать вместе с проверкой щупа на
            // `table-anonymous-objects-011` (три абсолюта, у одного задан
            // лишь `top`).
            // `fixed` под трансформом: содержащий блок — ближайший предок,
            // содержащий `fixed` (css-transforms-1 §transform-rendering,
            // css-contain-2 §3.2), а позиционированные между ними — нет
            // (`out-of-flow-in-multicolumn-029`: `fixed` внутри абсолюта
            // внутри трансформа). Родитель-трансформ держит его на месте.
            let tf_fixed = e.style.position == Some(crate::computed::Position::Fixed) && under_tf;
            // Без заданных сторон — тоже: на месте раскладка разрешила бы
            // проценты размеров от РОДИТЕЛЯ (`width: 100%` у абсолютного
            // родителя нулевой ширины, `out-of-flow-in-multicolumn-044`), а
            // статическую позицию по обеим осям даёт щуп.
            let far_fixed = tf_fixed && !fixed_cb_layer_box(inherited);
            let far_abs = !tf_fixed
                && abs_like
                && inherited.cb_ancestor
                && !crate::inline::establishes_cb(inherited)
                && (x_set || y_set);
            let far_abs = far_abs || far_fixed;
            // A negative `z-index` box whose containing block is the ICB goes
            // to the ICB layer too, painted in the bottom layer (`Underlay`,
            // CSS 2.1 §9.9 step 3): in place it was positioned from its
            // parent's box, e.g. a `body` lowered by a collapsed margin
            // (spec-examples `shape-outside-001`: `#failure-container`).
            let below_icb = orphan_abs
                && !fixed
                && e.style.z_index.is_some_and(|z| z < 0)
                && !stacking_context(inherited);
            let to_icb = !ordered_context
                && geometry_layer_ok
                && (fixed || orphan_abs)
                && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
                && !stays_positioned(&nodes[idx + 1..]);
            // В гибком контейнере и сетке слой содержащего блока закрыт: там
            // нет щупа статической позиции. Но при ОБЕИХ заданных осях щуп и не
            // нужен (CSS 2.1 §10.1 п.4 — содержащий блок ближайший
            // позиционированный предок, а не flex/grid-родитель): иначе коробка
            // раскладывалась от родителя (`align-self-with-flex-grid-parent`:
            // розовый квадрат уезжал с `.inner` на 270 px).
            let to_cb = !to_icb
                && (!ordered_context || (x_set && y_set))
                && geometry_layer_ok
                && far_abs
                && e.style.z_index.unwrap_or(0) >= 0
                && !stays_positioned(&nodes[idx + 1..]);
            let built = if to_icb || to_cb {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    fixed_axes: (x_set, y_set),
                    free_margin: (
                        if x_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.left, &e.style).unwrap_or(0.0)
                        },
                        if y_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.top, &e.style).unwrap_or(0.0)
                        },
                    ),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    replaced: matches!(
                        e.tag.as_str(),
                        "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
                    ),
                    ..Default::default()
                });
                let sent = if to_icb && fixed && PAGED.with(|p| p.get()) {
                    // Стопка страниц: фиксированный — в свой слой, по копии
                    // на лист без сдвига (`FIXED_LAYER`). Обёртка `LatePlace`
                    // та же, что у слоя ICB, — через вложенный слой.
                    crate::interact::icb_open();
                    let _ = crate::interact::icb_push(spot.clone(), built);
                    FIXED_LAYER.with(|f| f.borrow_mut().extend(crate::interact::icb_close()));
                    None
                } else {
                    // Слой содержащего блока рисуется после его потока, но
                    // позиционированные красятся в порядке разметки (шаг 8):
                    // ключ ставит коробку слоя среди них. `fixed` в слое ICB —
                    // тоже: без ключа он красился раньше собирателя, и
                    // поднятый в собиратель предок ложился поверх.
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: `fixed` без ключа — кусок 1704
                    // пары +4/−2 (`static-fixed-inside-abspos`,
                    // `position-fixed-001`: 0.00 -> «красное видно»); с
                    // ключом +4/−0.
                    // A positive `z-index` orders the box above the layer's
                    // auto/0 boxes (CSS 2.1 §9.9 steps 8–9): the deferral the
                    // in-place path gets from `layered` below
                    // (`shape-image-009`: `#test` z-index 2 under a z-index 1
                    // failure box, both hoisted).
                    let built = if !fixed && e.style.z_index.is_some_and(|z| z > 0) {
                        layered(built, &e.style, inherited, layer_ok, under_tf)
                    } else {
                        built
                    };
                    let built = if paint_last_ok(e, &nodes[idx + 1..]) {
                        gpui::PaintLast::new(built).key(paint_key).into_any_element()
                    } else {
                        built
                    };
                    let built = if to_icb && below_icb {
                        crate::interact::Underlay::new(built).into_any_element()
                    } else {
                        built
                    };
                    if to_icb {
                        crate::interact::icb_push(spot.clone(), built)
                    } else if far_fixed {
                        crate::interact::cb_push_fixed(spot.clone(), built)
                    } else {
                        crate::interact::cb_push(spot.clone(), built)
                    }
                };
                match sent {
                    None => {
                        // Пустая ось требует щупа: статическую позицию взять
                        // больше неоткуда. При заданных обеих осях на месте
                        // не остаётся ничего.
                        if !(x_set && y_set) {
                            out.push(crate::interact::spot_probe(spot, true));
                        }
                        continue;
                    }
                    Some(kept) => kept,
                }
            } else {
                built
            };
            // Абсолютная коробка с ОТРИЦАТЕЛЬНЫМ `z-index` не идёт ни в слой
            // ICB, ни в верхний слой: её место ПОД потоком (§9.9 шаг 3). Но
            // пустая ось у неё считается от СТАТИЧЕСКОЙ позиции, а гибкая
            // раскладка такой коробке её не даёт и ставит в начало содержимого
            // родителя. Нулевая распорка держит место в потоке, и коробка
            // висит от её угла — там, где написана.
            // Исключение — коробка блочного уровня, у которой задана БЛОЧНАЯ ось
            // (`top`/`bottom`), а свободна строчная, в горизонтальном письме
            // слева направо, и содержащий блок — сам родитель. Статическая
            // позиция по строчной оси здесь — левый край содержимого
            // родителя (§10.3.7), её раскладка на месте даёт и так, а
            // заданную ось §10.6.4 считает от СОДЕРЖАЩЕГО БЛОКА. На распорке
            // `top: 1px` отсчитывался от статической позиции — коробка
            // съезжала под весь поток (`margin-collapse-clear-012..016`:
            // красная подложка `z-index: -1` под жёлтым блоком).
            let below_cb_axis = e.style.position == Some(crate::computed::Position::Absolute)
                && e.style.z_index.is_some_and(|z| z < 0)
                && y_set
                && !x_set
                && !e.inline
                && inherited.rtl != Some(true)
                && inherited.vertical != Some(true)
                && e.style.vertical != Some(true)
                && matches!(
                    inherited.position,
                    Some(crate::computed::Position::Relative)
                        | Some(crate::computed::Position::Absolute)
                )
                && crate::inline::establishes_cb(inherited)
                && !inherited.cb_ancestor
                && !stacking_context(inherited)
                && !ordered_context;
            let below_free_axis = e.style.position == Some(crate::computed::Position::Absolute)
                && e.style.z_index.is_some_and(|z| z < 0)
                && !(x_set && y_set)
                && !below_cb_axis;
            // ЗАМЕРЕНО И ОТКАЧЕНО: уводить в верхний слой ВСЯКУЮ абсолютную
            // коробку с одной свободной осью (§9.9 шаг 8) — по симметрии с
            // `below_free_axis`. Полный свод CSS2: приобретено 3, ПОТЕРЯНО
            // 165 (вся семья `vertical-align-0NN` уходит в «красное видно»,
            // `floats-wrap-bfc-outside-001` 0.08 -> 7.28). Распорка держит
            // место свободной оси только там, где элемент и так вне строки;
            // в абзаце она рвёт строку. Возвращаться только с настоящей
            // статической позицией внутри строки.
            if !ordered_context && (at_static_position(&e.style) || below_free_axis) {
                // Позиционированный элемент рисуется ПОВЕРХ обычного
                // содержимого (CSS 2.1 §9.9, шаг 8) и без заданного `z-index`:
                // без верхнего слоя следующий за ним сосед закрашивал его
                // собой — блок стоял на месте, но был не виден (проба:
                // абсолютный кусок между «AA» и «BB» пропадал целиком, хотя
                // один в блоке рисовался верно).
                //
                // Позиционированный элемент рисуется ПОВЕРХ обычного
                // содержимого (CSS 2.1 §9.9, шаг 8), а порядок отрисовки у нас
                // — порядок детей. Отложенная отрисовка тут не работает ни в
                // каком виде (пробовали трижды: css-position 31 → 0, css-text
                // 966 → 810, падение процесса), поэтому содержимое уходит
                // ПОСЛЕДНИМ ребёнком родителя, а на своём месте остаётся
                // нулевая распорка с холстом-щупом. Разницу их положений
                // элемент забирает отрицательным полем — так он оказывается
                // там же, где был, но рисуется последним.
                // Отрицательный `z-index` рисуется ПОД содержимым потока
                // (CSS 2.1 §9.9, шаг 3), поэтому в верхний слой он не идёт:
                // там его место — поверх всего.
                let below = e.style.z_index.is_some_and(|z| z < 0);
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    replaced: matches!(
                        e.tag.as_str(),
                        "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
                    ),
                    line_align: static_line_align(e, inherited),
                    self_align: static_self_align(e, inherited),
                    ..Default::default()
                });
                let probe = crate::interact::spot_probe(spot.clone(), true);
                // Поля сдвигают абсолютный элемент ОТ статической позиции
                // (CSS 2.1 §10.3.7: auto-края = static + margin). Раскладка
                // под нами поля у absolute без краёв не считает — сдвиг
                // даёт absolute-обёртка (clip-path-rectangle-ref и родня:
                // эталонный зелёный стоял без своих margin: 50px).
                let ml = margin_px(e.style.margin.left, &e.style).unwrap_or(0.0);
                let mt = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
                // Под потоком (`below`) коробка остаётся абсолютной на месте
                // распорки, и поле от статической позиции ей уже даёт сама
                // раскладка (taffy: `static_position + margin`); обёртка
                // прибавляла его второй раз (`tab-size-inheritance-001`:
                // красная подложка на 50 точек правее).
                // Обёртка — содержащий блок коробки для раскладки, и её ширина
                // — доступная ширина shrink-to-fit (CSS 2.1 §10.3.7: ширина
                // содержащего блока минус статическое смещение и поля;
                // Blink `absolute_utils.cc` ComputeAbsoluteInlineSize берёт
                // `available_size` от края до края содержащего блока). Без
                // правого края обёртка была нулевой ширины, и `<h1>` с
                // полями по умолчанию ломался после каждого слова
                // (min-content). Правый край — только при ltr в
                // горизонтальном письме: rtl ставит коробку от правого края
                // обёртки, вертикальный заместитель нулевой и так.
                let mr = margin_px(e.style.margin.right, &e.style).unwrap_or(0.0);
                let stretch = inherited.rtl != Some(true) && inherited.vertical != Some(true);
                let built = if (ml != 0.0 || mt != 0.0) && !below {
                    let wrap = div().absolute().left(px(ml)).top(px(mt));
                    let wrap = if stretch { wrap.right(px(mr)) } else { wrap };
                    wrap.child(built).into_any_element()
                } else {
                    built
                };
                // A positive `z-index` orders the box above the auto/0
                // positioned boxes (CSS 2.1 §9.9 steps 8–9), as on the
                // CB-layer path above: `scalex` — a static-position abspos
                // with `z-index: 11` painted under its `z-index: 10` sibling.
                let built = if !below && e.style.z_index.is_some_and(|z| z > 0) {
                    layered(built, &e.style, inherited, layer_ok, under_tf)
                } else {
                    built
                };
                // Абсолют на статической позиции — тоже шаг 8: в собирателе
                // он встаёт среди позиционированных по ключу, а не поверх
                // всех соседей контейнера.
                let taken = if below {
                    Some(built)
                } else if paint_last_ok(e, &nodes[idx + 1..]) {
                    crate::interact::late_push(
                        spot,
                        gpui::PaintLast::new(built).key(paint_key).into_any_element(),
                    )
                } else {
                    crate::interact::late_push(spot, built)
                };
                match taken {
                    None => out.push(probe),
                    Some(kept) => {
                        // ЗАМЕРЕНО И ОТКАЧЕНО: заворачивать `kept` в
                        // `Underlay`, чтобы коробка с отрицательным `z-index`
                        // легла ПОД поток (§9.9 шаг 3) — срез из 259 пар семей
                        // *shape*: 0 и 0, НИ ОДНО число не сдвинулось.
                        // Подложка порядок не меняет: нижним слоям сцена даёт
                        // общий номер, а сортировка устойчива. Тройку
                        // `spec-examples/shape-outside-004…006` сломал коммит
                        // `05b7a4f` (28.08, гейт `x_set && y_set` в `movable`),
                        // и возвращать её надо порядком краски, а не слоем.
                        // Отрицательный `z-index` принадлежит БЛИЖАЙШЕМУ контексту
                        // наложения (CSS 2.1 прил. E, шаг 3), а позиционированный
                        // родитель с `z-index: auto` его не образует: ребёнок
                        // обязан лечь ПОД его фон (шаг 8 родителя выше шага 3
                        // корня). Держатель на месте красил ребёнка после фона
                        // родителя — красный поверх зелёного
                        // (`z-index-abspos-001`). Подложка — тот же приём, что у
                        // относительного с `z<0` ниже по функции. Прежний замер
                        // обёртки (запись выше, 259 пар *shape*: 0/0) шёл без
                        // гейта по родителю; `!cb_ancestor` держит правку там,
                        // где контекст наверняка корневой (у `fixed-pos-stacking-001`
                        // выше стоит `fixed` — он контекст, туда не заходим).
                        let kept = if below
                            && matches!(
                                inherited.position,
                                Some(crate::computed::Position::Relative)
                                    | Some(crate::computed::Position::Absolute)
                            )
                            && !inherited.cb_ancestor
                            && !stacking_context(inherited)
                        {
                            crate::interact::Underlay::new(kept).into_any_element()
                        } else {
                            kept
                        };
                        let contiguous = below && out.len() == below_run_end;
                        out.push(
                            div()
                                .relative()
                                .w_full()
                                .h_0()
                                .flex_shrink_0()
                                .child(kept)
                                .into_any_element(),
                        );
                        if below {
                            if !contiguous {
                                below_run_start = out.len() - 1;
                                below_zs.clear();
                            }
                            below_zs.push(e.style.z_index.unwrap_or(0));
                            let mut at = below_zs.len() - 1;
                            while at > 0 && below_zs[at - 1] > below_zs[at] {
                                below_zs.swap(at - 1, at);
                                out.swap(below_run_start + at - 1, below_run_start + at);
                                at -= 1;
                            }
                            below_run_end = out.len();
                        }
                    }
                }
                continue;
            }
            let _ = hoist_margins;
            // Замещаемому дорожка по содержимому не нужна: его размер по
            // ключевому слову — природный, считается в `image_with`.
            let layered_built = layered(built, &e.style, inherited, layer_ok, under_tf);
            let mut done = content_wrapper::for_element(layered_built, e, inherited, placement);
            // Тело под корнем-донором фона холста при `vertical-rl`: записать
            // левый край коробки корня для отрисовки холста (см. `canvas_paint`).
            // Корень по содержимому = margin-box тела плюс рамка и отбивка
            // корня слева.
            if record_root {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let root_b = inherited.borders();
                let offset = side(e.style.margin.left) + side(inherited.padding.left) + side(root_b.left);
                done = crate::interact::record_root_left(done, opts.doc_salt, offset);
            }
            // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
            // flow): свой анкор-ряд вокруг ОДНОГО узла — соседей не трогает.
            // Корню с фоном-картинкой не ставится (гасил canvas-слой).
            // Прижим — свойство ГЛАВНОГО потока, а он на документ один. Если
            // обособление на `html` или на `body` погасило распространение
            // письма тела в область просмотра (css-contain-2
            // §containment-types), главным потоком тело не стало: оно
            // остаётся обычным блоком в потоке горизонтального корня и к
            // правому краю окна не жмётся
            // (contain-body-w-m-001..004, contain-html-w-m-001..004).
            if matches!(e.tag.as_str(), "html" | "body")
                && e.style.vertical_rl == Some(true)
                && !e.style.wm_contained
            {
                if e.style.bg_image.is_none() {
                    done = div()
                        .w_full()
                        .flex()
                        .justify_end()
                        .child(done)
                        .into_any_element();
                } else if let Some(Len::Px(w)) = e.style.width {
                    // Корню с фоном-картинкой флекс-обёртка гасила слой
                    // краски — прижим вправо считается сдвигом по известной
                    // ширине (background-size-document-root-vrl-*).
                    let shift = (opts.viewport.0 - w).max(0.0);
                    if shift > 0.0 {
                        done = div().ml(px(shift)).child(done).into_any_element();
                    }
                }
            }
            // Релятивный элемент с отрицательным `z-index`: место в потоке —
            // своё, краска — под содержимым до него (CSS 2.1 §9.9, шаг 3).
            // Расширение на элементы сетки без `position` ЗАМЕРЕНО В МИНУС
            // (display-inline-grid 0.08 -> 8.20, inline-z-axis-002/004) —
            // подложка в строчной сетке рвёт свою же краску.
            // `<body>` исключён: его родитель — корневой элемент, а тот
            // всегда образует КОРНЕВОЙ контекст наложения. Шаги 1-2
            // приложения E — собственные фон и рамка корня, шаг 3 —
            // отрицательный `z-index` ПОВЕРХ них, а не под всем окном; братьев
            // у `<body>` нет, уходить не подо что
            // (`root-element-creates-stacking-context`).
            // Родитель — СВОЙ контекст наложения (transform, opacity < 1,
            // позиционированный с z-index, isolation, filter; CSS 2.1 прил. E,
            // css-transforms-1 §transform-rendering): отрицательный z-index
            // ребёнка ложится под его содержимое, но ПОВЕРХ его фона — не под
            // весь документ (`individual-transform/stacking-context-00*`,
            // `transform-stacking-001`).
            if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
                && e.tag != "body"
                && !stacking_context(inherited)
            {
                done = crate::interact::Underlay::new(done).into_any_element();
            }
            // Абсолют с `z-index < 0`, оставленный на месте (`below_cb_axis`):
            // краска — шаг 3 корневого контекста, под потоком родителя, как и
            // у держателя на распорке выше.
            if below_cb_axis {
                done = crate::interact::Underlay::new(done).into_any_element();
            }
            // Позиционированный блок с `z-index: auto | 0` рисуется на шаге 8
            // приложения E CSS 2.1 — ПОСЛЕ блоков и строк потока, — а у нас
            // порядок краски был порядком детей: следующий блок закрашивал
            // сдвинутый `relative` (`position-relative-035`) и абсолют с одной
            // свободной осью (`right-offset-003`). `PaintLast` меняет только
            // краску — раскладка и место в потоке те же, поэтому строки он не
            // рвёт (запись про распорку выше): внутри строки элементы идут
            // через `pending`, а не сюда. Положительный `z-index` и `fixed`
            // уже отложены `layered`, отрицательный — подложка выше.
            let step8 = matches!(
                e.style.position,
                Some(crate::computed::Position::Relative)
                    | Some(crate::computed::Position::Absolute)
                    | Some(crate::computed::Position::Sticky)
            ) && e.style.z_index.unwrap_or(0) == 0
                && !matches!(e.tag.as_str(), "html" | "body")
                && paint_last_ok(e, &nodes[idx + 1..]);
            // Непозиционированный элемент с `opacity` < 1 красится на том же слое, что
            // позиционированные с `z-index: 0` (css-color-4 §opacity: «painted
            // on the same layer … as positioned elements with stacking order
            // 0»; Blink кладёт такой слой в список z-порядка с нулём): после
            // блоков и строк потока, в порядке разметки (`t32-opacity-zorder-c`).
            let step8 = step8
                || (e.style.position.is_none_or(|p| p == crate::computed::Position::Static)
                    // `z-index` у непозиционированного не действует
                    // (CSS 2.1 §9.9.1 «Applies to: positioned elements»).
                    && (e.style.z_index.unwrap_or(0) == 0
                        || !z_index_applies(&e.style, inherited))
                    // Только прозрачность: у `contain`/`will-change`/
                    // `transform` положительный `z-index` потомков держится
                    // на краске на месте (`contain-paint-stacking-context-*`).
                    && (e.style.opacity.is_some_and(|o| o < 1.0)
                        // css-transforms-1 §transform-rendering: a transformed
                        // box establishes a stacking context and is painted
                        // as a positioned `z-index: 0` layer (Blink puts it in
                        // the z-order list with 0): `perspective-zero` — a
                        // static transformed box after a `relative` one.
                        || e.style.transform.is_some()
                        || e.style.translate.is_some())
                    && !e.style.z_index.is_some_and(|z| z > 0 && z_index_applies(&e.style, inherited))
                    && !matches!(e.tag.as_str(), "html" | "body")
                    && paint_last_ok(e, &nodes[idx + 1..]));
            if step8 {
                done = gpui::PaintLast::new(done).key(paint_key).into_any_element();
            }
            out.push(done);
        }
    }
    if !pending.is_empty() {
        out.push(paint_inline_step7(letter_scope.paragraph(&pending, inherited, opts)));
    }
    // `text-box-trim` (css-inline-3 §4.2): у блочного контейнера срезается
    // блочно-начальная сторона ПЕРВОЙ отформатированной строки и
    // блочно-конечная — ПОСЛЕДНЕЙ. Выражается отрицательным полем на первом и
    // последнем ребёнке: коробка ужимается ровно на срез, а содержимое
    // остаётся на месте.
    //
    // Строку ищет `text_box_line_style` по css-pseudo-4: у контейнера с
    // блочным содержимым это первая строка ПЕРВОГО in-flow блочного ребёнка,
    // и если у того строки нет (пустой `<div>`, пустая анонимная коробка) —
    // срезать нечего (`half-leading-block-box-001/003`). «Intervening
    // non-zero padding or borders» — отступы и рамки ПОТОМКОВ между
    // контейнером и строкой (`-004/-005`), а не самого контейнера: его
    // собственный отступ срезу не мешает (`-006`). Метрики — от корневой
    // строчной коробки найденной строки, то есть от стиля её блока.
    if (inherited.text_box_trim_start || inherited.text_box_trim_end)
        && !out.is_empty()
        && !ordered_context
    {
        // Срез с одной стороны: полулидинг строки плюс расстояние от
        // подъёма/спуска до заданной метрики края (`text` — ноль, `cap`/`ex`
        // — остаток над прописной/строчной, `alphabetic` — весь спуск).
        let trim_for = |line_style: &Computed, start: bool| -> f32 {
            let size = match line_style.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            let family = line_style.font_family.clone().unwrap_or_default();
            let (ascent, descent, cap) = crate::metrics::vmetrics_px(&family, size);
            let line = match line_style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => ascent + descent,
            };
            // Полулидинг — половина разницы между высотой строки и метрикой
            // содержимого (CSS 2.1 §10.8.1).
            let half = (line - (ascent + descent)) / 2.0;
            // Край — у КОРНЕВОЙ СТРОЧНОЙ КОРОБКИ найденной строки (css-inline-3
            // §text-box-trim: «to the specified metric of its root inline
            // box»): `text-box-edge` наследуемое, `inline::inherit` его несёт,
            // а явное `auto` на блоке строки перекрывает `ex` контейнера
            // (`not-ignore-nested-text-box-edge`; Blink `AdjustEdges`:
            // kAuto = kText).
            if start {
                let over = match line_style.text_box_over {
                    crate::computed::TextEdge::Cap => ascent - cap,
                    crate::computed::TextEdge::Ex => {
                        ascent - crate::metrics::ch_ex_px(&family, size).1
                    }
                    _ => 0.0,
                };
                half + over
            } else {
                let under = match line_style.text_box_under {
                    // Алфавитная линия может стоять НАД нулём глифа (BASE `romn`,
                    // `BaselineDiagnostic`: +50/1000 — `text-box-trim-end-002`).
                    crate::computed::TextEdge::Alphabetic => {
                        descent + crate::fonts::alphabetic_em(&family) * size
                    }
                    _ => 0.0,
                };
                half + under
            }
        };
        if inherited.text_box_trim_start
            && let Some(line_style) = text_box_line_style(nodes, inherited, true)
        {
            let trim = trim_for(&line_style, true);
            if trim > 0.0 {
                let first = out.remove(0);
                // Вертикальный блок кладёт детей рядом (`flex_row` у
                // `vertical-lr`, `flex_row_reverse` у `vertical-rl`): блок-старт —
                // левый или правый край, верхнее поле двигало строку ВДОЛЬ неё
                // (`text-box-trim-half-leading-block-box-002`).
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mt(px(-trim)),
                    (true, false) => div().ml(px(-trim)),
                    (true, true) => div().mr(px(-trim)),
                };
                out.insert(0, holder.child(first).into_any_element());
            }
        }
        if inherited.text_box_trim_end
            && let Some(line_style) = text_box_line_style(nodes, inherited, false)
        {
            let trim = trim_for(&line_style, false);
            if trim > 0.0 {
                let last = out.pop().expect("список не пуст");
                // Блок-конец вертикали: правый край у `vertical-lr`, левый у
                // `vertical-rl`.
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mb(px(-trim)),
                    (true, false) => div().mr(px(-trim)),
                    (true, true) => div().ml(px(-trim)),
                };
                out.push(holder.child(last).into_any_element());
            }
        }
    }
    // Верхний слой: то, что обязано рисоваться поверх соседей, идёт последним
    // и возвращается на своё место замеренным сдвигом.
    out.extend(crate::interact::late_close());
    containment_paint::collect(out, inherited)
}

/// `order`: визуальный порядок в гибкой строке.
///
/// Раскладка под нами это свойство не знает, поэтому детей переставляем сами.
/// Сортировка устойчивая — элементы с равным `order` сохраняют порядок
/// разметки, как того требует CSS.
pub(crate) fn reorder(mut nodes: Vec<Node>) -> Vec<Node> {
    let ordered = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.order.is_some(),
        Node::Text(_) => false,
    });
    if !ordered {
        return nodes;
    }
    // Анонимный элемент — непрерывный прогон текста В ПОРЯДКЕ РАЗМЕТКИ
    // (css-flexbox-1 §4), и `order` переставляет уже готовые элементы. После
    // сортировки прогоны «a a» и «b b» по обе стороны от `order: 1`
    // становились соседями и склеивались в один абзац
    // (`flexbox-anonymous-items-001`). Поэтому при двух и более непустых
    // прогонах каждый заворачивается в свой анонимный блок заранее.
    let blank = |t: &str| blank_text(t);
    let mut runs: Vec<Vec<Node>> = vec![];
    let mut cur: Vec<Node> = vec![];
    let mut rest: Vec<(usize, Node)> = vec![];
    for n in nodes.drain(..) {
        match n {
            Node::Text(_) => cur.push(n),
            other => {
                if !cur.is_empty() {
                    runs.push(std::mem::take(&mut cur));
                    rest.push((usize::MAX, Node::Text(String::new())));
                }
                rest.push((0, other));
            }
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
        rest.push((usize::MAX, Node::Text(String::new())));
    }
    let solid = runs
        .iter()
        .filter(|r| r.iter().any(|n| matches!(n, Node::Text(t) if !blank(t))))
        .count();
    let mut run_iter = runs.into_iter();
    for (tag, n) in rest {
        if tag != usize::MAX {
            nodes.push(n);
            continue;
        }
        let run = run_iter.next().unwrap_or_default();
        let has_text = run.iter().any(|n| matches!(n, Node::Text(t) if !blank(t)));
        if solid >= 2 && has_text {
            nodes.push(Node::Element(anon_element("div", run)));
        } else {
            nodes.extend(run);
        }
    }
    // css-flexbox-1 §5.4 (и css-grid-2 §9.1 по ссылке): «Absolutely-
    // positioned children of a flex container are treated as having
    // order: 0 for the purpose of determining their painting order relative
    // to flex items» — внепоточный ребёнок `order` не слушает, и сортировка
    // оставляет его в порядке разметки среди элементов с нулём
    // (`flexbox-paint-ordering-003`).
    nodes.sort_by_key(|n| match n {
        Node::Element(e)
            if matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) =>
        {
            0
        }
        Node::Element(e) => e.style.order.unwrap_or(0),
        Node::Text(_) => 0,
    });
    nodes
}

/// Схлопнуть отступы соседей вдоль горизонтальной оси потока.
///
/// Между двумя блоками остаётся больший из смежных отступов, а не их сумма.
/// Раскладка их складывает, поэтому у второго и следующих соседей ведущий
/// отступ уменьшается на уже занятый предыдущим.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v145, `scout-wm-2026-09e.md` E1):
    // предел §7.3.2 для ортогонального ребёнка с ЯВНЫМ `width: auto`
    // (гейт `width.is_none()` не видел `Len::Auto`) плюс потолок вместо
    // жёсткой ширины. Срез css-writing-modes+css-masking+css-shapes+
    // filter-effects+css-borders+css-text+CSS2 9052: +1 при −2 —
    // `three-levels-of-orthogonal-flows` 0.00 → 16.96,
    // `two-levels-of-orthogonal-flows-fixed` 0.09 → 2.91. Вложенные
    // ортогональные потоки считают предел от НЕПРАВИЛЬНОГО предка —
    // сначала нужен настоящий поиск ближайшего параллельного контейнера.
/// Доли полей и отступов блочных детей — от СТРОЧНОГО размера содержащего
/// блока (css-writing-modes-4 §7.2, Overview.bs:2018-2021: «percentages on
/// the margin and padding properties … are calculated with respect to the
/// inline size of the containing block»). Раскладка под нами (taffy,
/// `compute/flexbox.rs:187/447/723`) берёт базой ВСЕГДА ширину родителя: у
/// вертикального контейнера это блочный размер, при `width: auto` ещё и
/// неопределённый — доли выходили нулём (`percent-padding-vrl-004/006`,
/// `percent-margin-vrl-004`: по расчёту коробки 50×50 вместо 100×70 и
/// 100×139). Переводим в
/// точки ДО схлопывания: `collapse_flow_margins` читает долю через
/// `margin_px` от ФИЗИЧЕСКОЙ ширины.
/// `vertical` — письмо контейнера: база — его высота; у горизонтального
/// контейнера переводятся только вертикальные дети (их проценты taffy считал
/// на проходе с неопределённой шириной — `percent-padding-vrl-002`, отступы
/// сверху/снизу нулём), база — ширина. Сетка исключена: там содержащий блок —
/// область сетки, а не контейнер (css-grid-2, Overview.bs:718: «A grid item’s
/// grid area forms the containing block into which it is laid out»).
pub(crate) fn resolve_inline_pct(mut children: Vec<Node>, container: &Computed, vertical: bool) -> Vec<Node> {
    if matches!(
        container.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return children;
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let b = container.borders();
    let (size, edges) = if vertical {
        (
            container.height,
            [b.top, b.bottom, container.padding.top, container.padding.bottom],
        )
    } else {
        (
            container.width,
            [b.left, b.right, container.padding.left, container.padding.right],
        )
    };
    let Some(mut base) = px(size) else {
        return children;
    };
    // Строчный размер СБ — его content-box: при `border-box` заданная
    // величина включает рамки и отступы по той же оси.
    if container.border_box == Some(true) {
        base -= edges.iter().map(|l| px(*l).unwrap_or(0.0)).sum::<f32>();
    }
    let base = base.max(0.0);
    for node in children.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || !in_flow(&ch.style) {
            continue;
        }
        if !vertical && ch.style.vertical != Some(true) {
            continue;
        }
        let s = &mut ch.style;
        for side in [
            &mut s.margin.top,
            &mut s.margin.right,
            &mut s.margin.bottom,
            &mut s.margin.left,
            &mut s.padding.top,
            &mut s.padding.right,
            &mut s.padding.bottom,
            &mut s.padding.left,
        ] {
            if let Some(Len::Pct(k)) = side {
                *side = Some(Len::Px(*k * base));
            }
        }
    }
    children
}


/// Зеркальный ортогональный случай: ВЕРТИКАЛЬНЫЙ блок внутри горизонтального
/// контейнера. Его строчная ось — высота, и авто-размер по ней зажимается
/// высотой контейнера за вычетом вертикальных полей (css-writing-modes-3
/// §7.3). Доля полей здесь обычная — от ширины контейнера, её решает
/// раскладка сама.
pub(crate) fn orthogonal_vertical_children(children: Vec<Node>, container: &Computed) -> Vec<Node> {
    let has_vertical = children.iter().any(|n| match n {
        Node::Element(ch) => !ch.inline && ch.style.vertical == Some(true),
        _ => false,
    });
    if !has_vertical {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(true) {
            continue;
        }
        if !in_flow(&ch.style) {
            continue;
        }
        // Элемент СЕТКИ с невытягивающим выравниванием: по строчной оси
        // (у него вертикальной) он размером в содержимое, а не в область
        // (css-grid-1 §6.6 вместе с css-align-3 §6.1 — `stretch` растягивает,
        // остальное нет). Пока вертикальный абзац брал весь предел
        // ортогонального потока, эталоны `orthogonal-positioned-grid-items-*`
        // (`place-items: start`) вылезали за сетку на всю высоту окна.
        if matches!(
            container.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) && matches!(
            ch.style.align_self.or(container.align_items),
            Some(Align::Start) | Some(Align::Center) | Some(Align::End) | Some(Align::Baseline)
        ) {
            ch.style.hug_inline = true;
        }
        // Корень с vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
        // flow). Прижим самим стилем корня (align-self) — контейнеры-колонки
        // его уважают; для корня с ФОНОМ-КАРТИНКОЙ якорь ранее гасил
        // canvas-слой — тем страницам якорь не ставится (замерено).
        if matches!(ch.tag.as_str(), "html" | "body")
            && ch.style.vertical_rl == Some(true)
            && ch.style.align_self.is_none()
            && ch.style.bg_image.is_none()
        {
            ch.style.align_self = Some(crate::computed::Align::End);
        }
        // Ordinary orthogonal blocks now compute their own used inline size.
        if orthogonal_fixed_child::normal_block_flow(&ch.style, container) {
            ch.style.flex_shrink = Some(0.0);
            continue;
        }
        if ch.style.height.is_some() || ch.style.max_height.is_some() {
            continue;
        }
        let margin = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) => match container.width {
                Some(Len::Px(w)) => k * w,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let margins = margin(ch.style.margin.top) + margin(ch.style.margin.bottom);
        ch.style.max_height = Some(match container.height {
            Some(Len::Px(h)) => Len::Px((h - margins).max(0.0)),
            _ => Len::Pct(1.0),
        });
        // Тот же предел — и ПЕРЕНОСУ строк. Авто-строчный размер ортогонального
        // блока — shrink-to-fit к размеру, что «would stretch fit into … the
        // containing block’s size if that is fixed» (css-writing-modes-4
        // §7.3.2, Overview.bs:2175-2183), а stretch-fit вычитает поля ребёнка.
        // Blink: `length_utils.cc:117-146` (`kFitContent` →
        // `ShrinkToFit(available_size - margins.InlineSum())`), авто-длина
        // ортогонального ребёнка — `FitContent` (`length_utils.cc:555-569`).
        // `max_height` выше этого не даёт: вето `element()` «предок уже дал
        // предел» у вертикального блока без своей `height` оставляет предел
        // КОНТЕЙНЕРА, и текст переносился по 200 вместо 200 − 2·50
        // (`sizing-orthogonal-percentage-margin-001/002`: эталон с `height:
        // 100px` после сужения вето переносит по 100, тест — по 200).
        // Свой `ortho_limit` перебивает унаследованный при слиянии
        // (`inline.rs`: `own.ortho_limit.or(parent.ortho_limit)`); рамки и
        // отбивки ребёнка из него вычтет `element()`. Только блочный
        // контейнер (у сетки/гибкого содержащий блок другой) и только при
        // ненулевых полях — без них предел равен унаследованному.
        if let Some(Len::Px(h)) = container.height
            && margins > 0.0
            && matches!(container.display, None | Some(Display::Block))
        {
            let px = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            // Предел — внутренний размер СБ: при `border-box` заданная
            // высота включает его рамки и отбивки по той же оси.
            let inner = if container.border_box == Some(true) {
                let b = container.borders();
                h - px(b.top)
                    - px(b.bottom)
                    - px(container.padding.top)
                    - px(container.padding.bottom)
            } else {
                h
            };
            ch.style.ortho_limit = Some((inner - margins).max(0.0));
        }
    }
    out
}

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
pub(crate) fn collapse_flow_margins(children: Vec<Node>, reverse: bool, lead: Option<f32>) -> Vec<Node> {
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
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
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
    let mut out = children;
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
                Some(crate::computed::Position::Absolute)
                    | Some(crate::computed::Position::Fixed)
            )
            && matches!(
                e.style.overflow_x,
                None | Some(crate::computed::Overflow::Visible)
            )
            && matches!(
                e.style.overflow_y,
                None | Some(crate::computed::Overflow::Visible)
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
    let mut trailing: Option<f32> = lead.filter(|m| *m >= 0.0);
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
            || matches!(child.style.position, Some(crate::computed::Position::Absolute | crate::computed::Position::Fixed))
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
                None | Some(crate::computed::Overflow::Visible)
            )
            || !matches!(
                child.style.overflow_y,
                None | Some(crate::computed::Overflow::Visible)
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
    out
}

/// Блок вертикального письма занимает по горизонтали столько, сколько просит
/// содержимое, а не всю строку родителя.
///
/// Горизонтальная ось для него — ось ПОТОКА, а не строки: в Chrome контейнер
/// из трёх полос шириной 22 с отступами 16 занял 130 точек, а не всю ширину
/// окна. Блочная раскладка растягивает детей по ширине и слушать `align-self`
/// не обязана, поэтому обёртка-ряд: сам ряд занимает строку, а блок внутри
/// него жмётся к содержимому. Без этого колонки `vertical-rl` уезжали к
/// правому краю окна.
///
/// Приём этот — целиком про БЛОЧНЫЙ контейнер. css-writing-modes-4 §7.3 делит
/// раскладку ортогональной коробки надвое и про вторую половину говорит: «In
/// the positioning phase—calculating the positioning offsets, margins, borders,
/// and padding—…calculations are performed according to the writing mode of the
/// *containing block* of the box establishing the orthogonal flow»; §7.4
/// повторяет то же про «any properties related to positioning the box within
/// its containing block». А §7.3.3 включает подбор по содержимому только «when
/// the available inline space is infinite» — у элемента сетки и гибкого
/// элемента место определённое, и подбирать нечего.
pub(crate) fn vertical_hug(el: AnyElement, e: &Element, inherited: &Computed) -> AnyElement {
    let starts_here = e.style.vertical == Some(true) && inherited.vertical != Some(true);
    if !starts_here || e.style.width.is_some() {
        return el;
    }
    // Письмо, заданное на `body` (или `html`), — ГЛАВНОЕ письмо страницы: оно
    // управляет окном целиком, и содержимое `vertical-rl` начинается от
    // правого края окна, а не от края сжатой коробки.
    if matches!(e.tag.as_str(), "body" | "html") {
        return el;
    }
    // An absolutely positioned box is out of flow and is placed against the
    // padding box of its containing block (CSS 2.1 §10.1, css-position-3
    // §4). The in-flow hug row sits at the parent's CONTENT edge and became
    // the box's layout parent, so `top: 0; left: 0` landed inside the padding
    // (`available-size-001`: the vertical-rl `#red` 1ch below the green 0).
    if matches!(
        e.style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        return el;
    }
    // Элемент СЕТКИ и ГИБКИЙ элемент размер по блочной оси не подбирают: его
    // задаёт выравнивание, и решается оно письмом КОНТЕЙНЕРА (§7.3,
    // «positioning phase … according to the writing mode of the containing
    // block»). Blink выбирает пару свойств прямо по письму сетки
    // (`grid/grid_item.cc`: `is_for_columns == IsParallelWritingMode(
    // root_grid_writing_direction…, parent_grid_style…) ? ResolvedJustifySelf
    // : ResolvedAlignSelf`), а `normal` у незамещаемой коробки даёт
    // `kStretchImplicit` -> `Length::Stretch()` (`length_utils.cc`), то есть
    // размер дорожки: ортогональность тут не при чём вовсе.
    //
    // Обёртка-ряд эту развязку ломала дважды. В сетке элементом дорожки
    // становилась ОНА, а блок внутри обнимал содержимое: снимок
    // `orthogonal-positioned-grid-items-009--ref` — magenta (29..55, 103..289)
    // px снимка = 21.6x150 CSS вместо дорожки 200x150 (тот же прямоугольник на
    // ТЕСТЕ, где обёртки нет, отрисован точно: 29..278). В гибком контейнере
    // обёртка становилась гибким элементом, а `flex:` оставалось на коробке
    // ВНУТРИ неё, и ряд не рос: снимок `flexbox-mbp-horiz-002v` — жёлтый
    // (13..29) = 12.8 CSS против 193 у эталона.
    //
    // Строчная ось у такой коробки уже разведена по выравниванию в
    // `orthogonal_vertical_children` (`hug_inline` при НЕрастягивающем
    // `align-self`); это — недостающая парная развязка блочной оси, и после
    // снятия обёртки её ведёт сама раскладка по `justify-self`/`justify-items`
    // (`apply.rs`; `align_keyword` отдаёт `None` на `normal`, и решает taffy).
    //
    // Абсолютный ребёнок сетки элементом сетки НЕ является (css-grid-1 §9) —
    // его размер задают вставки, обёртка ему и не мешала; `in_flow` его здесь
    // и отсекает. Лунки (`GridLanes`) не включены сознательно: у них свой
    // второй проход дорожек и 23 зелёные пары, это отдельный замер.
    if matches!(
        inherited.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::Flex)
            | Some(Display::InlineFlex)
    ) && in_flow(&e.style)
    {
        return el;
    }
    // ЗАМЕРЕНО (не гипотеза): по этим тестам размер по оси строки — не корень
    // зла. Пробовал и растяжение на высоту родителя, и свой элемент-измеритель
    // (`fit-content` с зажимом по доступному) — на двенадцати тестах
    // ортогональных потоков сдвиг в пределах полупроцента. Настоящая поломка
    // видна замером против Chrome на `sizing-orthogonal-percentage-margin-001`:
    // элемент рисуется ПОЛОСОЙ 25×417 у левого края страницы, а должен быть
    // 100×100 внутри контейнера с полями 50. То есть вертикальный блок уходит
    // из коробки родителя — вот что чинить дальше.
    // The row is an adapter for normal block flow, not a CSS flex item.
    // Its own shrink default must not compress the child's used inline size:
    // sizing-orthog-vlr-in-htb-007 measured 306px instead of 394px content.
    div().flex().flex_row().flex_shrink_0().child(el).into_any_element()
}

/// Стиль блока, которому принадлежит первая (`start`) или последняя
/// отформатированная строка контейнера — для `text-box-trim`
/// (css-inline-3 §4.2, css-pseudo-4 «first formatted line»).
///
/// `None` — такой строки нет или до неё стоит отступ либо рамка: срезать
/// нечего. Контейнер со строчным содержимым отдаёт СВОЙ стиль; с блочным —
/// спускается в первый/последний in-flow блочный ребёнок (плавающие и
/// абсолютные в потоке не участвуют, пробельные узлы пропускаются). Пустой
/// блок, пустая анонимная коробка (пробельный `<span>` среди блоков —
/// `half-leading-block-box-001/003`), гибкий, сеточный и табличный контекст
/// обрывают поиск; отступ и рамка ПОТОМКА на срезаемой стороне — тоже
/// (`-004/-005`). Строчный элемент с блоком внутри (блок-в-строчном)
/// прозрачен: строка — в его блоке (`block-in-inline-*`).
pub(crate) fn text_box_line_style(nodes: &[Node], inherited: &Computed, start: bool) -> Option<Computed> {
    let has_block = nodes.iter().any(breaks_inline);
    let order: Vec<&Node> = if start {
        nodes.iter().collect()
    } else {
        nodes.iter().rev().collect()
    };
    for (i, n) in order.iter().enumerate() {
        let e = match n {
            Node::Text(t) if blank_text(t) => continue,
            // Непробельный текст — строка в этом контейнере.
            Node::Text(_) => return Some(inherited.clone()),
            Node::Element(e) => e,
        };
        if e.style.display == Some(Display::None) || out_of_flow(&e.style) {
            continue;
        }
        if e.style.display == Some(Display::Contents) {
            return text_box_line_style(&e.children, &inline::inherit(inherited, &e.style), start);
        }
        if breaks_inline(n) {
            // Через гибкий, сеточный и табличный контекст свойство не
            // распространяется (css-inline-3 §4.2, примечание).
            if matches!(
                e.style.display,
                Some(Display::Flex)
                    | Some(Display::Grid)
                    | Some(Display::Table)
                    | Some(Display::GridLanes)
            ) {
                return None;
            }
            let merged = inline::inherit(inherited, &e.style);
            let px_of = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let bs = merged.borders();
            let blocked = if start {
                px_of(merged.padding.top) != 0.0 || px_of(bs.top) != 0.0
            } else {
                px_of(merged.padding.bottom) != 0.0 || px_of(bs.bottom) != 0.0
            };
            if blocked {
                return None;
            }
            // Потомок, который САМ срезает эту сторону, срежет ту же строку в
            // своём хвосте `blocks()`: срез «до метрики» идемпотентен, второй
            // раз его не кладут (`ul, li { text-box-trim: trim-both }` —
            // `text-box-trim-list-001`). `merged` несёт собственные флаги
            // ребёнка: `inherit()` клонирует его стиль, а свойство не
            // наследуется.
            let own_trim = if start {
                merged.text_box_trim_start
            } else {
                merged.text_box_trim_end
            };
            if own_trim {
                return None;
            }
            return text_box_line_style(&e.children, &merged, start);
        }
        // Блок-в-строчном: оболочка прозрачна, строка — внутри блока.
        if real_inline(e) && contains_block(&e.children) {
            return text_box_line_style(&e.children, inherited, start);
        }
        // Строчное содержимое. Без блочных братьев это весь контекст
        // форматирования; с ними — анонимная коробка из прогона до блока,
        // и пустая (только пробелы и пустые спаны) строки не имеет.
        let has_line = if has_block {
            let run: Vec<Node> = order[i..]
                .iter()
                .take_while(|m| !breaks_inline(m))
                .map(|m| (*m).clone())
                .collect();
            holds_line_box(&run)
        } else {
            holds_line_box(nodes)
        };
        return has_line.then(|| inherited.clone());
    }
    None
}

/// Срез `text-box-trim` одной стороны строки в точках (css-inline-3 §4.2,
/// §4.3): полулидинг корневой строчной коробки плюс расстояние от подъёма
/// (спуска) до метрики края. Та же формула, что `trim_for` в хвосте
/// `blocks()`; нужна и вне его — точке обрыва `line-clamp`.
pub(crate) fn text_box_trim_px(line_style: &Computed, start: bool, opts: &RenderOpts) -> f32 {
    let size = match line_style.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let family = line_style.font_family.clone().unwrap_or_default();
    let (ascent, descent, cap) = crate::metrics::vmetrics_px(&family, size);
    let line = match line_style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => ascent + descent,
    };
    let half = (line - (ascent + descent)) / 2.0;
    if start {
        half + match line_style.text_box_over {
            crate::computed::TextEdge::Cap => ascent - cap,
            crate::computed::TextEdge::Ex => ascent - crate::metrics::ch_ex_px(&family, size).1,
            _ => 0.0,
        }
    } else {
        half + match line_style.text_box_under {
            crate::computed::TextEdge::Alphabetic => {
                descent + crate::fonts::alphabetic_em(&family) * size
            }
            _ => 0.0,
        }
    }
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
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    ))
            }))
            .and_then(|(i, ch)| {
                with_inner_cb(&e.style, || margin_px(ch.style.margin.bottom, &ch.style))
                    .map(|_| i)
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
    // Струна примыкающих полей соседей (§8.3.1). `emitted` — сколько точек уже
    // ЗАПИСАНО в стили этого зазора: раскладка складывает поля сама, и
    // верхнему полю следующего блока достаётся только разница.
    let mut strut: Option<Strut> = None;
    let mut emitted = 0.0f32;
    // Последняя коробка прогона с клиренсом: её остаток поля остаётся ВНУТРИ
    // родителя и наружу не уходит.
    let mut cleared_run: Option<usize> = None;
    for (idx, node) in out.iter_mut().enumerate() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу.
            if matches!(node, Node::Text(t) if blank_text(t)) {
                continue;
            }
            strut = None;
            continue;
        };
        // Строчный элемент С СОДЕРЖИМЫМ порождает строчную коробку, и поля
        // блоков через неё уже не примыкают.
        if inline_level_box(e) {
            if !e.children.is_empty() {
                strut = None;
            }
            continue;
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: обрывать струну на атомарном строчном
        // (`inline-block` и родня в потоке рождают строчную коробку). CSS2
        // -38, вся потеря — семья `bidi-box-model-*`: там такой сосед стоит
        // между блоками сплошь, и разрыв струны разводит их полями врозь.
        // Возвращаться вместе с настоящей строчной коробкой в раскладке.
        if !in_flow(&e.style) {
            band_clearance::remember_float_margin(e, strut, emitted);
            continue;
        }
        let top = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
        let bottom = margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0);
        let through = through_strut(e);
        let mut merged = match strut {
            Some(s) => {
                let m = adjoin(s, strut_of(top));
                // Верхний край насквозь-схлопнутой коробки встаёт там, где
                // разрешается струна до неё вместе с её верхним полем. Та же
                // строка даёт это и обычной коробке: нижнее поле соседа
                // раскладка уже поставила, верхнему достаётся разница.
                pin_inherited_margins(e, true, false);
                e.style.margin.top = Some(Len::Px(solve(m) - emitted));
                emitted = solve(m);
                m
            }
            None => {
                // Примыкать не к чему: верхнее поле остаётся как написано.
                emitted = top;
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
        if through.is_none()
            && e.style.clear.is_some()
            && through_strut_no_clear(e).is_some()
        {
            emitted = top;
            pin_inherited_margins(e, false, true);
            e.style.margin.bottom = Some(Len::Px(0.0));
            strut = Some(adjoin(strut_of(top), strut_of(bottom)));
            cleared_run = Some(idx);
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
            strut = Some(merged);
            continue;
        }
        strut = Some(strut_of(bottom));
        emitted = bottom;
        cleared_run = None;
    }
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

/// Первый (по направлению итератора) IN-FLOW блочный ребёнок: плавающие,
/// абсолютные и пустые строчные пропускаются, непробельный текст и строчный
/// элемент с содержимым (строчная коробка!) обрывают поиск.
pub(crate) fn first_in_flow<'a>(
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

/// Ширина СОДЕРЖИМОГО коробки в точках — содержащий блок её детей (CSS 2.1
/// §10.1 п.2) — для процентных полей внуков в цепочках схлопывания. `cb` —
/// содержащий блок самой коробки; `None` — в точках не выводится.
pub(crate) fn inner_width_px(c: &Computed, cb: Option<f32>) -> Option<f32> {
    let side = |l: Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => cb.map(|w| k * w),
        _ => None,
    };
    let b = c.borders();
    let edges = || Some(side(c.padding.left)? + side(c.padding.right)? + side(b.left)? + side(b.right)?);
    match c.width {
        Some(Len::Px(w)) if c.border_box == Some(true) => Some((w - edges()?).max(0.0)),
        Some(Len::Px(w)) => Some(w),
        Some(Len::Pct(k)) if c.border_box != Some(true) => cb.map(|w| k * w),
        // §10.3.3: блок в потоке с `width: auto` занимает содержащий блок за
        // вычетом своих полей, рамок и отступов (auto-поле — ноль).
        None | Some(Len::Auto) if in_flow(c) => {
            Some((cb? - side(c.margin.left)? - side(c.margin.right)? - edges()?).max(0.0))
        }
        _ => None,
    }
}

/// `f` под шириной содержимого коробки `c` вместо ширины уровня. Прежде
/// цепочка мерила доли внуков от содержащего блока УРОВНЯ: у `body` с
/// `width: auto` его нет, и `15%` у `#parent` внутри `#grand-parent {width:
/// 400px}` роняло цепочку целиком — поля абзаца и `#parent` складывались
/// (16 + 60 вместо 60, `margin-percentage-inherit-001`).
pub(crate) fn with_inner_cb<T>(c: &Computed, f: impl FnOnce() -> T) -> T {
    let level = COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get);
    let prev = COLLAPSE_CB_WIDTH_PX.with(|w| w.replace(inner_width_px(c, level)));
    // Кегль уровня — тоже от спуска: поля детей в `em` без своего кегля (и
    // с кеглем в `em`) меряются от кегля ЭТОЙ коробки, а не уровня выше.
    // Прежде `html{font-size:2em} p{font-size:.5em}` схлопывал поле абзаца
    // сквозь `body` по 8 точкам вместо 16 (`numbers-units-021`).
    let font = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
    let own = match c.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * font,
        _ => font,
    };
    let prev_font = COLLAPSE_FONT_PX.with(|w| w.replace(own));
    let out = f();
    COLLAPSE_CB_WIDTH_PX.with(|w| w.set(prev));
    COLLAPSE_FONT_PX.with(|w| w.set(prev_font));
    out
}

/// Струна примыкающих полей (CSS 2.1 §8.3.1): больший положительный и самый
/// отрицательный. Свёртка ассоциативна, поэтому все три случая спеки — сосед,
/// родитель с ребёнком и схлопывание насквозь — считаются одним кодом.
pub(crate) type Strut = (f32, f32);

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

/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// не содержит строчной коробки, и поля всех её детей в потоке тоже

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
pub(crate) fn margin_or_bail(l: Option<Len>, style: &Computed) -> Option<f32> {
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
pub(crate) fn through_strut_no_clear(e: &Element) -> Option<Strut> {
    through_strut_inner(e, true)
}

pub(crate) fn through_strut_inner(e: &Element, ignore_clear: bool) -> Option<Strut> {
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

pub(crate) fn leading_chain(
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
pub(crate) fn trailing_chain(
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
pub(crate) fn zero_at(children: &mut [Node], path: &[usize], top: bool, deep: bool) {
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
pub(crate) fn pin_inherited_margins(e: &mut Element, top: bool, bottom: bool) {
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
        l @ (Len::Ch(_) | Len::Ex(_)) => Some(crate::metrics::spacing_px(
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

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
/// Абзац для тех, кто собирает текст сам — содержимое поля ввода.
pub fn paragraph_public(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph(nodes, inherited, opts)
}

/// Знак стоит прямо в вертикальном письме с `text-orientation: mixed`
/// (UTR#50, vo=U, упрощённо): иероглифика, кана, CJK-знаки препинания и
/// полноширинные формы. Остальное — лежит боком.
pub(crate) fn upright_in_mixed(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK-знаки и пунктуация (「」、。 …)
        | 0x3040..=0x30FF // хирагана и катакана
        | 0x31F0..=0x31FF // фонетические расширения каны
        | 0x3200..=0x33FF // обведённые и совместимые CJK
        | 0x3400..=0x4DBF // иероглифика, расширение A
        | 0x4E00..=0x9FFF // иероглифика единая
        | 0xAC00..=0xD7AF // хангыль
        | 0xF900..=0xFAFF // совместимая иероглифика
        | 0xFE30..=0xFE4F // вертикальные формы совместимости
        | 0xFF01..=0xFF60 // полноширинные формы
        | 0x20000..=0x2FFFD // иероглифика, плоскость 2
    )
}

pub(crate) fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph_routed(nodes, inherited, opts, None)
}

pub(crate) fn paragraph_routed(
    nodes: &[Node], inherited: &Computed, opts: &RenderOpts,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Vertical paragraphs choose the physical text route after inline collection.
    if inherited.vertical == Some(true) {
        // `text-orientation: upright`: глифы СТОЯТ и идут сверху вниз —
        // никакого поворота. Это обычный горизонтальный абзац шириной в один
        // кегль (продвижение стоячего глифа = кегль, §7.4) с резкой по
        // знакам: каждый знак — своя строка, стопка растёт вниз.
        // У `sideways-*` ориентация текста ИГНОРИРУЕТСЯ (css-writing-modes-4
        // §text-orientation): глифы всегда лежат, стопка не строится.
        // `text-orientation` наследуется и действует на ТЕКСТ (§4.1): когда
        // все куски абзаца несут `upright` сами (`html::after { upright }`),
        // стопка обязана строиться так же, как при флаге на контейнере.
        let kids_upright = !nodes.is_empty()
            && nodes.iter().all(|n| match n {
                Node::Element(e) => e.style.upright == Some(true),
                Node::Text(t) => t.trim().is_empty(),
            });
        let upright = inherited.upright == Some(true) || kids_upright;
        if upright && inherited.sideways != Some(true) {
            let mut stack = inherited.clone();
            stack.vertical = None;
            stack.upright_stack = true;
            stack.break_word = Some(true);
            let em = match stack.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            // Каждый стоячий глиф продвигает строку РОВНО на кегль (§7.4):
            // шаг стопки — кегль, а не своя высота строки; полоса переноса
            // уже одного глифа — в строку ложится ровно один знак (два узких
            // нуля вставали рядом, и стопка выходила короче).
            // Толщина вертикальной строки — LINE-HEIGHT, как у горизонтальной
            // (стопка глифов стоит в полосе высоты строки, повернутой набок):
            // читается ДО подмены шага стопки кеглем, иначе полоса всегда
            // равнялась кеглю (`vertical-alignment-vrl-022`).
            let lane = match stack.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) | Some(Len::Pct(k)) => k * em,
                // До этой точки `ch` мог не разрешиться: стоячий ноль
                // продвигается на кегль (§7.4) — считаем сами.
                Some(Len::Ch(k)) => k * em,
                _ => em,
            };
            stack.line_height = Some(Len::Px(em));
            let inner = paragraph(nodes, &stack, opts);
            return div()
                .w(px(lane.max(em)))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .child(div().w(px(em * 0.9)).flex_shrink_0().child(inner))
                .into_any_element();
        }
        let mut horizontal = inherited.clone();
        horizontal.vertical = None;
        // Пометка для `text-combine-upright`: кускам внутри повёрнутого
        // абзаца нужен контр-поворот (см. atom-ветку ниже).
        horizontal.rotated_line = Some(true);
        // `vertical-lr`: колонки идут слева направо — строки подаются снизу
        // вверх, чтобы после поворота ПО ЧАСОВОЙ первая оказалась левой (у
        // vertical-rl порядок родной: первая строка правой колонкой).
        //
        // `sideways-lr` вертится ПРОТИВ часовой (css-writing-modes-4,
        // Abstract-Physical Mapping: line-left = низ, over = лево), и после
        // такого поворота первая горизонтальная строка САМА оказывается левой
        // колонкой. Подавать строки снизу вверх тут — второй разворот,
        // ровно он и давал 180° (`block-flow-direction-slr-043` 32.20).
        let ccw_line = inherited.sideways == Some(true) && inherited.vertical_rl != Some(true);
        if inherited.vertical_rl != Some(true) && !ccw_line {
            horizontal.lines_reversed = Some(true);
        }
        // Поворот — приём отрисовки ТЕКСТА. Замещаемое содержимое (картинка,
        // элемент формы) вертикальное письмо не поворачивает никогда: абзац
        // из одной картинки обязан выглядеть так же, как в горизонтальном
        // письме (`wm-propagation-body-*`: подпись теста — рисунок, и он
        // ложился боком).
        let mut plain = String::new();
        gather_text(nodes, &mut plain);
        // Неразрывный пробел — ТЕКСТ: он даёт строку и её толщину
        // (`<td>&nbsp;</td>` в вертикальном ряду, ch-units-vrl-005), а
        // `trim()` съедал его как юникод-пробел, и абзац уходил
        // горизонтальным путём шириной в один пробел.
        if plain.trim().is_empty() && !plain.contains('\u{a0}') {
            // Pure-atom paragraphs paint physical boxes without a text transform.
            let para = rotated_atom::without_text_turn(|| paragraph(nodes, &horizontal, opts));
            // Строка из одних атомов (картинка, пустая строчная коробка) идёт
            // горизонтальным путём и ложится у ВЕРХНЕГО края коробки абзаца.
            // Верно это, только пока inline-start — верх. Таблица
            // Abstract-Physical Mapping (css-writing-modes-4,
            // Overview.bs:1877-1888) даёт `sideways-lr` line-left = НИЗ, а
            // `direction: rtl` ставит inline-start на line-right
            // (Overview.bs:1673-1675) — у `vertical-*`/`sideways-rl` это тоже
            // НИЗ. Повёрнутый текст это уже знает (`VerticalText::ccw`), а
            // безтекстовая ветка — нет: подпись-картинка стояла у верха
            // (`wm-propagation-body-035/039/043/051`,
            // `block-flow-direction-slr-066`; при rtl —
            // `overconstrained-rel-pos-rtl-*`). Коробка абзаца растянута по
            // строчной оси рядом блочного потока, и колонка с прижимом к
            // концу ставит ряд к её низу; строки-опоры у ряда из одних атомов
            // нет (`inline::as_wrapped_row`), поэтому низ картинки ложится
            // ровно на низ коробки.
            let from_bottom = ccw_line != (inherited.rtl == Some(true));
            if from_bottom && nodes.iter().any(|n| matches!(n, Node::Element(_))) {
                return div()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(para)
                    .into_any_element();
            }
            return para;
        }
        let mut flow = horizontal.clone();
        flow.para_vertical = Some(inherited.vertical_rl == Some(true));
        let fallback = inherited.ortho_limit.unwrap_or(opts.viewport.1);
        flow.ortho_limit = Some(fallback);
        flow.orthogonal_inline = native_vertical::constraint(inherited, fallback);
        let built = std::cell::Cell::new(false);
        let request = native_paragraph_route::Request { style: &flow, built: &built };
        // Build atoms with legacy style; apply native flow only to a text Paragraph.
        let inner = paragraph_routed(nodes, &horizontal, opts, Some(&request));
        if built.get() {
            return inner;
        }
        // Спросить размер у родителя обход не может (замер внутри чужого
        // замера падает — см. `VerticalText::request_layout`). Зато предел
        // ортогонального потока уже принесён вниз стилем: задаём его ШИРИНОЙ
        // до поворота, и после поворота он становится высотой коробки — то
        // есть перенос считается по той оси, по которой идёт строка.
        let limit = inherited.ortho_limit.unwrap_or(opts.viewport.1);
        // …и всё же ОДИН случай жёсткую ширину не терпит: АБСОЛЮТНАЯ коробка
        // со свободной строчной осью. Её размер по этой оси — по содержимому
        // (§10.3.7), а жёсткая ширина делает `natural.width` тождественно
        // равной пределу, и высота выходит во весь предел ортогонального
        // потока — коробка растягивалась на весь содержащий блок и вылезала
        // за него. Гейт узкий: потоковых коробок, на которых мерились четыре
        // отката выше, он не касается.
        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        // Коробка на СТАТИЧЕСКОЙ позиции тоже абсолютна: `position` с неё
        // снято ради отсчёта, и без пометки `abs_static` гейт её не узнавал.
        let free_inline = (matches!(
            inherited.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) || inherited.abs_static
            || inherited.hug_inline)
            && !matches!(inherited.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
            && !(edge(inherited.inset.top) && edge(inherited.inset.bottom));
        // Ячейка вертикальной таблицы: предел строки — мера её КОЛОНКИ, а не
        // инлайн-размер всего стола. Колонку решает решётка (дорожка
        // `MinMax(MinContent, Auto)` = «наибольший min-content ячеек
        // колонки», css-tables-3 §computing-column-measures), но для этого ей
        // нужен вклад ячейки по МИНИМАЛЬНОМУ содержимому — а жёсткая ширина
        // до поворота делает `natural.width` тождественно равной пределу, и
        // вклад выходит равен всему столу: у `row-progression-vrl-002` все
        // три дорожки становились 140 и каждая ячейка рвала строку по
        // 7 знаков вместо 3/2/2. Потолок при этом остаётся: колонка не шире
        // инлайн-размера стола.
        let col_min = inherited.ortho_col && inherited.ortho_limit.is_some();
        // Atomic content shares native text's fixed and shrink-to-fit inline sizing.
        let inline_constraint = native_vertical::constraint(inherited, limit);
        let inner = if let Some(constraint) = inline_constraint {
            // A definite CSS inline size is also the percentage basis for anonymous
            // rows (including <br>); available space alone does not establish it.
            if constraint.fixed.is_some() {
                div().w(px(constraint.used(0.0, 0.0))).child(inner).into_any_element()
            } else {
                inner
            }
        } else if free_inline || col_min {
            div().max_w(px(limit)).child(inner).into_any_element()
        } else {
            div().w(px(limit)).child(inner).into_any_element()
        };
        // Intrinsic inline claims are independent of the first-line baseline metrics.
        let em = match inherited.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * opts.base_size(),
            _ => opts.base_size(),
        };
        let lh = match inherited.line_height {
            Some(Len::Px(v)) => Some(px(v)),
            Some(Len::Pct(k)) | Some(Len::Em(k)) => Some(px(k * em)),
            _ => None,
        };
        let central = inherited.sideways != Some(true) && inherited.text_sideways != Some(true);
        let vt = crate::interact::VerticalText::new(inner)
            .counter_clockwise(ccw_line)
            .lines_left_first(inherited.vertical_rl != Some(true) && !ccw_line)
            .first_line(measure_font(inherited, opts), px(em), lh, central)
            // Замер по МИНИМАЛЬНОМУ содержимому: заявленная высота повёрнутой
            // коробки становится вкладом ячейки в дорожку её колонки
            // (см. `col_min` выше). Ниже `fit_within` заявит эту же величину
            // высотой — предел (мера стола) её не режет, потому что
            // min-content колонки заведомо не больше него.
            .column_min(col_min)
            .inline_keyword(native_vertical::keyword(inherited))
            .keyed(crate::interact::vt_seq_key(
                text_id(&plain) ^ opts.doc_salt ^ (nodes.len() as u64).wrapping_mul(0x9E3779B9),
            ));
        // Настоящий предел от родителя (ортогональная ячейка): строка,
        // которая уже влезает, заявляет высоту честно — без неё гибкая
        // ячейка мерила коробку нулём и justify уводил глиф из виду.
        let vt = if let Some(constraint) = inline_constraint {
            vt.inline_constraint(constraint)
        } else if let Some(l) = inherited.ortho_limit {
            vt.fit_within(px(l))
        } else if inherited.hug_claim {
            // Родитель размером в содержимое по строчной оси
            // (`vertical_hug_children`): длину строки решать некому, и
            // коробка заявляет её сама — max-content под пределом `limit`
            // (обёртка выше уже `max_w`, замер по содержимому).
            vt.fit_within(px(limit))
        } else {
            vt
        };
        let vt = if let Some(Len::Px(cap)) = inherited.max_height {
            vt.claiming_height(px(cap))
        } else {
            vt
        };
        return vt.into_any_element();
    }
    // Первая строка со своим стилем: где она кончается, известно только после
    // переноса, поэтому абзац собирается замером (см. `float::FirstLine`).
    if let Some(first) = inherited.first_line.clone() {
        let mut base = inherited.clone();
        base.first_line = None;
        let nodes_owned = nodes.to_vec();
        let opts_owned = opts.clone();
        let mut plain = String::new();
        first_line_text::gather(nodes, inherited.preserve_newlines == Some(true), &mut plain);
        let plain = crate::inline::transform_case(&normalize_for_shadow(&plain), inherited);
        if !plain.trim().is_empty() {
            let size = match base.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => opts.base_size(),
            };
            let line = match base.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => size * k,
                _ => size * normal_fraction(&base, opts),
            };
            let for_build = first.clone();
            let depth = defer_depth();
            let build: crate::float::Split = std::rc::Rc::new(move |at, width| {
                let _depth = DepthScope::enter(depth);
                let mut styled = base.clone();
                styled.first_line = None;
                let mut para = paragraph_pieces(&nodes_owned, &styled, &opts_owned, at, &for_build);
                para = div().w(width).child(para).into_any_element();
                para
            });
            // Мерить надо ТЕМ начертанием, каким строка и будет набрана:
            // жирная первая строка занимает больше места, и разрез по
            // обычному шрифту не помещался бы в неё целиком.
            let mut font = opts.text.font();
            font.fallbacks = crate::computed::font_family::fallbacks(&first, font.fallbacks);
            if let Some(w) = first.font_weight {
                font.weight = gpui::FontWeight(w as f32);
            }
            if first.italic == Some(true) {
                font.style = gpui::FontStyle::Italic;
            }
            if let Some(family) = first.font_family.as_ref().filter(|f| !f.is_empty()) {
                font.family = family.clone().into();
            }
            let measure_size = match first.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * size,
                _ => size,
            };
            return crate::float::FirstLine::new(
                build,
                SharedString::from(plain.trim().to_string()),
                font,
                measure_size,
                line,
            )
            .into_any_element();
        }
    }
    paragraph_pieces_routed(nodes, inherited, opts, 0, &Computed::default(), native_request)
}

/// Свой кегль абзаца в точках — точка отсчёта для строки-опоры и для долей.
///
/// Отсчёт идёт от СВОЕГО кегля, а не от базового кегля документа: у коробки с
/// `font-size: 10px` строка обязана быть в 10 точек, а базовый (16) держал её
/// вдвое выше.
pub(crate) fn own_size(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    }
}

/// Есть ли в абзаце текст ПОМИМО внепоточных кусков.
///
/// Это ровно условие, при котором абзацу доступен ТЕКСТОВЫЙ путь: текст для
/// него собирает `inline::text_and_runs` из кусков `Piece::Text`, а
/// внепоточный кусок в него не входит (`inline.rs:2539`), и на пустом тексте
/// путь закрыт (`inline.rs:2543`). Внепоточный — и абсолют на статической
/// позиции, и абсолют с краями: оба уходят `Piece::Overlay`, оба своего
/// текста в строку не отдают.
pub(crate) fn has_flow_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(e) => {
            !matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute)
                    | Some(crate::computed::Position::Fixed)
            ) && has_flow_text(&e.children)
        }
    })
}

/// Куски текста с `vertical-align: top`/`bottom` (CSS 2.1 §10.8.1): отрезок
/// байт в тексте абзаца, край (`true` — верх) и высота строчной коробки
/// куска — его `line-height`.
///
/// `vertical-align` у нас наследуется (ради ячеек таблицы), поэтому краевым
/// считается только кусок, чьё значение ОТЛИЧАЕТСЯ от значения абзаца: иначе
/// каждый абзац ячейки с `vertical-align: top` прижимался бы весь.
pub(crate) fn edge_pieces(
    pieces: &[inline::Piece],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Vec<(std::ops::Range<usize>, bool, f32)> {
    use crate::computed::Align;
    let mut out: Vec<(std::ops::Range<usize>, bool, f32)> = Vec::new();
    // A rotated vertical paragraph is laid out in its pre-rotation frame,
    // whose top is the line-over side (css-writing-modes-4 §line-relative
    // directions), so `top`/`bottom` keep their meaning there.
    if inherited.vertical == Some(true) {
        return out;
    }
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        let top = match style.vertical_align {
            Some(Align::Start) => Some(true),
            Some(Align::End) => Some(false),
            _ => None,
        };
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            );
        if let Some(top) = top
            && style.vertical_align != inherited.vertical_align
            && !out_of_flow
            && !text.is_empty()
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => own_size(inherited, opts),
            };
            let h = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => size * normal_fraction(style, opts),
            };
            match out.last_mut() {
                // Соседние куски одного края — одна коробка (`<span>` с
                // вложенными кусками).
                Some((r, t, hh)) if r.end == at && *t == top => {
                    r.end = end;
                    *hh = hh.max(h);
                }
                _ => out.push((at..end, top, h)),
            }
        }
        at = end;
    }
    out
}

/// Лежит ли отрезок внутри краевого куска.
pub(crate) fn in_edge(edges: &[(std::ops::Range<usize>, bool, f32)], r: &std::ops::Range<usize>) -> bool {
    edges
        .iter()
        .any(|(e, _, _)| e.start < r.end.max(r.start + 1) && r.start < e.end)
}

/// Самый крупный кегль и наибольшая `line-height` кусков ВНЕ краевых: струт
/// строки и её базовая линия от прижатых к краю не зависят (§10.8.1).
pub(crate) fn flow_metrics(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> (f32, f32) {
    let strut = own_size(inherited, opts);
    let fraction = normal_fraction(inherited, opts);
    let em_base = opts.base_size();
    let (mut size_max, mut lh_max) = (strut, strut * fraction);
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let r = at..at + text.len();
        at = r.end;
        if in_edge(edges, &r) {
            continue;
        }
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em_base,
            _ => strut,
        };
        let own = match style.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            _ => size * fraction,
        };
        size_max = size_max.max(size);
        lh_max = lh_max.max(own);
    }
    (size_max, lh_max)
}

/// Строчные коробки кусков для построчной высоты (`Paragraph::line_boxes`,
/// CSS 2.1 §10.8.1): отрезок байт → `line-height` куска в точках, плюс
/// `line-height` струта блока. `None` — все куски одного кегля, гарнитуры и
/// высоты строки: строка тогда и так равна струту, абзац идёт прежним путём.
///
/// Прежде высота строки на ВЕСЬ абзац бралась по самому крупному куску
/// (`max_line_height`, `k × biggest`): одна крупная буква растила все строки,
/// а базовая линия мелкого текста в строке с крупным стояла посередине.
pub(crate) fn line_box_spans(
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<(Vec<(std::ops::Range<usize>, f32)>, f32)> {
    if inherited.vertical == Some(true)
        || inherited.rotated_line == Some(true)
        || inherited.text_fit.is_some()
    {
        return None;
    }
    let own = own_size(inherited, opts);
    let strut = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * own,
        None | Some(Len::Auto) => own * normal_fraction(inherited, opts),
        _ => return None,
    };
    let mut out: Vec<(std::ops::Range<usize>, f32)> = Vec::new();
    let mut mixed = false;
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let r = at..at + text.len();
        at = r.end;
        if text.is_empty() || in_edge(edges, &r) {
            continue;
        }
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * opts.base_size(),
            None => own,
            _ => return None,
        };
        let lh = match style.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            None | Some(Len::Auto) => size * normal_fraction(style, opts),
            _ => return None,
        };
        if (size - own).abs() > 0.01
            || (lh - strut).abs() > 0.01
            || style.font_family != inherited.font_family
        {
            mixed = true;
        }
        out.push((r, lh));
    }
    mixed.then_some((out, strut))
}

/// Можно ли абзацу ставить атомы в свою строку: горизонтальное письмо слева
/// направо, без раздачи по ширине (места атомов считаются от продвижения
/// распорки, а растяжку пробелов `Paragraph` раздаёт уже при отрисовке) и с
/// выделяемым текстом — путь `StyledText` атомов не несёт.
///
/// Абзац с руби идёт в строку и при `rtl`: иначе он остаётся в
/// ряду, где строка под аннотацию не растёт, а такой же абзац слева направо
/// растёт (`ruby-bidi-002`: эталон из ltr-абзаца с `text-align: right`).
pub(crate) fn atoms_fit_line(inherited: &Computed, ruby: bool) -> bool {
    inherited.vertical != Some(true)
        // A rotated paragraph of atoms only is a row whose end edge sits on
        // the end of the paragraph box (`paragraph_routed`, pure-atom
        // branch); a line box would add the strut's descent below the atoms
        // (`wm-propagation-body-035`: the caption image rose by the descent).
        && !(inherited.rotated_line == Some(true) && !rotated_atom::text_turn())
        && (ruby || inherited.rtl != Some(true))
        && inherited.no_select != Some(true)
        && inherited.pointer_events_none != Some(true)
        && crate::lines::align_for(inherited) != crate::lines::Align::Justify
        // Обрыв строки многоточием (`text-overflow: ellipsis`) режет текст по
        // знакам, а распорку атома знаком не считает: атом обрывался не там
        // (`text-overflow-016`, `text-overflow-ruby`) — такой абзац в ряду.
        && inherited.ellipsis != Some(true)
}

/// `vertical-align` атома для строки абзаца, если атом туда годится.
///
/// Атом раскладывается ДО замера абзаца по своему содержимому
/// (`Paragraph::lay_atoms`), поэтому в строку идут только атомы, чей размер от
/// ширины строки не зависит: без долей в размерах, полях и отступах. Абсолюты
/// (их место — щуп статической позиции), поля форм и руби остаются в ряду.
pub(crate) fn atom_line_align(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<crate::lines::AtomAlign> {
    use crate::lines::AtomAlign;
    let st = &e.style;
    let atomic = matches!(
        st.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineTable)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
    ) && st.inline_display != Some(true);
    let replaced = matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
    );
    // Контейнер руби — тоже атом строки: колонки баз с аннотациями
    // монолитны (§3.5), а строка обязана вырасти под аннотацию (§3.4), чего
    // гибкий ряд слов не умеет. Прочие роли (база, аннотация вне контейнера)
    // остаются в ряду.
    let ruby = ruby_role(e) == Some(crate::computed::RubyRole::Container);
    if !(atomic || replaced || ruby) || (st.ruby_role.is_some() && !ruby) {
        return None;
    }
    // Внутри `line-clamp` руби остаётся в ряду: вычислитель среза
    // (`interact::ClampCut`) делит высоту абзаца на РАВНЫЕ строки, а строка с
    // аннотацией выше прочих. ★ ЗАМЕРЕНО (03.10, 147 пар руби): в строке
    // `line-clamp-auto-with-ruby-001/003` зеленеют (5.2 → 0.13/0.26), но
    // `-002` (руби за срезом) уходит 0.09 → 5.23 — срез встаёт строкой выше.
    // Возвращать вместе с настоящими низами строк в `ClampEntry`.
    // Ортогональный поток внутри атома меряется от ДОСТУПНОГО места (§7.3
    // css-writing-modes-3), а замер «по содержимому» его не даёт: коробка с
    // `writing-mode: vertical-*` и строчной стороной `auto` внутри атома
    // выходила другой высоты (`inline-box-orthogonal-child-with-margins`).
    // Элемент сетки и гибкого ряда размер берёт от дорожки/ряда, и от
    // доступного места не зависит — такой атом остаётся в строке: иначе абзац
    // теста с `vertical-rl`-элементами сетки шёл прежним рядом, а эталон из
    // простых `inline-block` — строкой (`grid-container-baseline-
    // synthesized-001..004`: 0.00 -> 11.00).
    fn has_vertical(nodes: &[Node], in_box_layout: bool) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(k) => {
                let sized_by_parent = in_box_layout || k.style.height.is_some();
                // Ломает замер только ортогональный ФЛОАТ (его ширина «по
                // содержимому» берётся от доступного места); ортогональный
                // блок в потоке раскладывается одинаково, и исключать атом
                // ради него значило вести тест рядом, а эталон строкой
                // (`baseline-with-orthogonal-flow-001`).
                let floated = k.style.float.is_some_and(|f| f != 0);
                (k.style.vertical.is_some() && !sized_by_parent && floated)
                    || has_vertical(&k.children, box_layout(&k.style))
            }
            _ => false,
        })
    }
    fn box_layout(c: &Computed) -> bool {
        matches!(
            c.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        )
    }
    // Сам атом с вертикальным письмом допустим, если он сетка или гибкий ряд:
    // размер ему задают дорожки и содержимое, а не доступное место
    // (`grid-container-baseline-synthesized-002/004`).
    if (st.vertical.is_some() && !box_layout(st)) || has_vertical(&e.children, box_layout(st)) {
        return None;
    }
    // Замещаемый с `aspect-ratio`: соотношение разрешается от ДОСТУПНОГО
    // места, а замер по содержимому его не даёт (`zero-or-infinity-006`:
    // `aspect-ratio: 0/1` давал другую высоту).
    if replaced && (st.aspect_ratio.is_some() || st.aspect_ratio_auto.is_some()) {
        return None;
    }
    // Абсолютная замещаемая коробка с заданными краями места в строке не
    // занимает (`atom_element` отдаёт пустышку нулевого размера): атомом
    // строки она абзац с текстового пути не уводит. Прежде ряд слов набирал
    // соседний текст шире и ниже, чем эталон с тем же текстом без картинки
    // (`background-bg-pos-204-ref`).
    if matches!(
        st.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        if replaced && !at_static_position(st) {
            return Some(AtomAlign::Shift(0.0));
        }
        return None;
    }
    let fixed = |l: Option<Len>| !matches!(l, Some(Len::Pct(_)) | Some(Len::Calc(_)));
    let sides = |s: &crate::computed::Sides| {
        fixed(s.top) && fixed(s.right) && fixed(s.bottom) && fixed(s.left)
    };
    if ![
        st.width,
        st.height,
        st.min_width,
        st.min_height,
        st.max_width,
        st.max_height,
    ]
    .into_iter()
    .all(fixed)
        || !sides(&st.margin)
        || !sides(&st.padding)
    {
        return None;
    }
    // Сдвиги — как у текстового куска (`inline::shift_spans`), но от кегля
    // САМОГО атома; ось подъёма смотрит вверх.
    let merged = inline::inherit(inherited, st);
    let size = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => own_size(inherited, opts),
    };
    Some(match st.vertical_align {
        Some(crate::computed::Align::Start) => AtomAlign::Top,
        Some(crate::computed::Align::End) => AtomAlign::Bottom,
        Some(crate::computed::Align::Center) => AtomAlign::Middle,
        _ => match st.vertical_align_text {
            Some(true) => AtomAlign::TextTop,
            Some(false) => AtomAlign::TextBottom,
            None => {
                if let Some(v) = st.vertical_shift_px {
                    AtomAlign::Shift(-v)
                } else if let Some(l) = st.vertical_shift_len {
                    let family = merged.font_family.clone().unwrap_or_default();
                    AtomAlign::Shift(crate::metrics::spacing_px(Some(l), &family, size))
                } else if let Some(k) = st.vertical_shift {
                    AtomAlign::Shift(-k * size)
                } else if let Some(k) = st.vertical_shift_pct {
                    // Процент — от `line-height` самого атома (§10.8.1).
                    let own = match merged.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => size * normal_fraction(&merged, opts),
                    };
                    AtomAlign::Shift(-k * own)
                } else {
                    AtomAlign::Shift(0.0)
                }
            }
        },
    })
}

/// Абзац с готовым разрезом первой строки: `at` — сколько байт в неё вошло.
pub(crate) fn paragraph_pieces(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
) -> AnyElement {
    paragraph_pieces_routed(nodes, inherited, opts, first_line_at, first_line, None)
}

pub(crate) fn paragraph_pieces_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Бюджет строк знака обрыва (авто-режим) забирается РАЗОМ, до сборки
    // кусков: куски строят вложенные абзацы (`inline-block`, `<svg>`), и
    // чужой бюджет им доставаться не должен.
    let clamp_budget = crate::interact::take_para_budget();
    let clamp_tag = crate::interact::take_para_tag();
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: брать поперечное выравнивание ряда с самих
    // кусков, когда абзац своего не задал (`vertical-align: bottom` у
    // картинки). Ни это, ни `align-self` на самой картинке высоту строки не
    // меняют — строка всё равно выходит около 90 точек вместо 60, и картинка
    // просто прижимается к её низу. Дело не в выравнивании.
    // Текстовый путь абзаца — единственное место, где место куска вне потока
    // считается ДВУНАПРАВЛЕННО: строку режет и переставляет разбор UAX#9
    // внутри `lines.rs`, а `point_of` берёт продвижение от начала СВОЕЙ
    // строки. Ряд из слов (`inline.rs: as_wrapped_row`) о направлении не
    // знает вовсе — куски идут в ЛОГИЧЕСКОМ порядке, строка прижимается
    // целиком, и щуп садится в НАЧАЛО rtl-строки (замер: x = 328 при верных
    // 168, снимки `target/scoutrtlv/target/wpt-shots/`).
    //
    // Открыть текстовый путь при `direction: rtl` можно только абзацу, у
    // которого этот путь ДОСТУПЕН: на пустом тексте `inline::text_and_runs`
    // отдаёт `None` (`inline.rs:2543`), абзац всё равно сваливается в ряд, а
    // там кусок вне потока заворачивается в `inline.rs: overlay_in_row` со
    // СВОИМ, пустым `Spot` и теряет и `rtl`, и `next_line`. Ровно так устроена
    // семья `css-position/static-position/inline-level-absolute-in-block-
    // level-context-007..012` (rtl, абсолют `display: inline`, текста в абзаце
    // НЕТ, все шесть 0.00) — гейт оставляет её на прежнем пути.
    let flow_text = has_flow_text(nodes);
    let mut atom = |e: &Element| -> Option<inline::Piece> {
        let in_inline_cb = crate::inline::take_atom_cb();
        let svg_sized = svg_percentage_size::resolve(e, inherited);
        let e = svg_sized.as_ref().unwrap_or(e);
        // CSS 2.1 sections 10.3.8/10.6.5 use the replaced default size
        // before solving absolute insets; an empty frame is still replaced.
        let iframe_sized = (replaced_content::default_iframe(e)
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute | crate::computed::Position::Fixed)
            ))
            .then(|| replaced_content::empty_iframe_size(e, inherited, opts.viewport));
        let e = iframe_sized.as_ref().unwrap_or(e);
        // Абсолютный элемент на статической позиции ВНУТРИ строки — кусок вне
        // потока: место в строке он не занимает, поэтому абзац остаётся
        // текстовым и не теряет пробелы (`line-breaking-018`).
        // Вертикальное письмо сюда по-прежнему не пускается: там строчная ось
        // вертикальна, `point_of` отдаёт ДО-поворотные координаты, и класс
        // берётся отдельно (подкорень A3, три отката на `render.rs` «отдать
        // ось строки самому абзацу»).
        // ★ ЗАМЕРЕНО: без оговорки про вертикаль текстовый путь открывался
        // и ГОРИЗОНТАЛЬНОМУ rtl, и две зелёные пары уходили в «красное
        // видно» (`htb-rtl-ltr.tentative`, `htb-rtl-rtl` 0.16 -> 99.00) при
        // 24 приобретениях. Все приобретения — вертикальные
        // (`abs-pos-non-replaced-v{lr,rl}-*`), поэтому послабление
        // ограничено абзацем, чей содержащий блок пишет вертикально —
        // признак этого у нас `ortho_limit`, он ставится только внутри
        // вертикального содержащего блока (`element()`, `inline.rs:1012`).
        let plain_flow = inherited.vertical != Some(true)
            && (inherited.rtl != Some(true) || (flow_text && inherited.ortho_limit.is_some()));
        // Статическая позиция считается для ГИПОТЕТИЧЕСКОГО статического
        // элемента (§10.3.7: «if position had been static») — блокификация
        // `display: inline` под absolute на неё не влияет, метка
        // inline_display возвращает такой элемент в строчный путь.
        // БЛОЧНЫЙ абсолют в ПОВЁРНУТОМ абзаце вертикального письма — тоже
        // кусок вне потока, но с местом в начале СЛЕДУЮЩЕЙ строки
        // (`lines.rs: next_line_point`). Прежде он шёл атомом со щупом
        // `Spot`: атом выводил абзац из текстового пути в ряд слов, щуп
        // мерил до-поворотные координаты, а заместитель верхнего слоя
        // ложился горизонтальной полосой поперёк вертикального контейнера —
        // красное проступало у всех `css-position/static-position/v{lr,rl}-*`.
        // В повёрнутом абзаце коробка поворачивается вместе со строками, и её
        // до-поворотное место — ровно гипотетическая коробка §10.6.4.
        let rot_block = inherited.rotated_line == Some(true)
            && !inline_level(e)
            && e.style.inline_display != Some(true);
        if plain_flow
            && at_static_position(&e.style)
            && (inline_level(e) || e.style.inline_display == Some(true) || rot_block)
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.position = None;
            merged.abs_static = true;
            // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
            // путь блоков давал пустую коробку (clip-path-ellipse-2-ref).
            // Картинка — тем же порядком: у неё детей нет вовсе, и путь
            // блоков давал пустую коробку, то есть абсолютная картинка без
            // краёв не рисовалась ВООБЩЕ (`clip-rect-v*`, проба
            // `probe/absimg2.html`).
            let inner = if e.tag == "svg" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
            } else if e.tag == "img" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                // `clip: rect(...)` и маска у картинки живут в буфере группы:
                // путь наложения идёт мимо `grouped`, и без обёртки картинка
                // рисовалась бы целиком (`clip-rect-v*`).
                grouped(image(&copy), &e.style)
            } else {
                // Тот же буфер группы, что и у картинки: маска, обрезка и
                // фильтр иначе не доходят до коробки на статической позиции.
                grouped(
                    styled_div_with(e, &merged)
                        .children(blocks(&e.children, &merged, opts))
                        .into_any_element(),
                    &e.style,
                )
            };
            // Сторона, которой коробка вешается на статическую точку. При
            // `direction: ltr` — левый край (текстовый путь так и кладёт,
            // `lines.rs: prepaint_at(origin)`), при `rtl` — ПРАВЫЙ:
            // css-position-3 §abs-non-replaced-width (строки 1035-1043,
            // перепись CSS 2.1 §10.3.7) — «…if the 'direction' property of the
            // element establishing the static-position containing block is
            // 'ltr' set 'left' to the static position …; otherwise, set
            // 'right' to the static-position». Точка у обеих сторон ОДНА И ТА
            // ЖЕ: замер по снимкам — ltr-двойники `-v{lr,rl}-{004,005,028,029,
            // 104,105,136,137}` ставят левый край ровно на 168.0 и все восемь
            // 0.00, а эталон rtl-пар требует 88.0..168.0, то есть ту же 168.0
            // правым краем.
            //
            // Blink разводит это на два шага: `geometry/static_position.h:86`
            // даёт `kInlineEnd` при `!IsLtr()`, а `absolute_utils.cc:27-37`
            // `GetStaticPositionInsetBias` переводит его в `InsetBias::kEnd`.
            // Сторону задаёт направление СОДЕРЖАЩЕГО блока, а не собственное
            // письмо коробки (css-writing-modes-4 §7.1, строки 1926-1931).
            let rotated_rtl = inherited.rtl == Some(true) && inherited.rotated_line == Some(true);
            let inner = if inherited.rtl == Some(true) && !rot_block && !rotated_rtl {
                crate::interact::InlineStartHang::new(inner).into_any_element()
            } else {
                inner
            };
            // Кусок кладётся КОРНЕМ в статическую точку (`lines.rs`), а корень
            // своих полей не кладёт: от статической позиции коробку отодвигает
            // её поле (CSS 2.1 §10.3.7, `margin-left` в уравнении ширины). Под
            // обёрткой коробка — обычный ребёнок, и поле на месте
            // (`CSS2/text/text-indent-013-ref`: `margin-left: -10em` — чёрная
            // полоса на 328 вместо 168 закрывала PASS).
            let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
            let inner = if nz(merged.margin.left) || nz(merged.margin.top) {
                div().flex().flex_row().items_start().child(inner).into_any_element()
            } else {
                inner
            };
            return Some(inline::Piece::Overlay(
                inner,
                inline::OverlayAt {
                    next_line: rot_block,
                    bidi_hang: rotated_rtl && !rot_block,
                    ..Default::default()
                },
            ));
        }
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: контр-поворот физических четвёрок краёв
        // (`margin`/`padding`/`border-width` и четвёрки цвета, стиля,
        // видимости) у КАЖДОГО куска повёрнутого абзаца — поворот уносит их
        // с собой, а стороны письмом не переставляются (§3.2). Направление
        // проверено обоими: по часовой (верх←право, право←низ, низ←лево,
        // лево←верх) заметно лучше обратного — девять пар `grid-self-
        // baseline-*` шли 8.38/5.83/16.62/21.74/5.13/3.98/1.52/4.22/6.88, по
        // часовой стало 7.98/5.83/13.78/12.65/5.02/4.39/1.52/6.11/6.88,
        // против часовой 8.31/5.83/16.61/12.99/5.26/6.09/1.52/7.81/6.88.
        // Зелёной не стала НИ ОДНА: их держит отсутствие физической высоты
        // (см. откат про `max_w` выше), а `wm-propagation-body-049` ушла
        // 0.00 → 2.24. Итог на срезе 571 пары: 423 → 422.
        // Возвращать вместе с высотой повёрнутого блока по содержимому.
        // Стоячая коробка в повёрнутом абзаце: `inline-block` с явным
        // ГОРИЗОНТАЛЬНЫМ письмом контр-поворачивается — его содержимое
        // обязано стоять прямо (эмуляция tcy в эталонах compression-*).
        if inherited.rotated_line == Some(true)
            && e.style.display == Some(Display::InlineBlock)
            && e.style.vertical == Some(false)
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.rotated_line = None;
            let em = match merged.width {
                Some(Len::Px(v)) => v,
                _ => match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                },
            };
            let inner = styled_div_with(e, &merged)
                .children(blocks(&e.children, &merged, opts))
                .into_any_element();
            return Some(inline::Piece::Atom(
                crate::interact::CombinedUpright::upright_box(inner, em).into_any_element(),
            ));
        }
        if let Some(piece) = combined_text::piece(e, inherited, opts) {
            return Some(piece);
        }
        // Остановленная анимация атома — запечённым кадром, как у блока в
        // `animated()`, но ВМЕСТЕ с `rotate`/`scale`/`transform`: матрицу
        // атома строит `transformed()` ниже от `e.style`. В `animated()` атомы
        // не заходят вовсе, и кадр `individual-transform-combine` (шесть
        // `inline-block` под `animation-delay: -500000s`) не доходил ни до
        // сдвига, ни до матрицы (css-transforms-1 §transformable-element).
        let frozen_atom;
        let e = match bake_frozen(e, true) {
            Some(b) => {
                frozen_atom = b;
                &frozen_atom
            }
            None => e,
        };
        let ruby_atom;
        let e = match ruby_transform::used(e) {
            Some(used) => {
                ruby_atom = used;
                &ruby_atom
            }
            None => e,
        };
        // Боковые поля атома с собственным прижимом несёт ОБЁРТКА: внутри
        // неё они сдвигают коробку, но в продвижение строки не входят —
        // следующий кусок наезжал на предыдущий ровно на его поле
        // (эталоны `fixed-table-layout-021..023`: `img{vertical-align:top}`
        // плюс `margin-left`).
        let wrapped = match e.style.vertical_align {
            Some(crate::computed::Align::Start)
            | Some(crate::computed::Align::End)
            | Some(crate::computed::Align::Center) => true,
            _ => false,
        };
        let original_margin = rotated_atom::margin(&e.style, inherited);
        let bare;
        let e = if wrapped {
            let mut copy = e.clone();
            copy.style.margin = Default::default();
            bare = copy;
            &bare
        } else {
            e
        };
        // An absolutely positioned box inside a rotated vertical paragraph is
        // blockified (CSS 2.1 §9.7) and inherits the vertical writing mode
        // (css-writing-modes-4 §2.1): it is its own vertical block, not a
        // piece of the horizontal pre-rotation paragraph, whose clone has
        // `vertical` cleared. Carry the writing mode on the box itself.
        let vertical_abs;
        let e = if (inherited.rotated_line == Some(true) || inherited.upright_stack)
            && e.style.vertical.is_none()
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            )
            && !at_static_position(&e.style)
            && !replaced_tag(e)
        {
            let mut copy = e.clone();
            copy.style.vertical = Some(true);
            copy.style.vertical_rl = Some(inherited.vertical_rl == Some(true));
            copy.style.sideways = Some(inherited.sideways == Some(true));
            vertical_abs = copy;
            &vertical_abs
        } else {
            e
        };
        crate::inline::set_atom_cb(in_inline_cb);
        let built = atom_element(e, inherited, opts);
        crate::inline::set_atom_cb(false);
        let abs_cb = crate::inline::take_abs_cb();
        built.map(|el| {
            // `mix-blend-mode` на ЗАМЕЩАЕМОМ атоме строки: блочный путь,
            // атом с коробкой и флоат смешивают через `grouped`, а `<svg>`,
            // `<iframe>`, `<img>` в строке шли мимо, и режим пропадал
            // (`mix-blend-mode-svg`, `-iframe-parent`, `-iframe-sibling`:
            // красный квадрат вместо зелёного). Носитель несёт ТОЛЬКО режим:
            // маска и обрезка у атомов живут своими путями, полный стиль
            // применил бы их второй раз.
            let el = if replaced_tag(e) && e.style.blend.is_some_and(|b| b != 0) {
                let mut only = Computed::default();
                only.blend = e.style.blend;
                grouped(el, &only)
            } else {
                el
            };
            // Атомарный строчный — transformable element (css-transforms-1
            // §transformable-element: всё по блочной модели, «except for
            // non-replaced inline boxes»): `img`, `iframe`, `inline-block`,
            // `inline-table`, строчный `<svg>`. Обёртку получали только поля
            // форм — они и сейчас заворачиваются в `atom_element`, здесь их
            // пропускаем. Абсолюты через пустышку статической позиции не
            // трогаем: обёртка на нулевой пустышке взяла бы origin от нуля.
            let el = if matches!(
                e.tag.as_str(),
                "input" | "textarea" | "select" | "progress" | "meter"
            ) || matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) {
                el
            } else {
                transformed(el, &e.style, inherited)
            };
            // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
            // ряд строит базовую линию, а кускам с top/middle/bottom нужен
            // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
            // в строке с квадратом прижата к верху, у нас висела на базовой).
            // ПРОБОВАЛИ И ОТКАТИЛИ: выражать `text-top`/`text-bottom` у
            // атомарного куска прижимом к краю строки. Замерено по семьям
            // linebox/*, css1/*, *vertical*: 0 и 0 — этим парам нужен сдвиг
            // относительно ТЕКСТОВОЙ области родителя, а не край строки.
            use crate::computed::Align;
            let self_align = match e.style.vertical_align {
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            let el = match self_align {
                Some(a) => {
                    let mut w = crate::apply::margins(div().flex_shrink_0(), &original_margin);
                    w.style().align_self = Some(a);
                    // Доля куска считается от его КОНТЕЙНЕРА, а обёртка встаёт
                    // между ним и рядом: без своей ширины она сжимается по
                    // содержимому, и `width: 100%` внутри разрешался в ноль —
                    // картинка пропадала целиком (`background-repeat-002-ref`:
                    // `img{vertical-align:top}` + `width="100%"`). Долю
                    // повторяем на обёртке, чтобы отсчёт остался прежним.
                    if let Some(Len::Pct(k)) = e.style.width {
                        w = w.w(gpui::relative(k));
                    }
                    if let Some(Len::Pct(k)) = e.style.height {
                        w = w.h(gpui::relative(k));
                    }
                    w.child(el).into_any_element()
                }
                None => el,
            };
            // Отрицательный `z-index` строчного замещаемого: краска уходит
            // ПОД содержимое до него (CSS 2.1 §9.9 шаг 3) — как у блочного
            // (background-size-document-root-vrl-*: красный маркер обязан
            // лечь под зелёный фон iframe).
            let el = if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
            {
                crate::interact::Underlay::new(el).into_any_element()
            } else {
                el
            };
            // Абсолютная коробка с заданными краями места в строке не
            // занимает — `atom_element` вернул пустышку нулевого размера.
            // Атомом её отдавать нельзя: атом уводит абзац с текстового пути
            // в ряд, и содержащим блоком абсолюта становится коробка ВСЕГО
            // абзаца, а §10.1 п.4 требует прямоугольник фрагментов строчного
            // предка. `Overlay` абзац с текстового пути не уводит.
            if matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) && !at_static_position(&e.style)
                && !matches!(
                    e.tag.as_str(),
                    "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
                )
            {
                return inline::Piece::Overlay(
                    el,
                    inline::OverlayAt {
                        edges: abs_cb,
                        ..Default::default()
                    },
                );
            }
            inline::Piece::Atom(el)
        })
    };
    // Каждому атому — признак, можно ли поставить его В СТРОКУ абзаца
    // (`atom_line_align`): порядок записей совпадает с порядком `Piece::Atom`.
    // Руби несёт ещё и узлы своих аннотаций: по ним строка растёт
    // (`lines::ruby_extent`). Чужие узлы (руби вне строки внутри атома) атому
    // не достаются.
    let mut atom_aligns: Vec<Option<(crate::lines::AtomAlign, crate::lines::RubyExtents)>> =
        Vec::new();
    let mut atom_noted = |e: &Element| -> Option<inline::Piece> {
        let (piece, extents) = crate::lines::collect_ruby_extents(|| atom(e));
        if matches!(piece, Some(inline::Piece::Atom(_))) {
            let extents = if ruby_role(e) == Some(crate::computed::RubyRole::Container) {
                extents
            } else {
                crate::lines::RubyExtents::default()
            };
            // Абсолютная замещаемая — атом строки только РЯДОМ с текстом в
            // потоке: строка из одних внепоточных коробок нулевая (CSS 2.1
            // §9.4.2), а атом завёл бы ей струт (`clear-applies-to-001-ref`:
            // `<div>` с одной абсолютной картинкой вырастал на строку).
            let lone_abs = !flow_text
                && matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                );
            atom_aligns.push(
                (!lone_abs)
                    .then(|| atom_line_align(e, inherited, opts))
                    .flatten()
                    .map(|a| (a, extents)),
            );
        }
        piece
    };
    let mut pieces = inline::collect(nodes, inherited, &mut atom_noted);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    // Ряд пробелов через границу строчной коробки — один пробел (§16.6.1).
    // Проход идёт ПЕРВЫМ: всё дальше считает байтовые смещения по готовому
    // тексту кусков.
    inline::collapse_across_pieces(&mut pieces);
    // Точка переноса показывается знаком по СОСЕДЯМ, а они лежат в других
    // кусках — проход идёт по всему абзацу сразу.
    // Слогораздел идёт ПЕРВЫМ: он меняет сам текст кусков, а всё дальше
    // считает по готовому тексту байтовые смещения.
    inline::hyphenate_pieces(&mut pieces);
    inline::space_transform_pieces(&mut pieces);
    let mut pieces = pieces;
    // Пойдут ли атомы В СТРОКУ (решение то же, что ниже у распорок атомов):
    // тогда атом — содержимое строки, и край для среза пробелов он обрывает.
    let atoms_in_line = {
        let count = pieces
            .iter()
            .filter(|p| matches!(p, inline::Piece::Atom(_)))
            .count();
        count > 0
            && count == atom_aligns.len()
            && atom_aligns.iter().all(Option::is_some)
            && atoms_fit_line(
                inherited,
                atom_aligns
                    .iter()
                    .any(|a| a.as_ref().is_some_and(|(_, ex)| !ex.is_empty())),
            )
    };
    if atoms_in_line {
        inline::trim_edge_spaces_solid_atoms(&mut pieces);
    } else {
        inline::trim_edge_spaces(&mut pieces);
    }
    // Свой `unicode-bidi` у самого абзаца знаками не обрамлялся: их ставит
    // сборка КУСКОВ, а корень абзаца куском не бывает. Из-за этого
    // `bidi-override` на блоке не действовал вовсе (`pre-wrap-align-*-003`:
    // строки шли в исходном порядке вместо перевёрнутого).
    // Только ОТМЕНА и ИЗОЛЯЦИЯ: своё направление письма абзац и так знает —
    // оно уходит в основной уровень разбора двунаправленности.
    // Абзац без текста, из одних атомов (U+FFFC — нейтральные, UAX #9 N1/N2), от знаков
    // изоляции порядка не меняет, а знаки — текстовые куски — уводили её с
    // пути атомов: inline-block'и `dir=rtl`-блока (HTML `[dir] { unicode-bidi:
    // isolate }`) шли слева направо (`anchor-position-005`).
    let own_bidi = inherited.bidi_override == Some(true)
        || (inherited.bidi_isolate == Some(true)
            && pieces.iter().any(|p| {
                matches!(p, inline::Piece::Text { text, .. } if !text.trim().is_empty())
            }));
    let marks = if own_bidi {
        inline::bidi_marks(inherited, inherited)
    } else {
        (None, None)
    };
    if let (Some(open), Some(close)) = marks {
        pieces.insert(
            0,
            inline::Piece::Text {
                text: open.to_string(),
                style: inherited.clone(),
            },
        );
        pieces.push(inline::Piece::Text {
            text: close.to_string(),
            style: inherited.clone(),
        });
        // Жёсткий разрыв ЗАКАНЧИВАЕТ абзац разбора двунаправленности, и знак
        // отмены за ним уже не действует: его приходится ставить заново на
        // каждой строке (`pre-wrap-align-*-003`: перевёрнутой выходила только
        // первая строка).
        let again = format!("{close}\n{open}");
        for piece in pieces.iter_mut() {
            if let inline::Piece::Text { text, .. } = piece
                && text.contains('\n')
            {
                *text = text.replace('\n', &again);
            }
        }
    }
    // Буквица: первая буква абзаца — свой кусок со своим стилем. Кегль куска
    // доезжает до прогона (патч GPUI), поэтому она может быть крупнее строки.
    // Слой с `initial-letter` сюда не доходит: такая буквица уже ушла
    // плавающим узлом (`initial_letter_float`), и следующая буква абзаца не
    // должна стать второй буквицей с кеглем слоя.
    if let Some(first) = inherited
        .first_letter
        .as_deref()
        .filter(|f| f.initial_letter.is_none())
    {
        // Слой — копия стиля блока плюс объявления `::first-letter`
        // (`dom.rs` `layer()`): фон блока в нём чужой, букве его не красить —
        // тот же отсев, что в `initial_letter_float`.
        let mut first = first.clone();
        if first.background == inherited.background {
            first.background = None;
        }
        // Inherited values of the letter come from the element that holds
        // the letter, not from the block (Blink `FirstLetterPseudoElement::
        // StyleForFirstLetter`: parent style = the first letter text's
        // parent; css-pseudo-4 §first-letter-styling, the fictional tag sequence
        // sits inside the innermost element). Values the layer only copied
        // from the block must not override a nested element's own
        // (`display-contents-first-letter-002`: `<span>` color green).
        if first.color == inherited.color {
            first.color = None;
        }
        if first.font_family == inherited.font_family {
            first.font_family = None;
        }
        if first.font_weight == inherited.font_weight {
            first.font_weight = None;
        }
        if first.italic == inherited.italic {
            first.italic = None;
        }
        pieces = inline::split_first_letter(pieces, &first);
    }
    if first_line_at > 0 {
        pieces = inline::style_first_line(pieces, first_line_at, first_line);
    }
    // Межсловный интервал и отступ первой строки требуют строки из слов, а
    // единый текстовый блок их не умеет — поэтому решение принимается ДО
    // сборки блока, иначе оба свойства молча пропадали.
    let word = match inherited.word_spacing {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Отступ первой строки. Абсолютную часть разбор уже свёл к точкам, доля
    // же берётся от ширины содержащего блока и здесь ещё неизвестна — её
    // считает раскладка строк, когда ширина решена.
    // `calc(50% - 3px)` несёт обе части разом: `Paragraph` складывает
    // `px + pct × ширина строки` сам (`lines.rs`, css-text-3 §2.1).
    let mixed = match inherited.text_indent {
        Some(Len::Calc(i)) => crate::value::calc_get(i).pct_px(),
        _ => None,
    };
    let indent = crate::lines::Indent {
        px: match inherited.text_indent {
            Some(Len::Px(v)) => v,
            _ => mixed.map_or(0.0, |(_, px)| px),
        },
        pct: match inherited.text_indent {
            Some(Len::Pct(k)) => k,
            _ => mixed.map_or(0.0, |(pct, _)| pct),
        },
        each_line: inherited.text_indent_each_line == Some(true),
        hanging: inherited.text_indent_hanging == Some(true),
    };
    // Межсловный интервал считает своя раскладка строк: ряд из слов ломает
    // выключку, висящие пробелы и перенос. Ряд остаётся только под отступ
    // первой строки, который своей раскладке пока неизвестен.
    // Отступ первой строки умеет своя раскладка строк: она одна знает, где
    // строка кончается, и отрицательный отступ ей не помеха. Ряд из слов
    // остаётся запасным путём — на нём отступ становится распоркой, а она
    // отрицательной ширины не бывает.
    // Ряда из слов под отступ первой строки больше нет: `lines::rules` отдаёт
    // правила переноса ВСЕГДА, и своя раскладка строк умеет и отступ, и
    // отрицательный отступ.
    // Атомы — В СТРОКУ абзаца (CSS 2.1 §9.2.2, §10.8): каждый становится
    // распоркой (U+FEFF), её продвижение — ширина атома, а сам элемент
    // раскладывает и ставит на базовую линию своей строки `Paragraph`. Для
    // переноса распорка атома читается как U+FFFC (`Paragraph::linebreaks`):
    // так атом кладёт и Blink (`inline_items_builder.cc`, знак-заместитель
    // объекта), а класс CB даёт разрыв до и после (UAX #14 LB20).
    // Прежде абзац с атомом уходил в гибкий ряд слов (`as_wrapped_row`): одна
    // высота строки на ряд, базовая линия текста taffy не видна, `top`/
    // `bottom`/`text-top` не выражались.
    let mut line_atoms: Vec<(
        usize,
        AnyElement,
        crate::lines::AtomAlign,
        crate::lines::RubyExtents,
    )> = Vec::new();
    let atom_count = pieces
        .iter()
        .filter(|p| matches!(p, inline::Piece::Atom(_)))
        .count();
    if atoms_in_line && atom_count == atom_aligns.len() {
        let mut aligns = atom_aligns.into_iter().flatten();
        let mut at = 0usize;
        let mut out = Vec::with_capacity(pieces.len() + 2 * atom_count);
        let mut mark = inherited.clone();
        mark.word_space_char = None;
        mark.letter_spacing = Some(Len::Px(0.0));
        mark.inline_bg = None;
        mark.inline_border = None;
        for p in pieces {
            match p {
                inline::Piece::Atom(el) => {
                    let (align, extents) = aligns
                        .next()
                        .unwrap_or((crate::lines::AtomAlign::Shift(0.0), crate::lines::RubyExtents::default()));
                    line_atoms.push((at, el, align, extents));
                    out.push(inline::Piece::Text {
                        text: inline::SPACER.to_string(),
                        style: mark.clone(),
                    });
                    at += inline::SPACER.len();
                }
                inline::Piece::Text { text, style } => {
                    at += text.len();
                    out.push(inline::Piece::Text { text, style });
                }
                other => out.push(other),
            }
        }
        pieces = out;
    }
    let edges = edge_pieces(&pieces, inherited, opts);
    if inline::single_block(&pieces, opts.base_size())
        && let Some((text, runs)) = inline::text_and_runs(&pieces, &opts.text)
    {
        // `word-space-transform` смотрит на СОСЕДЕЙ точки переноса, а они
        // сплошь и рядом лежат в других кусках (`あ<wbr>い` — это три куска:
        // текст, точка, текст). По кускам преобразование их не видит, поэтому
        // идёт по собранному тексту абзаца. Замена знак-в-знак: нулевой
        // пробел и идеографический занимают в UTF-8 одинаково, поэтому
        // прогоны не съезжают.
        // Строка растёт под самый крупный кусок — как коробка строки в CSS.
        // Иначе крупный `<span>` вылезал бы на соседние строки.
        let biggest = inline::max_font_size(&pieces, own_size(inherited, opts), opts.base_size());
        // Прогон без своего кегля набирается кеглем АБЗАЦА, а им здесь стоит
        // самый крупный кусок: текст блока без объявленного `font-size` рядом
        // с крупным `<span>` вырастал до его кегля (`c43-rpl-ibx-000`: вся
        // строка в 3.75em). Свой кегль такого куска — кегль блока.
        let boxes = line_box_spans(&pieces, &edges, inherited, opts);
        let mut runs = runs;
        if (!line_atoms.is_empty() || !edges.is_empty() || boxes.is_some())
            && inherited.text_fit.is_none()
        {
            let own = own_size(inherited, opts);
            if biggest != own {
                for run in runs.iter_mut().filter(|r| r.font_size.is_none()) {
                    run.font_size = Some(gpui::px(own));
                }
            }
        }
        let mut opts = opts.clone();
        if biggest != opts.base_size() {
            opts.text.line_height = gpui::px(biggest * normal_fraction(inherited, &opts)).into();
        }
        let opts = &opts;
        // Selection controls handlers, while native text retains its layout and paint.
        let wrap_rules = crate::lines::rules(inherited);
        let native = wrap_rules.is_some() && native_request.is_some_and(|request| request.accepts(&pieces, !line_atoms.is_empty()));
        let selectable = inherited.no_select != Some(true) && inherited.pointer_events_none != Some(true);
        if !native && !selectable {
            return gpui::StyledText::new(SharedString::from(text))
                .with_runs(runs)
                .into_any_element();
        }
        // Native construction remains available without installing selection handlers.
        if let Some(wrap) = wrap_rules {
            let inherited = if native { native_request.unwrap().style } else { inherited };
            // Кегль абзаца — самый крупный кусок в нём: строка растёт под него,
            // и от него же считается высота строки в долях.
            //
            // ПРОБОВАЛИ И ОТКАТИЛИ: считать долю от кегля САМОГО блока
            // (струт §10.8), раз крупный кусок теперь растит строку каналом
            // `lh_spans`. Замерено: приобретено 4, потеряно 4 — три пары
            // `*-applies-to-008` уходят с 0.02 на 0.67. Возвращать вместе с
            // разбором `vertical-align: top/bottom` на тексте.
            // Куски у края строки в её струт не входят (§10.8.1): у
            // `vertical-align-121` строка из 30px текста и прижатого вверх
            // 60px куска — это 30px струта плюс вылет куска вниз, а не 60px
            // с текстом посередине.
            let (flow_biggest, flow_lh) = if edges.is_empty() {
                (biggest, 0.0)
            } else {
                flow_metrics(&pieces, &edges, inherited, opts)
            };
            let line = match inherited.line_height {
                Some(Len::Px(v)) => gpui::px(v),
                Some(Len::Pct(k)) => gpui::px(k * flow_biggest),
                Some(Len::Em(k)) => gpui::px(k * flow_biggest),
                _ if !edges.is_empty() => gpui::px(flow_lh),
                // Своей `line-height` у блока нет — её задают КУСКИ: у куска
                // со своей высотой строки она и берётся, у остальных доля от
                // кегля (§10.8.1). Канал `lh_spans` умеет строку только
                // растить, и объявленная `font: 100px/1` терялась.
                _ => gpui::px(inline::max_line_height(
                    &pieces,
                    own_size(inherited, opts),
                    opts.base_size(),
                    normal_fraction(inherited, opts),
                )),
            };
            // Построчные коробки (`line_box_spans`): высота строки абзаца — СТРУТ
            // блока, крупные куски растят только свои строки.
            let line = match &boxes {
                Some((_, strut)) => gpui::px(*strut),
                None => line,
            };
            let id = gpui::ElementId::Integer(text_id(&text));
            let family = inherited.font_family.clone().unwrap_or_default();
            let para = crate::lines::Paragraph::new(
                SharedString::from(text),
                runs,
                gpui::px(biggest),
                line,
                crate::lines::align_for(inherited),
                wrap,
            )
            // Preserve wrapping, direction and the sideways alphabetic baseline.
            .opaque_background(inherited)
            .reversed_lines(inherited.lines_reversed == Some(true))
            .vertical(inherited.para_vertical.is_some(), inherited.para_vertical == Some(true))
            .ortho_limit(inherited.ortho_limit.map(px))
            .vertical_central_baseline(inherited.sideways != Some(true) && inherited.text_sideways != Some(true))
            .rotated_central(
                inherited.rotated_line == Some(true)
                    && inherited.sideways != Some(true)
                    && inherited.text_sideways != Some(true),
            )
            .vertical_counter_clockwise(inherited.para_vertical == Some(false) && inherited.sideways == Some(true))
            .vertical_inline_constraint(inherited.orthogonal_inline, native_vertical::keyword(inherited))
            .plaintext(
                inherited
                    .bidi_plaintext
                    .unwrap_or(false)
                    .then(|| {
                        inherited
                            .text_align
                            .unwrap_or(crate::computed::TextAlign::Start)
                    })
                    .filter(|a| {
                        matches!(
                            a,
                            crate::computed::TextAlign::Start | crate::computed::TextAlign::End
                        )
                    }),
            )
            .spans(inline::wrap_spans(&pieces, inherited))
            .word_spans(inline::word_spans(&pieces, biggest))
            // Автозазоры идут ПЕРВЫМИ: поиск диапазона берёт первое
            // попадание, и зазор обязан перебить трекинг всего куска.
            .letter_spans(
                [
                    inline::autospace_spans(&pieces, biggest),
                    inline::letter_spans(&pieces, biggest),
                ]
                .concat(),
            )
            .shift_spans({
                let mut v = inline::shift_spans(&pieces, biggest, f32::from(line));
                v.retain(|(r, _)| !in_edge(&edges, r));
                v
            })
            .lh_spans({
                let mut v = inline::line_height_spans(
                    &pieces,
                    inherited,
                    biggest,
                    crate::metrics::normal_line(&inherited.font_family.clone().unwrap_or_default()),
                );
                v.retain(|(r, _)| !in_edge(&edges, r));
                v
            })
            // В повёрнутом абзаце руби в строку не идёт и строку не растит
            // (`atoms_fit_line`), а эталоны акцента сделаны из руби: рост
            // только у горизонтального (`text-emphasis-line-height-003*/004*`).
            .emph_spans(
                if inherited.rotated_line == Some(true) || inherited.vertical == Some(true) {
                    Vec::new()
                } else {
                    inline::emphasis_spans(
                        &pieces,
                        biggest,
                        crate::metrics::normal_line(&inherited.font_family.clone().unwrap_or_default()),
                    )
                },
            )
            // Линии украшений рисует сам абзац (css-text-decor-3 §2); у
            // повёрнутого — прежний путь набора.
            .decor_spans(inline::decor_spans(&pieces, &opts.text))
            .edge_spans(edges)
            .line_boxes(
                boxes.as_ref().map(|b| b.0.clone()).unwrap_or_default(),
                boxes.as_ref().map(|_| {
                    (
                        inline::strut_font(inherited, &opts.text),
                        gpui::px(own_size(inherited, opts)),
                    )
                }),
            )
            .rel_spans(inline::rel_spans(&pieces))
            .ruby_justify(inherited.ruby_justify == Some(true), inherited.ruby_unit)
            .justify_chars(inherited.justify_chars.unwrap_or(1))
            .align_last(
                inherited
                    .text_align_last
                    .map(|a| a.physical(inherited.rtl == Some(true)))
                    .map(crate::lines::align_of_value)
                    // `text-justify: none` forbids justification of the last
                    // line too: it aligns as `start` (css-text-3 §7.3,
                    // `text-justify-none-001` with `text-align-last: justify`).
                    .map(|a| match a {
                        crate::lines::Align::Justify if inherited.no_justify == Some(true) => {
                            if inherited.rtl == Some(true) {
                                crate::lines::Align::Right
                            } else {
                                crate::lines::Align::Left
                            }
                        }
                        other => other,
                    }),
            )
            .letter_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.letter_spacing,
                &family,
                biggest,
            )))
            .word_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.word_spacing,
                &family,
                biggest,
            )))
            .hanging(inherited.hanging)
            .indent(indent)
            .spacers(inline::spacers(&pieces))
            .spacer_edges(inline::spacer_edges(&pieces))
            .box_extents(inline::box_extents(&pieces))
            .flow_shapes(
                inherited
                    .flow_shapes
                    .clone()
                    .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new()))),
            )
            // Счётный режим (`line-clamp: <N>`) берёт предел из стиля;
            // авто-режим — из бюджета, посчитанного по точке среза.
            .line_clamp(clamp_budget.or_else(|| inherited.clamp_lines().map(|n| n as usize)))
            .clamp_marked(clamp_budget.is_some())
            .text_ellipsis(
                inherited.ellipsis == Some(true)
                    && inherited
                        .overflow_x
                        .is_some_and(|o| o != crate::computed::Overflow::Visible),
            )
            .overflow_marker(
                inherited.overflow_marker.clone(),
                Some(measure_font(inherited, opts)),
                Some(gpui::px(own_size(inherited, opts))),
            )
            .clamp_mark(inherited.clamp_mark.clone())
            .clamp_tag(clamp_tag)
            // Знак обрыва — анонимный строчный ребёнок блока: и
            // `visibility` у него блочная (css-overflow-3 §text-overflow,
            // css-overflow-4 §block-ellipsis): у скрытого блока знака не
            // видно, даже если кусок у среза `visible`
            // (`text-overflow-ellipsis-002`, `webkit-line-clamp-035`).
            .marker_color(Some(if inherited.hidden == Some(true) {
                gpui::transparent_black()
            } else {
                inherited
                    .color
                    .map(crate::value::Color::to_hsla)
                    .unwrap_or_else(gpui::black)
            }))
            .text_fit(inherited.text_fit)
            .fit_parts(
                // Масштабируемы только интервалы в ДОЛЯХ кегля; `px` и `em`
                // (от вычисленного кегля) подбор не трогает.
                [inherited.letter_spacing, inherited.word_spacing]
                    .iter()
                    .all(|l| matches!(l, None | Some(Len::Pct(_)))),
                matches!(
                    inherited.line_height,
                    Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Ex(_)) | Some(Len::Ch(_))
                ),
            )
            .hyphen_char(inherited.hyphen_char.clone())
            .tab_stops(inline::tab_stops(&pieces, inherited, &opts.text))
            .overlays(inline::overlays(pieces))
            .atoms(line_atoms)
            .ruby_trim(inherited.text_box_trim_start, inherited.text_box_trim_end);
            let para = if selectable {
                para.selectable(id, opts.selection_color())
            } else {
                para
            };
            if native {
                native_request.unwrap().built.set(true);
            }
            return para.into_any_element();
        }
    }
    // Ряд из слов и атом сходятся по базовой линии по РАЗНЫМ правилам: GPUI
    // базовой линии текста в taffy не отдаёт вовсе (`vendor/gpui`), и для
    // куска текста taffy берёт НИЖНИЙ край коробки
    // (`vendor/taffy/src/compute/flexbox.rs:1704`, `height + margin.bottom`),
    // а вложенный атом свою первую базовую линию пропагирует
    // (`flexbox.rs:405`). Кусок заявляет 1.0em, атом — 0.8em: атом тонет на
    // спуск шрифта, а короб строки растёт до 1.2em вместо `line-height`
    // (§10.8). Замерено зондом `target/probe/atom-probe2.html`: при
    // `font: 100px/1 Ahem` короб 120, атом на +20.
    let has_atom = pieces.iter().any(|p| matches!(p, inline::Piece::Atom(_)));
    let em_base = opts.base_size();
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        // Стоячие знаки в вертикальном письме (`text-orientation: mixed`,
        // CJK): набор идёт вертикальными формами шрифта — возможность `vert`
        // подставляет глиф, а продвижение берётся из его вертикальных метрик
        // (css-writing-modes-3 §7.3, реализация в DirectWrite-слое). Пока
        // только для кусков, стоячих ЦЕЛИКОМ: смешанный кусок потребовал бы
        // резки на прогоны по ориентации.
        let vert_style;
        let style = if style.rotated_line == Some(true)
            && style.sideways != Some(true)
            && t.chars().any(upright_in_mixed)
            && t.chars().all(|c| c.is_whitespace() || upright_in_mixed(c))
        {
            let mut s = style.clone();
            s.font_features.push(("vert".into(), 1));
            vert_style = s;
            &vert_style
        } else {
            style
        };
        // На кусок текста идут ТОЛЬКО текстовые свойства: фон, отступы и
        // рамка принадлежат абзацу целиком, а не каждому его слову.
        let d = apply(div(), &style.text_only())
            .max_w_full()
            .child(SharedString::from(t.clone()));
        d.into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = inherited
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(inherited.rtl == Some(true));
    inline::as_wrapped_row(
        pieces,
        inherited.vertical_align,
        Some(align),
        inherited.rtl == Some(true),
        match inherited.text_indent {
            Some(Len::Px(v)) => v,
            // Ряд из слов долю и прежде не применял (`Pct` идёт нулём); у
            // смеси берутся хотя бы точки — как у чистых точек.
            Some(Len::Calc(i)) => crate::value::calc_get(i).pct_px().map_or(0.0, |(_, px)| px),
            _ => 0.0,
        },
        inherited.nowrap == Some(true),
        &mut render_text,
        inherited.vertical != Some(true) && inherited.rotated_line != Some(true),
    )
}

/// Единица руби (css-ruby-1 §2.3.2): содержимое одной базы или одной
/// аннотации. Пустой вектор — анонимная пустая единица, добавленная спариванием.
pub(crate) type RubyUnit = Vec<Node>;

/// Уровень аннотаций сегмента: `<rtc>` или ряд `<rt>` прямо в контейнере
/// (анонимный контейнер аннотаций, css-ruby-1 §2.2 п.8).
pub(crate) struct RubyLevel {
    pub(crate) units: Vec<RubyUnit>,
    /// `<rtc>` без `<rt>` внутри — одна анонимная аннотация, накрывающая ВСЕ
    /// базы сегмента (§2.3.2 «spanning annotation»).
    pub(crate) spanning: bool,
    /// Стиль самого `<rtc>`: его аннотации наследуют от него (в том числе
    /// половинный кегль из листа агента).
    pub(crate) container: Option<Computed>,
}

/// Сегмент руби (css-ruby-1 §2.3.1): ряд баз и уровни аннотаций к нему.
pub(crate) struct RubySegment {
    pub(crate) bases: Vec<RubyUnit>,
    pub(crate) levels: Vec<RubyLevel>,
}

/// Пуста ли единица: только схлопываемые пробелы и руби-теги без содержимого.
/// Любой другой элемент — содержимое, даже пустой `<div>` с шириной
/// (`ruby-align-001`: `rt > div { width: 160px }`).
pub(crate) fn ruby_unit_blank(unit: &[Node]) -> bool {
    unit.iter().all(|n| match n {
        Node::Text(t) => blank_text(t),
        Node::Element(k) if ruby_role(k).is_some_and(|r| r != crate::computed::RubyRole::Container) => {
            ruby_unit_blank(&k.children)
        }
        Node::Element(_) => false,
    })
}

/// Роль элемента в руби (css-ruby-1 §2.1): своё `display: ruby*`, иначе —
/// тег (A.1: `ruby/rb/rt/rbc/rtc`). Авторский `display` на руби-теге роль
/// СНИМАЕТ (`display: block` на `<rt>` — обычный блок, как в Blink, где
/// `IsInlineRubyText` смотрит на `Display()`, а не на тег): роль по тегу
/// действует только без своего `display`.
pub(crate) fn ruby_role(e: &Element) -> Option<crate::computed::RubyRole> {
    use crate::computed::RubyRole;
    if let Some(role) = e.style.ruby_role {
        return Some(role);
    }
    if e.style.display.is_some() {
        return None;
    }
    match e.tag.as_str() {
        "ruby" => Some(RubyRole::Container),
        "rb" => Some(RubyRole::Base),
        "rt" => Some(RubyRole::Text),
        "rbc" => Some(RubyRole::BaseContainer),
        "rtc" => Some(RubyRole::TextContainer),
        _ => None,
    }
}

/// Разрезать детей `<ruby>` на сегменты и единицы (css-ruby-1 §2.2 п.3-8, §2.3).
///
/// База после аннотации открывает новый сегмент. Пробелы между руби-коробками
/// решаются по соседям (§2.2 п.4-6): краевые и межуровневые (база →
/// аннотация) выбрасываются; между двумя `<rb>` — своя база, между двумя
/// `<rt>` — своя аннотация; аннотация → база — межсегментный, свой сегмент из
/// одной анонимной базы (так его рисуют эталоны `ruby-box-generation-*`).
/// Прочий строчный контент образует анонимную базу (п.3). `<rp>` не
/// показывается (A.1). Аннотации внутри `<rbc>` (неправильно вложенные, п.2)
/// пока идут содержимым базы — анонимный руби-контейнер для них: шаг 3.
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v146, `scout-ruby-2026-09d.md` шаг 4):
/// дополнительный лидинг §3.4 полем на атом руби (`ruby_leading` +
/// `row.mt/mb`). Срез css-ruby+css-transforms+css-masking+filter-effects+
/// css-writing-modes+css-inline+css-overflow+css-break 3936: −7 —
/// `ruby-align-001/001a/space-around` (0.05…0.08 → 0.69…1.01),
/// `rt-display-001` (0.01 → 0.62), `ruby-lang-specific-style-001`,
/// `ruby-overhang-none`, `ruby-tab-in-base-002`; обещанных плюсов срез
/// не показал. Поле на атоме растит короб строки, но и сдвигает базу
/// относительно соседей — нужен настоящий лидинг строки, а не поле.
pub(crate) fn ruby_segments(children: &[Node]) -> Vec<RubySegment> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Text,
        Rb,
        Rbc,
        Rt,
        Rtc,
        Blank,
        Drop,
    }
    // Вид — по РОЛИ (тег или `display: ruby*`, `ruby_role`): `span.rt
    // { display: ruby-text }` — аннотация (`rt-display-001`), `span#rbc
    // { display: ruby-base-container }` — контейнер баз (`rbc-rtc-basic-001`).
    let kind_of = |n: &Node| match n {
        Node::Text(t) if blank_text(t) => Kind::Blank,
        Node::Text(_) => Kind::Text,
        Node::Element(k) if k.tag == "rp" => Kind::Drop,
        Node::Element(k) => match ruby_role(k) {
            Some(crate::computed::RubyRole::Base) => Kind::Rb,
            Some(crate::computed::RubyRole::BaseContainer) => Kind::Rbc,
            Some(crate::computed::RubyRole::Text) => Kind::Rt,
            Some(crate::computed::RubyRole::TextContainer) => Kind::Rtc,
            _ => Kind::Text,
        },
    };
    let base_kind = |k: Kind| matches!(k, Kind::Text | Kind::Rb | Kind::Rbc);
    let ann_kind = |k: Kind| matches!(k, Kind::Rt | Kind::Rtc);
    let kinds: Vec<Kind> = children.iter().map(kind_of).collect();
    // Ближайший непробельный сосед слева (`-1`) или справа (`1`).
    let neighbour = |from: usize, step: isize| -> Option<Kind> {
        let mut i = from as isize + step;
        while i >= 0 && (i as usize) < kinds.len() {
            let k = kinds[i as usize];
            if !matches!(k, Kind::Blank | Kind::Drop) {
                return Some(k);
            }
            i += step;
        }
        None
    };
    fn fresh() -> RubySegment {
        RubySegment { bases: Vec::new(), levels: Vec::new() }
    }
    fn flush_run(run: &mut RubyUnit, cur: &mut RubySegment) {
        if !run.is_empty() {
            cur.bases.push(std::mem::take(run));
        }
    }
    fn close_segment(cur: &mut RubySegment, out: &mut Vec<RubySegment>) {
        if !cur.bases.is_empty() || !cur.levels.is_empty() {
            out.push(std::mem::replace(cur, fresh()));
        }
    }
    // База после аннотаций — новый сегмент (§2.3.1). Явный `<rbc>` — свой
    // контейнер баз, а сегмент — ОДИН контейнер баз с аннотациями за ним
    // (§2.3.1): база после `<rbc>` его не продолжает, даже без аннотаций.
    // Прежде `<rbc>e</rbc><rbc>f</rbc><rbc>g</rbc><rtc>h</rtc>` склеивались
    // в один сегмент, и `h` вставала над `e`, а не над `g`
    // (`ruby-box-generation-001-ref`).
    fn base_starts(cur: &mut RubySegment, out: &mut Vec<RubySegment>, loose_level: &mut bool, sealed: &mut bool) {
        if !cur.levels.is_empty() || *sealed {
            close_segment(cur, out);
            *loose_level = false;
        }
        *sealed = false;
    }
    let mut out: Vec<RubySegment> = Vec::new();
    let mut cur = fresh();
    // Анонимная база из текста и строчных элементов (§2.2 п.3).
    let mut run: RubyUnit = Vec::new();
    // Открыт ли анонимный уровень из `<rt>` прямо в контейнере.
    let mut loose_level = false;
    // Базы текущего сегмента пришли из явного `<rbc>`.
    let mut sealed = false;
    for (i, node) in children.iter().enumerate() {
        match kinds[i] {
            Kind::Drop => {}
            Kind::Text => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                run.push(node.clone());
            }
            Kind::Rb => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                flush_run(&mut run, &mut cur);
                cur.bases.push(vec![node.clone()]);
            }
            Kind::Rbc => {
                base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                flush_run(&mut run, &mut cur);
                // Анонимные базы перед `<rbc>` — свой сегмент.
                close_segment(&mut cur, &mut out);
                loose_level = false;
                sealed = true;
                let Node::Element(k) = node else { continue };
                // Внутри `<rbc>`: каждый `<rb>` — база, пробел между двумя
                // `<rb>` — своя база, краевые пробелы — вон, прочее —
                // анонимная база.
                let kids: Vec<Kind> = k.children.iter().map(kind_of).collect();
                let mut inner: RubyUnit = Vec::new();
                for (j, c) in k.children.iter().enumerate() {
                    match kids[j] {
                        Kind::Rb => {
                            if !inner.is_empty() {
                                cur.bases.push(std::mem::take(&mut inner));
                            }
                            cur.bases.push(vec![c.clone()]);
                        }
                        Kind::Blank => {
                            let prev = kids[..j].iter().rev().copied().find(|k| *k != Kind::Blank);
                            let next = kids[j + 1..].iter().copied().find(|k| *k != Kind::Blank);
                            match (prev, next) {
                                (Some(Kind::Rb), Some(Kind::Rb)) => cur.bases.push(vec![c.clone()]),
                                (Some(Kind::Text), Some(_)) | (Some(_), Some(Kind::Text)) => {
                                    inner.push(c.clone())
                                }
                                _ => {}
                            }
                        }
                        Kind::Drop => {}
                        _ => inner.push(c.clone()),
                    }
                }
                if !inner.is_empty() {
                    cur.bases.push(inner);
                }
            }
            Kind::Rt => {
                flush_run(&mut run, &mut cur);
                if !loose_level {
                    cur.levels.push(RubyLevel { units: Vec::new(), spanning: false, container: None });
                    loose_level = true;
                }
                cur.levels.last_mut().expect("уровень только что открыт").units.push(vec![node.clone()]);
            }
            Kind::Rtc => {
                flush_run(&mut run, &mut cur);
                loose_level = false;
                let Node::Element(k) = node else { continue };
                let rts: Vec<RubyUnit> = k
                    .children
                    .iter()
                    .filter(|c| {
                        matches!(c, Node::Element(r)
                            if ruby_role(r) == Some(crate::computed::RubyRole::Text))
                    })
                    .map(|c| vec![c.clone()])
                    .collect();
                let spanning = rts.is_empty();
                let units = if spanning {
                    // Одна анонимная аннотация из всего содержимого, без
                    // краевых пробелов (§2.2 п.4).
                    let mut all: Vec<Node> = k.children.clone();
                    while matches!(all.first(), Some(Node::Text(t)) if blank_text(t)) {
                        all.remove(0);
                    }
                    while matches!(all.last(), Some(Node::Text(t)) if blank_text(t)) {
                        all.pop();
                    }
                    vec![all]
                } else {
                    rts
                };
                cur.levels.push(RubyLevel { units, spanning, container: Some(k.style.clone()) });
            }
            Kind::Blank => match (neighbour(i, -1), neighbour(i, 1)) {
                // Краевой пробел контейнера (п.4).
                (None, _) | (_, None) => {}
                // Межуровневый: база → аннотация (п.5).
                (Some(p), Some(n)) if base_kind(p) && ann_kind(n) => {}
                // Пробел у явного `<rbc>` — между двумя контейнерами баз, то
                // есть межсегментный (п.6): свой сегмент, иначе аннотация
                // после следующей базы спарилась бы с ним (эталон
                // `ruby-box-generation-001`: `<rbc><rb><span> </span></rb></rbc>`).
                (Some(p @ (Kind::Rb | Kind::Rbc)), Some(n @ (Kind::Rb | Kind::Rbc)))
                    if p == Kind::Rbc || n == Kind::Rbc =>
                {
                    flush_run(&mut run, &mut cur);
                    close_segment(&mut cur, &mut out);
                    loose_level = false;
                    sealed = false;
                    out.push(RubySegment { bases: vec![vec![node.clone()]], levels: Vec::new() });
                }
                // Межбазовый (п.6): своя единица, спаривается по порядку.
                (Some(Kind::Rb | Kind::Rbc), Some(Kind::Rb | Kind::Rbc)) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    flush_run(&mut run, &mut cur);
                    cur.bases.push(vec![node.clone()]);
                }
                // Пробел внутри анонимной базы.
                (Some(p), Some(n)) if base_kind(p) && base_kind(n) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    run.push(node.clone());
                }
                // Межаннотационный (п.6) — только между двумя `<rt>` контейнера.
                (Some(Kind::Rt), Some(Kind::Rt)) if loose_level => {
                    cur.levels.last_mut().expect("уровень открыт").units.push(vec![node.clone()]);
                }
                // Аннотация → строчное содержимое: пробел открывает анонимную
                // базу следующего сегмента вместе с этим содержимым (§2.2
                // п.3: анонимная база оборачивает ПОДРЯД идущие строчные
                // коробки, пробел — тоже строчный текст). Эталон
                // `ruby-box-generation-001` так и пишет:
                // `<rb><span> <span>l</span> </span></rb>`.
                (Some(p), Some(Kind::Text)) if ann_kind(p) => {
                    base_starts(&mut cur, &mut out, &mut loose_level, &mut sealed);
                    run.push(node.clone());
                }
                // Межсегментный (п.6): аннотация → база — свой сегмент.
                (Some(p), Some(n)) if ann_kind(p) && base_kind(n) => {
                    close_segment(&mut cur, &mut out);
                    loose_level = false;
                    sealed = false;
                    out.push(RubySegment { bases: vec![vec![node.clone()]], levels: Vec::new() });
                }
                _ => {}
            },
        }
    }
    flush_run(&mut run, &mut cur);
    close_segment(&mut cur, &mut out);
    out
}

/// Точки стороны коробки: только явный `px` (None непроходной).
pub(crate) fn px_of2(l: &Option<Len>) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    }
}

/// `inset()`/`rect()`/`xywh()` with a `round` radius in points: the group
/// buffer rounds the clip rectangle (css-shapes-1 §basic-shape-rect);
/// percentages resolve against the reference box at paint time.
pub(crate) fn rounded_rect_clip(c: &Computed) -> bool {
    matches!(c.clip_round_len, Some(Len::Px(v) | Len::Pct(v)) if v > 0.0)
        && (c.clip_inset.is_some()
            || c.clip_edges.is_some()
            || c.clip_xywh.is_some()
            || c.clip_polygon.is_some())
}

/// Отрисовать поддерево в отдельный буфер, когда эффекту нужна готовая
/// картинка целиком.
///
/// Таких случаев три: размытие поддерева (`filter: blur`), смешивание с
/// кадром по формулам CSS (`mix-blend-mode`) и изоляция (`isolation`), где
/// поддерево обязано сложиться отдельно, прежде чем попасть в кадр.
pub(crate) fn grouped(el: AnyElement, c: &Computed) -> AnyElement {
    let blur = c.filter.map_or(0.0, |f| f.blur);
    let blend = c.blend.unwrap_or(0);
    // Вершины полигона в `em`/`ex`/`ch`/`vw`/`vh` (css-shapes-1 `polygon()`:
    // `<length-percentage>`) меряются ЗДЕСЬ, как у `clip: rect()` ниже: на
    // разборе кегль и окно неизвестны, а отрисовка (`Grouped::paint`, `coord`)
    // знает только точки и доли — остальное шло нулём, и полоса схлопывалась
    // в линию (clip-path-polygon-013: 4 полосы из 6, 30400/480000 = 6.33).
    // Смесь `calc()` по-прежнему отбрасывается ещё на разборе.
    let poly_font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let poly_family = c.font_family.clone().unwrap_or_default();
    let poly_vp = PAINT_VIEWPORT.with(|v| v.get());
    let poly_unit = |l: Len| -> Len {
        match l {
            Len::Px(_) | Len::Pct(_) => l,
            Len::Vw(k) => Len::Px(k * poly_vp.0),
            Len::Vh(k) => Len::Px(k * poly_vp.1),
            other => crate::metrics::fallback_len_px(other, &poly_family, poly_font)
                .map(Len::Px)
                .unwrap_or(other),
        }
    };
    let polygon_px: Vec<(Len, Len)> = c
        .clip_polygon
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|&(x, y)| (poly_unit(x), poly_unit(y)))
        .collect();
    let polygon = polygon_px.as_slice();
    // Маска-изображение (css-masking §7.1): источник уходит строкой, его
    // альфа гасит готовый буфер группы при композите; резолв — при
    // отрисовке, когда известен размер коробки. Базовая форма `clip-path`
    // (circle/ellipse) идёт тем же путём — растровой альфа-маской, как и
    // эллиптический `border-radius: H / V` (углы rx≠ry растеризатор круглить
    // не умеет; круглые пары дополняются из обычного радиуса).
    // Большой НЕОДНОРОДНЫЙ круглый радиус — тоже маской: растеризатор жмёт
    // каждый угол к половине меньшей стороны, а спека — одним множителем от
    // суммы СМЕЖНЫХ радиусов (§5.5): `border-radius: 100px 100px 0 0` на
    // 200x100 — законный полукруг, растеризатор рисовал стадион
    // (clip-path-semicircle-ref). Однородные радиусы совпадают с растеризатором.
    // Фигурные углы (`corner-shape`, css-borders-4) — той же маской: запись
    // несёт радиусы (точки либо доли, резолв при растре) и параметр K по углам.
    // Радиус в единицах шрифта на `e.style` ещё не разрешён: `rrect_spec`
    // читает только точки и доли и писал бы `0` — маску БЕЗ скругления поверх
    // квада, с которого `apply_radius` скругление снял. Меряем тем же
    // `poly_unit`, что вершины полигона (`contain-paint-clip-002`: 4em = 64 →
    // ужатие css-backgrounds-3 §5.5 до 60). Blink берёт форму обрезки из
    // вычисленного стиля (`paint_property_tree_builder.cc:3127-3143`).
    let rrect = c.radius_masked().then(|| {
        let mut own = c.clone();
        own.resolve_radius_lengths(|r| *r = r.map(poly_unit));
        format!("shape:{}", crate::background::rrect_spec(&own, None))
    });
    // `border-shape` (css-borders-4): фон и содержимое режутся ВНЕШНИМ
    // контуром рамки (Blink клипует фон внешней фигурой, у двух фигур —
    // внутренней; кольцо у нас лежит непрозрачным слоем сверху, итог тот же).
    // Две фигуры при ПРОЗРАЧНОЙ рамке: кольца не видно, а фон обязан
    // исчезнуть вместе с внутренней фигурой — маска берёт её
    // (border-shape-collapsed-shape-clips-background, t3). Запись:
    // `t r b l` опорной коробки, обводка, `t r b l` выноса, `:`, фигура.
    let bshape = c.border_shape.as_ref().map(|bs| {
        let (stroke, colour) = c.border_shape_stroke();
        // Рамка, несущая заливку `border-area`, видима — маска берёт внешнюю
        // фигуру, как у цветной рамки.
        let colour = crate::background::border_paint(c, colour);
        // Обрезка переполнения — ВНУТРЕННИМ контуром (css-borders-4
        // §border-shape-overflow-interaction; Blink `InnerPath`): у двух фигур
        // внутренняя, у одной — внешняя минус обводка (отрицательная обводка
        // в записи, `background::border_shape_mask_svg`). Кольцо при этом
        // ложится НАД буфером (`Grouped::over`, ниже).
        let (shape, kind, stroke) = match &bs.inner {
            Some((inner, k)) if colour.a <= 0.0 || c.border_shape_clips() => (inner.as_str(), *k, 0.0),
            Some(_) => (bs.outer.as_str(), bs.outer_box, 0.0),
            None if c.border_shape_clips() => (bs.outer.as_str(), bs.outer_box, -stroke),
            None => (bs.outer.as_str(), bs.outer_box, stroke),
        };
        let [ot, or_, ob, ol] = c.geometry_outsets(kind);
        let [et, er, eb, el] = c.border_shape_ext();
        format!("bordershape:{ot} {or_} {ob} {ol} {stroke} {et} {er} {eb} {el}:{shape}")
    });
    // Шрифтовые единицы в командах `shape()` — СВОИМ кеглем и семейством
    // (css-values-4 §6.1): `shape_to_path` на отрисовке знает только запасной
    // кегль 16 и системный шрифт, и `2ch`/`10em` при `font: 5px Ahem`
    // выходили ≈17.8/160 вместо 10/50 (clip-path-shape-002-units 0.81).
    // Корневой кегль — тот же, что читает `Len::parse` для `rem`.
    let clip_shape = c.clip_shape.clone().map(|s| {
        if !s.starts_with("shapedef:") {
            return s;
        }
        let px = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = c.font_family.clone().unwrap_or_default();
        let (ch, ex) = crate::metrics::ch_ex_px(&family, px);
        crate::computed::font_lengths_to_px(&s, px, crate::value::root_font_px(), ex, ch)
    });
    let mask = c
        .mask_image
        .clone()
        .or(clip_shape)
        .or(bshape)
        .or(rrect)
        .map(|m| resolve_mask_refs(&m));
    // `clip: rect()` действует только на абсолютный элемент (CSS 2.1).
    // ПАРК: буфер группы создаёт stacking context, которого у `clip` нет —
    // z-переплетение детей с внешними соседями рвётся
    // (clip-no-stacking-context, 1 пара).
    // Единицы шрифта в `clip: rect(...)` меряются ЗДЕСЬ: на разборе кегль ещё
    // неизвестен, а к отрисовке он уже слит. Прежде ненулевые `em`/`ex`
    // читались как `auto`, и обрезки не было вовсе (`visufx/clip-079` и родня).
    let clip_rect = c
        .clip_len
        .map(|sides| {
            let base = match c.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            };
            let family = c.font_family.clone().unwrap_or_default();
            sides.map(|l| match l {
                Some(Len::Px(v)) => Some(v),
                Some(other) => Some(crate::metrics::spacing_px(Some(other), &family, base)),
                None => None,
            })
        })
        .or(c.clip_rect)
        .filter(|_| {
        matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
    });
    // Голое слово коробки — срез краями этой коробки от border-box:
    // margin-box шире на поля, padding-box уже на рамку, content-box — на
    // рамку и отбивку (clip-path-marginBox-*, -paddingBox-*, -contentBox-*).
    let bare_inset = if c.clip_bare_box && c.clip_inset.is_none() {
        let b = c.borders();
        let s = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let [t, r, bo, l] = match c.clip_ref {
            Some(1) => [
                -s(c.margin.top),
                -s(c.margin.right),
                -s(c.margin.bottom),
                -s(c.margin.left),
            ],
            Some(2) => [s(b.top), s(b.right), s(b.bottom), s(b.left)],
            Some(3) => [
                s(b.top) + s(c.padding.top),
                s(b.right) + s(c.padding.right),
                s(b.bottom) + s(c.padding.bottom),
                s(b.left) + s(c.padding.left),
            ],
            _ => [0.0; 4],
        };
        Some([Len::Px(t), Len::Px(r), Len::Px(bo), Len::Px(l)])
    } else {
        None
    };
    // Голая коробка режет ВМЕСТЕ со скруглением углов (css-masking-1
    // §5.1 «<geometry-box>… including any corner shaping (e.g.
    // border-radius)»): радиус коробки — радиус рамки, сдвинутый на ту же
    // толщину (css-backgrounds-3 §5.2 inner/outer curves). Пока — только
    // равные круглые углы и равные стороны сдвига.
    let bare_round = bare_inset.and_then(|inset| {
        if c.radius_ell.is_some() {
            return None;
        }
        let r = match (c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl) {
            (Some(Len::Px(a)), Some(Len::Px(b)), Some(Len::Px(d)), Some(Len::Px(e)))
                if a == b && b == d && d == e && a > 0.0 =>
            {
                a
            }
            _ => return None,
        };
        let d = match inset {
            [Len::Px(t), Len::Px(rr), Len::Px(bo), Len::Px(l)]
                if t == rr && rr == bo && bo == l =>
            {
                t
            }
            _ => return None,
        };
        // margin-box наружу: радиус растёт на поле, когда он не меньше поля
        // (css-shapes-1 §6.1 — при r < m нужна поправка, её здесь нет).
        if d < 0.0 && r < -d {
            return None;
        }
        let r = (r - d).max(0.0);
        (r > 0.0).then_some(Len::Px(r))
    });
    let clip_inset = c.clip_inset.or(bare_inset);
    if blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && c.isolate != Some(true)
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.clip_edges.is_none()
        && c.clip_xywh.is_none()
    {
        return el;
    }
    // Чистая изоляция — буфер без собственного эффекта: ни размытия, ни
    // смешивания, ни маски, ни обрезки. Такой буфер коробкой не режется
    // (`interact::Grouped::spill`).
    let pure_isolation = blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.clip_edges.is_none()
        && c.clip_xywh.is_none();
    let mut wrapper = crate::interact::Grouped::new(el);
    wrapper.spill = pure_isolation || mask_geometry::unclipped(c);
    wrapper.blur = blur;
    wrapper.blend = u32::from(blend);
    wrapper.mask = mask;
    // Наружные тени коробки с `border-shape` повторяют фигуру и лежат
    // СНАРУЖИ неё — под буфером группы и вне его маски (css-borders-4
    // §border-shape-shadow-interaction; Blink `PaintNormalBoxShadow`, ветка
    // `HasBorderShape`). Квад тени (`apply::apply_paint`) и слой резкой
    // тени (`decorations`) у такой коробки не ставятся.
    if let Some(bs) = c.border_shape.clone()
        && !c.shadows.is_empty()
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs
            .inner
            .clone()
            .map(|(s, k)| (s, c.geometry_outsets(k)));
        let shadows = c.resolved_shadows(false);
        wrapper.under = Some(Box::new(move |bw, bh, sl, st, aw, ah| {
            crate::background::border_shape_shadow_svg(
                (bs.outer.as_str(), outer_out),
                inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                stroke,
                &shadows,
                false,
                bw,
                bh,
                sl,
                st,
                aw,
                ah,
            )
        }));
    }
    // Кольцо рамки `border-shape` при обрезке переполнения — НАД буфером
    // группы: содержимое и фон режутся внутренним контуром (маска выше), а
    // рамка лежит снаружи него и поверх обрезанных детей, как в Blink
    // (`PaintBorderShape` после детей не нужен — дети до внутреннего контура
    // не доходят). В декорациях кольцо тогда не ставится.
    if let Some(bs) = c.border_shape.clone()
        && c.border_shape_clips()
    {
        let (stroke, colour) = c.border_shape_stroke();
        let colour = crate::background::border_paint(c, colour);
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs
            .inner
            .clone()
            .map(|(s, k)| (s, c.geometry_outsets(k)));
        if (inner.is_some() || stroke > 0.0) && colour.a > 0.0 {
            wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
                crate::background::border_shape_ring_svg(
                    (bs.outer.as_str(), outer_out),
                    inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                    stroke,
                    colour,
                    bw,
                    bh,
                    sl,
                    st,
                    aw,
                    ah,
                )
            }));
        }
    }
    // Контур `outline` повторяет фигуру (Blink `BorderShapePainter::
    // PaintOutline`): полоса по внешнему контуру, над группой — контур лежит
    // снаружи фигуры и красится последним (CSS 2.1 прил. E, шаг 10), а маска
    // группы его бы срезала. Квад контура в декорациях не ставится.
    if let Some(bs) = c.border_shape.clone()
        && let Some((w, off, colour)) = c.shaped_outline()
        && colour.a > 0.0
    {
        let (stroke, _) = c.border_shape_stroke();
        let outer_out = c.geometry_outsets(bs.outer_box);
        let single = bs.inner.is_none();
        let double = c
            .outline
            .as_ref()
            .is_some_and(|o| o.style == Some(crate::computed::OUTLINE_DOUBLE));
        wrapper.over.push(Box::new(move |bw, bh, sl, st, aw, ah| {
            crate::background::border_shape_outline_svg(
                (bs.outer.as_str(), outer_out),
                single,
                stroke,
                off,
                w,
                double,
                colour,
                bw,
                bh,
                sl,
                st,
                aw,
                ah,
            )
        }));
    }
    wrapper.mask_size = c.mask_size;
    wrapper.mask_fit = c.mask_fit.unwrap_or(0);
    wrapper.mask_no_repeat = c.mask_no_repeat.unwrap_or((false, false));
    wrapper.mask_repeat_list = c.mask_repeat_list.clone().unwrap_or_default();
    wrapper.mask_repeat_modes = c.mask_repeat_modes.clone().unwrap_or_default();
    wrapper.mask_luminance = c.mask_luminance == Some(true);
    wrapper.mask_alpha_mode = c.mask_alpha_mode == Some(true);
    wrapper.mask_pos = c.mask_pos;
    wrapper.mask_pos_far = c.mask_pos_far;
    wrapper.mask_pos_list = c.mask_pos_list.clone().unwrap_or_default();
    // Коробки маски (css-masking §7.10-7.11): сдвиги краёв от border-box
    // внутрь — рамка (padding-box) либо рамка+отступ (content-box).
    let box_off = |kind| mask_geometry::offsets(c, kind);
    wrapper.mask_origin_off = box_off(c.mask_origin);
    wrapper.clip_rect = clip_rect;
    wrapper.clip_inset = clip_inset;
    wrapper.clip_edges = c.clip_edges;
    wrapper.clip_xywh = c.clip_xywh;
    if rounded_rect_clip(c) {
        wrapper.clip_round = c.clip_round_len;
    } else if bare_round.is_some() {
        wrapper.clip_round = bare_round;
    }
    // `clip`/`mask-clip` живут в системе координат элемента ДО трансформа, а
    // трансформ рисуется ВНУТРИ буфера группы — коробка клипа обязана ехать
    // вместе (clip-transform-order: сдвинутый рисунок резался по старому
    // месту). Честно поддержан только сдвиг; поворот с клипом — парк.
    if let Some(t) = &c.transform {
        wrapper.clip_shift = (
            t.translate.0,
            t.translate.1,
            t.translate_pct.0,
            t.translate_pct.1,
        );
    }
    wrapper.mask_composite = c.mask_composite.clone().unwrap_or_default();
    wrapper.mask_clip_off = c.mask_clip.filter(|k| *k != 255).map(|k| box_off(Some(k)));
    // SVG-ребёнок: коробки маски уже посчитаны от его stroke-box
    // (`svg::masked_layers`), рамки и отбивки у него нет.
    if let Some((origin, clip)) = c.mask_box_override {
        wrapper.mask_origin_off = origin;
        wrapper.mask_clip_off = clip;
    }
    if c.mask_user_scale > 0.0 {
        wrapper.mask_scale = c.mask_user_scale;
    }
    // Точки уходят КАК ЕСТЬ (Len): проценты и пиксели резолвятся при
    // отрисовке от опорной коробки формы (css-masking §1.3.1.1): margin-box
    // расширяет bounds на поля, content-box сужает на рамку+паддинг
    // (clip-path-polygon-008: полигон в margin-box; masking 82→84).
    wrapper.polygon = polygon.to_vec();
    wrapper.polygon_evenodd = c.clip_polygon_evenodd;
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    wrapper.poly_expand = match c.clip_ref {
        Some(1) => [
            side(c.margin.top),
            side(c.margin.right),
            side(c.margin.bottom),
            side(c.margin.left),
        ],
        Some(2) => [-side(b.top), -side(b.right), -side(b.bottom), -side(b.left)],
        Some(3) => [
            -(side(b.top) + side(c.padding.top)),
            -(side(b.right) + side(c.padding.right)),
            -(side(b.bottom) + side(c.padding.bottom)),
            -(side(b.left) + side(c.padding.left)),
        ],
        _ => [0.0; 4],
    };
    wrapper.into_any_element()
}

/// css-transforms-2 §grouping-property-values: «групповые» свойства делают
/// из элемента группу, и ИСПОЛЬЗУЕМОЕ значение `transform-style` у него —
/// `flat`, чем бы ни было записано. Без гейта зелёные
/// `preserve3d-and-filter-no-perspective` (filter),
/// `transform3d-preserve3d-009` (overflow),
/// `mix-blend-mode-with-transform-and-preserve-3D` (blend),
/// `clip-not-absolute-positioned-003`, `corner-shape-bevel-overflow-composite`
/// и `view-transition-name-is-grouping` уходят в красное.
pub(crate) fn flattens_3d(c: &Computed) -> bool {
    use crate::computed::Overflow;
    let clipped = |o: Option<Overflow>| matches!(o, Some(o) if o != Overflow::Visible);
    c.opacity.is_some_and(|o| o < 1.0)
        || c.filter.is_some()
        || c.filter_ref.is_some()
        || c.backdrop_blur.is_some()
        || clipped(c.overflow_x)
        || clipped(c.overflow_y)
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || c.blend.is_some_and(|b| b != 0)
        || c.isolate == Some(true)
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
}

pub(crate) fn transformed(el: AnyElement, c: &Computed, parent: &Computed) -> AnyElement {
    transformed_with(el, c, parent, None)
}

/// `transformed` with a shared reference box (`interact::Transformed::ref_box`):
/// a table row or row group transform spread over its cells.
pub(crate) fn transformed_with(
    el: AnyElement,
    c: &Computed,
    parent: &Computed,
    ref_box: Option<crate::interact::RefBox>,
) -> AnyElement {
    // `transform: inherit` / `transform-origin: inherit` (css-cascade-4
    // §inherit: «the property's specified and computed values are the
    // inherited value»). Разбор ставит только бит (computed.rs:6535), а
    // значение родителя кладёт `inline::inherit` (inline.rs:897) в СЛИТЫЙ
    // стиль. Блочный путь строит обёртку по `&e.style` (render.rs:4663), и
    // унаследованное значение терялось (`transform-inherit-001/002`,
    // `-origin-001/002`, `css-transform-inherit-scale`). Бит решается здесь,
    // от того же родителя; у формы (`&merged`) результат тот же.
    let inherited_tf;
    let c = {
        use crate::computed::inh;
        let bits = c.inherit_bits & (inh::TRANSFORM | inh::TRANSFORM_ORIGIN);
        if bits == 0 {
            c
        } else {
            let mut own = c.clone();
            if bits & inh::TRANSFORM != 0 {
                own.transform = parent.transform;
            }
            if bits & inh::TRANSFORM_ORIGIN != 0 {
                own.transform_origin = parent.transform_origin;
                own.transform_origin_px = parent.transform_origin_px;
                own.transform_origin_z = parent.transform_origin_z;
            }
            inherited_tf = own;
            &inherited_tf
        }
    };
    // Объёмный контекст: своя ячейка нужна владельцу `preserve-3d`, чужая —
    // КАЖДОМУ его прямому ребёнку, даже без собственного `transform`:
    // изнанка решается по НАКОПЛЕННОЙ матрице (`backface-visibility-hidden-004`
    // — у `.card.front` своего преобразования нет вовсе).
    let keeps_3d = c.preserve_3d == Some(true) && !flattens_3d(c);
    // Вынесенный блок-в-строчном (`Computed::hoisted_block`): по DOM он
    // внук, и плоский строчный хозяин — лист контекста: ни ячейка объёма, ни
    // перспектива деда ему не достаются (css-transforms-2
    // §3d-rendering-context; `3d-rendering-context-and-inline`:
    // `rotateX(-90deg)` внутри `display: inline` под `preserve-3d; rotateX(90deg)`
    // не раскручивается обратно; `perspective-children-only-inline`).
    let under_3d = if c.hoisted_block { None } else { parent.frame_3d.clone() };
    if c.transform.is_none() && c.perspective.is_none() && !keeps_3d && under_3d.is_none() {
        return el;
    }
    let mut wrapper = crate::interact::Transformed::new(el);
    wrapper.ref_box = ref_box;
    wrapper.under_3d = under_3d;
    wrapper.frame_3d = if keeps_3d { c.frame_3d.clone() } else { None };
    // Перспектива РОДИТЕЛЯ читается объёмным путём (css-transforms-2
    // §3d-transform-rendering п.3 — только прямого родителя, внукам не
    // достаётся: perspective-children-only-*); своя — наполняет ячейку для
    // детей. Элементу с `perspective` без `transform` обёртка тоже нужна —
    // ради ячейки; сам он идёт плоским путём с единичной матрицей.
    // ★ ЗАМЕРЕНО (06.09, v110, +13/−2): две потери остаются.
    // `perspective-children-only-inline` — блок внутри `display: inline`
    // выносится расщеплением строчного (block-in-inline) и становится прямым
    // ребёнком в дереве отрисовки, поэтому берёт перспективу, хотя по DOM он
    // внук. `overflow-perspective-001` (0.00 → 2.92) — прокручиваемая коробка:
    // начало перспективы считается от коробки, а не от области прокрутки.
    wrapper.under_perspective = if c.hoisted_block {
        None
    } else {
        parent.perspective_frame.clone()
    };
    wrapper.perspective = c.perspective;
    wrapper.perspective_frame = c.perspective_frame.clone();
    if let Some(o) = c.perspective_origin {
        wrapper.perspective_origin = o;
    }
    wrapper.perspective_origin_px = c.perspective_origin_px;
    // Изнанка ставится ДО раннего выхода: ребёнок объёмного контекста без
    // своего `transform` (`backface-visibility-hidden-004` `.card.front`,
    // `transform3d-backface-visibility-006`, `backface-visibility-with-
    // sibling-001`) решает её по накопленной матрице родителя
    // (css-transforms-2 §backface-visibility), а флаг прежде выставлялся
    // только на пути с собственным преобразованием — красный ребёнок под
    // `rotateX(180deg); preserve-3d` оставался виден.
    wrapper.backface_hidden = c.backface_hidden == Some(true);
    let Some(t) = c.transform else {
        return wrapper.into_any_element();
    };
    // Отдельные `rotate`/`scale` — ПЕРЕД списком `transform` (css-transforms-2
    // §ctm п.4-5 и п.7; Blink `ComputedStyle::ApplyTransform`,
    // style/computed_style.cc:1464-1487). Разбор писал их только в разложение
    // (`rotate_rad`/`scale`), а `Transformed` рисует по `lin`/`tr`/`m4` —
    // до экрана они не доходили вовсе. `translate` здесь не нужен: он уже
    // сдвинул коробку (`apply.rs`).
    let mut t = t.after_individual(c.rotate_prop, c.scale_prop);
    // Чистый px-сдвиг уже сдвинул коробку в раскладке
    // (`Computed::folded_shift`, `apply.rs`) — матрица остаётся единичной.
    if c.folded_shift().is_some() {
        t.tr[0][0] = 0.0;
        t.tr[1][0] = 0.0;
        t.m4[0][3] = 0.0;
        t.m4[1][3] = 0.0;
        t.translate = (0.0, 0.0);
    }
    wrapper.rotate = t.rotate_rad;
    wrapper.skew = t.skew_rad;
    wrapper.scale = t.scale;
    wrapper.translate = t.translate;
    wrapper.translate_pct = t.translate_pct;
    wrapper.lin = t.lin;
    wrapper.tr = t.tr;
    wrapper.m4 = t.m4;
    wrapper.m4_pct = t.m4_pct;
    wrapper.has_3d = t.has_3d;
    // Обратная сторона (css-transforms-2 §backface-visibility, «m33 < 0 →
    // the element is not rendered») решается на отрисовке по собственной
    // 4×4 (флаг выставлен выше): раньше здесь подменяли элемент пустым
    // `div()`, и коробка теряла место в раскладке
    // (backface-visibility-hidden-002: эталон держит пустые 100px;
    // -child-translate: высота обёртки от скрытого ребёнка).
    if let Some(o) = c.transform_origin {
        wrapper.origin = o;
    }
    wrapper.origin_px = c.transform_origin_px;
    wrapper.origin_z = c.transform_origin_z;
    wrapper.into_any_element()
}

/// Развернуть именованные области сетки в номера линий.
///
/// `grid-template-areas` — способ разложить макет именами вместо цифр. Ни
/// GPUI, ни taffy имён не знают, но знают номера: имя ищется в раскладке
/// контейнера, и ребёнок получает готовый прямоугольник линий.
pub(crate) fn place_named_areas(areas: &[Vec<String>], children: Vec<Node>) -> Vec<Node> {
    use crate::computed::Placement;
    children
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) => {
                let Some(name) = e.style.grid_area_name.clone() else {
                    return Node::Element(e);
                };
                // Прямоугольник имени: первая и последняя строка, первый и
                // последний столбец, где оно встречается.
                let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (row, cells) in areas.iter().enumerate() {
                    for (col, cell) in cells.iter().enumerate() {
                        if *cell == name {
                            r0 = r0.min(row);
                            r1 = r1.max(row + 1);
                            c0 = c0.min(col);
                            c1 = c1.max(col + 1);
                        }
                    }
                }
                if r0 != usize::MAX {
                    // Линии в CSS считаются с единицы.
                    e.style.grid_row = Some((
                        Placement::Line(r0 as i16 + 1),
                        Placement::Line(r1 as i16 + 1),
                    ));
                    e.style.grid_col = Some((
                        Placement::Line(c0 as i16 + 1),
                        Placement::Line(c1 as i16 + 1),
                    ));
                }
                Node::Element(e)
            }
            other => other,
        })
        .collect()
}

/// Обернуть элемент лентой прокрутки, если `overflow` её просит.
///
/// `auto` и `scroll` в CSS означают именно ленту; обрезка без прокрутки —
/// это `hidden`, и подменять одно другим значило терять содержимое.
/// Вынуть из поддерева ленты прокрутки абсолютных потомков, которым лента не
/// содержащий блок.
///
/// §11.1.1: предок обрезает ТОЛЬКО того потомка, для которого он содержащий
/// блок. У абсолютного элемента без позиционированного предка содержащий блок
/// — область просмотра (§10.1 п.4), и `overflow: scroll|auto` его не касается.
///
/// Лента строит поддерево в замыкании, которое зовёт
/// `ScrollArea::request_layout` — уже после `icb_close()`, и `icb_push` вернул
/// бы элемент назад «рисовать на месте». Поэтому кандидатов вынимаем ЗДЕСЬ.
///
/// Условия — те же, что у `to_icb`, плюс два ужесточения: только
/// НЕПОСРЕДСТВЕННЫЕ дети ленты и только при ОБЕИХ заданных осях (по свободной
/// оси место сообщает щуп, а он остался бы в замыкании).
pub(crate) fn hoist_from_scroll(e: &mut Element, inherited: &Computed, opts: &RenderOpts) {
    use crate::computed::Position;
    if !crate::interact::icb_active() || inside_deferred() {
        return;
    }
    let merged = crate::inline::inherit(inherited, &e.style);
    // Лента внутри позиционированного предка не выносит ничего: у её потомков
    // содержащий блок есть.
    if merged.cb_ancestor || crate::inline::establishes_cb(&merged) {
        return;
    }
    if matches!(
        merged.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
    ) {
        return;
    }
    let take: Vec<bool> = (0..e.children.len())
        .map(|i| {
            let Node::Element(c) = &e.children[i] else {
                return false;
            };
            let x_set = edge_set(c.style.inset.left) || edge_set(c.style.inset.right);
            let y_set = edge_set(c.style.inset.top) || edge_set(c.style.inset.bottom);
            c.style.position == Some(Position::Absolute)
                && c.style.z_index.unwrap_or(0) >= 0
                && x_set
                && y_set
                && !stays_positioned(&e.children[i + 1..])
        })
        .collect();
    if !take.iter().any(|t| *t) {
        return;
    }
    let mut keep = Vec::with_capacity(e.children.len());
    for (i, child) in std::mem::take(&mut e.children).into_iter().enumerate() {
        if !take[i] {
            keep.push(child);
            continue;
        }
        // `blocks` на одном узле повторяет ВЕСЬ путь `to_icb`: строит элемент
        // и отдаёт открытому слою ICB. При обеих заданных осях щуп не нужен,
        // поэтому список возвращается пустым.
        let left = blocks(std::slice::from_ref(&child), &merged, opts);
        if left.is_empty() {
            continue;
        }
        // Условия предиката разошлись с `to_icb`: узел остаётся на месте, а не
        // теряется.
        drop(left);
        keep.push(child);
    }
    e.children = keep;
}

pub(crate) fn scrollable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    use crate::computed::Overflow;
    let horizontal = e.style.overflow_x == Some(Overflow::Scroll);
    let vertical = e.style.overflow_y == Some(Overflow::Scroll);
    if !horizontal && !vertical {
        return None;
    }
    let mut node = e.clone();
    // Слой ICB закрывается раньше, чем `ScrollArea` позовёт `build`, — поэтому
    // выносим кандидатов сейчас, из ещё не отданного в замыкание клона.
    hoist_from_scroll(&mut node, inherited, opts);
    let node = node;
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(
        move |handle: &gpui::ScrollHandle, h: bool, v: bool| -> AnyElement {
            let _depth = DepthScope::enter(depth);
            // Внутренний узел рисуется без прокрутки: ею занимается лента.
            let mut inner = node.clone();
            inner.style.overflow_x = None;
            inner.style.overflow_y = None;
            inner.style.scroller = true;
            // Доли высоты коробки — от её СОДЕРЖАЩЕГО блока (CSS 2.1 §10.5,
            // §10.7: «calculated with respect to the height of the generated
            // box's containing block»), а не от ленты: лента `d` своей высоты
            // не имеет, и под ней `height`/`min-height`/`max-height` в долях
            // решались как `auto`/`0`/`none`. Blink обёртки не заводит: доля
            // решается от `PercentageResolutionBlockSize` родителя
            // (`length_utils.cc:200-213`), и сквозь анонимную прослойку база
            // тоже идёт родительская (`CalculateChildPercentageSize`,
            // `length_utils.cc:1814-1818`). Прежде это прятало сжатие ленты
            // (`flex_shrink` 1 у обёртки по умолчанию); после P4 (сжатие узла на
            // обёртке) лента в потоке не жмётся, и эталон
            // `fieldset-as-item-overflow-ref` (`max-height: 100%` под
            // `height: 100px`) вылезал вниз на 100 px. Развязка та же, что у
            // `pct_height_to_px`: только поточный ребёнок блочного родителя с
            // высотой в точках (у гибкого и сеточного долю решает раскладка, у
            // абсолюта содержащий блок другой); `border-box` родителя — мимо,
            // его `height` не высота содержимого.
            if in_flow(&node.style)
                && matches!(inherited.display, None | Some(Display::Block))
                && inherited.border_box != Some(true)
                && let Some(Len::Px(ph)) = inherited.height
                && ph > 0.0
            {
                let of = |l: Option<Len>| match l {
                    Some(Len::Pct(k)) => Some(Len::Px(k * ph)),
                    other => other,
                };
                inner.style.height = of(inner.style.height);
                inner.style.min_height = of(inner.style.min_height);
                inner.style.max_height = of(inner.style.max_height);
            }
            // Наружный отступ принадлежит коробке, а не видимой области:
            // оставленный внутри, он увеличивал ленту на свою величину, и
            // содержимое было видно ниже края панели.
            let outer_margin = inner.style.margin;
            inner.style.margin = Default::default();
            let (built, native_box) = scroll_box::build(node.node_id, handle, h, v, outer_margin,
                || element(&inner, &inherited, &opts));
            if native_box { return built; }
            use gpui::{InteractiveElement, StatefulInteractiveElement};
            let mut d = crate::apply::margins(div(), &outer_margin)
                .id(gpui::ElementId::Integer(node.node_id as u64 + 1))
                .track_scroll(handle)
                .child(built);
            // Элемент потока родителя — ЭТА обёртка, а не внутренний узел: ей и
            // сжатие, которое блоку в потоке выключено (`flex_shrink: 0` из
            // `blocks()`), а во flex-контексте — 1. Без него автоминимум
            // прокрутки (0) давал ленте ужаться под первым же потолком
            // родителя до нуля (`line-clamp-007`: `overflow:auto`-ребёнок
            // клэмп-контейнера пропадал целиком).
            if let Some(shrink) = node.style.flex_shrink {
                d.style().flex_shrink = Some(shrink);
            }
            if h {
                d = d.overflow_x_scroll();
            }
            if v {
                d = d.overflow_y_scroll();
            }
            // Лента — видимая область КОРОБКИ (CSS 2.1 §11.1.1: «content is
            // clipped»), а без своего размера она растягивалась на всю ширину
            // родителя: `inner` с `width: 200px` переполнение уже не режет
            // (`overflow` с него снят выше), и глиф за краем коробки был виден
            // целиком (`min-height-104/106`: красный третий «X» Ahem на
            // 200…300 px). Ширина ленты — рамочная ширина коробки; считается
            // только из точек, иначе лента остаётся прежней.
            // Размеры — из СЛИТОГО стиля: в собственном стиле узла `16ch` и
            // `10em` ещё не решены в точки (их решает `inherit`), и лента без
            // ширины не резала ничего (`white-space-pre-wrap-trailing-spaces-
            // 021`: висящие пробелы `overflow: auto`-коробки шириной в `ch`
            // выходили за её край).
            let sized = crate::inline::inherit(&inherited, &inner.style);
            if h && let Some(Len::Px(w)) = sized.width {
                let side = |l: Option<Len>| match l {
                    None | Some(Len::Auto) => Some(0.0),
                    Some(Len::Px(v)) => Some(v),
                    _ => None,
                };
                let b = sized.borders();
                let edges = side(sized.padding.left)
                    .zip(side(sized.padding.right))
                    .zip(side(b.left).zip(side(b.right)))
                    .map(|((pl, pr), (bl, br))| pl + pr + bl + br);
                let lane = if inner.style.border_box == Some(true) {
                    Some(w)
                } else {
                    edges.map(|e| w + e)
                };
                if let Some(lw) = lane {
                    d = d.w(px(lw));
                    // Сжатие ленты решает РОДИТЕЛЬ. В блочном потоке `blocks()` уже
                    // записал `flex_shrink: 0`, и он перенесён на ленту выше. У
                    // элемента гибкого ряда действует свой `flex-shrink`
                    // (начальное 1, css-flexbox-1 Overview.bs:2586-2588; автоминимум
                    // ленты прокрутки — ноль, :1301): `width: 200px; min-width: 0`
                    // в контейнере 100 обязан ужаться до 100. Безусловный ноль
                    // выпускал ленту за контейнер (`grid-baseline-003-ref`,
                    // 0.00 → 0.72).
                    let flex_item = matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    );
                    if node.style.flex_shrink.is_none() && !flex_item {
                        d = d.flex_shrink_0();
                    }
                }
            }
            d.into_any_element()
        },
    );
    Some(
        crate::interact::ScrollArea::new(
            gpui::ElementId::Integer(e.node_id as u64),
            horizontal,
            vertical,
            build,
        )
        .into_any_element(),
    )
}

/// Обернуть элемент ручкой изменения размера, если `resize` разрешает.
pub(crate) fn resizable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if e.style.pointer_events_none == Some(true) {
        return None;
    }
    let (horizontal, vertical) = e.style.resize?;
    let axis = match (horizontal, vertical) {
        (true, true) => crate::interact::ResizeAxis::Both,
        (true, false) => crate::interact::ResizeAxis::Horizontal,
        _ => crate::interact::ResizeAxis::Vertical,
    };
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |w: Option<f32>, h: Option<f32>| {
        let _depth = DepthScope::enter(depth);
        // Заданный мышью размер побеждает разметку — как и в браузере, где
        // он пишется в инлайн-стиль элемента.
        let mut mixed = node.clone();
        mixed.style.resize = None;
        if let Some(w) = w {
            mixed.style.width = Some(Len::Px(w));
        }
        if let Some(h) = h {
            mixed.style.height = Some(Len::Px(h));
        }
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::interact::Resizable::new(gpui::ElementId::Integer(e.node_id as u64), axis, build)
            .into_any_element(),
    )
}

/// Обернуть элемент плавным переходом, если он задан.
///
/// Поддерево пересобирается по доле перехода — иначе смешанный стиль некуда
/// применить: у собранного элемента стиль уже зафиксирован.
pub(crate) fn transitioned(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let seconds = e.style.transition?;
    let hover = e.hover.clone()?;
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |k: f32| {
        let _depth = DepthScope::enter(depth);
        let mut mixed = node.clone();
        mixed.style = node.style.blend(&hover, k);
        // Слой наведения снят: его роль уже сыграла доля перехода, иначе
        // стиль прыгнул бы поверх плавного.
        mixed.hover = None;
        mixed.style.transition = None;
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::transition::Transition::new(
            gpui::ElementId::Integer(e.node_id as u64),
            seconds,
            build,
        )
        .into_any_element(),
    )
}

/// Интерполяция стиля между кадрами анимации.
///
/// Интерполируются те свойства, которые анимируют на практике и которые можно
/// подменить у уже собранного элемента: прозрачность, заливка, цвет текста,
/// сдвиг и размеры. Всё остальное берётся с ближайшего кадра — перестраивать
/// поддерево каждый кадр нельзя, это стоило бы дороже самой анимации.
pub(crate) fn frame_at(frames: &[(f32, Computed)], t: f32) -> Computed {
    let t = t.clamp(0.0, 1.0);
    let mut prev = &frames[0];
    let mut next = &frames[frames.len() - 1];
    for pair in frames.windows(2) {
        if t >= pair[0].0 && t <= pair[1].0 {
            prev = &pair[0];
            next = &pair[1];
            break;
        }
    }
    let span = (next.0 - prev.0).max(0.0001);
    let k = ((t - prev.0) / span).clamp(0.0, 1.0);
    let mut out = prev.1.clone();
    let lerp = |a: f32, b: f32| a + (b - a) * k;
    if let (Some(a), Some(b)) = (prev.1.opacity, next.1.opacity) {
        out.opacity = Some(lerp(a, b));
    }
    let mix = |a: crate::value::Color, b: crate::value::Color| crate::value::Color {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    };
    if let (Some(a), Some(b)) = (prev.1.background, next.1.background) {
        out.background = Some(mix(a, b));
    }
    if let (Some(a), Some(b)) = (prev.1.color, next.1.color) {
        out.color = Some(mix(a, b));
    }
    let len = |a: Option<Len>, b: Option<Len>| -> Option<Len> {
        match (a?, b?) {
            (Len::Px(x), Len::Px(y)) => Some(Len::Px(lerp(x, y))),
            (Len::Pct(x), Len::Pct(y)) => Some(Len::Pct(lerp(x, y))),
            // Разные природы (`0px → 200vw`, `0% → 200vw`) — покомпонентно,
            // как `calc()`; несводимая смесь — ближайший кадр.
            (x, y) => Some(crate::value::lerp_len(x, y, k).unwrap_or(x)),
        }
    };
    out.width = len(prev.1.width, next.1.width).or(out.width);
    out.height = len(prev.1.height, next.1.height).or(out.height);
    // Фильтры интерполируются покомпонентно; `none` = нейтральный
    // (filter-effects-1 §Interpolation, css-filters-animation-*).
    if prev.1.filter.is_some() || next.1.filter.is_some() {
        let a = prev
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        let b = next
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        out.filter = Some(crate::computed::Filter {
            grayscale: lerp(a.grayscale, b.grayscale),
            brightness: lerp(a.brightness, b.brightness),
            saturate: lerp(a.saturate, b.saturate),
            invert: lerp(a.invert, b.invert),
            sepia: lerp(a.sepia, b.sepia),
            opacity: lerp(a.opacity, b.opacity),
            hue_rotate: lerp(a.hue_rotate, b.hue_rotate),
            contrast: lerp(a.contrast, b.contrast),
            blur: lerp(a.blur, b.blur),
        });
    }
    if prev.1.backdrop_blur.is_some() || next.1.backdrop_blur.is_some() {
        out.backdrop_blur = Some(lerp(
            prev.1.backdrop_blur.unwrap_or(0.0),
            next.1.backdrop_blur.unwrap_or(0.0),
        ));
    }
    // Цветовые функции подложки — покомпонентно, как у `filter`: эталоны
    // `css-backdrop-filters-animation-*` — статический `backdrop-filter`
    // середины, и без интерполяции тест разошёлся бы с ним.
    if prev.1.backdrop_color.is_some() || next.1.backdrop_color.is_some() {
        let a = prev
            .1
            .backdrop_color
            .unwrap_or_else(crate::computed::Filter::neutral);
        let b = next
            .1
            .backdrop_color
            .unwrap_or_else(crate::computed::Filter::neutral);
        out.backdrop_color = Some(crate::computed::Filter {
            grayscale: lerp(a.grayscale, b.grayscale),
            brightness: lerp(a.brightness, b.brightness),
            saturate: lerp(a.saturate, b.saturate),
            invert: lerp(a.invert, b.invert),
            sepia: lerp(a.sepia, b.sepia),
            opacity: lerp(a.opacity, b.opacity),
            hue_rotate: lerp(a.hue_rotate, b.hue_rotate),
            contrast: lerp(a.contrast, b.contrast),
            blur: 0.0,
        });
    }
    // `drop-shadow()` — покомпонентно; у `none` — тень с нулевыми длинами и
    // цветом `transparent` (filter-effects-1 Overview.bs:3460-3463, :418).
    // Цвет — в умноженном на альфу виде: `black → transparent` на середине
    // даёт `rgba(0,0,0,0.5)` (`css-filters-animation-drop-shadow`).
    if prev.1.drop_shadow.is_some() || next.1.drop_shadow.is_some() {
        let none = crate::computed::Shadow {
            x: 0.0,
            y: 0.0,
            blur: 0.0,
            spread: 0.0,
            color: crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        };
        let a = prev.1.drop_shadow.unwrap_or(none);
        let b = next.1.drop_shadow.unwrap_or(none);
        let alpha = lerp(a.color.a, b.color.a);
        let pm = |x: f32, y: f32| {
            if alpha > 0.0 {
                lerp(x * a.color.a, y * b.color.a) / alpha
            } else {
                0.0
            }
        };
        out.drop_shadow = Some(crate::computed::Shadow {
            x: lerp(a.x, b.x),
            y: lerp(a.y, b.y),
            blur: lerp(a.blur, b.blur),
            spread: lerp(a.spread, b.spread),
            color: crate::value::Color {
                r: pm(a.color.r, b.color.r),
                g: pm(a.color.g, b.color.g),
                b: pm(a.color.b, b.color.b),
                a: alpha,
            },
        });
    }
    // `translate`/`rotate`/`scale`: `none` с одной стороны заменяется
    // тождеством (css-transforms-2 §individual-transforms: 0px, 0deg, 1).
    // Сдвиг раньше смешивался только при ОБОИХ значениях, и `to { translate:
    // … }` над стилем без сдвига стоял на месте.
    if prev.1.translate.is_some() || next.1.translate.is_some() {
        let zero = (Len::Px(0.0), Len::Px(0.0));
        let (a, b) = (
            prev.1.translate.unwrap_or(zero),
            next.1.translate.unwrap_or(zero),
        );
        out.translate = Some((
            len(Some(a.0), Some(b.0)).unwrap_or(a.0),
            len(Some(a.1), Some(b.1)).unwrap_or(a.1),
        ));
    }
    if prev.1.rotate_prop.is_some() || next.1.rotate_prop.is_some() {
        out.rotate_prop = Some(lerp(
            prev.1.rotate_prop.unwrap_or(0.0),
            next.1.rotate_prop.unwrap_or(0.0),
        ));
    }
    if prev.1.scale_prop.is_some() || next.1.scale_prop.is_some() {
        let (a, b) = (
            prev.1.scale_prop.unwrap_or((1.0, 1.0)),
            next.1.scale_prop.unwrap_or((1.0, 1.0)),
        );
        out.scale_prop = Some((lerp(a.0, b.0), lerp(a.1, b.1)));
    }
    // `transform` — разложенными матрицами (`Transform::lerp_2d`,
    // css-transforms-1 §matrix-interpolation); `none` — тождество. Объёмный
    // список или необратимая сторона — дискретно, ближайшим кадром (как было).
    if prev.1.transform.is_some() || next.1.transform.is_some() {
        let a = prev.1.transform.unwrap_or_default();
        let b = next.1.transform.unwrap_or_default();
        if (a.lin != b.lin || a.tr != b.tr)
            && !a.has_3d
            && !b.has_3d
            && let Some(m) = a.lerp_2d(&b, k)
        {
            out.transform = Some(m);
        }
    }
    // Отдельное свойство — тоже преобразование: коробка с ним несёт
    // `transform` (так делает и разбор `rotate`/`scale`), иначе
    // `transformed()` вышел бы раньше свёртки.
    if (out.rotate_prop.is_some() || out.scale_prop.is_some()) && out.transform.is_none() {
        out.transform = Some(Default::default());
    }
    out
}

/// Остановленная анимация (`AnimSpec::frozen`), запечённая в копию элемента:
/// кадр `(-delay)/duration` подставляется прямо в стиль, и дальше работает
/// весь обычный конвейер. `transforms` — нести ли и `rotate`/`scale`/
/// `transform`: их матрицу строит `transformed()` СНАРУЖИ элемента, от стиля,
/// который ему передан. Блочный путь (`transformed(animated(e), &e.style)`)
/// их не берёт: `!important` у нас кадры не перекрывает, а обязан
/// (css-cascade-5 §cascade-origin) — `translation-animation-on-important-
/// property` с `transform: none !important` уехала бы на середину пути.
pub(crate) fn bake_frozen(e: &Element, transforms: bool) -> Option<Element> {
    let (Some(frames), Some(spec)) = (e.anim.as_ref(), e.style.animation.as_ref()) else {
        return None;
    };
    if !spec.frozen() {
        return None;
    }
    Some(animation_frame::sample(e, &frame_at(frames, spec.frozen_t()), transforms))
}

/// Блочный элемент.
pub(crate) fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    // Высота ряда от внешней колонки — только ЭТОМУ элементу (`flow::OUTER_ROW`).
    let outer_row = crate::flow::take_outer_row();
    let mut merged = inline::inherit(inherited, &e.style);
    list_item::inherited_style(e, inherited, &mut merged);
    // Якорный шаг: ключи реестров кадра (свой `node_id` для содержащего
    // блока детей, порядок сборки, ключ клетки) и размеры от якоря —
    // `anchor-size()`, растяжка в клетке `position-area` — из реестра
    // ПРОШЛОГО кадра, до раскладки.
    merged.self_node = e.node_id;
    merged.anchor_seq = crate::anchor::next_seq();
    merged.anchor_key = crate::anchor::key_of(e);
    // Вариант `position-try-fallbacks`, выбранный на прошлом кадре, — в стиль
    // ДО размеров и раскладки (§fallback: «the element keeps those styles»).
    crate::anchor::apply_chosen(&mut merged);
    crate::anchor::resolve_sizes(&mut merged, inherited);
    // `dir="auto"` — сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку содержимого.
    // Разбор двунаправленности выберет её сам при наборе, но выключка и
    // прижим текста читают `rtl` из стиля, и без этого шага блок с арабским
    // текстом прижимался влево.
    if e.attr("dir") == Some("auto") && e.style.rtl.is_none() {
        let mut text = String::new();
        gather_text(&e.children, &mut text);
        let strong = text.chars().find_map(|ch| {
            use unicode_bidi::BidiClass::*;
            match unicode_bidi::bidi_class(ch) {
                L => Some(false),
                R | AL => Some(true),
                _ => None,
            }
        });
        if let Some(rtl) = strong {
            merged.rtl = Some(rtl);
        }
    }
    // Предел ОРТОГОНАЛЬНОГО потока для строк вертикального письма внутри
    // (CSS Writing Modes §7.3). Искать его надо вверх по дереву, поэтому он
    // несётся вниз наследуемым полем. Ближе всего собственная определённая
    // высота элемента; за ней — высота ближайшего контейнера прокрутки с
    // наложенным на неё `max-height`; в самом конце — окно (его подставляет
    // потребитель в `paragraph`).
    {
        // `height: 5em` доживает сюда неразрешённым — доля считается от
        // кегля самого блока (outline-inline-vlr-006: предел колонки 5em).
        let em_base = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): переводить сюда и единицы шрифта
        // (`max-height: 8ch`), чтобы предел ортогонального потока не терялся.
        // Срез вертикального письма 1086 пар: приобретено 0, потеряно 7 —
        // `available-size-003…018` (0.05-0.11 -> «красное видно»). Тесты
        // прямо пишут: «**max**-height does not give the element a definite
        // block size» (§7.3.1 берёт предел у ОПРЕДЕЛЁННОГО размера, а
        // `max-height` определённым не делает). Значит и нынешний перевод
        // `max-height` в точках — тоже неверный источник предела.
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * em_base),
            _ => None,
        };
        // `box-sizing: border-box`: заданные высота и её пределы — это
        // РАМОЧНАЯ коробка, а предел строк — размер СОДЕРЖИМОГО (§7.3.1
        // «inner size»; Blink переводит в content-box в
        // `SetOrthogonalFallbackInlineSize`). Атомный путь это уже делает
        // (`edges` при `border_box` выше), блочный — нет: эталоны
        // `sizing-orthog-vlr-in-htb-007`, `vrl-in-htb-007/010`
        // (`box-sizing: border-box; height: 400px`, рамка 3) переносили на
        // 400, тест после вычета рамок в `aaa7d8d` — на 394.
        let own_edges = if e.style.border_box == Some(true) {
            let b = e.style.borders();
            px_of(b.top).unwrap_or(0.0)
                + px_of(b.bottom).unwrap_or(0.0)
                + px_of(e.style.padding.top).unwrap_or(0.0)
                + px_of(e.style.padding.bottom).unwrap_or(0.0)
        } else {
            0.0
        };
        let content = |v: f32| (v - own_edges).max(0.0);
        let h = px_of(e.style.height).map(content);
        let min_h = px_of(e.style.min_height).map(content);
        let max_h = px_of(e.style.max_height).map(content);
        // Предел, поставленный СВОЕЙ высотой, уже содержимый (content-box
        // по умолчанию, border-box переведён выше) — вычитать из него нечего. Вычет нужен
        // только УНАСЛЕДОВАННОМУ пределу, см. хунк ниже.
        // Доля высоты — от высоты СОДЕРЖАЩЕГО блока, если та определённая
        // (CSS 2.1 §10.5: «calculated with respect to the height of the
        // generated box's containing block»). `px_of` её не понимал, и
        // ортогональный `height: 50%` в контейнере 400px переносил строки по
        // унаследованному пределу 394, а не по своим 200
        // (`sizing-orthog-prct-vlr-in-htb-004`: ~7 колонок вместо ~13).
        // Только для потокового ребёнка БЛОЧНОГО родителя: у абсолюта база —
        // его содержащий блок, у элемента сетки — область, у гибкого — своя
        // развязка.
        let h = h.or(match (e.style.height, inherited.height) {
            (Some(Len::Pct(k)), Some(Len::Px(ph)))
                if in_flow(&e.style)
                    && matches!(inherited.display, None | Some(Display::Block)) =>
            {
                Some(k * ph)
            }
            _ => None,
        });
        let mut own_limit = false;
        if h.is_some() || min_h.is_some() || max_h.is_some() {
            // Клэмп как у CSS-высоты: max режет, min ПЕРЕБИВАЕТ max; без
            // своей высоты базой служит НАЧАЛЬНЫЙ содержащий блок, и он же —
            // общий потолок («larger than ICB» не расширяет место).
            let mut avail = h.unwrap_or(opts.viewport.1);
            if let Some(m) = max_h {
                avail = avail.min(m);
            }
            if let Some(m) = min_h {
                avail = avail.max(m);
            }
            avail = avail.min(opts.viewport.1);
            // У блока без вертикали предел ставится детям всегда; у самого
            // вертикального — только если родитель не дал своего
            // (table-cell-002: max-height ячейки).
            // Своя ОПРЕДЕЛЁННАЯ высота вертикального блока — это его строчный
            // размер, и перенос решает она, а не предел предка: запасной
            // предел §7.3.1 нужен лишь там, где места не задано. Вето
            // «предок уже дал предел» писалось под `max-height` ячейки
            // (table-cell-002), а `max-height` определённого размера не даёт
            // — поэтому вето сужено до случая без своей `height`.
            if e.style.vertical != Some(true) || merged.ortho_limit.is_none() || h.is_some() {
                merged.ortho_limit = Some(avail);
                own_limit = true;
            }
        }
        // Свои рамки и отбивки вдоль СТРОЧНОЙ оси (при вертикальном письме —
        // физически верх и низ) съедают предел, который блок передаёт детям:
        // §7.3.1 берёт запасной предел от ВНУТРЕННЕГО размера содержащего
        // блока («the containing block's **inner** max size»,
        // css-writing-modes-4 Overview.bs:2141), то есть от content-box.
        // Blink делает тот же вычет явно — `space_utils.cc:59-72`
        // `SetOrthogonalFallbackInlineSize`, комментарий «Calculate the
        // content-box size»; он берёт предел у НЕПОСРЕДСТВЕННОГО родителя, а
        // у нас предел несётся вниз наследуемым полем (`inline.rs:1012`),
        // поэтому вычет обязан идти на КАЖДОМ уровне.
        // Видно это только у `sideways-lr`: там строка начинается у
        // ПРОТИВОПОЛОЖНОГО края коробки (`VerticalText::ccw`), и лишняя
        // высота уводит весь рисунок; у письма по часовой она свисает
        // пустым хвостом (`block-flow-direction-vlr-010`, `vrl-009`,
        // `srl-049` зелены при том же дефекте).
        if !own_limit
            && merged.vertical == Some(true)
            && let Some(l) = merged.ortho_limit
        {
            let b = e.style.borders();
            let edges = px_of(b.top).unwrap_or(0.0)
                + px_of(b.bottom).unwrap_or(0.0)
                + px_of(e.style.padding.top).unwrap_or(0.0)
                + px_of(e.style.padding.bottom).unwrap_or(0.0);
            if edges > 0.0 {
                merged.ortho_limit = Some((l - edges).max(0.0));
            }
        }
    }
    pseudo_line_layers::install(e, &mut merged);
    // Единицы окна разрешаются здесь: размер окна знает только сборщик.
    merged.resolve_viewport(opts.viewport);
    orthogonal_inline::resolve(&mut merged, inherited, opts.viewport, e.tag == "html");
    PAINT_VIEWPORT.with(|v| v.set(opts.viewport));
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::forms::element(e, &merged, opts) {
        return transformed(el, &merged, inherited);
    }
    // Рамка строится ОДИН раз до match: прежний `is_some() => unwrap()`
    // читал файл с диска и разбирал вложенный документ дважды за кадр.
    // Неразобранная рамка по-прежнему падает в общий рукав.
    let mut built_iframe = if e.tag == "iframe" {
        iframe(e, opts)
    } else if e.tag == "object" && object_is_document(e) {
        // `<object data="….html">` — тот же вложенный документ, что и рамка
        // (HTML §4.8.7): адрес переносится из `data` в `src`, размеры берутся
        // уже разрешёнными (`10em` в собственном стиле — ещё `Len::Em`).
        let mut copy = e.clone();
        let url = e.attr("data").unwrap_or_default().to_string();
        copy.attrs.push(("src".to_string(), url));
        copy.style.width = merged.width;
        copy.style.height = merged.height;
        iframe(&copy, opts)
    } else {
        None
    };
    match e.tag.as_str() {
        // CSS Lists 3 §2: a block list item generates its own marker even
        // when its parent is an ordinary block rather than a list container.
        _ if (e.style.display == Some(Display::ListItem)
            || (e.tag == "li" && e.style.display.is_none()))
            && !matches!(
                e.tag.as_str(),
                "img" | "svg" | "embed" | "object" | "video" | "canvas" | "iframe"
            ) =>
        {
            list_item::render_with_style(e, inherited, &merged, opts)
        }
        // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4): слитый стиль
        // его уже несёт, а копия для замещаемой коробки — нет. Без переноса
        // блочная картинка под `body { image-orientation: none }` всё равно
        // разворачивалась по метке EXIF.
        "img" => {
            // Единицы шрифта в размерах БЛОЧНОЙ картинки разрешаются так же,
            // как у строчной (рукав `"img"` в `atom`): `image()` = `image_with(e,
            // None)` в собственном стиле держал `Len::Em`, и `height: 1em` при
            // `font-size: 3.75em` пропадал — картинка оставалась 15×15
            // (`c43-rpl-bbx-002`). CSS 2.1 §4.3.2: `em` — вычисленный кегль
            // САМОГО элемента, его и даёт `resolve_em` от кегля родителя.
            let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
            if inline_level(e) {
                replaced_used_style::inline_percentage_width(&mut copy.style, AVAIL_W.get());
            }
            copy.style.image_orient_none = merged.image_orient_none;
            // Признак определённого блока (§10.5) — от слитого стиля: держатель
            // строится из сырого, и `apply` иначе выбрасывал `height: %`
            // (см. строчный рукав `"img"` в `atom_element`; ячейка и лунки —
            // свои пути).
            if !matches!(inherited.display, Some(Display::TableCell) | Some(Display::GridLanes)) {
                copy.style.cb_height_def = merged.cb_height_def;
            }
            // Единицы окна — в точки, как у строчной картинки.
            copy.style.resolve_viewport(opts.viewport);
            image_with(&copy, Some(atom_base_font(inherited, opts)))
        }
        // Замещаемые с картинкой-источником рисуются как <img>: embed через
        // src, object через data, video через poster (css-images §5:
        // object-fit/-position действуют на всех замещаемых).
        "embed" if e.attr("src").is_some() => image(e),
        // Вложенный документ (`<iframe>` и `<object>` с документом в `data`)
        // построен выше; рукав стоит ПЕРЕД картиночным `object`, иначе
        // документ уходил в `image()` пустой коробкой. Объект с документом,
        // который не прочитался, в картинку не превращается — падает в общий
        // рукав и показывает запасное содержимое (HTML §4.8.7).
        "iframe" | "object" if built_iframe.is_some() => built_iframe.take().unwrap(),
        // БЛОЧНЫЙ кадр без пригодного документа — всё равно замещаемая
        // коробка: по умолчанию 300×150 (CSS 2.1 §10.3.2 «…the used value of
        // 'width' becomes 300px», §10.6.2 — 150px; HTML §15.4.4 у `<iframe>`
        // атрибуты `width/height` — размеры). Прежде он шёл общим рукавом
        // пустым блоком: ширина растягивалась на содержащий блок, высота
        // была нулевой, и красная рамка вылезала из-под зелёной
        // (`block-replaced-height-004/005/007`). Строчный кадр не трогаем —
        // там коробка без вклада в строку замерена и откачена (рукав
        // `"iframe"` в `atom_element`).
        // Плавающий кадр сюда не идёт: замерено — `float-replaced-height-005/
        // 007` (0.00 → «красное видно»), у флоата свой путь размеров.
        // Только кадр в обычном блочном потоке: флоат снимает `float` с
        // копии и кладёт её в синтетический гибкий ряд (`wrap_floats`) или в
        // бандовый хост (`flow-root`) — там размер считает свой путь
        // (замерено: `float-replaced-height-005/007` 0.00 → «красное видно»).
        "iframe"
            if !e.style.float.is_some_and(|f| f != 0)
                && matches!(inherited.display, None | Some(Display::Block))
                && e.style.flow_root != Some(true)
                && !matches!(
                    e.style.width,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                ) =>
        {
            let mut copy = pct_height_to_px(e, inherited);
            if !matches!(copy.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
                copy.style.width = Some(Len::Px(300.0));
            }
            let pct_ok = matches!(copy.style.height, Some(Len::Pct(_))) && merged.cb_height_def;
            if !matches!(copy.style.height, Some(Len::Px(_))) && !pct_ok {
                copy.style.height = Some(Len::Px(150.0));
            }
            copy.style.cb_height_def = merged.cb_height_def;
            styled_div(&copy).flex_shrink_0().into_any_element()
        }
        "object" if e.attr("data").is_some() && !object_is_document(e) => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: давать рамке БЕЗ адреса резервную ширину 300
        // точек при `display: block` (§10.3.4 -> §10.3.2). Замерено по срезу
        // из 416 пар семей *image*/*replaced*: приобретено 0, потеряно 1 —
        // `float-replaced-height-004` 0.00 -> «красное видно». Это второй
        // заход на резервный размер бесадресной рамки; первый (коробка
        // 300×150 целиком) стоил 13 пар, запись выше.
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => {
            // Тот же stretch-fit, что у строчного атома (`atom_element`).
            // Содержащий блок — `inherited` (родитель), а не `merged`: это
            // уже собственный стиль рисунка.
            let cb_w = match inherited.width {
                Some(Len::Px(v)) if v > 0.0 => Some(v),
                _ => CB_WIDTH.get().filter(|v| *v > 0.0),
            };
            // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`.
            svg_replaced(e, &crate::svg::stretch_fit(e, cb_w), &merged).unwrap_or_else(|| {
                styled_div_with(e, &merged)
                    .child(SharedString::from("[рисунок]"))
                    .into_any_element()
            })
        }
        // `w_full` — растяжка по СТРОЧНОЙ оси горизонтального родителя. В
        // вертикальном родителе ширина — блочный размер (css-writing-modes-4
        // §7.1): полная ширина раздувала линию `width: 10px` на всё тело
        // (`horizontal-rule-vlr-003`), а строчную ось (высоту) растягивает сам
        // ряд блочного потока.
        "hr" => {
            let d = styled_div_with(e, &merged);
            if inherited.vertical == Some(true) {
                d.into_any_element()
            } else {
                d.w_full().into_any_element()
            }
        }
        // Синтетический узел обтекания формой (см. wrap_floats).
        "shape-flow" => shape_flow(e, &merged, opts),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        // Лунки идут путём сетки taffy (`dom::lanes_as_grid`); сюда доходит
        // только узел, собранный мимо того прохода, — он переводится тем же
        // правилом. Рукописная раскладка `lanes` удалена: все оси (вертикальное
        // письмо, `rtl`) ведёт `taffy::compute::grid::lanes`.
        _ if merged.display == Some(Display::GridLanes) => {
            let mut copy = e.clone();
            crate::dom::lanes_to_grid(&mut copy.style);
            element(&copy, inherited, opts)
        }
        // Таблица — независимый контекст форматирования: её строки в бюджет
        // `line-clamp` не входят (css-overflow-4 §5.3, `webkit-line-clamp-013`).
        // Табличный путь минует общий, где стоит этот сторож (`makes_bfc`),
        // поэтому он ставится здесь.
        _ if matches!(
            e.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        ) =>
        {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Ряд, группа рядов или ячейка ВНЕ таблицы получают анонимную
        // таблицу-обёртку (css-tables-3 §3.1): иначе ячейки складывались
        // столбиком обычных блоков.
        _ if matches!(
            e.style.display,
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
        ) =>
        {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            let wrapper = anon_element("table", vec![Node::Element(e.clone())]);
            table(&wrapper, &merged, opts)
        }
        "table" => {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Список с заданной раскладкой — это уже не список, а контейнер:
        // на `ul` верстают навигацию и наборы чипов.
        "ul" | "ol" if e.style.display.is_none() => list_container::render(e, &merged, opts),
        // `white-space: pre*` значим не меньше тега: переводы строк сохраняет
        // именно он, и на `<div style="white-space: pre">` разметка обязана
        // вести себя так же, как на `<pre>`.
        // Преформат отдельным рисователем — ТОЛЬКО для непереносящегося
        // `white-space: pre`. Переводы строк хранят четыре режима, и три из
        // них переносят строки: `pre-wrap`, `pre-line`, `break-spaces`. Пока
        // сюда уходили все четыре, эти три шли мимо нашей строчной раскладки,
        // где и живут висящие пробелы, разрыв после сохранённого пробела и
        // правила куска. Отсюда же `break-spaces` был неотличим от `pre-wrap`.
        // Внутри преформата может стоять кусок со СВОИМ `white-space`, и он
        // переносится, хотя абзац — нет. Отдельный рисователь преформата
        // правил куска не знает, поэтому такой случай уходит в обычную
        // строчную раскладку (`white-space-pre-031`).
        _ => {
            let mut d = styled_div_with(e, &merged);
            let overflow_plan = absolute_overflow::Plan::new(&merged, inherited);
            if let Some(plan) = &overflow_plan {
                plan.prepare(d.style());
            }
            // ЗАМЕРЕНО И ЗАКРЕПЛЕНО: минимума высоты в видимую область на
            // коробке корня БОЛЬШЕ НЕТ. §10.6.3 — высота корня `auto`, ростом
            // с окно обязан быть начальный содержащий блок, а не коробка:
            // рамка и фон корня уходили полосами до низа окна
            // (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
            // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и
            // содержащим блоком им служит корневой `div` стенда.
            //
            // Замер по семьям backgrounds/*, *root*, positioning/*,
            // containing-block*, normal-flow/*, *margin*: приобретено 11,
            // потеряна одна (`background-root-024`: слой донора-тела лежит в
            // детях корня и отсчитывался от его padding-box).
            // Многоколоночный поток. Своей многоколоночной раскладки нет, но
            // сетка даёт то же расположение: число рядов считаем по числу
            // детей, а заполнение идёт по колонкам — тогда порядок совпадает
            // с браузерным (сверху вниз, затем в следующую колонку).
            // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
            // целых колонок этой ширины влезает в коробку, столько их и будет
            // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
            // считать не от чего, и остаётся прежняя дорожечная раскладка.
            // Умолчание `column-gap: normal` — один кегль (css-align §8.3).
            // Вычисленные значения, а не заданные: `resolve_em` живёт в
            // `inline::inherit`, поэтому точки лежат в `merged`
            // (`render.rs:13235`), а в `e.style` остаётся `Len::Em`. Кегль
            // для `column-gap: normal` — СОБСТВЕННЫЙ кегль элемента
            // (css-align §8.3; Blink `length_utils.cc:1356`
            // `style.GetFontDescription().ComputedPixelSize()`), и он тоже
            // разрешён только в `merged`.
            let used_gap = match merged.column_gap {
                Some(Len::Px(v)) => v,
                _ => match merged.font_size {
                    Some(Len::Px(size)) => size,
                    _ => opts.base_size(),
                },
            };
            // `column-*` — только у блочных контейнеров (css-multicol-1 §2):
            // сетка ими не режется (`grid-multicol-001`).
            let multicol = multicol_container(&e.style);
            // css-multicol-1 §3.4 шаги (05)-(07): N считается по ВЫЧИСЛЕННЫМ
            // 'column-width', 'column-gap' и используемой ширине коробки.
            // Пока брались заданные значения, `column-width: 6em` не
            // проходил гейт `Len::Px`, `count_from_width` был `None`,
            // `width_driven` — ложью, и многоколоночник не включался вовсе:
            // `multicol-width-001` рисовался одним абзацем в 30 знаков
            // вместо пяти колонок по шесть (снимок обеих сторон в
            // `target/scout-mctextflow-2026-09.md` §3.4). Blink
            // абсолютизирует 'column-width' в `float` ещё на вычисленном
            // значении (`css_properties.json5:7513`
            // `ConvertComputedLength<float>`) и в `length_utils.cc:1311`
            // `ResolveUsedColumnCount` читает уже точки.
            let column_width = merged.column_width.filter(|_| multicol);
            let column_count = merged.column_count.filter(|_| multicol);
            // Ось прогрессии колонок — СТРОЧНАЯ ось многоколоночника
            // (css-multicol-1 §2, `Overview.bs:375-379`: «The column boxes are
            // ordered in the inline base direction»). В вертикальном письме она
            // вертикальна, блочная — горизонтальна, у `vertical-rl` от ПРАВОГО
            // края (css-writing-modes-4 §3.1). `inline-size`/`block-size`
            // разложены в физические `height`/`width` ещё в каскаде, поэтому
            // строчный размер коробки здесь — `height`, блочный — `width`.
            // `direction: rtl` в вертикали (колонки снизу вверх) — прежним путём.
            let col_rl = merged.vertical_rl == Some(true);
            let col_axis = if merged.vertical == Some(true) && merged.rtl != Some(true) {
                if col_rl {
                    crate::flow::StackAxis::VerticalRl
                } else {
                    crate::flow::StackAxis::VerticalLr
                }
            } else {
                crate::flow::StackAxis::Horizontal
            };
            let col_vert = col_axis.is_vertical();
            let col_inline_size = if col_vert { merged.height } else { merged.width };
            let count_from_width = match (column_width, col_inline_size) {
                (Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
                    Some((((box_w + used_gap) / (w + used_gap)).floor().max(1.0)) as u16)
                }
                _ => None,
            };
            // Used column-count (css-multicol §3.4, как ResolveUsedColumnCount
            // в blink): заданы оба — МЕНЬШЕЕ из числа и «сколько влезает»;
            // только ширина — сколько влезает.
            let used_count = match (column_count, count_from_width) {
                (Some(c), Some(fw)) => Some(c.min(fw)),
                (Some(c), None) => Some(c),
                (None, fw) => fw,
            };
            let width_driven = column_count.is_none()
                && count_from_width.is_none()
                && matches!(column_width, Some(Len::Px(w)) if w > 0.0);
            // Заданный `column-height` без числа колонок — одна колонка, но с
            // рядами (`column-height-012`: `column-height:40px` и 80px
            // содержимого — два ряда по 40).
            let height_driven =
                e.style.column_height.is_some() && used_count.is_none_or(|n| n <= 1);
            // `column-count: 1` — тоже многоколоночник (css-multicol-1 §2; Blink
            // `ComputedStyle::SpecifiesColumns`: «!HasAutoColumnCount()»), и
            // спаннер в нём режет содержимое на ряды: ряд до спаннера — свой
            // контекст форматирования и держит свои флоаты
            // (`multicol-span-float-002`: `Pink` вставал между флоатами первой
            // строки), рамка предка режется по фрагментам
            // (`multicol-span-all-children-height-008`). Только при спаннере:
            // без него одна колонка по-прежнему рисуется обычным блоком
            // (переполнения одной колонки вбок здесь нет —
            // `scout-multicol-2026-09.md` §10). Элемент списка — мимо:
            // сегментный путь маркер не рисует
            // (`multicol-span-all-list-item-001/002`).
            let lone_span = used_count == Some(1)
                && !height_driven
                && e.style.column_wrap.is_none()
                && e.list_item.is_none()
                && has_deep_spanner(e);
            if let Some(cols) = used_count
                .filter(|n| *n > 1)
                .or(width_driven.then_some(0))
                .or(height_driven.then_some(1))
                .or(lone_span.then_some(1))
            {
                // Сплошной текст режется на колонки по строкам, а не по детям:
                // один длинный абзац иначе оставался в первой колонке целиком.
                // `columns: auto <w>` без ширины коробки решается в замере —
                // туда уходит и число, и ширина колонки (§3.4).
                let col_w_px = match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                };
                let want = (cols > 0).then_some(cols as usize);
                // Ряды колонок (css-multicol-2 §ch, §cwr): высота ряда —
                // `column-height`, а при `column-wrap: wrap` без него —
                // высота коробки (Blink `RowHeight()`:
                // `remaining_content_block_size_`, issue 11754 вариант 2);
                // `column-wrap: auto` = `wrap` при заданном
                // `column-height`. `row-gap: normal` в колонках — 1em (§rg).
                let col_h = match e.style.column_height {
                    Some(Len::Px(h)) if h >= 0.0 => Some(h),
                    _ => None,
                };
                // Блочный размер коробки: в вертикальном письме — ширина.
                let box_h = match if col_vert { e.style.width } else { e.style.height } {
                    Some(Len::Px(h)) if h > 0.0 => Some(h),
                    _ => None,
                };
                let wrap = e.style.column_wrap.unwrap_or(col_h.is_some());
                let em = match e.style.font_size {
                    Some(Len::Px(size)) => size,
                    _ => opts.base_size(),
                };
                let row_gap = match e.style.gap.and_then(|g| g.0) {
                    Some(Len::Px(v)) => v.max(0.0),
                    Some(Len::Em(k)) => k * em,
                    _ => em,
                };
                let rows = (col_h.is_some() || wrap).then_some(crate::flow::Rows {
                    h: col_h.or(if wrap { box_h } else { None }),
                    gap: row_gap,
                    wrap,
                    cap: false,
                });
                // Потолок баланса (`Rows::cap`): СОБСТВЕННАЯ высота коробки в
                // точках. ★ Прежний MC-BALANCE-CAP (float.rs, v164, +3/−9)
                // спускал внешний потолок во ВЛОЖЕННЫЕ многоколоночники без
                // своей высоты — все девять потерь такие; здесь потолок не
                // передаётся вниз (`in_stack`: внутри копии другой стопки —
                // нет), не трогает `column-fill: auto` (там `fixed`) и ряды
                // (`column-height`). `grid-container-fragmentation-004`: 350 в
                // 4 колонках по 100 — баланс уходил в 125, в четвёртой
                // колонке красное 50..100.
                // Потолок баланса — ИСПОЛЬЗУЕМАЯ высота коробки, а не голая `height`:
                // Blink `ConstrainColumnBlockSize` (`column_layout_algorithm.cc:
                // 1774-1793`) берёт `max = min(max-height, height)`, затем
                // `max = max(max, min-height)` («A specified min-block-size may
                // increase the maximum length»; CSS 2.1 §10.7). `multicol-fill-
                // balance-005`: `height:20px; max-height:40px; min-height:100px` —
                // коробка 100, баланс 200/2 = 100 ровно в неё; с потолком 20 колонки
                // выходили по 20, и красный фон 20..100 был виден. `min-height` не в
                // точках (доля, `em`) — потолка нет, как до P5: ниже используемой
                // высоты резать нельзя, а её здесь не знаем.
                let (max_block, min_block) = if col_vert {
                    (e.style.max_width, e.style.min_width)
                } else {
                    (e.style.max_height, e.style.min_height)
                };
                let cap_h = box_h.and_then(|h| {
                    let h = match max_block {
                        Some(Len::Px(m)) if m >= 0.0 => h.min(m),
                        _ => h,
                    };
                    match min_block {
                        None | Some(Len::Auto) => Some(h),
                        Some(Len::Px(m)) => Some(h.max(m)),
                        _ => None,
                    }
                });
                // Вложенный многоколоночник во внешней колонке (`flow::OUTER_ROW`):
                // ряды высотой во внешний фрагментаинер без зазора — граница
                // ряда совпадает с границей внешней колонки (css-break-4 §2.1;
                // Blink `ConstrainColumnBlockSize`, `LayoutLine` :1009-1026
                // «wrap … if we're participating in an outer fragmentation
                // context»). Последний ряд балансируется (css-multicol-1 §7.1
                // «only the last fragment is balanced»).
                let nest_phase = outer_row.map_or(0.0, |r| r.1);
                let nest_rows = outer_row.map(|r| r.0).filter(|_| {
                    col_h.is_none() && e.style.column_wrap.is_none() && !col_vert
                });
                let rows = match nest_rows {
                    Some(hh) => Some(crate::flow::Rows {
                        h: Some(hh),
                        gap: 0.0,
                        wrap: true,
                        cap: false,
                    }),
                    None => rows,
                };
                let rows = rows.or_else(|| {
                    (cap_h.is_some()
                        && e.style.column_fill_auto != Some(true)
                        && !crate::flow::in_stack())
                    .then_some(crate::flow::Rows {
                        h: cap_h,
                        gap: row_gap,
                        wrap: false,
                        cap: true,
                    })
                });
                // Спаннер среди инлайнового потока: режем детей на сегменты,
                // каждый сегмент — свой поток колонок, спаннер — блок между
                // ними (css-multicol §6). С рядами (`column-wrap: wrap` +
                // `column-height`) сегменты не годятся: у каждого свои ряды от
                // нуля, а Blink ведёт ОДИН курсор по коробке
                // (`column_layout_algorithm.cc` `intrinsic_block_size_`,
                // `LayoutSpanner`: спаннер, не влезший в остаток ряда, — со
                // следующего ряда; `column-height-006/013/017…020`). Такой
                // многоколоночник идёт единой стопкой, спаннер — её ребёнком
                // (`StackChild::span`). Внутри копии другой стопки — по-прежнему
                // сегментами: перенос ряда во внешнюю колонку не написан
                // (`column-height-029`, scout-columnwrap-2026-09b.md §2.3).
                // Спаннер бывает НЕ прямым ребёнком: css-multicol-1
                // §column-span (`Overview.bs:1497-1499`) — «A spanning element
                // may be lower than the first level of descendants as long as
                // they are part of the same formatting context, and there is
                // nothing between the spanning element and multicol container
                // that establishes a containing block for fixed position
                // descendants». Спаннер выносится ИЗ ПОТОКА и режет
                // многоколоночник на «до», «спаннер во всю ширину» и «после»,
                // а его предки внутри многоколоночника разрезаются вместе с
                // ним. Поднимаем таких потомков к прямым детям ОДИН раз, до
                // всех решений ниже: дальше и сегментный путь, и единая
                // стопка, и текстовый `column_flow` видят спаннер прямым
                // ребёнком. Blink ведёт для этого путь `ColumnSpannerPath`
                // (`column_spanner_path.h`), у нас пути нет — предки режутся
                // прямо в дереве (`hoist_spanners`).
                // Сегментный путь ниже: метка родителя предыдущего спаннера,
                // если между ними не легло ни одного ряда колонок, и выложен
                // ли уже хоть один кусок (для сторожей полей).
                let mut span_prev: Option<String> = None;
                let mut seg_open = false;
                let hoisted;
                let e = match hoist_spanners(&e.children) {
                    Some(kids) => {
                        let mut c = e.clone();
                        c.children = kids;
                        hoisted = c;
                        &hoisted
                    }
                    None => e,
                };
                let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
                let unified = rows.is_some_and(|r| r.wrap && r.h.is_some()) && !crate::flow::in_stack();
                // Хвостовой ряд колонок при `column-fill: auto` НЕ стоит перед
                // спаннером, и §column-fill («content in a multi-column line that
                // does not immediately precede a spanner») велит заполнять его
                // подряд до высоты коробки. Сегмент ниже теряет высоту
                // (`sub.style.height = None`) и уходил в балансировку:
                // `no-balancing-after-column-span` — 200×50 двумя колонками
                // вместо 100×100 одной. Высота хвоста — остаток коробки после
                // спаннера (Blink `ConstrainColumnBlockSize`:
                // `max -= CurrentContentBlockOffset(line_offset)`). Только когда
                // до хвоста стоит ОДИН измеримый спаннер и больше ничего:
                // высоту сбалансированных рядов до спаннера здесь не знаем.
                let rest_h: Option<f32> = match (e.style.column_fill_auto, box_h, rows) {
                    (Some(true), Some(total), None) => e
                        .children
                        .iter()
                        .rposition(&is_span)
                        .and_then(|j| {
                            let px = |l: &Option<Len>| match l {
                                None => Some(0.0),
                                Some(Len::Px(v)) => Some(*v),
                                _ => None,
                            };
                            let mut used = 0.0f32;
                            let mut spans = 0usize;
                            for n in &e.children[..=j] {
                                if is_blank(n) {
                                    continue;
                                }
                                let Node::Element(sp) = n else {
                                    return None;
                                };
                                if !is_span(n) {
                                    return None;
                                }
                                spans += 1;
                                let st = inline::inherit(&merged, &sp.style);
                                used += shape_full(sp, 4, ShapeCx::COLUMNS)?.0
                                    + px(&st.margin.top)?
                                    + px(&st.margin.bottom)?;
                            }
                            (spans == 1).then(|| (total - used).max(0.0))
                        }),
                    _ => None,
                };
                if e.children.iter().any(&is_span) && !unified {
                    // `<fieldset>`: отрисованная легенда стоит ВНЕ колонок —
                    // многоколоночность получает анонимная коробка содержимого
                    // fieldset (HTML §15.3.13 «The fieldset and legend
                    // elements», пересказ; Blink `LayoutFieldset` +
                    // `FieldsetContentBox`). Прежде легенда падала в первый ряд и
                    // занимала колонку (`multicol-span-all-fieldset-001…003`:
                    // эталон — `fieldset > legend + div.inner` с колонками).
                    // Отрисованная — первая `legend` в потоке.
                    let mut kids = e.children.clone();
                    if e.tag == "fieldset" {
                        if let Some(i) = kids.iter().position(|n| {
                            matches!(n, Node::Element(c) if c.tag == "legend" && !out_of_flow(&c.style))
                        }) {
                            let legend = kids.remove(i);
                            d = d.children(blocks(&[legend], &merged, opts));
                        }
                    }
                    for chunk in kids.split_inclusive(&is_span) {
                        let (body, span) = match chunk.split_last() {
                            Some((last, head)) if is_span(last) => (head, Some(last)),
                            _ => (chunk, None),
                        };
                        if body.iter().any(|n| !is_blank(n)) {
                            // Ряд колонок между спаннерами — новый контекст
                            // форматирования: поля спаннеров сквозь него не
                            // схлопываются (§column-span: «margins on elements
                            // inside a column box will not collapse with the margin
                            // of a spanner»). Сам ряд для taffy — лист или
                            // не-блок, насквозь его поле не проходит.
                            span_prev = None;
                            seg_open = true;
                            let mut seg = e.clone();
                            seg.children = body.to_vec();
                            seg.style.column_span = None;
                            // Одна колонка со спаннером (`lone_span`) — ряд рисуется
                            // обычным блоком: `column_flow` спускается в
                            // единственного блочного ребёнка и рисует только его
                            // текст, теряя коробку (фон, рамку, высоту фрагмента
                            // `container` в `multicol-span-all-children-height-005/
                            // 008`). Клон `sub` с одной колонкой спаннеров не несёт,
                            // и гейт `lone_span` его снова не пускает.
                            let flow = if lone_span {
                                None
                            } else {
                                column_flow(&seg, &merged, opts, want, col_w_px)
                            };
                            if let Some(el) = flow {
                                d = d.child(el);
                            } else {
                                // Блочный сегмент: рекурсия в общий рендер —
                                // он сам выберет укладку колонок; коробка
                                // (фон/рамки/поля) остаётся на хосте.
                                let mut sub = seg.clone();
                                sub.style.background = None;
                                // Всё, что многоколоночник рисует и сдвигает КОРОБКОЙ,
                                // уже стоит на хосте `d` (`styled_div_with(e, ..)`);
                                // клон ряда повторял это на каждом ряду: контур и
                                // тень вокруг каждого ряда, двойная прозрачность,
                                // фильтр и трансформ, двойной сдвиг `relative`, а у
                                // `position: absolute` ряды выпадали из потока и
                                // ложились друг на друга (`multicol-span-all-
                                // fieldset-002/003`, `-button-002/003`). Обрезку
                                // переполнения делает хост: по css-multicol-1
                                // §overflow режет коробка многоколоночника, а не ряд.
                                // Ряд остаётся содержащим блоком (`relative`), как
                                // прежде.
                                sub.style.gradient = None;
                                sub.style.bg_image = None;
                                sub.style.border_image = None;
                                sub.style.shadows = Vec::new();
                                sub.style.inset_shadows = Vec::new();
                                sub.style.outline = None;
                                sub.style.opacity = None;
                                sub.style.transform = None;
                                sub.style.filter = None;
                                sub.style.mask_image = None;
                                sub.style.backdrop_blur = None;
                                if matches!(
                                    sub.style.position,
                                    Some(crate::computed::Position::Absolute)
                                        | Some(crate::computed::Position::Fixed)
                                ) {
                                    sub.style.position = Some(crate::computed::Position::Relative);
                                }
                                sub.style.inset = Default::default();
                                sub.style.z_index = None;
                                sub.style.overflow_x = None;
                                sub.style.overflow_y = None;
                                sub.style.margin = Default::default();
                                sub.style.padding = Default::default();
                                sub.style.border_width = Default::default();
                                sub.style.width = None;
                                sub.style.height = None;
                                // Хвост после спаннера при `column-fill: auto` —
                                // остаток коробки (`rest_h`): укладка заполняет
                                // колонки подряд, а не балансирует. Только
                                // измеримым блокам: неизмеримый ряд уходит в
                                // запасную сетку, где заданная высота растянула бы
                                // дорожки `auto`.
                                if span.is_none()
                                    && body.iter().all(|n| {
                                        is_blank(n)
                                            || matches!(n, Node::Element(k)
                                                if !k.inline
                                                    && shape_full(k, 4, ShapeCx::COLUMNS).is_some())
                                    })
                                {
                                    if let Some(rest) = rest_h {
                                        sub.style.height = Some(Len::Px(rest));
                                    }
                                }
                                d = d.child(div().children(blocks(
                                    &[Node::Element(sub)],
                                    &merged,
                                    opts,
                                )));
                            }
                        }
                        if let Some(Node::Element(sp)) = span {
                            // Хост сегментов — блок taffy (`Display::Block`,
                            // `vendor/gpui/src/style.rs:862`), и поля соседних детей
                            // он схлопывает сам (`vendor/taffy/src/compute/
                            // block.rs:186-210`): соседние спаннеры ОДНОГО предка —
                            // верно («the margins of two adjacent spanners will
                            // collapse with each other»; `multicol-span-all-margin-
                            // 003`). Запрещённое схлопывание гасит нулевая гибкая
                            // сторожка — её taffy насквозь не проходит
                            // (`has_styles_preventing_being_collapsed_through`:
                            // `!style.is_block()`): (1) перед ПЕРВЫМ куском-спаннером
                            // — многоколоночник сам контекст форматирования, и
                            // верхнее поле спаннера сквозь его верх не уходит;
                            // (2) между спаннерами РАЗНЫХ исходных предков (метка
                            // `kamin-span-parent`, `spanner_parts`).
                            let key = sp.attr("kamin-span-parent").unwrap_or("").to_string();
                            if !seg_open || span_prev.as_ref().is_some_and(|k| *k != key) {
                                d = d.child(div().flex());
                            }
                            seg_open = true;
                            span_prev = Some(key);
                            let inner = inline::inherit(&merged, &sp.style);
                            // Спаннер — независимый контекст форматирования
                            // (§column-span). Голый `styled_div_with` — блок taffy,
                            // и тот схлопывал поле первого/последнего ребёнка
                            // сквозь край спаннера (`vendor/taffy/src/compute/
                            // block.rs:186`): в `multicol-span-all-margin-nested-
                            // firstchild-001` `<span style="margin: 2em 0">` уводил
                            // `<h6>` вниз, и чёрного фона не было видно вовсе.
                            // Оболочка — гибкая колонка, ровно как у блока общего
                            // пути (`d.flex().flex_col()` при пустом `display`), где
                            // поля детей сводит сам `blocks()` — а он с предикатом
                            // `own_context_style` поле наружу больше не отдаёт.
                            let shell = styled_div_with(sp, &inner);
                            let shell = if matches!(inner.display, None | Some(Display::Block))
                                && inner.vertical != Some(true)
                            {
                                shell.flex().flex_col()
                            } else {
                                shell
                            };
                            d = d.child(shell.children(blocks(
                                &sp.children,
                                &inner,
                                opts,
                            )));
                        }
                    }
                    // Нижняя сторожка: нижнее поле последнего спаннера остаётся
                    // внутри многоколоночника — он независимый контекст
                    // форматирования (CSS 2.1 §8.3.1).
                    d = d.child(div().flex());
                    return d.into_any_element();
                }
                // Текстовый путь `text-box-trim` не знает (ни у краёв колонок, ни у
                // первой/последней строки) — такой многоколоночник идёт стопкой
                // со строками (`line_run_shape`; `text-box-trim-multicol-009/010`).
                let trim_host = merged.text_box_trim_start || merged.text_box_trim_end;
                if let Some(el) = column_flow(e, &merged, opts, want, col_w_px)
                    .filter(|_| nest_rows.is_none() && !trim_host)
                {
                    // Коробка элемента остаётся своей: отступы и фон
                    // принадлежат ей, поток живёт внутри.
                    return d.child(el).into_any_element();
                }
                if cols == 0 {
                    // Число колонок при `columns: auto <w>` решается только в
                    // замере текстового потока; блочный фоллбек — дорожками.
                    if let Some(w) = col_w_px {
                        d = d.grid().grid_cols_min(px(w));
                    }
                } else {
                    // Блочные дети с ИЗВЕСТНЫМИ высотами — честная укладка по
                    // колонкам с балансом и монолитами (css-break, фаза 1;
                    // план target/scout-multicol.md / scout-fragmentation.md).
                    // Коробка ребёнка и его вертикальные поля отдельно:
                    // поля схлопываются между соседями и на границах колонок.
                    // Флоаты в поддереве ломают известность высоты.
                    // Прямые абсолюты многоколоночника — не в стопку: их
                    // содержащий блок — весь контейнер, рисуются его детьми
                    // рядом со стопкой (`out-of-flow-in-multicolumn-094…097`
                    // при нулевой записи в стопке уходили в колонку).
                    // Статическая позиция (CSS 2.1 §10.6.4: «where the box
                    // would have been if position were static»; §10.3.7 — то
                    // же по строчной оси) — правило ПОЗИЦИОНИРОВАННОЙ
                    // коробки. У плавающей своё место по §9.5, и щуп ей не
                    // положен: нулевая запись в стопке меняет `kids.len()`, с
                    // ним `balance_last` и весь план `fill_avoiding`
                    // (`multicol-fill-balance-038` — монолитный флоат с
                    // полями 40/70 при `margin-bottom: -30px` у соседа:
                    // 0.32 -> «красное видно», замерено на v206). Blink
                    // перебирает в `LayoutFragmentainerDescendants` только
                    // `oof_positioned_candidates`; флоат идёт обычной
                    // укладкой (`PositionFloat`).
                    let positioned = |s: &Computed| {
                        matches!(
                            s.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    };
                    // Строчный размер колонки для меры строк (`with_lines`):
                    // css-multicol-1 §3.4 (11) «W := max(0, (U + column-gap)/N −
                    // column-gap)» при U в точках; иначе строки не меряются.
                    // Ширина `auto` блока в потоке — ширина содержимого родителя
                    // (CSS 2.1 §10.3.3), когда та в точках и у коробки нет боковых
                    // полей, рамок и отбивок (`text-box-trim-multicol-011-ref`:
                    // многоколоночник без ширины в `.container` 640).
                    let line_inline_size = col_inline_size.or_else(|| {
                        let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                        let b = e.style.borders();
                        (!col_vert
                            && matches!(e.style.width, None | Some(Len::Auto))
                            && matches!(e.style.display, None | Some(Display::Block))
                            && e.style.float.unwrap_or(0) == 0
                            && !out_of_flow(&e.style)
                            && [&e.style.margin.left, &e.style.margin.right, &e.style.padding.left, &e.style.padding.right, &b.left, &b.right]
                                .into_iter()
                                .all(zero)
                            // Только простой поток: строки и листья с высотой в точках.
                            // Блок с переполнением своей высоты (`css-break/block-max-
                            // height-001-ref`: 160 с ребёнком 200) стопка рисует иначе,
                            // чем прежний путь рисует тест с `max-height` (0.00 -> 11).
                            && e.children.iter().all(|n| match n {
                                Node::Text(_) => true,
                                Node::Element(k) => {
                                    k.inline
                                        || inline_content(k)
                                        || (k.children.iter().all(is_blank)
                                            && matches!(k.style.height, Some(Len::Px(_))))
                                }
                            }))
                        .then_some(inherited.width)
                        .flatten()
                        .filter(|w| matches!(w, Len::Px(_)))
                    });
                    let line_col_w = match line_inline_size {
                        Some(Len::Px(u)) if merged.border_box != Some(true) && cols > 0 => {
                            Some(((u + used_gap) / cols as f32 - used_gap).max(0.0))
                        }
                        _ => None,
                    };
                    // Строчные прогоны среди блоков — анонимными блоками (CSS 2.1
                    // §9.2.1.1), только когда строки можно измерить.
                    let grouped_e = line_col_w.and_then(|_| group_inline_runs(e));
                    let ge: &Element = grouped_e.as_ref().unwrap_or(e);
                    // Плавающий прямой ребёнок во всю ширину колонки рядом с
                    // собой ничего не терпит: строки и блоки встают под ним,
                    // как под блоком, — в стопку колонок он идёт БЛОКОМ и
                    // рвётся по колонкам вместе с потоком (css-break-3 §4:
                    // флоат — фрагментируемая коробка; `css-break/float-001`:
                    // флоат 200px в колонках по 100 — прежний путь рисовал его
                    // соседом стопки одним куском).
                    let zero_m = |l: &Option<Len>| match l {
                        None => true,
                        Some(Len::Px(v)) => v.abs() < 0.01,
                        _ => false,
                    };
                    let full_float = |c: &Element| {
                        c.style.float.is_some_and(|f| f != 0)
                            && matches!(c.style.width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
                            && zero_m(&c.style.margin.left)
                            && zero_m(&c.style.margin.right)
                            // Поля флоата не схлопываются и у края колонки не
                            // усекаются (CSS 2.1 §8.3.1), а у блока стопки —
                            // да: флоат с вертикальными полями — прежним путём
                            // (`multicol-fill-balance-037`: `margin: 40px 0`).
                            && zero_m(&c.style.margin.top)
                            && zero_m(&c.style.margin.bottom)
                            && c.style.shape_outside.is_none()
                            && !positioned(&c.style)
                    };
                    let floats_blocked = ge
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if full_float(c)))
                    .then(|| {
                        let mut g = ge.clone();
                        for n in g.children.iter_mut() {
                            if let Node::Element(c) = n
                                && full_float(c)
                            {
                                c.style.float = None;
                                c.style.clear = None;
                                c.attrs.push(("kamin-float-block".into(), "1".into()));
                            }
                        }
                        g
                    });
                    let ge: &Element = floats_blocked.as_ref().unwrap_or(ge);
                    // Обёртка flex/сетки с ЕДИНСТВЕННЫМ элементом-`clone`
                    // (`clone_wrapper_item`): по блочной оси такая обёртка
                    // раскладывается ровно как блок с этим ребёнком, и фрагменты
                    // клонированного украшения строятся у самого элемента.
                    let unwrapped = ge
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if clone_wrapper_item(c).is_some()))
                        .then(|| {
                            let mut g = ge.clone();
                            for n in g.children.iter_mut() {
                                if let Node::Element(c) = n
                                    && let Some(item) = clone_wrapper_item(c)
                                {
                                    *c = item;
                                }
                            }
                            g
                        });
                    let ge: &Element = unwrapped.as_ref().unwrap_or(ge);
                    // Плавающие прямые дети — как прежде: не в стопку,
                    // рисуются её соседями.
                    let direct_oof: Vec<Element> = ge
                        .children
                        .iter()
                        .filter_map(|n| match n {
                            Node::Element(c)
                                if out_of_flow(&c.style) && !positioned(&c.style) =>
                            {
                                Some(c.clone())
                            }
                            _ => None,
                        })
                        .collect();
                    // Позиционированные прямые дети несут МЕСТО В ПОТОКЕ —
                    // номер среди детей, ушедших в стопку: точка статической
                    // позиции лежит В КОЛОНКЕ, а не под стопкой, и взять её
                    // больше неоткуда — раскладка под нами про колонки не
                    // знает. Счёт идёт по тем же детям, что отбирает
                    // `stackable` ниже: непустые и не внепоточные.
                    let oof_static: Vec<(usize, Element)> = {
                        let mut at = 0usize;
                        let mut out: Vec<(usize, Element)> = Vec::new();
                        for n in ge.children.iter().filter(|n| !is_blank(n)) {
                            let Node::Element(c) = n else {
                                at += 1;
                                continue;
                            };
                            if positioned(&c.style) {
                                out.push((at, c.clone()));
                            } else if !out_of_flow(&c.style) {
                                at += 1;
                            }
                        }
                        out
                    };
                    // Высота внешнего фрагментаинера для вложенного рядами
                    // (`nested_rows_shape`): `column-fill: auto` и блочный размер
                    // в точках, без своих рядов.
                    let outer_frag = (e.style.column_fill_auto == Some(true)
                        && rows.is_none()
                        && !col_vert)
                        .then(|| col_h.or(box_h))
                        .flatten();
                    let first_flow = ge
                        .children
                        .iter()
                        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| n as *const Node);
                    // Чью меру дала `nested_rows_shape` — только их копии рядами.
                    let nested_auto: std::cell::RefCell<Vec<u64>> = Default::default();
                    // Вложенный многоколоночник `height: auto` под БАЛАНСОМ внешнего
                    // (`nested_whole`): одна сбалансированная строка колонок целиком.
                    // css-multicol-1 §2: вложенный многоколоночник — сам мультиколонный
                    // контейнер, его колонки и линейки рисуются внутри внешней колонки
                    // (Blink `ColumnLayoutAlgorithm` для внутреннего — та же раскладка).
                    // Копия стопки шла узкой веткой `styled_div_with` и рисовала его
                    // плоско — без колонок и линеек (`multicol-rule-color-inherit-001`).
                    let nested_whole: std::cell::RefCell<Vec<u64>> = Default::default();
                    let whole_ok = e.style.column_fill_auto != Some(true)
                        && rows.is_none_or(|r| r.cap)
                        && nest_rows.is_none()
                        && !col_vert;
                    // Дети, чью высоту меряет раскладка копии (`StackChild::measure`).
                    let measured_kids: std::cell::RefCell<Vec<(u64, f32)>> = Default::default();
                    let measure_ok = e.style.column_fill_auto == Some(true)
                        && rows.is_none()
                        && nest_rows.is_none()
                        && !col_vert
                        && (col_h.is_some() || matches!(e.style.height, Some(Len::Px(_))));
                    let last_flow = ge
                        .children
                        .iter()
                        .rev()
                        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| n as *const Node);
                    let stackable: Option<Vec<(Element, Shape)>> = with_lines(&merged, line_col_w, opts, || ge
                        .children
                        .iter()
                        .filter(|n| !is_blank(n))
                        .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| match n {
                            // Вложенный многоколоночник `height: auto` с верха
                            // внешней колонки — рядами (`nested_rows_shape`).
                            Node::Element(c)
                                if first_flow == Some(n as *const Node)
                                    && matches!(c.style.height, None | Some(Len::Auto))
                                    && zero_len(c.style.margin.top)
                                    && outer_frag.is_some()
                                    && line_col_w.is_some()
                                    && nested_rows_box(c) =>
                            {
                                let c = &resolved_lengths(c, &merged);
                                let (Some(hh), Some(cw)) = (outer_frag, line_col_w) else {
                                    return None;
                                };
                                match nested_rows_shape(c, &merged, hh, cw, opts) {
                                    Some(h) => {
                                        nested_auto.borrow_mut().push(c.node_id);
                                        Some(((*c).clone(), h))
                                    }
                                    None => shape_full(c, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h)),
                                }
                            }
                            Node::Element(c)
                                if whole_ok
                                    && matches!(c.style.height, None | Some(Len::Auto))
                                    && line_col_w.is_some()
                                    && nested_rows_box(c) =>
                            {
                                // Высоту даёт раскладка самой копии (`StackChild::measure`):
                                // внутренний многоколоночник балансирует себя сам, и мера
                                // `shape_full` его колонок не видит (сумма детей).
                                let c = resolved_lengths(c, &merged);
                                let w = line_col_w?;
                                let m = |l: &Option<Len>| match l {
                                    Some(Len::Px(v)) => Some(*v),
                                    None | Some(Len::Auto) => Some(0.0),
                                    _ => None,
                                };
                                let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                                nested_whole.borrow_mut().push(c.node_id);
                                measured_kids.borrow_mut().push((c.node_id, w));
                                Some((c, (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                            }
                            // `position: relative` укладке не мешает — сдвиг
                            // накладывается на месте (корень A1).
                            Node::Element(c)
                                if !c.inline
                                    && (c.style.position.is_none()
                                        || c.style.position
                                            == Some(crate::computed::Position::Relative))
                                    && (c.style.float.unwrap_or(0) == 0
                                        || block_like_float(&c.style)) =>
                            {
                                // В вертикальном письме мера — по БЛОЧНОЙ оси
                                // (css-multicol-1 §2: «The column height is the
                                // length of the column box in the block
                                // direction»): поддерево меряется ПОВЁРНУТЫМ
                                // клоном (`transpose_tree`), рисуется исходным.
                                // Длины коробки — в точках (`resolved_lengths`):
                                // и мере, и копиям, и распоркам роста — одно дерево.
                                let mut c = resolved_lengths(c, &merged);
                                // `text-box-trim` многоколоночника режет его ПЕРВУЮ и
                                // ПОСЛЕДНЮЮ отформатированную строку (css-inline-3
                                // §4.2) — у строчного блока-ребёнка с края потока они
                                // его же. Метка, а не флаг стиля: срез на разрывах
                                // решает ближайшая коробка со своим флагом
                                // (`brk_trim`), и флаг ребёнка отнял бы его у хоста
                                // (`text-box-trim-multicol-005`).
                                if inline_content(&c) {
                                    if merged.text_box_trim_start && first_flow == Some(n as *const Node) {
                                        c.attrs.push(("kamin-host-trim-start".into(), "1".into()));
                                    }
                                    if merged.text_box_trim_end && last_flow == Some(n as *const Node) {
                                        c.attrs.push(("kamin-host-trim-end".into(), "1".into()));
                                    }
                                }
                                let c = &c;
                                if col_vert {
                                    let t = transpose_tree(c, col_rl)?;
                                    shape_full(&t, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h))
                                } else {
                                    shape_full(c, 4, ShapeCx::COLUMNS)
                                        .map(|h| ((*c).clone(), h))
                                        .or_else(|| {
                                            // Мера `shape_full` не выразила ребёнка (флоаты,
                                            // внепоточные потомки, таблица со сросшимися
                                            // рамками …). Прежде отказ ОДНОГО ребёнка
                                            // отправлял весь многоколоночник в сетку без
                                            // фрагментации — содержимое вовсе не переходило
                                            // в следующую колонку. css-break-3 §4: любая
                                            // блочная коробка фрагментируема; её высоту даёт
                                            // сама раскладка копии в колонку (`StackChild::
                                            // measure`, Blink меряет ребёнка тем же
                                            // алгоритмом, что и кладёт), а разрез — по краю
                                            // колонки (`slice`, без точек класса A).
                                            // Только колонки с заданной высотой
                                            // (`column-fill: auto`): у баланса высота
                                            // коробки сама зависит от меры, а раскладка
                                            // копии флоаты в высоту не берёт (а коробка
                                            // многоколоночника — корень контекста — берёт).
                                            // Принудительного разрыва внутри мера без
                                            // точек тоже не видит.
                                            if !measure_ok || forced_inside(c, 6) {
                                                return None;
                                            }
                                            let w = line_col_w?;
                                            let m = |l: &Option<Len>| match l {
                                                Some(Len::Px(v)) => Some(*v),
                                                None | Some(Len::Auto) => Some(0.0),
                                                _ => None,
                                            };
                                            let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                                            measured_kids.borrow_mut().push((c.node_id, w));
                                            Some(((*c).clone(), (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                                        })
                                }
                            }
                            _ => None,
                        })
                        .collect());
                    if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
                        // Копий у ребёнка — сколько колонок он может занять: без
                        // рядов ровно `cols` (как прежде), с рядами — по своей
                        // высоте против высоты ряда, с запасом на поля и срезы.
                        // Спаннер между колонками не режется — копий ему не
                        // надо, и в счёт он не входит (`column-height-019`:
                        // спаннер 85px при ряде 5px).
                        let copies = match rows {
                            Some(r) => {
                                let per = r.h.unwrap_or(f32::MAX).max(1.0);
                                let span = kids
                                    .iter()
                                    .filter(|(c, _)| c.style.column_span != Some(true))
                                    .map(|(_, s)| (s.0 / per).ceil() as usize)
                                    .max()
                                    .unwrap_or(0);
                                (span + 2).max(cols as usize).min(48)
                            }
                            None => cols.max(1) as usize,
                        };
                        // `column-fill: auto`: колонки заполняются подряд до
                        // `column-height`, а без него — до высоты коробки
                        // (css-multicol-1 §3.3, как прежде).
                        // `column-fill: auto` заполняет колонку до БЛОЧНОГО
                        // размера коробки — в вертикальном письме это ширина.
                        let fixed = if let Some(hh) = nest_rows {
                            (e.style.column_fill_auto == Some(true)).then_some(hh)
                        } else if e.style.column_fill_auto == Some(true) {
                            col_h.or(match if col_vert { e.style.width } else { e.style.height } {
                                Some(Len::Px(h)) => Some(h),
                                _ => None,
                            })
                        } else {
                            None
                        };
                        // Рост от вытолкнутых монолитов — распорки в
                        // DOM-клонах до сборки копий (`grow_pushed`).
                        // Многострочный колоночный flex — строками, параллельными
                        // потоками (`split_flex_lines`). Только при заполнении
                        // `column-fill: auto` с заданной высотой и без рядов:
                        // баланс считал бы содержимое по сумме записей, а строки
                        // идут бок о бок.
                        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.10): то же при балансе
                        // (`rows.is_none()` без `fixed`) с оценкой баланса по
                        // самой длинной строке группы (`runs_guess`). css-break/
                        // flexbox 319: +0/−2 — `multi-line-row-flex-fragmentation-
                        // 037/038` 0.07 → «красное видно»; балансные 033-035, 048
                        // не взяты. Только колонки (без рядов) при балансе — 0/0.
                        // Балансу нужен свой подбор высоты по строкам, а не оценка.
                        // Строки flex и распорки роста (`grow_pushed`) меряют
                        // ФИЗИЧЕСКОЕ дерево и знают только вертикальную ось —
                        // в вертикальном письме их нет (следующий шаг).
                        let (kids, kid_par, kid_parent, kid_starts) = if fixed.is_some() && rows.is_none() && !col_vert {
                            let col_w = match merged.width {
                                Some(Len::Px(w)) if merged.border_box != Some(true) && cols > 0 => {
                                    Some((w - used_gap * (cols as f32 - 1.0)) / cols as f32)
                                }
                                _ => None,
                            };
                            split_flex_lines(kids, col_w, &merged)
                        } else {
                            let n = kids.len();
                            (kids, vec![crate::flow::Par::default(); n], vec![None; n], (0..=n).collect())
                        };
                        // `break-inside: avoid` без настоящего монолита — у любого
                        // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
                        // коробка выше колонки рвётся, а не переполняет её.
                        let mut kid_par = kid_par;
                        for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
                            if p.group == 0 {
                                p.avoid_only = c.style.break_inside_avoid && avoid_only_monolith(c);
                                p.float = c.attr("kamin-float-block").is_some();
                                p.clears = c.style.clear.is_some();
                            }
                        }
                        let kids = if col_vert {
                            kids
                        } else {
                            grow_pushed(kids, cols as usize, fixed, rows, copies, &kid_par)
                        };
                        // `box-decoration-break: clone`: геометрия фрагментов —
                        // ДО сборки копий: каждая копия такой коробки строится
                        // отдельной коробкой своей высоты (`clone_fragment`).
                        // Щуп — как у `grow_pushed`, но с НАСТОЯЩИМ параллельным
                        // потоком соседей (та же мера, что у `StackChild` ниже):
                        // иначе план соседа с потоком разошёлся бы с укладкой.
                        // Без `clone` среди детей не считается вовсе.
                        let clone_plan: Vec<Vec<(f32, f32)>> =
                            if !col_vert && kids.iter().any(|(c, _)| clone_dec(c).is_some()) {
                                let probe: Vec<crate::flow::Kid> = kids
                                    .iter()
                                    .enumerate()
                                    .map(|(pi, (c, s))| {
                                        let mut m = c.clone();
                                        m.style.margin.top = None;
                                        m.style.margin.bottom = None;
                                        let (over, cuts, forced, solid) = match shape_full(
                                            &m,
                                            4,
                                            ShapeCx {
                                                unclamped: true,
                                                ..ShapeCx::COLUMNS
                                            },
                                        )
                                        .filter(|_| {
                                            fixed.is_some()
                                                && plain_block_tree(&m, 4)
                                                && visible_overflow(&m.style)
                                        })
                                        .filter(|u| u.0 > s.0 + 0.01)
                                        {
                                            Some(u) => (u.0, u.3, u.4, u.5),
                                            None => (s.0, s.3.clone(), s.4.clone(), s.5.clone()),
                                        };
                                        crate::flow::Kid {
                                            h: s.0,
                                            mt: s.1,
                                            mb: s.2,
                                            monolith: solid_box(c),
                                            cuts,
                                            force_before: edge_break(c, false),
                                            force_after: edge_break(c, true),
                                            avoid_before: edge_avoid(c, false),
                                            avoid_after: edge_avoid(c, true),
                                            forced,
                                            solid,
                                            span: c.style.column_span == Some(true) && !c.inline,
                                            over,
                                            clone_dec: clone_dec(c),
                                            // Тот же предикат, что у `StackChild` ниже:
                                            // иначе план соседа разошёлся бы с укладкой.
                                            overflow_top: fixed.is_some()
                                                && rows.is_none()
                                                && !parallel_items_inside(c, 4),
                                            repeat: repeat_leads(c, fixed, rows),
                                            par: kid_par[pi],
                                        }
                                    })
                                    .collect();
                                crate::flow::ColumnStack::frags_of(
                                    &probe,
                                    cols as usize,
                                    fixed,
                                    rows,
                                    copies,
                                )
                            } else {
                                Vec::new()
                            };
                        let rule = if e.style.column_rule_visible == Some(true) {
                            Some((
                                match e.style.column_rule_width {
                                    Some(Len::Px(v)) => v,
                                    Some(Len::Em(k)) => {
                                        k * match e.style.font_size {
                                            Some(Len::Px(fs)) => fs,
                                            _ => opts.base_size(),
                                        }
                                    }
                                    _ => 3.0,
                                },
                                e.style
                                    .column_rule_color
                                    .or(merged.color)
                                    .unwrap_or(crate::value::Color {
                                        r: 0.0,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 1.0,
                                    })
                                    .to_hsla(),
                            ))
                        } else {
                            None
                        };
                        // С рядами линейки (`column-rule` со втяжкой/разрывом,
                        // `row-rule` — css-multicol-2 §rg/§crc → css-gaps-1)
                        // красит `GapRulePainter` по границам колонок и
                        // спаннеров из стопки (`ColumnStack::gap_items`);
                        // простая полоса `rule` тогда не рисуется. Без рядов —
                        // как прежде (`multicol-rule-*` не трогаются).
                        let gap_spec = rows
                            .filter(|r| r.wrap)
                            .and_then(|_| multicol_gap_rule_spec(e, &merged, opts, used_gap, row_gap));
                        let gap_items = gap_spec
                            .as_ref()
                            .map(|_| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt ^ 0x4D43_4F4C));
                        let rule = rule.filter(|_| gap_spec.is_none());
                        // Где начинается ребёнок в первой внешней колонке — для
                        // вложенного рядами с заданной высотой (`nest_row`): все
                        // предыдущие встают целиком в первую колонку, без
                        // принудительных разрывов и параллельных строк flex.
                        // Внешний многоколоночник с БАЛАНСОМ и единственным ребёнком —
                        // вложенным заданной высоты `h` без точек разреза: баланс
                        // делит его поровну, и высота внешней колонки известна до
                        // укладки — `h / cols` (не выше потолка коробки; css-multicol-1
                        // §7.1; `multicol-breaking-005`: 300 в трёх колонках по 100).
                        let balanced_frag: Option<f32> = (fixed.is_none()
                            && rows.is_none_or(|r| r.cap)
                            && cols > 1
                            && kids.len() == 1)
                            .then(|| {
                                let (c, s) = &kids[0];
                                (nested_rows_box(c)
                                    && matches!(c.style.height, Some(Len::Px(_)))
                                    && s.3.is_empty()
                                    && s.1.abs() < 0.01)
                                    .then(|| {
                                        let per = s.0 / cols as f32;
                                        rows.and_then(|r| r.h).map_or(per, |cap| per.min(cap))
                                    })
                            })
                            .flatten()
                            .filter(|h| *h > 1.0);
                        let fixed_nest = fixed.or(balanced_frag);
                        let nest_at: Vec<Option<f32>> = {
                            let mut v = Vec::with_capacity(kids.len());
                            let (mut y, mut prev_mb, mut ok) = (0.0f32, 0.0f32, true);
                            for (i, (c, s)) in kids.iter().enumerate() {
                                let lead = if i == 0 { s.1 } else { prev_mb.max(s.1) };
                                let hh = fixed_nest.unwrap_or(0.0);
                                v.push((ok && fixed_nest.is_some() && y + lead < hh - 0.01).then_some(y + lead));
                                if edge_break(c, false)
                                    || edge_break(c, true)
                                    || kid_par.get(i).is_some_and(|p| p.group != 0)
                                    || y + lead + s.0 > hh + 0.01
                                {
                                    ok = false;
                                }
                                y += lead + s.0;
                                prev_mb = s.2;
                            }
                            v
                        };
                        let children: Vec<crate::flow::StackChild> = kids
                            .into_iter()
                            .enumerate()
                            .map(|(ix, (c, (h, mt, mb, cuts, forced, solid)))| {
                                // `box-decoration-break: clone` — фрагменты ЭТОГО
                                // ребёнка по плану. Неразрезанная коробка идёт
                                // `slice`: вид тот же, а её переполнение остаётся
                                // параллельным потоком (`clone-003`: ребёнок 185
                                // в коробке 70 продолжается во второй колонке).
                                let frag_geom: Vec<(f32, f32)> =
                                    clone_plan.get(ix).cloned().unwrap_or_default();
                                let dec = clone_dec(&c).filter(|_| frag_geom.len() > 1 && !col_vert);
                                // Элемент строки flex (`split_flex_lines`) наследует от
                                // СВОЕГО контейнера, а не от многоколоночника.
                                let merged_k = kid_parent.get(ix).cloned().flatten();
                                let merged: &Computed = merged_k.as_ref().unwrap_or(&merged);
                                let mut copy = c;
                                // Поля кладёт укладка колонок, не коробка. В
                                // вертикальном письме блочные поля — левое и
                                // правое; схлопывание сквозь верх (`strip_through_top`)
                                // там не считается вовсе (мера повёрнутая).
                                if col_vert {
                                    copy.style.margin.left = None;
                                    copy.style.margin.right = None;
                                } else {
                                    copy.style.margin.top = None;
                                    copy.style.margin.bottom = None;
                                    // И поле, схлопнутое СКВОЗЬ верх (`through` в
                                    // `mt`): стопка уже положила его `lead`-ом,
                                    // второй раз его вставил бы корень копии.
                                    strip_through_top(&mut copy, 4);
                                }
                                // Коробка из одних флоатов меряется высотой их
                                // ряда (`float_only_box`), но сама по §10.6.3
                                // высотой НОЛЬ — вне колонок это делает
                                // `collapse_margins`, а копия фрагмента
                                // строится мимо него. Без нуля в каждой
                                // колонке проступил бы фон контейнера
                                // (`floats-clear-multicol-*`: `background:
                                // red`); флоаты переполняют нулевую коробку, и
                                // маска колонки режет их по разрезам меры.
                                if !col_vert && float_only_box(&copy).is_some() && through_strut(&copy).is_some() {
                                    copy.style.height = Some(Len::Px(0.0));
                                }
                                // Параллельный поток (css-break-3 §3):
                                // содержимое, переполняющее коробку с заданной
                                // высотой, продолжается в следующей колонке
                                // САМО ПО СЕБЕ, а сосед встаёт сразу под
                                // коробкой. Протяжённость потока — та же мера
                                // без обрезки высотой; вместе с ней берём её
                                // НЕусечённые точки разреза и монолитные
                                // диапазоны (в пределах `h` они совпадают с
                                // обычными: усечение только отбрасывает записи
                                // ниже `h`).
                                // ТРОЕ ворот, все замерены:
                                //  1) `column-fill: auto` — иначе поток уходит
                                //     в балансировку и меняет высоту колонки
                                //     (`single-line-column-flex-
                                //     fragmentation-051`);
                                //  2) поддерево обычных блоков — иначе мера
                                //     `shape_full` приближённая и
                                //     протяжённость выдуманная;
                                //  3) коробка своё переполнение показывает —
                                //     у обрезающей и прокручиваемой потока нет.
                                // Четвёртые ворота — внутри меры: бюджет
                                // разжатия ОДИН на путь (`ShapeCx::unclamped`,
                                // перепривязка `cx` в `shape_full`). Без него
                                // разъезжался ЭТАЛОН четырёх пар
                                // `flex-item-content-overflow-*`.
                                let copy_m = if col_vert { transpose_tree(&copy, col_rl) } else { None };
                                let (over, cuts, forced, solid) = match with_lines(&merged, line_col_w, opts, || shape_full(
                                    copy_m.as_ref().unwrap_or(&copy),
                                    4,
                                    ShapeCx {
                                        unclamped: true,
                                        ..ShapeCx::COLUMNS
                                    },
                                ))
                                .filter(|_| {
                                    // Сетка-стопка с обычными блочными элементами
                                    // меряется так же точно, как блок (`grid_stack`:
                                    // ряд = элемент), а `overflow-x: clip` блочное
                                    // переполнение не прячет (css-overflow-3:
                                    // `clip` парой к `visible` не делает коробку
                                    // прокручиваемой). `grid-container-
                                    // fragmentation-006`: сетка 200 с содержимым
                                    // 400 в четырёх колонках.
                                    let plain = plain_block_tree(&copy, 4)
                                        || stacked_flex_tree(&copy, 4)
                                        || (grid_stack(&copy)
                                            && copy.children.iter().all(|n| match n {
                                                Node::Element(k) => k.inline || plain_block_tree(k, 3),
                                                _ => true,
                                            }));
                                    let ov = copy_m.as_ref().unwrap_or(&copy);
                                    let block_visible = matches!(
                                        ov.style.overflow_y,
                                        None | Some(crate::computed::Overflow::Visible)
                                    ) && matches!(
                                        ov.style.overflow_x,
                                        None | Some(crate::computed::Overflow::Visible)
                                            | Some(crate::computed::Overflow::Clip)
                                    );
                                    fixed.is_some() && plain && block_visible
                                })
                                .filter(|s| s.0 > h + 0.01)
                                {
                                    Some(s) => (s.0, s.3, s.4, s.5),
                                    None => (h, cuts, forced, solid),
                                };
                                // Мера с дотягом внепоточных — поток, а не
                                // коробка (`OOF_OWN`): сосед встаёт под концом
                                // коробки, абсолют продолжается в колонках.
                                let (h, over) = match OOF_OWN.with(|m| m.borrow().get(&copy.node_id).copied()) {
                                    // Только когда за коробкой в стопке есть
                                    // сосед: последней коробке её дотяг —
                                    // мера колонок многоколоночника.
                                    Some((own, full))
                                        if (full - h).abs() < 0.01
                                            && !solid_box(&copy)
                                            && ix + 1 < kid_par.len() =>
                                    {
                                        (own, over.max(full))
                                    }
                                    _ => (h, over),
                                };
                                // Относительный сдвиг — не коробке, а фрагменту
                                // (css-break-3 §5.5): его кладёт `ColumnStack`
                                // вместе со срезом.
                                let rel = hoist_relative(&mut copy);
                                // Срез строки хоста (`kamin-host-trim-*`) — рисует
                                // `blocks()` копии по её флагам.
                                if copy.attr("kamin-host-trim-start").is_some() {
                                    copy.style.text_box_trim_start = true;
                                }
                                if copy.attr("kamin-host-trim-end").is_some() {
                                    copy.style.text_box_trim_end = true;
                                }
                                // Вложенный многоколоночник с ВЕРХА внешней колонки
                                // (первый ребёнок без поля) и заданной высотой —
                                // рядами во внешний фрагментаинер (`flow::OUTER_ROW`).
                                // С верха колонки граница k-го ряда — ровно k·H, и
                                // внешняя стопка режет коробку краем колонки
                                // (`fill_at`, не монолит) точно по рядам; мера
                                // коробки — её заданная высота. Сдвинутый вниз
                                // (первый ряд = остаток колонки) — следующий шаг.
                                let nest_row = fixed_nest.filter(|hh| {
                                    *hh > 0.0
                                        && (rows.is_none() || balanced_frag.is_some())
                                        && !col_vert
                                        && kid_par.get(ix).is_none_or(|p| p.group == 0)
                                        && nested_rows_box(&copy)
                                        && match nest_at.get(ix).copied().flatten() {
                                            // С верха колонки — и заданная высота, и
                                            // `auto` с мерой рядами.
                                            Some(y0) if y0 < 0.01 => {
                                                matches!(copy.style.height, Some(Len::Px(_)))
                                                    || nested_auto.borrow().contains(&copy.node_id)
                                            }
                                            // Ниже верха — только заданная высота: мера
                                            // коробки от рядов не зависит.
                                            Some(_) => matches!(copy.style.height, Some(Len::Px(_))),
                                            None => false,
                                        }
                                });
                                let nest_phase_k = nest_at.get(ix).copied().flatten().unwrap_or(0.0);
                                let inner = inline::inherit(&merged, &copy.style);
                                // Копии на случай разреза между колонками:
                                // элемент GPUI рисуется один раз, а фрагмент
                                // нужен свой в каждой колонке. Больше, чем
                                // колонок, ребёнок занять не может.
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): строить копию
                                // через `element(&copy, &merged, opts)`, чтобы сетка
                                // и гибкий контейнер внутри стопки рисовались
                                // (`grid-container-fragmentation-*`): срез
                                // фрагментации 445 -> 405 (+12/−52) — рамки, тени,
                                // `break-between-avoid-*`, `fieldset` ушли в
                                // красное: общий путь элемента кладёт слои и
                                // выносит абсолюты иначе, чем ждёт стопка.
                                // Возвращать узкой веткой только для сетки.
                                /// css-position-3 §abspos-breaking: «User
                                /// agents must not paginate the content of
                                /// fixed-positioned boxes». Копия фрагмента —
                                /// ПОЛНЫЙ клон поддерева, и `position: fixed`
                                /// внутри неё уезжает в слой ICB из КАЖДОЙ
                                /// копии (`render.rs:1790` -> `:1823` ->
                                /// `icb_push`). Слой лежит вне коробки
                                /// многоколоночника, маска колонки
                                /// (`flow.rs:998`) его не режет — на экране
                                /// вышло бы столько зелёных коробок, сколько
                                /// колонок. Оставляем фиксированного потомка
                                /// только в ПЕРВОЙ копии: там же, где стоит
                                /// его щуп статической позиции.
                                /// Blink делает это тем же разделением —
                                /// `out_of_flow_layout_part.cc:1607`: «This
                                /// does not include repeated fixed-positioned
                                /// elements».
                                /// Устанавливает ли коробка содержащий блок для
                                /// `position: fixed` (css-position-3 §fixed-cb:
                                /// «the nearest ancestor box that establishes a
                                /// fixed positioning containing block»;
                                /// css-transforms-1 §3: трансформ даёт
                                /// «containing block for all descendants … and
                                /// fixed-position descendants»). Список ДОСЛОВНО
                                /// тот же, что в `inline::inherit`
                                /// (`transform_ancestor`): `transform`,
                                /// `contain: layout`, `contain: paint`. Шире
                                /// брать нельзя — `blocks` решает по `under_tf`
                                /// из `inherit`, и расхождение дало бы коробку и
                                /// в копии, и в слое ICB.
                                fn fixed_cb_box(c: &crate::computed::Computed) -> bool {
                                    c.transform.is_some()
                                        || c.contain_layout == Some(true)
                                        || c.contain_paint == Some(true)
                                        || c.will_change & crate::computed::wc::CB_FIXED != 0
                                }
                                /// css-position-3 §abspos-breaking: «User
                                /// agents must not paginate the content of
                                /// fixed-positioned boxes». Копия фрагмента —
                                /// ПОЛНЫЙ клон поддерева, и `position: fixed`
                                /// внутри неё уезжает в слой ICB из КАЖДОЙ
                                /// копии (`render.rs:1790` -> `:1823` ->
                                /// `icb_push`). Слой лежит вне коробки
                                /// многоколоночника, маска колонки
                                /// (`flow.rs:998`) его не режет — на экране
                                /// вышло бы столько зелёных коробок, сколько
                                /// колонок. Оставляем фиксированного потомка
                                /// только в ПЕРВОЙ копии: там же, где стоит
                                /// его щуп статической позиции.
                                /// Blink делает это тем же разделением —
                                /// `out_of_flow_layout_part.cc:1607`: «This
                                /// does not include repeated fixed-positioned
                                /// elements».
                                ///
                                /// Запрет этот — про коробки, чей содержащий
                                /// блок ОКНО. Если содержащий блок `fixed`
                                /// лежит ВНУТРИ контекста фрагментации
                                /// (трансформированный или обособленный предок
                                /// внутри копии, либо сама коробка
                                /// многоколоночника), коробка — обычный абсолют
                                /// того предка, и §abspos-breaking выше требует
                                /// обратного: «positioned relative to its
                                /// containing block ignoring any fragmentation
                                /// breaks … may subsequently be broken over
                                /// several fragmentation containers». В слой ICB
                                /// такая коробка у нас и не уходит: `blocks`
                                /// (`render.rs:3966`) считает её `abs_like` при
                                /// `under_tf` и оставляет НА МЕСТЕ, значит копия
                                /// ≥ 1 без неё теряет единственную отрисовку.
                                /// Blink делит так же:
                                /// `fixedpos_containing_block`
                                /// (`out_of_flow_layout_part.cc:1369, :2978`) —
                                /// обычный фрагментаинерный потомок, и только
                                /// оконные попадают в
                                /// `repeated_fixedpos_descendants` (:1515).
                                ///
                                /// `fixed_cb` — встретился ли по пути ВНИЗ от
                                /// коробки многоколоночника предок, который
                                /// устанавливает содержащий блок для `fixed`.
                                /// Предки ВЫШЕ многоколоночника сюда не входят:
                                /// их содержащий блок вне контекста, коробка по
                                /// спеке одна, и место ей — копия 0.
                                fn drop_viewport_fixed(n: &Node, fixed_cb: bool) -> Option<Node> {
                                    match n {
                                        Node::Element(k)
                                            if !fixed_cb
                                                && k.style.position
                                                    == Some(crate::computed::Position::Fixed) =>
                                        {
                                            None
                                        }
                                        Node::Element(k) => {
                                            let mut c = k.clone();
                                            let deeper = fixed_cb || fixed_cb_box(&k.style);
                                            c.children = k
                                                .children
                                                .iter()
                                                .filter_map(|kid| drop_viewport_fixed(kid, deeper))
                                                .collect();
                                            Some(Node::Element(c))
                                        }
                                        other => Some(other.clone()),
                                    }
                                }
                                // Линейки промежутков (css-gaps-1) у копии
                                // фрагмента: слой строится ТОЛЬКО при заданном
                                // стиле линейки (`gap_rule_spec`), как в
                                // `element()`, — ни одна старая пара сюда не
                                // попадает. Буфер проб — СВОЙ на копию: пробы
                                // всех копий одного узла иначе сливаются в один
                                // буфер, первая копия забирает всё (`take`) и
                                // строит дорожки по смеси поднятых на `from`
                                // копий. Отрезки красятся в координатах полной
                                // раскладки копии, маска колонки (`flow.rs`,
                                // `with_content_mask`) режет их вместе с
                                // содержимым — вид `slice` css-break-3 §4.
                                let copy_ix = std::cell::Cell::new(0usize);
                                let whole = nest_row.is_none()
                                    && nested_whole.borrow().contains(&copy.node_id);
                                let build = |first: bool, part: usize| {
                                    // `box-decoration-break: clone`: копия — САМ
                                    // фрагмент (`clone_fragment`), и корень, и
                                    // наследуемый стиль берутся у НЕГО. ★ Откат
                                    // 07.09 (v153) держался ровно здесь: корень
                                    // строился со стилем ИСХОДНОЙ коробки, и
                                    // каждая копия выходила полной коробкой
                                    // (`clone-007` 2.08 = ровно квадрат 100×100,
                                    // `-026` 4.98 = 240×25×4 вылета). НЕпоследний
                                    // фрагмент режется по съеденному содержимому,
                                    // последний — по концу видимого переполнения
                                    // (`over`, `clone-002`). Лишние копии (фрагментов
                                    // меньше, чем копий) не ставятся и остаются
                                    // исходной коробкой.
                                    let frag = match (dec, frag_geom.get(part)) {
                                        (Some((dt, db)), Some(&(from, fh))) => {
                                            let clip = frag_geom.get(part + 1).map_or(
                                                (over - dt - db - from).max(fh - dt - db),
                                                |n| n.0 - from,
                                            );
                                            Some(clone_fragment(&copy, dt, db, from, fh, clip))
                                        }
                                        _ => None,
                                    };
                                    let frag_inner =
                                        frag.as_ref().map(|f| inline::inherit(&merged, &f.style));
                                    let src: &Element = frag.as_ref().unwrap_or(&copy);
                                    let src_inner: &Computed = frag_inner.as_ref().unwrap_or(&inner);
                                    let kids: Vec<Node> = if first {
                                        src.children.clone()
                                    } else {
                                        // Семя — только коробка многоколоночника
                                        // и корень копии. `e` считается наравне
                                        // с предками внутри копии: содержащий
                                        // блок `fixed` на самой коробке контекста
                                        // фрагментации — это Blink
                                        // `fixedpos_containing_block`
                                        // (`out_of_flow_layout_part.cc:1369`),
                                        // фрагментаинерный потомок, а не
                                        // повторяемая коробка. Предки ВЫШЕ `e`
                                        // не в счёт: их содержащий блок вне
                                        // контекста. `hoist_relative` выше
                                        // снимает лишь ВСТАВКИ,
                                        // `transform`/`contain` остаются на
                                        // месте — проверка по `copy.style`
                                        // законна.
                                        let fixed_cb_root =
                                            fixed_cb_box(&e.style) || fixed_cb_box(&copy.style);
                                        src.children
                                            .iter()
                                            .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
                                            .collect()
                                    };
                                    let frag_gap_rules = gap_rule_spec(&copy, &inner, opts);
                                    let frag_gap_key = frag_gap_rules.as_ref().map(|_| {
                                        let ix = copy_ix.get();
                                        copy_ix.set(ix + 1);
                                        (copy.node_id ^ opts.doc_salt)
                                            ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                    });
                                    let frag_gap_guard =
                                        frag_gap_key.map(crate::interact::GapGuard::enter);
                                    // css-break-3 §5.5: «Fragmentation … occurs
                                    // before relative positioning, transforms,
                                    // and any other graphical effects. Such
                                    // effects are applied per fragment». Разрезы
                                    // трансформ не двигают (`shape_full` его и не
                                    // читает), но САМ трансформ обязан быть на
                                    // каждом фрагменте. Общий путь вешает его
                                    // через `transformed()` (render.rs:1719,
                                    // :5938, :8185); узкая ветка копии шла мимо
                                    // всех трёх, и `transform` у ребёнка
                                    // многоколоночника пропадал целиком
                                    // (`transform-000…005`: `translateX(60px)`
                                    // контейнера гасил `left:-60px` потомков, а
                                    // без него содержимое уезжало из колонки).
                                    // Начало отсчёта пока общее на всю коробку,
                                    // а не своё на фрагмент, — для `translate`
                                    // это точно, для `rotate`/`scale` нет.
                                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строить эту
                                    // копию через общий `element()` вместо узкой
                                    // ветки `styled_div_with`. Срез 3029 пар:
                                    // 1900 -> 1902 (+14/-12), и семь потерь —
                                    // грубые (99.00, страница разъезжается):
                                    // `multi-line-column-flex-fragmentation-035`,
                                    // `multi-line-row-flex-fragmentation-039/040/
                                    // 059`, `multicol-nested-013/021`,
                                    // `multicol-fill-balance-nested-000`. Тот же
                                    // путь, на котором прежде мерился откат -52.
                                    // Копия ТАБЛИЦЫ — своим рисователем.
                                    // `styled_div_with` + `blocks` кладут детей
                                    // таблицы обычными блоками, и `element()`
                                    // заворачивает КАЖДЫЙ ряд в СВОЮ анонимную
                                    // таблицу (ветка `TableRowGroup | TableRow |
                                    // TableCell`, render.rs:11622): дорожки
                                    // считаются по одному ряду, ячейка сжимается
                                    // по содержимому, а фон ряда и ячейки
                                    // теряется вовсе. Фрагментация идёт ДО
                                    // графических эффектов и применяется к
                                    // каждому фрагменту (css-break-3 §5.5), но
                                    // РАСКЛАДКА фрагмента — та же табличная
                                    // (css-tables-3 §fragmentation).
                                    // ЗАМЕРЕНО пробами (`target/probe-bt/`,
                                    // стенд v150): фон САМОЙ таблицы рисуется
                                    // (`p4-2col-bgtbl` 0.00) и блок с шириной в
                                    // точках рисуется (`p5-b-w100-div50` 0.00), а
                                    // фон ячейки (`p6-td-bg`), фон ряда
                                    // (`p6-tr-bg`), `width: auto`
                                    // (`p5-c-w100-divauto`) и `width: 100%`
                                    // (`p6-div-w100pct`) не рисуются НИЧЕМ —
                                    // 15625 красных точек из 15625.
                                    // `transformed` — как в общей ветке ниже:
                                    // `table()` его не вешает (в `element()`
                                    // таблица идёт мимо него), а копия обязана
                                    // нести трансформ на каждом фрагменте.
                                    if let Some(hh) = nest_row {
                                        let mut mc = copy.clone();
                                        mc.children = kids;
                                        // Своя метка узла на каждую копию: буфер линеек
                                        // промежутков (`gap_items_for` по `node_id`) у
                                        // копий одного узла сливался в один, и первая
                                        // копия забирала линейки всех рядов — во
                                        // втором и третьем ряду их не было
                                        // (`multicol-breaking-002`, 0.65).
                                        mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                                        // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
                                        // копия кладётся корнем, и её многоколоночнику
                                        // нужна ширина в точках для меры строк.
                                        if matches!(mc.style.width, None | Some(Len::Auto))
                                            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
                                        {
                                            mc.style.width = Some(Len::Px(w));
                                        }
                                        // `height: auto` — высота из меры рядами
                                        // (`nested_rows_shape`): стопка с рядами
                                        // отдаёт полный последний ряд, а коробка
                                        // кончается на сбалансированном хвосте.
                                        if matches!(mc.style.height, None | Some(Len::Auto)) {
                                            let b = mc.style.borders();
                                            let px = |l: &Option<Len>| match l {
                                                Some(Len::Px(v)) => *v,
                                                _ => 0.0,
                                            };
                                            let bot = px(&mc.style.padding.bottom) + px(&b.bottom);
                                            mc.style.height = Some(Len::Px((h - bot).max(0.0)));
                                            mc.style.border_box = None;
                                        }
                                        drop(frag_gap_guard);
                                        crate::flow::set_outer_row(Some((hh, nest_phase_k)));
                                        let el = element(&mc, &merged, opts);
                                        crate::flow::set_outer_row(None);
                                        return el;
                                    }
                                    if whole {
                                        let mut mc = copy.clone();
                                        mc.children = kids;
                                        if matches!(mc.style.width, None | Some(Len::Auto))
                                            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
                                        {
                                            mc.style.width = Some(Len::Px(w));
                                        }
                                        drop(frag_gap_guard);
                                        return element(&mc, &merged, opts);
                                    }
                                    if table_box(&copy) {
                                        let mut tc = copy.clone();
                                        tc.children = kids;
                                        drop(frag_gap_guard);
                                        return transformed(
                                            table(&tc, &inner, opts),
                                            &inner,
                                            &merged,
                                        );
                                    }
                                    // Абсолютный потомок ищет ближайшего
                                    // позиционированного предка (CSS 2.1
                                    // §10.1), а раскладка под нами знает только
                                    // непосредственного родителя: коробка, чей
                                    // родитель содержащим блоком НЕ является,
                                    // уезжает в слой (`cb_push`,
                                    // render.rs:4230 `to_cb`). Общий путь
                                    // `element()` слой заводит
                                    // (render.rs:13511-13529), а узкая ветка
                                    // копии фрагмента возвращается из
                                    // `element()` раньше
                                    // (`return d.into_any_element()`,
                                    // render.rs:13274) — и до сих пор такая
                                    // коробка либо всплывала в ЧУЖОЙ внешний
                                    // слой (позиционированный предок ВЫШЕ
                                    // многоколоночника), либо, слоя нет,
                                    // рисовалась на месте: от края случайного
                                    // родителя вместо содержащего блока.
                                    //
                                    // css-position-3 §abspos-breaking: «In a
                                    // fragmented flow, an absolutely positioned
                                    // box is positioned relative to its
                                    // containing block ignoring any
                                    // fragmentation breaks (as if the flow were
                                    // continuous). The box may subsequently be
                                    // broken over several fragmentation
                                    // containers». Копия и есть этот
                                    // непрерывный поток: `flow.rs` `prepaint`
                                    // кладёт её `layout_as_root(Definite(col_w),
                                    // Definite(full_h))` во всю высоту и
                                    // поднимает на срез, а колонку вырезает
                                    // маска — коробке, попавшей в слой КОРНЯ
                                    // КОПИИ, фрагментация достаётся даром. То же
                                    // деление у Blink: кандидат, чей содержащий
                                    // блок внутри контекста, идёт
                                    // `LayoutFragmentainerDescendants`
                                    // (`out_of_flow_layout_part.cc:1498`).
                                    //
                                    // Предикат — тот же `establishes_cb`, что в
                                    // `element()`, и по стилю КОПИИ:
                                    // `hoist_relative` выше снимает только
                                    // ВСТАВКИ, сам `position: relative` (как и
                                    // `transform`/`contain`) на копии остаётся.
                                    // Прямые дети копии ничего не меняют: у них
                                    // `establishes_cb(inherited)` истинно, они и
                                    // раньше рисовались на месте.
                                    // Имена областей сетки — в номера линий, как в
                                    // общем `element()` (`place_named_areas`): ни
                                    // GPUI, ни taffy имён не знают, и копия
                                    // фрагмента клала элементы автоматически
                                    // (`grid-item-fragmentation-026`: оба в
                                    // области `a`, второй уезжал во 2-й ряд).
                                    let kids = match &copy.style.grid_areas {
                                        Some(areas)
                                            if matches!(
                                                copy.style.display,
                                                Some(Display::Grid) | Some(Display::InlineGrid)
                                            ) =>
                                        {
                                            place_named_areas(areas, kids)
                                        }
                                        _ => kids,
                                    };
                                    let frag_cb_layer = crate::inline::establishes_cb(&inner);
                                    if frag_cb_layer {
                                        crate::interact::cb_open_with(fixed_cb_layer_box(&inner));
                                    }
                                    let mut body = blocks(&kids, src_inner, opts);
                                    if frag_cb_layer {
                                        body.extend(crate::interact::cb_close());
                                    }
                                    drop(frag_gap_guard);
                                    let mut d = styled_div_with(src, src_inner);
                                    // Ось блочного потока ВНУТРИ копии — горизонтальная
                                    // (css-writing-modes-4 §3.1), как у вертикального
                                    // блока в `element()`: гибкий ряд, у `vertical-rl`
                                    // обратный. Гибкому и сеточному ось ставит `apply`.
                                    if col_vert && matches!(src.style.display, None | Some(Display::Block)) {
                                        d = d.flex();
                                        d = if col_rl { d.flex_row_reverse() } else { d.flex_row() };
                                    }
                                    // Голый `styled_div_with` — БЛОК taffy (`apply.rs`
                                    // `apply_layout`: блоку вызова нет, gpui `Display::Block`
                                    // → taffy Block), а блок общего пути — гибкая колонка
                                    // (`d.flex().flex_col()` при пустом `display`, та же
                                    // оболочка у спаннера выше). На колонку опирается
                                    // `blocks()`: коробке с `aspect-ratio`, auto-шириной и
                                    // высотой в точках он ставит `Align::Start` (css-sizing-4
                                    // §5.1 «calculated the same as for a replaced element
                                    // with a natural aspect ratio»; Blink `length_utils.cc:
                                    // 535-562` → `FitContent`), а блочный алгоритм taffy
                                    // `align-self` не читает и тянет её во всю ширину
                                    // родителя. Прежде вылет прятала маска шириной в
                                    // колонку; после multicol-rest P7 (css-multicol-1 §8.1:
                                    // «visibly overflows and is not clipped to the column
                                    // box») он виден (`block-aspect-ratio-052`: зелёный 345
                                    // вместо 25, четыре фрагмента — 420×100). Гейт узкий —
                                    // только копия с таким ребёнком; колонка для ЛЮБОЙ
                                    // копии блока — отдельным замером.
                                    let ratio_kid = |n: &Node| {
                                        matches!(n, Node::Element(k)
                                            if !k.inline
                                                && k.style
                                                    .aspect_ratio
                                                    .is_some_and(|r| r.is_finite() && r > 0.0)
                                                && matches!(k.style.width, None | Some(Len::Auto))
                                                && matches!(k.style.height, Some(Len::Px(_))))
                                    };
                                    if src.style.display.is_none()
                                        && src_inner.vertical != Some(true)
                                        && kids.iter().any(ratio_kid)
                                    {
                                        d = d.flex().flex_col();
                                    }
                                    // Для ЛЮБОЙ flex/grid-копии, не только с линейками:
                                    // эталоны css-gaps (`…-fragmentation-008-ref`) кладут
                                    // ту же сетку без правил, и с гейтом «только с
                                    // линейками» тест рисовал сетку, а эталон — нет
                                    // (v93: 008 3.75, 009 5.18, 010 4.50).
                                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v94): то же для
                                    // flex-копий. css-break 2874: +26/−15, и девять потерь
                                    // — 99.00 (`multi-line-row-flex-fragmentation-084…090`,
                                    // `multi-line-column-flex-fragmentation-056/057`:
                                    // страница разъезжается), ещё 065–071 на 1.5–13.
                                    // Сетка даёт +17 в css-break без потерь.
                                    // Flex-копия — во всю ширину колонки, как и
                                    // сетка: flex-корень с `width: auto` taffy
                                    // кладёт шириной СОДЕРЖИМОГО (`flexbox.rs`
                                    // `determine_container_main_size`, ветвь
                                    // `Definite` → `longest_line_length`), и
                                    // элемент `width: 100%` выходил нулевым, а
                                    // `width: 100px` в колонке 50 не сжимался.
                                    // Замер одного этого (v94): +9/−15, все
                                    // потери — `row-gap`, их закрывает мера
                                    // гибкой стопки (`flex_items` в `shape_full`).
                                    if matches!(copy.style.width, None | Some(Len::Auto))
                                        && matches!(
                                            copy.style.display,
                                            Some(Display::Grid)
                                                | Some(Display::InlineGrid)
                                                | Some(Display::Flex)
                                        )
                                    {
                                        d = d.w_full();
                                    }
                                    // Пустая сетка: taffy раскладывает бездетный
                                    // узел ЛИСТОМ (vendor/taffy/src/tree/
                                    // taffy_tree.rs: `(_, false) =>
                                    // compute_leaf_layout`), явные дорожки ему
                                    // не видны, и копия выходила нулевой при
                                    // мере 200 (`grid-container-fragmentation-
                                    // 002`: `grid-template-rows: 200px`). Дорожка
                                    // существует без элементов (css-grid-1
                                    // §7.1) — высота копии та же, что в мере.
                                    if kids.is_empty()
                                        && matches!(
                                            copy.style.display,
                                            Some(Display::Grid) | Some(Display::InlineGrid)
                                        )
                                        && grid_rows_px(&copy.style).is_some()
                                    {
                                        d = d.min_h(px(h));
                                    }
                                    if let (Some(key), Some(spec)) = (frag_gap_key, frag_gap_rules) {
                                        // Копия кладётся `layout_as_root(Definite(col_w), …)`
                                        // (`flow.rs` `ColumnStack::prepaint`), а taffy у
                                        // flex/grid-КОРНЯ с `width: auto` берёт размер
                                        // содержимого, не доступное место
                                        // (`vendor/taffy/src/compute/flexbox.rs`
                                        // `determine_container_main_size`, ветвь
                                        // `Definite` → `longest_line_length`): дорожки
                                        // `1fr` выходили нулевыми, и вся сетка была
                                        // невидима (`grid-gap-decorations-fragmentation-
                                        // 008/010/016`: только серый фон). Блок
                                        // растягивается сам; flex/grid получают 100% —
                                        // корень разрешает долю против `available_space`
                                        // (`taffy/src/compute/mod.rs` `compute_root_layout`).
                                        // Под детьми копии — как в `element()`
                                        // (css-gaps-1: «just above the border»).
                                        body.insert(
                                            0,
                                            crate::interact::GapRulePainter::new(
                                                crate::interact::gap_items_for(key),
                                                spec,
                                            )
                                            .into_any_element(),
                                        );
                                    }
                                    transformed(
                                        d.children(body).into_any_element(),
                                        &inner,
                                        &merged,
                                    )
                                };
                                // Монолиты (css-break-3 §4.1) — их разрыв
                                // запрещён, и в следующую колонку они уходят
                                // целиком: `break-inside: avoid`,
                                // прокручиваемая или обрезающая коробка,
                                // замещаемый элемент, таблица и ячейка,
                                // атомарная строчная коробка. Сюда же —
                                // сплошной СТРОЧНЫЙ набор: резать его можно
                                // только между строками, а строк укладка
                                // колонок не видит, и разрез приходился бы
                                // посреди строки.
                                // Монолитен ПРОКРУЧИВАЕМЫЙ контейнер (css-break-4
                                // §4.1 «scroll containers»); `hidden`/`clip` —
                                // обрезка, не прокрутка, и режется как блок
                                // (корень A4).
                                let scrolls = |o: Option<crate::computed::Overflow>| {
                                    matches!(o, Some(crate::computed::Overflow::Scroll))
                                };
                                let block_kid = |n: &Node| {
                                    matches!(n, Node::Element(k)
                                        if !k.inline || k.style.display == Some(Display::Block))
                                };
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): `contain: size` как
                                // монолит (Blink `IsMonolithic`) — срез фрагментации
                                // 469 -> 467 (+1/−3): `single-line-column-flex-
                                // fragmentation-051/063` режутся у Blink иначе (рост
                                // элемента от фрагментации, корень R5 скаута).
                                // Рост лёг `705fd58`; `contain: size` — `size_monolith`,
                                // тот же предикат, что у `solid_box` в мере и пробе
                                // `grow_pushed` (`scout-break-2026-09e.md`).
                                let monolith = nest_row.is_none() && (size_monolith(&copy)
                                    || copy.style.break_inside_avoid
                                    || scrolls(copy.style.overflow_x)
                                    || scrolls(copy.style.overflow_y)
                                    || matches!(
                                        copy.tag.as_str(),
                                        "img"
                                            | "svg"
                                            | "canvas"
                                            | "video"
                                            | "embed"
                                            | "object"
                                            | "iframe"
                                    )
                                    // Таблица и ячейка — не монолиты
                                    // (css-break-4 §4.1).
                                    || matches!(
                                        copy.style.display,
                                        Some(Display::InlineBlock)
                                            | Some(Display::InlineFlex)
                                            | Some(Display::InlineGrid)
                                    )
                                    // Сплошной СТРОЧНЫЙ набор тоже монолит:
                                    // резать его можно лишь между строками, а
                                    // строк укладка колонок не видит, и разрез
                                    // приходился бы посреди строки.
                                    // ПУСТАЯ коробка с высотой режется по своей
                                    // высоте (css-break-4 §4.2; корень A3).
                                    || (copy.children.iter().any(|n| !is_blank(n))
                                        && !copy.children.iter().any(block_kid)
                                        // Строки измерены (`line_run_shape`) — режется
                                        // между строк, не монолит.
                                        && cuts.is_empty()));
                                // Высоту меряет раскладка копии (`StackChild::measure`):
                                // точек разреза мера не дала, и строчный набор без них
                                // не монолит — режется краем колонки. Монолит — только
                                // по собственным причинам коробки (css-break-3 §4.1).
                                let measure = measured_kids
                                    .borrow()
                                    .iter()
                                    .find(|(id, _)| *id == copy.node_id)
                                    .map(|(_, w)| *w);
                                let monolith = if whole {
                                    true
                                } else if measure.is_some() {
                                    nest_row.is_none()
                                        && (size_monolith(&copy)
                                            || copy.style.break_inside_avoid
                                            || scrolls(copy.style.overflow_x)
                                            || scrolls(copy.style.overflow_y))
                                } else {
                                    monolith
                                };
                                // Пока строятся копии — «внутри стопки»: вложенный
                                // многоколоночник со спаннером остаётся на
                                // сегментном пути (см. `unified` выше).
                                let _nested = crate::flow::StackScope::enter();
                                let span = copy.style.column_span == Some(true) && !copy.inline;
                                // Переполняющие колонки (css-multicol-1 §8.2: «A multicol
                                // container can have more columns than it has room for due
                                // to: a declaration that constrains the column height … In
                                // this case, additional column boxes are created in the
                                // inline direction») — ТОЛЬКО ребёнку, который несёт
                                // абсолютного потомка: его содержащий блок сплошной, и
                                // абсолют режется по колонкам сам (css-position-3
                                // §abspos-breaking), а копий у ребёнка было ровно
                                // `column-count` — хвост уходил «за кадр» (`flow.rs`
                                // `fill_at`, `copy + 1 >= limit`;
                                // `out-of-flow-in-multicolumn-007`: CB 300 при колонке 100,
                                // копий 2 из 3). ★ Прежний патч без гейта «несёт абсолют»
                                // (scout-fragoof-2026-09d §7) замерен +7/−14: все потери —
                                // дети БЕЗ абсолютов (вложенные многоколоночники, флекс,
                                // `multicol-fill-balance-*`), у которых мера `shape_full`
                                // не совпадает с рисунком. Гейт `plain_block_tree` — тот же,
                                // что у параллельного потока выше. Прочим детям — прежнее
                                // число копий, и стопка без такого ребёнка байт-в-байт
                                // прежняя (`ColumnStack::new` берёт наибольшее число копий).
                                let kid_copies = match fixed {
                                    Some(per)
                                        if rows.is_none()
                                            && !span
                                            && per > 0.0
                                            && visible_overflow(&e.style)
                                            && visible_overflow(&copy.style)
                                            && plain_block_tree(&copy, 4)
                                            && carries_abspos(&copy, 4) =>
                                    {
                                        ((h.max(over) / per).ceil() as usize + 1)
                                            .min(16)
                                            .max(copies)
                                    }
                                    // Высота неизвестна до раскладки: копий — на
                                    // переполняющие колонки (css-multicol-1 §8.2).
                                    Some(per) if measure.is_some() && rows.is_none() && !span && per > 0.0 => {
                                        copies.max(8).min(16)
                                    }
                                    _ => copies,
                                };
                                crate::flow::StackChild {
                                    measure,
                                    el: side_margin_wrap(build(true, 0), &copy, col_vert),
                                    frags: if span {
                                        Vec::new()
                                    } else {
                                        (1..kid_copies)
                                            .map(|i| side_margin_wrap(build(false, i), &copy, col_vert))
                                            .collect()
                                    },
                                    monolith,
                                    cuts,
                                    // Тот же подъём, что в мере (Х5): иначе
                                    // укладка колонок не увидит разрыва,
                                    // который мера уже посчитала.
                                    force_before: edge_break(&copy, false),
                                    force_after: edge_break(&copy, true),
                                    avoid_before: edge_avoid(&copy, false),
                                    avoid_after: edge_avoid(&copy, true),
                                    forced,
                                    solid,
                                    h,
                                    mt,
                                    mb,
                                    span,
                                    over,
                                    rel,
                                    clone_dec: dec,
                                    // Монолит с верха колонки переполняет её
                                    // (`flow.rs` `fill_at`, `overflow_to`) —
                                    // только при `column-fill: auto` без рядов
                                    // и без элементов ряда в поддереве.
                                    overflow_top: fixed.is_some()
                                        && rows.is_none()
                                        && !parallel_items_inside(&copy, 4),
                                    // Вложенный многоколоночник — маска режет вбок
                                    // (`flow.rs` `StackChild::nested_cols`).
                                    nested_cols: multicol_inside(&copy, 4),
                                    par: kid_par[ix],
                                    positioned: !span
                                        && (matches!(
                                            copy.style.position,
                                            Some(crate::computed::Position::Relative)
                                                | Some(crate::computed::Position::Sticky)
                                        ) || copy.style.transform.is_some())
                                        && copy.style.z_index.unwrap_or(0) == 0,
                                    // Хвост непоследнего фрагмента таблицы — её фоном
                                    // (`flow.rs` `StackChild::slack`).
                                    slack: if col_vert {
                                        None
                                    } else if table_box(&copy) {
                                        copy.style.background.map(|c| c.to_hsla())
                                    } else if dec.is_none() && over <= h + 0.01 {
                                        // Продолжение одного лишь параллельного
                                        // потока (`over`) — не продолжение коробки:
                                        // она кончилась, хвоста у неё нет.
                                        slack_fill(&copy)
                                    } else {
                                        None
                                    },
                                    laid_w: Default::default(),
                                    // Повтор шапки/подвала таблицы — полосы своими
                                    // копиями (`flow::Repeat`); та же мера, что у
                                    // щупов (`repeat_leads`).
                                    repeat: repeat_bands(&copy, fixed, rows)
                                        .filter(|_| !span && !col_vert)
                                        .map(|(head, foot, geom)| crate::flow::Repeat {
                                            head,
                                            foot,
                                            geom,
                                            head_els: match head {
                                                Some(_) => (1..kid_copies).map(|i| build(false, i)).collect(),
                                                None => Vec::new(),
                                            },
                                            foot_els: match foot {
                                                Some(_) => (0..kid_copies).map(|i| build(false, i)).collect(),
                                                None => Vec::new(),
                                            },
                                        }),
                                }
                            })
                            .collect();
                        // Щуп статической позиции — НУЛЕВОЙ записью стопки на
                        // месте позиционированного ребёнка: высоты нет, полей
                        // нет, точек разреза нет, монолитных диапазонов нет —
                        // план укладки от него не двигается (`fill_at`:
                        // `rest = 0` всегда влезает в остаток колонки), а
                        // холст `interact::spot_probe` запоминает экранную
                        // дырку. Сама коробка в стопку НЕ идёт: она рисуется
                        // после стопки заместителем `spot_place` и сдвигается
                        // в эту дырку. Тем это отличается от прежней пробы
                        // «нулевая запись в стопке», о которой говорит
                        // комментарий у `direct_oof`: там в колонку уходил САМ
                        // элемент, и его резала маска
                        // (`out-of-flow-in-multicolumn-094…097`).
                        //
                        // Blink берёт статическую позицию оттуда же —
                        // `out_of_flow_layout_part.cc`,
                        // `LayoutFragmentainerDescendants`: позиция кандидата
                        // считается относительно ФРАГМЕНТАИНЕРА.
                        let mut children = children;
                        let oof_spots: Vec<crate::interact::SpotCell> =
                            oof_static.iter().map(|_| Default::default()).collect();
                        for (i, (at, oof)) in oof_static.iter().enumerate().rev() {
                            // Заданную ось считает раскладка от содержащего
                            // блока, щуп правит только ПУСТУЮ (CSS 2.1
                            // §10.3.7) — тот же гейт `fixed_axes`, что у слоёв
                            // в `blocks()`.
                            oof_spots[i].set(crate::interact::Spot {
                                fixed_axes: (
                                    edge_set(oof.style.inset.left)
                                        || edge_set(oof.style.inset.right),
                                    edge_set(oof.style.inset.top)
                                        || edge_set(oof.style.inset.bottom),
                                ),
                                rtl: merged.rtl == Some(true),
                                vertical: merged.vertical == Some(true),
                                vertical_rl: merged.vertical_rl == Some(true),
                                own_vertical: oof.style.vertical == Some(true),
                                ..Default::default()
                            });
                            let probe = crate::flow::StackChild {
                                measure: None,
                                el: crate::interact::spot_probe(oof_spots[i].clone(), true),
                                frags: Vec::new(),
                                monolith: false,
                                cuts: Vec::new(),
                                force_before: false,
                                force_after: false,
                                avoid_before: false,
                                avoid_after: false,
                                forced: Vec::new(),
                                solid: Vec::new(),
                                h: 0.0,
                                mt: 0.0,
                                mb: 0.0,
                                span: false,
                                over: 0.0,
                                rel: (0.0, 0.0),
                                clone_dec: None,
                                overflow_top: false,
                                nested_cols: false,
                                repeat: None,
                                par: crate::flow::Par::default(),
                                slack: None,
                                laid_w: Default::default(),
                                positioned: false,
                            };
                            // Номер — среди ДЕТЕЙ ДО раскрытия строк flex (`split_flex_lines`).
                            let at = kid_starts.get(*at).copied().unwrap_or(children.len()).min(children.len());
                            children.insert(at, probe);
                        }
                        // Стопка тянется по СТРОЧНОЙ оси: в вертикальном письме
                        // это высота, значит коробка кладёт её гибким рядом
                        // (поперечная ось растягивает высоту), у `vertical-rl` —
                        // от ПРАВОГО края (`flex_row_reverse`), там начало
                        // блочной оси.
                        let d = if col_vert {
                            let d = d.flex();
                            if col_rl { d.flex_row_reverse() } else { d.flex_row() }
                        } else {
                            d
                        };
                        let mut d = d.child(
                            crate::flow::ColumnStack::new(
                                children,
                                cols as usize,
                                used_gap,
                                fixed,
                                rule,
                                rows,
                                gap_items.clone(),
                                intrinsic_inline_size(&e.style, inherited).then(|| {
                                    crate::flow::Intrinsic(match column_width {
                                        Some(Len::Px(w)) if w > 0.0 => Some(w),
                                        _ => None,
                                    })
                                }),
                            )
                            .with_axis(col_axis)
                            .with_row_phase(if nest_rows.is_some() { nest_phase } else { 0.0 })
                            // Линейки последней линии — до низа содержимого коробки
                            // заданной высоты (Blink `PaintColumnRules`), без
                            // спаннеров и рядов (`multicol-rule-nested-balancing-001`).
                            .with_rule_stretch(
                                match merged.height {
                                    Some(Len::Px(h))
                                        if h > 0.0
                                            && !col_vert
                                            && nest_rows.is_none()
                                            && e.style.border_box != Some(true)
                                            && !e.children.iter().any(|n| matches!(n, Node::Element(c) if spanner_box(c))) =>
                                    {
                                        Some(h)
                                    }
                                    _ => None,
                                },
                            ),
                        );
                        // Флоаты — прежним ходом, соседями стопки.
                        for oof in &direct_oof {
                            d = d.child(element(oof, &merged, opts));
                        }
                        // Заместитель на месте щупа: рисуется ПОСЛЕ стопки и
                        // после флоатов (позиционированная коробка выше и
                        // поточного содержимого, и плавающих — CSS 2.1 §9.9,
                        // шаг 8 против шагов 4 и 5; на этом держится
                        // `abspos-after-spanner`, где под зеленью поточная
                        // красная коробка), а встаёт туда, где щуп стоял в
                        // колонке.
                        // Процентная высота абсолюта — от высоты отбивки
                        // содержащего блока (CSS 2.1 §10.5, §10.1 п. 4), а здесь им
                        // служит САМ многоколоночник. Заместитель же кладёт коробку
                        // в нулевую обёртку (`spot_place`), и раскладка под нами
                        // считала проценты от неё — коробка схлопывалась в ноль
                        // (`single-line-row-flex-fragmentation-019/020`: `height:
                        // 50%` без `top`). Пересчитываем в точки заранее, когда
                        // высота многоколоночника известна в точках.
                        let cb_h: Option<f32> = (e.style.position.is_some()
                            && e.style.position != Some(crate::computed::Position::Static)
                            && !col_vert)
                            .then(|| {
                                let px = |l: Option<Len>| match l {
                                    None => Some(0.0),
                                    Some(Len::Px(v)) => Some(v),
                                    _ => None,
                                };
                                let b = e.style.borders();
                                let pad = px(e.style.padding.top)? + px(e.style.padding.bottom)?;
                                let bor = px(b.top)? + px(b.bottom)?;
                                match e.style.height {
                                    Some(Len::Px(h)) if e.style.border_box == Some(true) => Some((h - bor).max(pad)),
                                    Some(Len::Px(h)) => Some(h.max(0.0) + pad),
                                    _ => None,
                                }
                            })
                            .flatten();
                        for (i, (_, oof)) in oof_static.iter().enumerate() {
                            let mut oof = oof.clone();
                            if let Some(ch) = cb_h
                                && oof.style.position == Some(crate::computed::Position::Absolute)
                            {
                                for l in [&mut oof.style.height, &mut oof.style.min_height, &mut oof.style.max_height] {
                                    if let Some(Len::Pct(k)) = *l {
                                        *l = Some(Len::Px(k * ch));
                                    }
                                }
                            }
                            d = d.child(crate::interact::spot_place(
                                oof_spots[i].clone(),
                                element(&oof, &merged, opts),
                            ));
                        }
                        if let (Some(buf), Some(spec)) = (gap_items, gap_spec) {
                            d = d.child(crate::interact::GapRulePainter::new(buf, spec).into_any_element());
                        }
                        return d.into_any_element();
                    }
                    let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
                    let rows = count.div_ceil(cols as usize).max(1) as u16;
                    let gap = used_gap;
                    d = d
                        .grid()
                        .grid_template_cols(
                            (0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect(),
                        )
                        .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
                        .gap_x(px(gap));
                    d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
                }
            } else if let Some(Len::Px(w)) = column_width {
                // Ширина колонки без их числа — это «сколько влезет»: ровно
                // то, что умеет короткая форма дорожек в GPUI.
                d = d.grid().grid_cols_min(px(w));
            // Ось блочного потока не зависит от `display` (css-writing-modes-4
            // §3.1): inline-block, ячейка, list-item с вертикальным письмом
            // раскладывают детей той же горизонтальной осью, что и голый блок
            // (`block-flow-direction-*`, `line-box-direction-*`).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: явный `Some(Block)` — он же стоит у
            // блокифицированных (абсолют в сетке), и
            // `grid-positioned-children-writing-modes-001` 0.39 -> 1.32 даже с
            // гейтом «родитель не сетка».
            // `list-item` сюда НЕ пускать (★ ЗАМЕРЕНО: `li` с одним текстом
            // становился рядом — `line-box-direction-vrl-019/vlr-020`
            // 6.58 -> 12.55).
            } else if merged.vertical == Some(true)
                // `display: table-caption` — это `Display::Block` С МЕТКОЙ
                // (`computed.rs:3125`), и голый `Some(Block)` сюда пускать
                // нельзя (замеренный откат выше). Метку же ставит ТОЛЬКО
                // само объявление `display: table-caption`, тег `<caption>`
                // её не несёт, а подпись ВНУТРИ стола сюда не приходит вовсе
                // — её строит `table()`. Письмо применяется к подписи
                // (css-writing-modes-4 §3.1, «Applies to: all elements
                // except table row groups, column groups, rows, columns»),
                // значит ось её блочного потока задаёт письмо, а не `display`.
                // Blink: `table_layout_algorithm.cc:67`
                // `ConstraintSpaceBuilder(space, caption.Style().GetWritingDirection(), true)`.
                && (matches!(
                    e.style.display,
                    None | Some(Display::InlineBlock) | Some(Display::TableCell)
                ) || e.style.is_caption == Some(true)
                    // Объявленный `display: flow-root` — тот же блок со своим
                    // контекстом (`computed.rs` ставит ему `Some(Block)` с
                    // меткой `flow_root`); блокифицированный абсолют метки не
                    // несёт, откат выше его не касается. Без этого
                    // `flow-root` в `vertical-rl` раскладывал детей
                    // горизонтальным блоком: хост полос мерился по
                    // min-content, флоаты-колонки эталона
                    // `css-break/background-image-001` вставали поперёк строки
                    // и пропадали.
                    || (e.style.flow_root == Some(true)
                        && e.style.display == Some(Display::Block)))
            {
                // Вертикальное письмо: ось блочного потока — горизонтальная.
                // Дети идут слева направо (`vertical-lr`) или справа налево
                // (`vertical-rl`).
                d = d.flex();
                d = if merged.vertical_rl == Some(true) {
                    d.flex_row_reverse()
                } else {
                    d.flex_row()
                };
                // `sideways-lr`: строка идёт снизу вверх — начало строчной
                // оси у НИЖНЕГО края (css-writing-modes-4 §block-flow).
                // Прижим коробки к низу — ЗАПЛАТКА того времени, когда абзац
                // вертелся по часовой и его содержимое росло от верха.
                // С поворотом против часовой (`VerticalText::ccw`) строка сама
                // начинается у нижнего края, и второй прижим снова уводит
                // рисунок. Снимать ВМЕСТЕ с патчем поворота и мерить
                // `wm-propagation-body-047`, `abs-pos-border-offset-002` —
                // ровно те две пары, ради которых заплатка ставилась.
                if e.style.width.is_none() {
                    d = d.flex_shrink_0();
                }
            } else if e.style.display.is_none() {
                // Блок без явного `display` — блочная раскладка taffy, а не
                // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
                // выше родителя ужимался, тогда как браузер даёт ему вылезти.
                // Схлопывание вертикальных отступов при этом делает сама
                // раскладка — включая протекание через пустой блок.
                d = d.flex().flex_col();
                // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
                // детям с заданной шириной (в blocks): items_end на контейнере
                // снимал stretch у всех, и абзац в rtl ужимался до текста —
                // text-align внутри пустел (text-align-end-001: bw=124.8
                // вместо 300). ЗАМЕРЕНО: +18 css-text при −2..3 wm и −4 mix
                // (spot-механика abs-pos-border-offset полагалась на
                // items_end — новый след) — нетто +10.
            }
            // Ряд по умолчанию — но не тогда, когда письмо справа налево:
            // там ряд обязан идти в обратную сторону, и общая ветка его
            // разворот отменяла. Письмо — СЛИТОЕ: `direction` наследуется
            // (css-writing-modes-4 §2.1), и ряд под `body { direction: rtl }`
            // без своего `direction` шёл слева направо — поля
            // `margin-inline-start` эталонов вставали не между элементами
            // (`gap-001-rtl-ref`, `gap-003-rtl-ref`).
            if e.style.display == Some(Display::Flex)
                && e.style.flex_dir.is_none()
                && merged.rtl != Some(true)
            {
                // При вертикальном письме умолчание `row` — это ось строки, а
                // она идёт сверху вниз.
                d = if merged.vertical == Some(true) {
                    d.flex_col()
                } else {
                    d.flex_row()
                };
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: отдавать субсетке обычной сетки
            // РАЗРЕШЁННЫЙ кусок родительских дорожек — тем же кодом, что и
            // проход лунок (`if item.style.subgrid` в `lanes`), с вычетом
            // своих краёв и правкой зазора. Кусок брался из ЯВНЫХ линий
            // ребёнка (в обычной сетке размещение делает вендор, и другого
            // источника `at`/`span` до раскладки нет). Срез css-grid (1133
            // пары, 646 зелёных): 645, приобретено НОЛЬ, и
            // `row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`
            // ушла 0.00 → HUNG (вложенная субсетка зацикливает раскладку).
            // Возвращать только вместе с размещением субсетки НА НАШЕЙ
            // стороне: пока `at`/`span` берутся из линий, вложенный случай
            // получает кусок от куска и сходится не всегда.
            // Имена областей разворачиваются здесь: контейнер и его дети
            // видны одновременно только на этом уровне.
            let children = match &e.style.grid_areas {
                Some(areas) => place_named_areas(areas, e.children.clone()),
                None => e.children.clone(),
            };
            // Resolve physical margins before preparing the vertical formatting context.
            let children = if merged.vertical == Some(true) {
                // Поле самого контейнера по ведущей стороне оси потока —
                // для схлопывания с первым ребёнком (§8.3.1). Ведущая
                // сторона: левая у `vertical-lr`/`sideways-*`, правая у
                // `vertical-rl`. Открыта, если там нет ни рамки, ни
                // внутреннего отступа; у корня поля не схлопываются вовсе.
                let reverse = merged.vertical_rl == Some(true);
                let lead_margin = if e.tag == "html" {
                    None
                } else {
                    let b = e.style.borders();
                    let (border, pad, own) = if reverse {
                        (b.right, e.style.padding.right, e.style.margin.right)
                    } else {
                        (b.left, e.style.padding.left, e.style.margin.left)
                    };
                    // Независимый контекст форматирования (overflow не
                    // `visible`, флоат, `display: flow-root`, `contain`) не
                    // схлопывает своё поле с детьми (CSS2 §8.3.1, css-writing-
                    // modes-4 §7.4): `margin-collapse-vlr-017`/`vrl-016`
                    // (`overflow: hidden`, v100: 0.00 → «красное видно»).
                    let bfc = !matches!(
                        e.style.overflow_x,
                        None | Some(crate::computed::Overflow::Visible)
                    ) || !matches!(
                        e.style.overflow_y,
                        None | Some(crate::computed::Overflow::Visible)
                    ) || e.style.float.is_some()
                        || e.style.display.is_some()
                        || e.style.flow_root == Some(true)
                        // css-align-3 §align-block — тот же список, что и в
                        // `own_context`: своё поле такая коробка с полем
                        // первого ребёнка не схлопывает.
                        || e.style.align_content_block
                        || e.style.contain_layout == Some(true)
                        || e.style.contain_paint == Some(true);
                    let sealed = bfc
                        || margin_px(border, &e.style).unwrap_or(0.0) > 0.0
                        || margin_px(pad, &e.style).unwrap_or(0.0) > 0.0;
                    if sealed {
                        None
                    } else {
                        Some(margin_px(own, &e.style).unwrap_or(0.0))
                    }
                };
                // Доли полей/отступов — в точки от высоты контейнера ДО
                // схлопывания (см. `resolve_inline_pct`).
                vertical_hug::children(
                    orthogonal_children(
                        vertical_flow_margins::children(
                            resolve_inline_pct(children, &merged, true),
                            &merged,
                            reverse,
                            lead_margin,
                        ),
                        &merged,
                        opts.viewport.0,
                    ),
                    &e.style,
                    &merged,
                )
            } else {
                orthogonal_vertical_children(resolve_inline_pct(children, &merged, false), &merged)
            };
            let mut kids: Vec<AnyElement> = Vec::new();
            kids.extend(clip_layer(&merged, opts));
            // Бюджет строк обрезки: сторожа контекста живут, пока строится
            // поддерево — пробы детей пишут строки в буфер контейнера.
            // Многоколонник клэмпом не режется (`continue: collapse` там как
            // `auto`, §5.2; `styled_div_with` срез не ставит): без этого гейта
            // бюджет абзаца счётного режима поставил бы «…» (`line-clamp-039`).
            let is_clamp = (e.style.clamp_lines().is_some()
                || (e.style.clamp_auto == Some(true) && auto_clamp_limit(&merged).is_some()))
                && !multicol_container(&e.style);
            let _clamp_guard = is_clamp.then(|| crate::interact::ClampGuard::enter(e.node_id));
            let makes_bfc = matches!(
                merged.overflow_x,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || matches!(
                merged.overflow_y,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || merged.float.is_some()
                || merged.flow_root == Some(true)
                // Независимый контекст форматирования и без overflow/float/
                // flow-root: гибкий контейнер, сетка, таблица — своя
                // раскладка по определению, и строки внутри в бюджет
                // `line-clamp` не входят (css-overflow-4: «skip lines in
                // independent formatting contexts»; `webkit-line-clamp-012/013`).
                || matches!(
                    merged.display,
                    Some(Display::Flex)
                        | Some(Display::InlineFlex)
                        | Some(Display::Grid)
                        | Some(Display::InlineGrid)
                        | Some(Display::GridLanes)
                        | Some(Display::Table)
                        | Some(Display::InlineTable)
                        | Some(Display::TableCell)
                )
                // `<fieldset>` — тоже отдельная раскладка
                // (`webkit-line-clamp-027`).
                || e.tag == "fieldset";
            let _bfc_guard = (!is_clamp && makes_bfc && crate::interact::clamp_context().is_some())
                .then(crate::interact::ClampGuard::enter_bfc);
            // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
            // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
            // ранний `return` не съел запись. Абсолютные дети дорожек не
            // занимают (css-grid-1 §9), а пустой анонимный блок — это
            // распорка лент (`spacer()`), не элемент.
            if let Some(key) = crate::interact::gap_context()
                && !matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                )
                && !(e.node_id == 0 && e.children.is_empty())
            {
                // Проба ложится на паддинг-бокс; линейкам нужен рамочный.
                let fs = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let bw = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    Some(Len::Em(k)) => k * fs,
                    _ => 0.0,
                };
                let b = e.style.borders();
                kids.push(crate::interact::gap_item_probe(
                    crate::interact::gap_items_for(key),
                    [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)],
                ));
            }
            if let Some((key, skip)) = crate::interact::clamp_context() {
                // Строки дают пробы абзацев (paragraph_probed); здесь — только
                // коробка с краской: блок прячется целиком, если срез внутри.
                // Поточная коробка со СВОИМ контекстом форматирования точек
                // среза внутри не имеет (css-overflow-4 §5.3: строки
                // независимых контекстов не считаются, точка — только между
                // блоками): пересечённая потолком, она уходит целиком, как
                // коробка с заданной высотой (`line-clamp-auto-033`:
                // `flow-root` под «Line 4»). Флоат и абсолют — не поточные,
                // строчный атом — внутри строки.
                let monolithic = makes_bfc
                    && !merged.float.is_some_and(|f| f != 0)
                    && !matches!(
                        merged.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    )
                    && !matches!(
                        merged.display,
                        Some(Display::InlineBlock)
                            | Some(Display::InlineFlex)
                            | Some(Display::InlineGrid)
                            | Some(Display::InlineTable)
                    );
                if !is_clamp && (has_box_style_probe(&e.style) || monolithic) {
                    // Нижние рамка и паддинг фрагментированной коробки
                    // остаются в потоке (css-overflow-4 §5.3): проба несёт
                    // их вместе с границами паддинг-бокса.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bp_after = side(e.style.borders().bottom) + side(e.style.padding.bottom);
                    kids.push(crate::interact::clamp_probe(
                        crate::interact::clamp_lines_for(key),
                        0.0,
                        skip,
                        e.style.height.is_some() || e.style.min_height.is_some() || monolithic,
                        bp_after,
                        // Коробка — не абзац: знак обрыва на неё не садится
                        // (он всегда в конце строки, css-overflow-4 §5.3).
                        None,
                        None,
                    ));
                } else if !is_clamp
                    && !skip
                    && e.children.iter().all(is_blank)
                    && !merged.float.is_some_and(|f| f != 0)
                    && !matches!(
                        merged.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    )
                    && matches!(merged.display, None | Some(Display::Block))
                {
                    kids.push(crate::interact::clamp_empty_probe(
                        crate::interact::clamp_lines_for(key),
                    ));
                }
            }
            // Абсолютный потомок ищет ближайшего позиционированного предка
            // (§10.1), а раскладка под нами знает только непосредственного
            // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
            // содержащим блоком не является, переезжает сюда.
            let cb_layer = crate::inline::establishes_cb(&merged);
            if cb_layer {
                crate::interact::cb_open_with(fixed_cb_layer_box(&merged));
            }
            // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
            // задан стиль хотя бы одной линейки: начальное `none` означает,
            // что рисовать нечего, и ни одна старая пара сюда не попадает
            // (см. `gap_rule_spec`).
            let gap_rules = gap_rule_spec(e, &merged, opts);
            let gap_buf = gap_rules
                .is_some()
                .then(|| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt));
            let _gap_guard = gap_buf
                .as_ref()
                .map(|_| crate::interact::GapGuard::enter(e.node_id ^ opts.doc_salt));
            // css-gaps-1 §gap-decorations: «Gap decorations are painted just
            // above the border of the container» — ПОД детьми. Слой идёт до
            // них: буфер проб он всё равно читает в `paint`, а prepaint всего
            // дерева у gpui проходит раньше (эталоны `grid-gap-decorations-042`
            // и `flex-033` кладут линейки `z-index: -1`, `008/023` — элементы
            // `z-index: 2`).
            if let (Some(buf), Some(spec)) = (gap_buf, gap_rules) {
                kids.push(crate::interact::GapRulePainter::new(buf, spec).into_any_element());
            }
            kids.extend(blocks(&children, &merged, opts));
            if cb_layer {
                kids.extend(crate::interact::cb_close());
            }
            if is_clamp {
                // Бюджет среза — высота ПОЛЯ СОДЕРЖИМОГО: при `box-sizing:
                // border-box` из `max-height` вычитаются вертикальные рамки и
                // паддинги (`line-clamp-auto-003`: 138 − 2×(1+4) = 128).
                let bb_y = if merged.border_box == Some(true) {
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bw = merged.borders();
                    side(bw.top)
                        + side(bw.bottom)
                        + side(merged.padding.top)
                        + side(merged.padding.bottom)
                } else {
                    0.0
                };
                // Потолок высоты участвует в выборе точки среза только в
                // авто-режиме (`line-clamp: auto` / `4 auto`); счётный
                // `line-clamp: 4` и `-webkit-line-clamp` режут ТОЛЬКО по числу
                // строк, а не влезшее в `max-height` переполняет коробку
                // (`line-clamp-035`, `webkit-line-clamp-with-max-height`).
                let max_h = match auto_clamp_limit(&merged) {
                    Some(v) if e.style.clamp_auto == Some(true) => Some((v - bb_y).max(0.0)),
                    _ => None,
                };
                // `text-box-trim: trim-end` клампа: последняя строка перед
                // обрывом срезается (css-inline-3 §4.2; Blink триммит строку у
                // точки клампа). Метрика — по стилю контейнера.
                let clamp_trim = if merged.text_box_trim_end {
                    text_box_trim_px(&merged, false, opts)
                } else {
                    0.0
                };
                kids.push(
                    crate::interact::ClampCut::new(
                        e.node_id,
                        crate::interact::clamp_lines_for(e.node_id),
                        e.style.clamp_lines(),
                        max_h,
                    )
                    .trim_end(clamp_trim)
                    .into_any_element(),
                );
            }
            // Абсолют с `anchor()`-вставками: раскладка поставила его к краю
            // содержащего блока (нулевая вставка), сдвиг до края якоря
            // считает заместитель на подготовке кадра. Без якорных вставок
            // коробка возвращается как есть.
            let child = d.children(kids).into_any_element();
            let child = match overflow_plan { Some(plan) => plan.wrap(child), None => child };
            crate::anchor::place(child, &merged, inherited)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse;

    fn find_class<'a>(nodes: &'a [Node], class: &str) -> Option<&'a Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.attr("class")
                    .is_some_and(|c| c.split_whitespace().any(|x| x == class))
                {
                    return Some(e);
                }
                if let Some(found) = find_class(&e.children, class) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Разворачивает обёртки документа до содержимого страницы.
    fn page_children(html: &str) -> Vec<Node> {
        fn dive(n: &[Node]) -> Vec<Node> {
            match n.first() {
                Some(Node::Element(e)) if e.tag == "html" || e.tag == "body" => dive(&e.children),
                _ => n.to_vec(),
            }
        }
        dive(&parse(html, ""))
    }

    #[test]
    fn margin_collapse_matches_the_browser_on_the_fixture_case() {
        // Ровно тот случай, на котором сравнение с Chrome показало сдвиг на
        // 10 точек: блок-обёртка без своего отступа сверху и ребёнок с ним.
        let page = page_children(
            "<div class=\"page\">\
               <div class=\"wrap\" style=\"margin: 0 0 10px\">w</div>\
               <div class=\"stack\" style=\"margin: 0 0 10px\">\
                 <div class=\"mt\" style=\"margin-top: 24px\">m</div>\
               </div>\
             </div>",
        );
        let children = match &page[0] {
            Node::Element(e) => collapse_margins(&e.children, false),
            _ => panic!("нет страницы"),
        };
        let stack = children
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.attr("class") == Some("stack") => Some(e),
                _ => None,
            })
            .expect("нет обёртки");
        // Отступ ребёнка вынесен наружу (24) и уменьшен на уже отданные
        // предыдущим блоком 10 — суммарный зазор остаётся 24, как в браузере.
        assert_eq!(stack.style.margin.top, Some(Len::Px(14.0)), "у обёртки");
        let child_top = stack.children.iter().find_map(|n| match n {
            Node::Element(e) => Some(e.style.margin.top),
            _ => None,
        });
        assert_eq!(child_top, Some(Some(Len::Px(0.0))), "у ребёнка снят");
    }

    #[test]
    fn out_of_flow_neighbours_keep_their_margins() {
        // Плавающий блок в схлопывании не участвует: его поле стоит как
        // написано, и соседа он не обкрадывает.
        let nodes = parse(
            "<div style=\"margin: 16px; float: left\">a</div>\
             <div style=\"margin: 16px; float: left\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
            _ => panic!("нет body"),
        };
        for (i, n) in body.iter().enumerate() {
            let Node::Element(e) = n else { continue };
            assert_eq!(
                e.style.margin.top,
                Some(Len::Px(16.0)),
                "плавающий блок {i} потерял поле"
            );
        }
    }

    #[test]
    fn margins_in_em_collapse_too() {
        // `margin: 1em 0` — самая частая запись отступа в разметке: без
        // перевода в точки схлопывание не срабатывало вовсе.
        let nodes = parse(
            "<div style=\"margin-bottom: 1em\">a</div><div style=\"margin-top: 2em\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
            _ => panic!("нет body"),
        };
        let second = match &body[1] {
            Node::Element(e) => e.style.margin.top,
            _ => panic!("нет второго блока"),
        };
        // 32 всего, из них 16 уже дал нижний отступ предыдущего блока.
        assert_eq!(second, Some(Len::Px(16.0)), "получено {second:?}");
    }

    #[test]
    fn adjacent_margins_collapse_into_the_larger() {
        // В CSS нижний отступ одного блока и верхний отступ следующего не
        // складываются: остаётся больший. Иначе документ растёт сверху вниз.
        let nodes = parse(
            "<div style=\"margin-bottom: 10px\">a</div><div style=\"margin-top: 24px\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
            _ => panic!("нет body"),
        };
        let second = match &body[1] {
            Node::Element(e) => e.style.margin.top,
            _ => panic!("нет второго блока"),
        };
        // 24 всего, из них 10 уже дал нижний отступ предыдущего блока.
        assert_eq!(second, Some(Len::Px(14.0)), "получено {second:?}");
    }

    #[test]
    fn first_child_margin_leaks_through_a_borderless_parent() {
        // Отступ первого ребёнка в CSS — тот же отступ, что у родителя, если
        // между ними нет ни рамки, ни внутреннего отступа.
        let nodes = parse(
            "<div class=\"wrap\"><div class=\"in\" style=\"margin-top: 24px\">x</div></div>",
            "",
        );
        // Примыкание ТРАНЗИТИВНО (§8.3.1): поле уходит на самую внешнюю
        // коробку цепи, а у всех внутренних снимается. Пока подъём шёл на
        // один уровень, то же поле поднималось повторно на каждом.
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => b,
            _ => panic!("нет body"),
        };
        assert_eq!(
            body.style.margin.top,
            Some(Len::Px(24.0)),
            "отступ вынесен на внешнюю коробку"
        );
        let wrap_top =
            find_class(std::slice::from_ref(&inner[0]), "wrap").and_then(|e| e.style.margin.top);
        assert_eq!(wrap_top, Some(Len::Px(0.0)), "у обёртки отступ снят");
        let child_top =
            find_class(std::slice::from_ref(&inner[0]), "in").and_then(|e| e.style.margin.top);
        assert_eq!(child_top, Some(Len::Px(0.0)), "у ребёнка отступ снят");
    }
}

/// Высота строки в точках — для статической позиции блочного элемента.
pub(crate) fn line_height_px(style: &Computed, opts: &RenderOpts) -> f32 {
    let size = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    match style.line_height {
        Some(Len::Px(v)) => v,
        // Голое число хранится долей: это множитель к кеглю.
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => size * 1.2,
    }
}

/// Есть ли в поддереве непустой текст — по нему считается высота строки.
pub(crate) fn has_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(e) => has_text(&e.children),
    })
}

/// Доля кегля для `line-height: normal` — по метрикам шрифта элемента.
///
/// Постоянная доля неверна: у Ahem `normal` ровно кегль, у текстовых шрифтов
/// около 1.15–1.3. Из-за постоянной 1.31 коробка с `line-height: 1em` и
/// соседняя без него расходились по высоте строк (`pre-wrap-008`).
pub(crate) fn normal_fraction(style: &Computed, opts: &RenderOpts) -> f32 {
    // Без своего семейства текст набирается шрифтом ДОКУМЕНТА
    // (`opts.text.font_family`, у стенда — Times New Roman), а щуп метрик
    // пустое имя меряет как `GENERIC_SANS` (Segoe UI, `metrics.rs`
    // `use_text_system`): `normal` выходил 1.33 вместо 1.15 у того шрифта,
    // которым строка нарисована, и струт строки из одних атомов был выше
    // атома (`inlines-017-ref`: ячейка 21.3px при картинке 20px).
    let family = style
        .font_family
        .clone()
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| {
            if style.monospace == Some(true) {
                crate::metrics::mono_family_for(style.lang.as_deref()).to_string()
            } else {
                opts.text.font_family.to_string()
            }
        });
    let measured = crate::metrics::normal_line(&family);
    if measured > 0.0 {
        measured
    } else {
        opts.normal_line_height
    }
}

/// Пустой ли текстовый узел ПО CSS.
///
/// Схлопывается только `space`, `tab`, `CR`, `LF`. `str::trim` снимает весь
/// юникодный пробел, и узел из идеографических U+3000 (или неразрывных
/// U+00A0) считался пустым: строка из них пропадала целиком, а абзац рвался
/// там, где рваться не должен (`trailing-ideographic-space-017`).
pub(crate) fn blank_text(t: &str) -> bool {
    t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}
