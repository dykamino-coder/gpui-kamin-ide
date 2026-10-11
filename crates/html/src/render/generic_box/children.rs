//! Сборка детей общей коробки: ортогональные оси, clamp и контексты окраски.

mod probes;
pub(super) use probes::child_probes;

use crate::dom::Element;
use crate::layout::block::reorder::{orthogonal_vertical_children, resolve_inline_pct};
use crate::layout::block::struts::margin_px;
use crate::layout::block::vertical_flow_margins;
use crate::layout::grid::place_named_areas;
use crate::layout::multicol::gap_rules::gap_rule_spec;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::absolute_overflow;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::layout::writing_mode::orthogonal_children::orthogonal_children;
use crate::layout::writing_mode::vertical_hug;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::text_box_trim_px;
use gpui::{AnyElement, IntoElement, ParentElement};

#[allow(clippy::too_many_arguments)]
pub(crate) fn generic_children(
    e: &Element,
    d: gpui::Div,
    merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    overflow_plan: Option<absolute_overflow::Plan>,
) -> AnyElement {
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
                None | Some(crate::style::computed::Overflow::Visible)
            ) || !matches!(
                e.style.overflow_y,
                None | Some(crate::style::computed::Overflow::Visible)
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
    let _clamp_guard = is_clamp.then(|| crate::text::clamp::ClampGuard::enter(e.node_id));
    let makes_bfc = matches!(
        merged.overflow_x,
        Some(crate::style::computed::Overflow::Hidden) | Some(crate::style::computed::Overflow::Scroll)
    ) || matches!(
        merged.overflow_y,
        Some(crate::style::computed::Overflow::Hidden) | Some(crate::style::computed::Overflow::Scroll)
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
    let _bfc_guard = (!is_clamp && makes_bfc && crate::text::clamp::clamp_context().is_some())
        .then(crate::text::clamp::ClampGuard::enter_bfc);
    // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
    // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
    // ранний `return` не съел запись. Абсолютные дети дорожек не
    // занимают (css-grid-1 §9), а пустой анонимный блок — это
    // распорка лент (`spacer()`), не элемент.
    child_probes(e, &merged, opts, &mut kids, makes_bfc, is_clamp);
    // Абсолютный потомок ищет ближайшего позиционированного предка
    // (§10.1), а раскладка под нами знает только непосредственного
    // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
    // содержащим блоком не является, переезжает сюда.
    let cb_layer = crate::text::inline::establishes_cb(&merged);
    if cb_layer {
        crate::layout::positioned::containing_block::cb_open_with(fixed_cb_layer_box(&merged));
    }
    // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
    // задан стиль хотя бы одной линейки: начальное `none` означает,
    // что рисовать нечего, и ни одна старая пара сюда не попадает
    // (см. `gap_rule_spec`).
    let gap_rules = gap_rule_spec(e, &merged, opts);
    let gap_buf = gap_rules
        .is_some()
        .then(|| crate::paint::gap_rules::gap_items_for(e.node_id ^ opts.doc_salt));
    let _gap_guard = gap_buf
        .as_ref()
        .map(|_| crate::paint::gap_rules::GapGuard::enter(e.node_id ^ opts.doc_salt));
    // css-gaps-1 §gap-decorations: «Gap decorations are painted just
    // above the border of the container» — ПОД детьми. Слой идёт до
    // них: буфер проб он всё равно читает в `paint`, а prepaint всего
    // дерева у gpui проходит раньше (эталоны `grid-gap-decorations-042`
    // и `flex-033` кладут линейки `z-index: -1`, `008/023` — элементы
    // `z-index: 2`).
    if let (Some(buf), Some(spec)) = (gap_buf, gap_rules) {
        kids.push(
            crate::paint::gap_rules::painter::GapRulePainter::new(buf, spec).into_any_element(),
        );
    }
    kids.extend(blocks(&children, &merged, opts));
    if cb_layer {
        kids.extend(crate::layout::positioned::containing_block::cb_close());
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
            side(bw.top) + side(bw.bottom) + side(merged.padding.top) + side(merged.padding.bottom)
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
            crate::text::clamp::ClampCut::new(
                e.node_id,
                crate::text::clamp::clamp_lines_for(e.node_id),
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
    let child = match overflow_plan {
        Some(plan) => plan.wrap(child),
        None => child,
    };
    crate::layout::positioned::anchor::place::place(child, &merged, inherited)
}
