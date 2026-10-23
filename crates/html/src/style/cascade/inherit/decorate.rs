//! Стадия коробки, оформление текста: линии text-decoration распространяются на потомков в потоке
//! (css-text-decor-3 §2.1) с цветом, толщиной, смещением и шрифтом украшающей коробки.

use super::*;
use crate::document::state::ValueCx;

/// Украшения текста (css-text-decor-3 §2.1): линии не наследуются, а
/// РАСПРОСТРАНЯЮТСЯ на всех потомков в потоке, кроме атомарных строчных
/// (`inline-block`, `inline-table`…) и вынесенных из потока (флоаты,
/// абсолюты); цвет, рисунок, толщина и метрики — от украшающей коробки.
/// `display: contents` коробки не даёт и своих линий не кладёт.
pub(super) fn decorate(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    own_px: f32,
    vcx: ValueCx<'_>,
) {
    use crate::style::computed::Display as D;
    use crate::style::computed::Position as P;
    use crate::style::computed::{DECOR_THROUGH, DECOR_UNDER, Decor, DecorFont, DecorLen};
    let over_lang = c.lang.as_deref().is_some_and(|l| {
        let l = l.to_ascii_lowercase();
        ["ja", "ko", "mn"]
            .iter()
            .any(|p| l == *p || l.starts_with(&format!("{p}-")))
    });
    // Ребёнок flex/grid/lanes-контейнера блокифицирован (css-display-3
    // §2.7): `inline-block` там уже не атомарная строчная коробка, и линии
    // идут дальше (Blink `StopPropagateTextDecorations` смотрит display ПОСЛЕ
    // `AdjustStyleForDisplay`; эталоны `grid-lanes/baseline/*-ref`).
    let blockified = matches!(
        parent.display,
        Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes)
    );
    let blocked = matches!(own.position, Some(P::Absolute) | Some(P::Fixed))
        || own.float.is_some_and(|f| f != 0)
        || (!blockified
            && matches!(
                own.display,
                Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
            ));
    c.decors = if blocked {
        Vec::new()
    } else {
        parent.decors.clone()
    };
    // Линии, пришедшие в блок, рисуются на его анонимной строчной коробке
    // (css-text-decor-3 §2.1, пример 1): метрики и положение — от шрифта
    // этого блока (`text-decoration-subelements-004`).
    let block = match own.display {
        None => own.block_tag && own.inline_display != Some(true),
        Some(D::Contents) => false,
        Some(_) => true,
    };
    if block {
        for d in c.decors.iter_mut() {
            d.font = DecorFont {
                family: c.font_family.clone(),
                monospace: c.monospace,
                weight: c.font_weight,
                italic: c.italic,
                stretch: c.font_stretch,
                size: own_px,
            };
            d.position = c.underline_pos.unwrap_or(0);
            d.over_lang = over_lang;
        }
    }
    if let Some(lines) = c.td_lines.filter(|l| *l != 0)
        && own.display != Some(D::Contents)
    {
        let family = crate::style::cascade::compute::decor_family(c, vcx);
        let resolve =
            |l| crate::style::cascade::compute::resolve_decor_len(l, &family, own_px, vcx);
        let thickness = match c.td_thickness.map(resolve).unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            t => t,
        };
        let offset = match c.underline_offset.unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            DecorLen::Px(v) => DecorLen::Px(v),
            _ => DecorLen::Auto,
        };
        let inset = match c.td_inset {
            Some(None) => None,
            Some(Some(pair)) => Some(pair),
            None => Some([DecorLen::Px(0.0); 2]),
        };
        let d = Decor {
            lines,
            style: c.td_style.unwrap_or_default(),
            color: c.used_color(&c.td_color),
            thickness,
            offset,
            position: c.underline_pos.unwrap_or(0),
            inset,
            clone: own.bdb_clone,
            source: own.decor_box,
            rtl: c.wm().rtl(),
            over_lang,
            font: DecorFont {
                family: c.font_family.clone(),
                monospace: c.monospace,
                weight: c.font_weight,
                italic: c.italic,
                stretch: c.font_stretch,
                size: own_px,
            },
        };
        // Повторное слияние того же стиля (`inherit(merged, own)`) не должно
        // класть линию второй раз.
        if c.decors.last() != Some(&d) {
            c.decors.push(d);
        }
    }
    c.underline = Some(c.decors.iter().any(|d| d.lines & DECOR_UNDER != 0));
    c.line_through = Some(c.decors.iter().any(|d| d.lines & DECOR_THROUGH != 0));
}
