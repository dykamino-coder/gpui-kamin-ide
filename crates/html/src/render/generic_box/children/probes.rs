//! Пробы границ детей для gap rules и line clamp.

use crate::dom::Element;
use crate::dom::whitespace::is_blank;
use crate::layout::options::RenderOpts;
use crate::layout::table::anon::has_box_style_probe;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn child_probes(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    kids: &mut Vec<AnyElement>,
    makes_bfc: bool,
    is_clamp: bool,
    span: Option<crate::text::clamp::ClampSpan>,
) {
    if let Some(items) = opts.cx.gap_items.clone()
        && !matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
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
        kids.push(crate::paint::gap_rules::gap_item_probe(
            items,
            [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)],
            [bw(merged.margin.right), bw(merged.margin.bottom)],
        ));
    }
    if let Some((key, skip)) = crate::text::clamp::clamp_context(&opts.cx) {
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
            && merged.float.is_none_or(|f| f == 0)
            && !matches!(
                merged.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && !matches!(
                merged.display,
                Some(Display::InlineBlock)
                    | Some(Display::InlineFlex)
                    | Some(Display::InlineGrid)
                    | Some(Display::InlineTable)
            );
        // A box with a definite height ends where its size says, even when
        // unstyled: lines overflowing it fit with it (`line-clamp-auto-040`).
        let sized = e.style.height.is_some()
            && !skip
            && matches!(merged.display, None | Some(Display::Block));
        let side = px_lossy_zero;
        // Нижние рамка и паддинг фрагментированной коробки
        // остаются в потоке (css-overflow-4 §5.3): проба несёт
        // их вместе с границами паддинг-бокса; end margin too — it
        // collapses along the ancestor chain of the clamp point (Blink
        // `LineClampAncestorChain`), so an in-flow block with any of them
        // is probed even without paint (`line-clamp-024`, `-auto-027`).
        let bp_after = side(e.style.borders().bottom) + side(e.style.padding.bottom);
        let in_flow_block = !skip
            && merged.float.is_none_or(|f| f == 0)
            && !matches!(
                merged.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && matches!(merged.display, None | Some(Display::Block));
        // Font- and line-relative lengths (`3lh`, `1em`) of the end edges.
        let fs = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let lh = crate::text::text_box::line_height_px(merged, opts);
        let len_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * fs),
            Some(Len::Lh(k)) => Some(k * lh),
            _ => None,
        };
        let end_margin = if in_flow_block || merged.float.is_some_and(|f| f != 0) {
            len_px(merged.margin.bottom).unwrap_or(0.0)
        } else {
            0.0
        };
        // `min-height` as a border-box extent from the border-box top.
        let min_end = match len_px(e.style.min_height) {
            Some(m) if in_flow_block && m > 0.0 => {
                if merged.border_box == Some(true) {
                    m
                } else {
                    m + side(e.style.borders().top) + side(e.style.padding.top) + bp_after
                }
            }
            _ => 0.0,
        };
        let capped = in_flow_block && e.style.max_height.is_some();
        let ends =
            in_flow_block && (bp_after > 0.0 || end_margin != 0.0 || min_end > 0.0 || capped);
        if !is_clamp && (has_box_style_probe(&e.style) || monolithic || sized || ends) {
            kids.push(crate::text::clamp::clamp_probe(
                crate::text::clamp::clamp_lines_for(key, opts.cx.vcx()),
                opts.cx.clamp_measure().map(|(_, m)| m),
                0.0,
                skip,
                // `min-height` alone does not make the box monolithic: it
                // is fragmented at the clamp point like any block
                // (`line-clamp-auto-062`); only a definite `height` does,
                // or `min-height` of a size-contained box, whose height
                // does not depend on its content (`line-clamp-auto-047`).
                e.style.height.is_some()
                    || (e.style.min_height.is_some() && e.style.contain_size == Some(true))
                    || monolithic,
                bp_after,
                crate::text::clamp::BoxEnds {
                    margin: end_margin,
                    min_end,
                    capped,
                    node: e.node_id,
                },
                // Коробка — не абзац: знак обрыва на неё не садится
                // (он всегда в конце строки, css-overflow-4 §5.3).
                None,
                None,
                span,
                side(e.style.borders().top),
                matches!(
                    merged.position,
                    Some(crate::style::computed::Position::Absolute)
                ),
            ));
        } else if !is_clamp
            && !skip
            // Only out-of-flow children: the box itself is empty and
            // zero-height in the flow (`line-clamp-with-abspos-023`).
            && e.children.iter().all(|n| {
                is_blank(n)
                    || matches!(n, crate::dom::Node::Element(c) if matches!(
                        c.style.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    ))
            })
            && merged.float.is_none_or(|f| f == 0)
            && !matches!(
                merged.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && matches!(merged.display, None | Some(Display::Block))
        {
            kids.push(crate::text::clamp::clamp_empty_probe(
                crate::text::clamp::clamp_lines_for(key, opts.cx.vcx()),
                opts.cx.clamp_measure().map(|(_, m)| m),
            ));
        }
    }
}
