//! Пробы границ детей для gap rules и line clamp.

use crate::dom::Element;
use crate::layout::table::anon::has_box_style_probe;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn child_probes(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    kids: &mut Vec<AnyElement>,
    makes_bfc: bool,
    is_clamp: bool,
) {
    if let Some(key) = crate::paint::gap_rules::gap_context()
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
            crate::paint::gap_rules::gap_items_for(key),
            [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)],
        ));
    }
    if let Some((key, skip)) = crate::text::clamp::clamp_context() {
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
        if !is_clamp && (has_box_style_probe(&e.style) || monolithic) {
            // Нижние рамка и паддинг фрагментированной коробки
            // остаются в потоке (css-overflow-4 §5.3): проба несёт
            // их вместе с границами паддинг-бокса.
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let bp_after = side(e.style.borders().bottom) + side(e.style.padding.bottom);
            kids.push(crate::text::clamp::clamp_probe(
                crate::text::clamp::clamp_lines_for(key),
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
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && matches!(merged.display, None | Some(Display::Block))
        {
            kids.push(crate::text::clamp::clamp_empty_probe(
                crate::text::clamp::clamp_lines_for(key),
            ));
        }
    }
}
