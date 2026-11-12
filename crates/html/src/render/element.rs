//! Сборка дерева: узел-элемент в элемент GPUI (`element()`).
//!
//! Вход — `dom::Element` и унаследованный стиль, выход — `AnyElement`. Здесь
//! сливается стиль с наследованием, разрешаются единицы окна, якорные ключи
//! кадра, `dir="auto"`, предел ортогонального потока и полосы прокрутки; затем
//! элемент уходит в формы, рамку (`iframe`/`<object>` с документом) или общий
//! диспетчер по виду коробки (`dispatch_element`).

use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
mod orthogonal;
mod quirks;
pub(super) use orthogonal::orthogonal_limit;

mod dispatch;
pub(super) use dispatch::dispatch_element;
mod merged;
pub(crate) use merged::merged_style;

use crate::dom::Element;
use crate::layout::build_contracts::Effect;
use crate::layout::replaced::iframe::{iframe, object_is_document};
use gpui::AnyElement;

/// Блочный элемент.
pub(crate) async fn element_stack_for<const QUERY: bool>(
    stk: &mut reblessive::Stk,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    crate::work::add!(RebuiltNodes, 1);
    let entered = opts.inner(|cx| cx.contracts.reflow_depth += 1);
    let opts = &*entered;
    let own = crate::layout::positioned::containing_block::metrics::fragmentation::options(e, opts);
    let opts = own.as_deref().unwrap_or(opts);
    // Строки внутри коробки другого контекста фрагментации рекордеру потока
    // не пишутся (`flow::flow_lines`).
    let nested = opts.frag.watch.as_ref().and_then(|w| {
        let watch = w.for_flex_item(e.node_id).or_else(|| {
            (w.takes_lines()
                && !crate::layout::fragment::flow::flow_lines_in(e, w.blocks().map(|b| b.id)))
            .then(|| w.nested())
        })?;
        let mut inner = Box::new(opts.clone());
        inner.frag.watch = Some(watch);
        Some(inner)
    });
    let built = crate::layout::fragment::paginate::watch::wrap(
        e.node_id,
        element_inner::<QUERY>(stk, e, inherited, nested.as_deref().unwrap_or(opts)).await,
        opts,
    );
    crate::layout::depth::wrap(built, opts)
}

#[cfg(test)]
mod lines_tests;

async fn element_inner<const QUERY: bool>(
    stk: &mut reblessive::Stk,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let viewport_opts = opts.inner_if(opts.cx.values.viewport != opts.viewport, |cx| {
        cx.values.viewport = opts.viewport
    });
    let opts = &*viewport_opts;
    let (frame, vcx) = (opts.cx.frame(), opts.cx.vcx());
    let sized = crate::layout::replaced::media::sized(e, inherited, opts, vcx);
    let e = &*sized;
    // Высота ряда от внешней колонки — только ЭТОМУ элементу (`FragCx::outer_space`).
    let (outer_space, inner_opts) = opts.take_outer_space();
    let opts = inner_opts.as_deref().unwrap_or(opts);
    let merged = merged_style(e, inherited, opts);
    let scroll_projection = crate::interactive::scroll_extent::projection(
        opts.cx.scroll.frame.as_ref(),
        inherited.self_node,
        Some(&merged),
        opts.cx.scroll.sticky.as_ref(),
    );
    // Проецируемое поддерево строится в цепочке сдвига проекции (`ShiftLink`).
    let projected_opts = scroll_projection
        .as_ref()
        .map(|p| opts.inner(|cx| cx.scroll.sticky = Some(p.link().clone())));
    let opts = projected_opts.as_deref().unwrap_or(opts);
    let geometry = crate::layout::positioned::containing_block::metrics::record::capture(
        e.node_id, &merged, opts,
    );
    let slot = opts
        .cx
        .position
        .continuation_slots
        .as_ref()
        .is_some_and(|roots| roots.contains(&e.node_id))
        .then_some(e.node_id);
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::paint::controls::element(e, &merged, opts) {
        let el = opts
            .cx
            .contracts
            .build
            .effects(el, Effect::transform(&merged, inherited, frame));
        let el = match geometry {
            Some(geometry) => geometry.wrap(el),
            None => el,
        };
        let el = crate::paint::continuation::slot::<QUERY>(slot, el);
        return crate::interactive::scroll_extent::project(el, scroll_projection);
    }
    // Рамка строится ОДИН раз до match: прежний `is_some() => unwrap()`
    // читал файл с диска и разбирал вложенный документ дважды за кадр.
    // Неразобранная рамка по-прежнему падает в общий рукав.
    let built_iframe = if e.tag == "iframe" {
        iframe(e, inherited, opts)
    } else if e.tag == "object" && object_is_document(e) {
        // `<object data="….html">` — тот же вложенный документ, что и рамка
        // (HTML §4.8.7): адрес переносится из `data` в `src`, размеры берутся
        // уже разрешёнными (`10em` в собственном стиле — ещё `Len::Em`).
        let mut copy = e.clone();
        let url = e.attr("data").unwrap_or_default().to_string();
        copy.attrs.push(("src".to_string(), url));
        copy.style.used_mut().reset_box_layout.width = merged.reset_box_layout.width;
        copy.style.used_mut().reset_box_layout.height = merged.reset_box_layout.height;
        iframe(&copy, inherited, opts)
    } else {
        None
    };
    let built =
        dispatch_element::<QUERY>(stk, e, merged, inherited, opts, outer_space, built_iframe).await;
    crate::interactive::scroll_extent::project(
        crate::paint::continuation::slot::<QUERY>(
            slot,
            match geometry {
                Some(geometry) => geometry.wrap(built),
                None => built,
            },
        ),
        scroll_projection,
    )
}

pub(crate) fn element_for<const QUERY: bool>(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let opts = &*opts.inner(|cx| {
        cx.contracts.lengths = Default::default();
        if !cx.contracts.in_render {
            cx.contracts.subtrees = Default::default();
        }
    });
    reblessive::Stack::new()
        .enter(|stk| element_stack_for::<QUERY>(stk, e, inherited, opts))
        .finish()
}

pub(crate) fn element(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    element_for::<false>(e, inherited, opts)
}
