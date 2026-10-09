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
pub(crate) mod paragraph;
pub(crate) use crate::render::paragraph::*;
pub(crate) use crate::render::paragraph::pieces::*;
pub(crate) mod blocks;
pub(crate) use crate::render::blocks::*;
pub(crate) mod element;
pub(crate) use crate::render::element::*;

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



/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// не содержит строчной коробки, и поля всех её детей в потоке тоже

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
