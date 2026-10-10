//! Блочный поток: дети блока в элементы (blocks).

mod prepare_children;
pub(super) use prepare_children::prepare_children;

mod ordered_children;
use ordered_children::ordered_children;
mod flow_children;
use flow_children::flow_children;

use crate::dom::Node;
use crate::interactive::sticky::sticky_probe;
use crate::layout::block::available_width;
use crate::layout::block::containing::{AVAIL_W, AvailWGuard, CB_WIDTH, scopeguard_cb};
use crate::layout::block::margins::CELL_BFC;
use crate::layout::block::struts::zero_len;
use crate::layout::float::band_flow_host::{
    BAND_CBH, BAND_CBW, BAND_FL, BAND_WM, BandCbhGuard, BandCbwGuard, BandFlGuard, BandWmGuard,
};
use crate::layout::float::initial_letter::initial_letter_float;
use crate::layout::float::wrap::wrap_floats;
use crate::layout::page::paged::PAGED;
use crate::layout::replaced::replaced_used_style;
use crate::paint::stacking::by_layer;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::AnyElement;
pub(super) mod canvas;
pub(super) mod flow;
pub(super) mod positioned;
pub(crate) use crate::render::blocks::canvas::*;
use crate::render::blocks::flow::*;
pub(crate) use crate::render::blocks::positioned::*;

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
    let (collapsed, ordered_context, under_tf) = prepare_children(nodes, inherited, opts, cell_bfc);
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed = ordered_children(collapsed, ordered_context, inherited, opts);
    let flex_ctx = matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let letter_scope = first_letter_scope::Scope::new(&collapsed, inherited);
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
    let vert_host = inherited.vertical == Some(true) && inherited.sideways != Some(true);
    // Вне хоста и там, где у раскладки свой счёт строк и разрывов: под
    // `line-clamp` (точка среза считает строки и флоаты за ней —
    // `line-clamp-with-floats-003/004`, `webkit-line-clamp-025`; отложенный
    // ряд `float_flow` для этого и заведён) и на печатных листах (монолитная
    // коробка хоста не режется между страницами —
    // `monolithic-overflow-020-print`).
    let measured_ok = !flex_ctx
        && inherited.line_clamp.is_none()
        && crate::text::clamp::clamp_context().is_none()
        && !PAGED.with(std::cell::Cell::get)
        && !matches!(
            inherited.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && (inherited.vertical != Some(true) || vert_host)
        && (inherited.vertical_rl != Some(true) || vert_host)
        && (inherited.rtl != Some(true) || inherited.vertical != Some(true));
    let _fl_guard =
        BandFlGuard(BAND_FL.with(|f| f.replace(inherited.first_line.as_deref().cloned())));
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
    let collapsed = flow_children(collapsed, ordered_context, inherited, opts, flex_context);
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
                e.style.position != Some(crate::style::computed::Position::Absolute)
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
    let below_run_start = 0usize;
    let below_run_end = usize::MAX;
    let below_zs: Vec<i32> = vec![];
    // Липкому ребёнку нужны две вещи, которых он сам не видит: коробка
    // родителя и видимая часть ленты. Их снимает распорка — она идёт первой,
    // потому что готовит замер до отрисовки детей.
    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => e.style.position == Some(crate::style::computed::Position::Sticky),
        _ => false,
    });
    let frame: crate::interactive::sticky::element::StickyCell = Default::default();
    if sticky {
        out.push(sticky_probe(frame.clone()));
    }
    let pending: Vec<Node> = vec![];
    // Слой верхней отрисовки этого контейнера: позиционированные элементы
    // складывают сюда содержимое, а забирается оно последними детьми.
    crate::layout::positioned::containing_block::late_open();
    let nodes = collapsed.as_slice();
    blocks_flow(
        nodes,
        run_breaks,
        pending,
        out,
        letter_scope,
        inherited,
        opts,
        ordered_context,
        under_tf,
        frame,
        below_run_end,
        below_run_start,
        below_zs,
    )
}
