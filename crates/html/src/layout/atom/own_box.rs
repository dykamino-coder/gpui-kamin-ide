//! Строчный кусок со своей коробкой (фон, рамка, отступы) или своим письмом.
// owner: A

use crate::dom::{Element, Node};
use crate::paint::effects::grouped::grouped;
use crate::render::{RenderOpts, blocks, pseudo_line_layers, styled_div_with};
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
    let mut merged = inherit(inherited, &e.style);
    // CSS 2 sections 5.12.1-5.12.2 include inline-block containers,
    // but not ordinary inline boxes, in the pseudo-line scope.
    if e.style.display == Some(Display::InlineBlock) && e.style.inline_display != Some(true) {
        pseudo_line_layers::install(e, &mut merged);
    }
    let mut box_ = styled_div_with(e, &merged);
    // Writing Modes 4 §4.3: orthogonal content has no baseline in the line's
    // axis. As in Blink LogicalBoxFragment::LastBaseline, let the line
    // synthesize an alphabetic baseline at the atomic margin box's under edge.
    if e.style.display == Some(Display::InlineBlock)
        && e.style.inline_display != Some(true)
        && merged.vertical == Some(true)
        && inherited.vertical != Some(true)
        && inherited.rotated_line != Some(true)
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
    // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): открывать здесь слой содержащего
    // блока, как это делает блочный путь (`cb_open`/`cb_close` ниже по
    // файлу), чтобы абсолютный ребёнок строчного `position: relative`
    // не уезжал к внешнему содержащему блоку. Срез из 229 пар:
    // 205 → 205 при гейте «только настоящая строчная коробка», и
    // 205 → 197 без него (`position-relative-table-{tbody,thead,tfoot,
    // tr}-*-absolute-child` уходили 0.00 → «красное видно»).
    // До `position-absolute-in-inline-*` правка НЕ доезжает: там у
    // строчного нет своей коробки, он идёт прогоном текста, и вешать
    // слой не на что — чинить надо в сборке прогонов.
    // `inline-block` с ВЕРТИКАЛЬНЫМ письмом — контейнер блоков со своей
    // осью блочного потока (css-writing-modes-4 §3.1): дети-блоки идут
    // колонками справа налево (`vertical-rl`) или слева направо
    // (`block-flow-direction-vrl-011`). Путь атома минует общий гейт в
    // `element()`, поэтому ось ставится здесь.
    if e.style.display == Some(Display::InlineBlock)
        && merged.vertical == Some(true)
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
        box_ = if merged.vertical_rl == Some(true) {
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
    // Only `inline-block`: table parts reach this path too, and their
    // absolute children keep the table's own handling
    // (`position-relative-table-*-absolute-child`).
    let cb_layer = e.style.display == Some(Display::InlineBlock)
        && !genuine_inline
        && crate::text::inline::establishes_cb(&merged);
    if cb_layer {
        crate::layout::positioned::containing_block::cb_open_strict(
            &merged,
            crate::paint::stacking::fixed_cb_layer_box(&merged),
        );
    }
    let mut kids = crate::layout::multicol::gap_rules::decorated_children(e, &merged, opts, || {
        blocks(&e.children, &merged, opts)
    });
    if cb_layer {
        kids.extend(crate::layout::positioned::containing_block::cb_close());
    }
    Some(grouped(box_.children(kids).into_any_element(), &e.style))
}
