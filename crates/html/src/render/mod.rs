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
pub(crate) use crate::layout::fragment::*;
pub(crate) use crate::layout::fragment::probe::*;
pub(crate) use crate::layout::fragment::grid_bands::*;
pub(crate) use crate::layout::fragment::clone::*;
pub(crate) use crate::layout::fragment::line_shape::*;
pub(crate) use crate::layout::fragment::shape_contents::*;
pub(crate) use crate::layout::fragment::push::*;
pub(crate) use crate::layout::fragment::flex_lines::*;
pub(crate) use crate::layout::fragment::table_bands::*;
pub(crate) use crate::layout::fragment::breaks::*;
pub(crate) use crate::layout::page::paged::*;
pub(crate) use crate::layout::page::names::*;
pub use crate::layout::page::names::{PageMarginDecls, PageMarginDeclsFn, first_page_name};
pub use crate::layout::page::paged::{render_paged, render_paged_select};
pub(crate) use crate::paint::effects::mask::*;
pub(crate) use crate::paint::effects::grouped::*;
pub(crate) use crate::paint::effects::transform::*;
pub(crate) use crate::layout::block::containing::*;
pub(crate) use crate::layout::block::reorder::*;
pub(crate) use crate::layout::block::margins::*;
pub(crate) use crate::layout::block::struts::*;
pub(crate) use crate::layout::writing_mode::*;
pub(crate) use crate::text::ruby::*;
pub(crate) use crate::text::text_box::*;
pub(crate) use crate::animation::frames::*;
pub(crate) use crate::interactive::scroll_box::*;
pub(crate) use crate::layout::grid::*;

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


/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// не содержит строчной коробки, и поля всех её детей в потоке тоже

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
/// Абзац для тех, кто собирает текст сам — содержимое поля ввода.
pub fn paragraph_public(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph(nodes, inherited, opts)
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
