//! Строчный кусок со своей коробкой (фон, рамка, отступы) или своим письмом.

use crate::dom::pseudo_line_layers;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::paint::box_tree::styled::styled_div_with;
use crate::paint::effects::grouped::grouped;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::{has_text, line_height_px};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(super) fn own_box_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    let mut merged = inherit(inherited, &e.style, vcx);
    // Atomic containers publish the same node/order keys as block containers.
    merged.self_node = e.node_id;
    merged.anchor_seq = opts.cx.anchor_seq_for(e);
    merged.anchor_key = crate::layout::positioned::anchor::key_of(e);
    // Viewport units resolve against the viewport (css-values-4 §6.1.2), as in
    // `element()`: the atom path skipped it and `height: 350vh` on an
    // `inline-block` with `vertical-align: top` collapsed to its content
    // (`monolithic-overflow-014/015-print`).
    merged.resolve_viewport(opts.viewport, vcx);
    // CSS 2 sections 5.12.1-5.12.2 include inline-block containers,
    // but not ordinary inline boxes, in the pseudo-line scope.
    if e.style.display == Some(Display::InlineBlock) && e.style.inline_display != Some(true) {
        pseudo_line_layers::install(e, &mut merged, vcx);
    }
    let mut box_ = styled_div_with(e, &merged, &opts.cx);
    // Writing Modes 4 §4.3: orthogonal content has no baseline in the line's
    // axis. As in Blink LogicalBoxFragment::LastBaseline, let the line
    // synthesize an alphabetic baseline at the atomic margin box's under edge.
    if e.style.display == Some(Display::InlineBlock)
        && e.style.inline_display != Some(true)
        && merged.wm().vertical()
        && !inherited.wm().vertical()
        && !inherited.wm().turned()
    {
        box_.style().baseline_unavailable = Some(true);
    }
    // Строчная коробка БЕЗ содержимого всё равно высотой в строку:
    // рамка и фон рисуются по кеглю, а не по тексту. Без этого
    // `<span style="border-left:30px solid green">  </span>` выходил
    // нулевой высоты и не рисовался вовсе
    // (`line-edge-white-space-collapse-001`).
    // Обособленная блочная ось высоту уже задала (пусть нулевую) —
    // подставлять кегль строки поверх неё нельзя.
    // …и только у НАСТОЯЩЕГО строчного: у `inline-block` высота идёт от
    // содержимого, и подставленный кегль строки накрывал детей-блоков
    // (`border-left-width-applies-to-012`: квадрат 96 выходил 19).
    let genuine_inline = e.style.display.is_none() || e.style.inline_display == Some(true);
    if genuine_inline
        && merged.height.is_none()
        && !has_text(&e.children)
        && !merged.contains_height()
    {
        box_ = box_.h(px(line_height_px(&merged, opts)));
    }
    // `vertical-align` коробки в строке: верх/низ/середина СТРОКИ
    // (CSS 2.1 §10.8.1) — как у строчной таблицы выше. Без этого
    // `inline-block` с `vertical-align: top` сидел на базовой линии
    // и в высокой строке уезжал вниз.
    let self_align = match e.style.vertical_align {
        Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
        Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
        Some(Align::Center) => Some(gpui::AlignItems::Center),
        _ => None,
    };
    if let Some(a) = self_align {
        box_.style().align_self = Some(a);
    }
    // Проверено: открывать здесь слой содержащего блока, как блочный путь
    // (`cb_open`/`cb_close` ниже по файлу), ничего не меняет — до
    // `position-absolute-in-inline-*` правка не доезжает: у строчного там нет
    // своей коробки, он идёт прогоном текста, чинить надо в сборке прогонов.
    // Без гейта «только настоящая строчная коробка» слой ещё и ломает
    // `position-relative-table-{tbody,thead,tfoot,tr}-*-absolute-child`.
    // `inline-block` с ВЕРТИКАЛЬНЫМ письмом — контейнер блоков со своей
    // осью блочного потока (css-writing-modes-4 §3.1): дети-блоки идут
    // колонками справа налево (`vertical-rl`) или слева направо
    // (`block-flow-direction-vrl-011`). Путь атома минует общий гейт в
    // `element()`, поэтому ось ставится здесь.
    if e.style.display == Some(Display::InlineBlock)
        && merged.wm().vertical()
        && e.children.iter().any(|n| {
            matches!(n, Node::Element(k)
                if !k.inline || matches!(k.style.display, Some(Display::Block)))
        })
    {
        // Предел строчной оси детей — собственная высота коробки (как
        // `element()` сеет `ortho_limit` от высоты предка): без него
        // колонки тянулись за низ (`block-flow-direction-vrl-012`).
        let em_base = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * em_base),
            _ => None,
        };
        if let Some(h) = px_of(e.style.height) {
            let b = e.style.borders();
            let edges = if e.style.border_box == Some(true) {
                px_of(b.top).unwrap_or(0.0)
                    + px_of(b.bottom).unwrap_or(0.0)
                    + px_of(e.style.padding.top).unwrap_or(0.0)
                    + px_of(e.style.padding.bottom).unwrap_or(0.0)
            } else {
                0.0
            };
            merged.ortho_limit = Some((h - edges).max(0.0));
        }
        box_ = box_.flex();
        box_ = if merged.wm().block_rl() {
            box_.flex_row_reverse()
        } else {
            box_.flex_row()
        };
    }
    // Маска, обрезка формой и фильтр действуют и на СТРОЧНУЮ коробку
    // (css-masking §1: `clip-path` применяется ко всем элементам):
    // блочный путь заворачивает её в буфер группы, а атомный шёл
    // мимо, и `clip-path` на `inline-block` не резал ничего
    // (`clip-path-contentBox-1d/1e`). Обёртка сама возвращает
    // элемент как есть, когда группировать нечего.
    // An atomic inline (`inline-block` and kin) is a block container: when it
    // is positioned (or otherwise a containing block) its padding box is the
    // containing block of absolute descendants (CSS 2.1 §10.1 item 4), so
    // grandchildren must leave their non-positioned parent block for this
    // box's layer, as on the generic block path (`generic_box/children.rs`).
    // Without the layer the box resolved its insets against its parent
    // (`justify-self-static-position-001-ref`: `left: 20px` from the inner
    // block's padding edge, 5px off). Genuine inline boxes stay excluded
    // (see the measurement above).
    // Atomic flex/grid containers own the same CB scope. Table parts keep
    // the table-specific handling of their absolute children.
    let cb_layer = matches!(
        e.style.display,
        Some(Display::InlineBlock | Display::InlineFlex | Display::InlineGrid)
    ) && !genuine_inline
        && crate::text::inline::establishes_cb(&merged);
    let cb_layer = cb_layer.then(|| {
        crate::layout::positioned::layer_scope::LayerScope::strict(
            &opts.cx.positioning,
            &merged,
            crate::paint::stacking::fixed_cb_layer_box(&merged),
        )
    });
    let mut kids =
        crate::layout::multicol::gap_rules::decorated_children(e, &merged, opts, |opts| {
            opts.cx.build.blocks(&e.children, &merged, opts)
        });
    if let Some(layer) = cb_layer {
        kids.extend(layer.finish());
    }
    Some(grouped(
        box_.children(kids).into_any_element(),
        &e.style,
        vcx,
    ))
}
