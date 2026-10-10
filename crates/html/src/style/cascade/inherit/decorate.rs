//! Оформление текста при наследовании: линии text-decoration, их цвет, толщина и смещение, распространение на потомков.

use super::*;

/// Украшения текста (css-text-decor-3 §2.1): линии не наследуются, а
/// РАСПРОСТРАНЯЮТСЯ на всех потомков в потоке, кроме атомарных строчных
/// (`inline-block`, `inline-table`…) и вынесенных из потока (флоаты,
/// абсолюты); цвет, рисунок, толщина и метрики — от украшающей коробки.
/// `display: contents` коробки не даёт и своих линий не кладёт.
pub(super) fn decorate(parent: &Computed, own: &Computed, c: &mut Computed, own_px: f32) {
    use crate::style::computed::Display as D;
    use crate::style::computed::Position as P;
    use crate::style::computed::{DECOR_THROUGH, DECOR_UNDER, Decor, DecorFont, DecorLen};
    // Семейство для `ch`/`ex` — как у `resolve_em`: родовое `monospace`
    // имени не даёт, а меряться должно тем шрифтом, которым набран текст.
    let family = c.font_family.clone().unwrap_or_else(|| {
        if c.monospace == Some(true) {
            crate::text::metrics::mono_family_for(c.lang.as_deref()).to_string()
        } else {
            String::new()
        }
    });
    let resolve = |l: DecorLen| match l {
        DecorLen::Raw(raw) => crate::text::metrics::fallback_len_px(raw, &family, own_px)
            .map_or(DecorLen::Auto, DecorLen::Px),
        other => other,
    };
    // Наследуемое смещение — вычисленной длиной (доля остаётся долей).
    c.underline_offset = own
        .underline_offset
        .map(resolve)
        .or(parent.underline_offset);
    c.underline_pos = own.underline_pos.or(parent.underline_pos);
    let over_lang = c.lang.as_deref().is_some_and(|l| {
        let l = l.to_ascii_lowercase();
        ["ja", "ko", "mn"]
            .iter()
            .any(|p| l == *p || l.starts_with(&format!("{p}-")))
    });
    let blocked = matches!(own.position, Some(P::Absolute) | Some(P::Fixed))
        || own.float.is_some_and(|f| f != 0)
        || matches!(
            own.display,
            Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
        );
    c.skip_ink = own.skip_ink.or(parent.skip_ink);
    c.skip_spaces = own.skip_spaces.or(parent.skip_spaces);
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
    if let Some(lines) = own.td_lines.filter(|l| *l != 0)
        && own.display != Some(D::Contents)
    {
        let black = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let thickness = match own.td_thickness.map(resolve).unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            t => t,
        };
        let offset = match c.underline_offset.unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            DecorLen::Px(v) => DecorLen::Px(v),
            _ => DecorLen::Auto,
        };
        let inset = match own.td_inset {
            Some(None) => None,
            Some(Some(pair)) => Some(pair.map(resolve)),
            None => Some([DecorLen::Px(0.0); 2]),
        };
        let d = Decor {
            lines,
            style: own.td_style.unwrap_or_default(),
            color: own.td_color.or(c.color).unwrap_or(black),
            thickness,
            offset,
            position: c.underline_pos.unwrap_or(0),
            inset,
            clone: own.bdb_clone,
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
