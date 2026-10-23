//! Сборка детей общей коробки: ортогональные оси, clamp и контексты окраски.

use crate::layout::options::RenderOpts;
use crate::paint::box_tree::styled::clip_layer;
use crate::render::blocks::blocks;
mod formatting_context;
mod probes;
pub(crate) use probes::child_probes;

use crate::dom::Element;
use crate::layout::block::reorder::{orthogonal_vertical_children, resolve_inline_pct};
use crate::layout::block::struts::margin_px;
use crate::layout::block::vertical_flow_margins;
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::gap_rules::decorated_children;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::absolute_overflow;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::layout::writing_mode::orthogonal_children::orthogonal_children;
use crate::layout::writing_mode::vertical_hug;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement};
use std::borrow::Cow;

#[allow(clippy::too_many_arguments)]
pub(crate) fn generic_children(
    e: &Element,
    d: gpui::Div,
    merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    overflow_plan: Option<absolute_overflow::Plan>,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let makes_bfc = formatting_context::independent(&merged, &e.tag);
    let children = if let Some(children) = super::child_borrow::borrowed(e, &merged) {
        Cow::Borrowed(children)
    } else {
        let children = match &e.style.grid_areas {
            Some(areas) => place_named_areas(areas, e.children.clone()),
            None => e.children.clone(),
        };
        // Resolve physical margins before preparing the vertical formatting context.
        let children = if merged.wm().vertical() {
            // Поле самого контейнера по ведущей стороне оси потока —
            // для схлопывания с первым ребёнком (§8.3.1). Ведущая
            // сторона: левая у `vertical-lr`/`sideways-*`, правая у
            // `vertical-rl`. Открыта, если там нет ни рамки, ни
            // внутреннего отступа; у корня поля не схлопываются вовсе.
            let reverse = merged.wm().block_rl();
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
                // modes-4 §7.4): `margin-collapse-vlr-017`/`vrl-016` (`overflow: hidden`).
                let sealed = makes_bfc
                    || margin_px(border, &e.style, opts.cx.vcx()).unwrap_or(0.0) > 0.0
                    || margin_px(pad, &e.style, opts.cx.vcx()).unwrap_or(0.0) > 0.0;
                if sealed {
                    None
                } else {
                    Some(margin_px(own, &e.style, opts.cx.vcx()).unwrap_or(0.0))
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
                        opts.cx.vcx(),
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
        Cow::Owned(children)
    };
    let mut kids: Vec<AnyElement> = Vec::new();
    kids.extend(clip_layer(&merged, opts));
    // Бюджет строк обрезки: сторожа контекста живут, пока строится
    // поддерево — пробы детей пишут строки в буфер контейнера.
    // Многоколонник клэмпом не режется (`continue: collapse` там как
    // `auto`, §5.2; `styled_div_with` срез не ставит): без этого гейта
    // бюджет абзаца счётного режима поставил бы «…» (`line-clamp-039`).
    let is_clamp = (e.style.clamp_lines().is_some()
        || (e.style.clamp_auto == Some(true)
            && e.style.clamp_block_container()
            && auto_clamp_limit(&merged).is_some()))
        && !multicol_container(&e.style);
    let clamp_outer = if is_clamp {
        Some((e.node_id, false))
    } else {
        crate::text::clamp::clamp_context(&opts.cx)
    };
    let child_opts = opts.inner(|cx| {
        if is_clamp {
            cx.clamp.atom_depth = 0;
            cx.clamp.build = cx.clamp.build.container(e.node_id);
        } else if makes_bfc {
            cx.clamp.build = cx.clamp.build.bfc();
        }
    });
    let opts = &child_opts;
    // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
    // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
    // ранний `return` не съел запись. Абсолютные дети дорожек не
    // занимают (css-grid-1 §9), а пустой анонимный блок — это
    // распорка лент (`spacer()`), не элемент.
    // Paragraph numbers of this box's descendants (clamp point, tree order).
    let clamp_span = crate::text::clamp::open_clamp_span(&opts.cx);
    child_probes(
        e,
        &merged,
        opts,
        &mut kids,
        makes_bfc,
        is_clamp,
        clamp_span.clone(),
    );
    // Абсолютный потомок ищет ближайшего позиционированного предка
    // (§10.1), а раскладка под нами знает только непосредственного
    // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
    // содержащим блоком не является, переезжает сюда.
    let cb_layer = crate::text::inline::establishes_cb(&merged);
    let cb_scope = cb_layer.then(|| {
        crate::layout::positioned::layer_scope::LayerScope::cb(
            &opts.cx.position.state,
            fixed_cb_layer_box(&merged),
        )
    });
    // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
    // задан стиль хотя бы одной линейки: начальное `none` означает,
    // что рисовать нечего, и ни одна старая пара сюда не попадает
    // (см. `gap_rule_spec`).
    // css-gaps-1 §gap-decorations: «Gap decorations are painted just
    // above the border of the container» — ПОД детьми. Слой идёт до
    // них: буфер проб он всё равно читает в `paint`, а prepaint всего
    // дерева у gpui проходит раньше (эталоны `grid-gap-decorations-042`
    // и `flex-033` кладут линейки `z-index: -1`, `008/023` — элементы
    // `z-index: 2`).
    // The viewport scroll tree is built after the outer document ICB
    // closes. Its padding box still owns the initial containing block
    // (CSS 2.1 §10.1), including absolute descendants below body margins.
    let viewport = e.tag == "html-viewport";
    let viewport_scope = viewport
        .then(|| crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.position.state));
    kids.extend(decorated_children(e, &merged, opts, |opts| {
        // A clamp container establishes an independent formatting context
        // (CSS Overflow 4 section continue; Blink LineClampData). blocks()
        // receives only Computed, whose inherited clamp value cannot identify
        // the container. Preserve this box's known BFC boundary for its float
        // containment without turning its descendants into clamp containers.
        let formatting = is_clamp.then(|| Computed {
            flow_root: Some(true),
            ..merged.clone()
        });
        blocks(&children, formatting.as_ref().unwrap_or(&merged), opts)
    }));
    if let Some(scope) = viewport_scope {
        kids.extend(scope.finish());
    }
    let d = crate::text::clamp::hide_box_after_point(
        d,
        clamp_outer,
        clamp_span,
        &merged,
        is_clamp,
        &opts.cx,
    );
    if let Some(scope) = cb_scope {
        kids.extend(scope.finish());
    }
    // Абсолют с `anchor()`-вставками: раскладка поставила его к краю
    // содержащего блока (нулевая вставка), сдвиг до края якоря
    // считает заместитель на подготовке кадра. Без якорных вставок
    // коробка возвращается как есть.
    let child = d.children(kids).into_any_element();
    let child = match overflow_plan {
        Some(plan) => plan.wrap(child, vcx),
        None => child,
    };
    crate::layout::positioned::anchor::place::place_in_context(child, &merged, inherited, &opts.cx)
}
