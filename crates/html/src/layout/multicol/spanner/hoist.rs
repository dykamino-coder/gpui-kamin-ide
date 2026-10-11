//! Подъём охватчиков и внутренний инлайн-размер многоколоночника.

use super::fragment::spanner_parts;
use super::{SpanPart, has_deep_spanner, passes_spanner, splits_for_spanner};
use crate::dom::Node;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Поднять спаннеров-потомков к прямым детям многоколоночника, разрезав их
/// предков (css-multicol-1 §column-span, `Overview.bs:1497-1499`). `None` —
/// поднимать нечего, дерево не трогаем.
///
/// Blink держит для этого отдельный путь `ColumnSpannerPath`
/// (`column_spanner_path.h`: «A path from the multicol container and down to
/// a column spanner, each container represented as a step on the path») и
/// ведёт раскладку предков по нему: `BlockLayoutAlgorithm` на шаге пути
/// обрывает свой фрагмент перед спаннером
/// (`block_layout_algorithm.cc:1052-1058`), а `ColumnLayoutAlgorithm`
/// достаёт сам спаннер (`GetSpannerFromPath`,
/// `column_layout_algorithm.cc:224`) и кладёт его между линиями колонок. У
/// нас раскладка колонок принимает спаннера ТОЛЬКО прямым ребёнком
/// (`render.rs` `is_span`, `StackChild::span`), поэтому тот же разрез
/// делается в дереве до неё.
pub(crate) fn hoist_spanners(kids: &[Node]) -> Option<Vec<Node>> {
    if !kids.iter().any(|n| {
        matches!(n, Node::Element(c)
            if (passes_spanner(c) || splits_for_spanner(c)) && has_deep_spanner(c))
    }) {
        return None;
    }
    let mut out: Vec<Node> = Vec::with_capacity(kids.len() + 2);
    for p in spanner_parts(kids) {
        match p {
            SpanPart::Body(b) => out.extend(b),
            SpanPart::Span(s) => out.push(s),
        }
    }
    Some(out)
}

/// Многоколоночный контейнер: `column-*` применяются только к блочным
/// контейнерам (css-multicol-1 §2), сетка и гибкий контейнер ими не
/// становятся (`grid-multicol-001`,
/// `column-property-should-not-apply-on-grid-container-001`).
/// Решает ли ширину этой коробки её СОДЕРЖИМОЕ.
///
/// Ключевые слова `min-content`/`max-content`/`fit-content` требуют
/// внутреннего размера прямо (css-sizing-3 §4.1); у плавающей, абсолютной и
/// строчной коробки то же самое зовётся shrink-to-fit (CSS 2.1 §10.3.5) — тот
/// же перечень, что у предиката `shrink_to_fit` в `apply.rs:940`. Элемент
/// гибкого контейнера и сетки тоже меряется содержимым: его основа —
/// `max-content` (css-flexbox-1 §9.2 п.3.A).
pub(crate) fn intrinsic_inline_size(c: &Computed, parent: &Computed) -> bool {
    if matches!(
        c.width,
        Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
    ) {
        return true;
    }
    if !matches!(c.width, None | Some(Len::Auto)) {
        return false;
    }
    c.float.unwrap_or(0) != 0
        || matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        || matches!(
            c.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
        || matches!(
            parent.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
        )
}

pub(crate) fn multicol_container(c: &Computed) -> bool {
    // Заданный `column-height` тоже делает коробку многоколоночной
    // (css-multicol-2 §multi-column-model: «whose column-width, column-count,
    // or column-height property is not auto»; `column-height-012`).
    (c.column_count.is_some() || c.column_width.is_some() || c.column_height.is_some())
        && !matches!(
            c.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        )
}
