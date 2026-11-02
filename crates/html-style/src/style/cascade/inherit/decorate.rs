//! Стадия коробки, оформление текста: линии text-decoration распространяются на потомков в потоке
//! (css-text-decor-3 §2.1) с цветом, толщиной, смещением и шрифтом украшающей коробки.

use super::*;
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;

/// Украшения текста (css-text-decor-3 §2.1): линии не наследуются, а
/// РАСПРОСТРАНЯЮТСЯ на всех потомков в потоке, кроме атомарных строчных
/// (`inline-block`, `inline-table`…) и вынесенных из потока (флоаты,
/// абсолюты); цвет, рисунок, толщина и метрики — от украшающей коробки.
/// `display: contents` коробки не даёт и своих линий не кладёт.
pub(super) fn decorate(
    input: crate::box_facts::DecorationInputs<'_, crate::style::computed::Decor>,
    own: &Computed,
    c: &mut BoxStyle,
    own_px: f32,
    vcx: ValueCx<'_>,
) {
    use crate::style::computed::{DECOR_THROUGH, DECOR_UNDER, Decor, DecorFont, DecorLen};
    let over_lang = c.inh.text_flow.lang.as_deref().is_some_and(|l| {
        let l = l.to_ascii_lowercase();
        ["ja", "ko", "mn"]
            .iter()
            .any(|p| l == *p || l.starts_with(&format!("{p}-")))
    });
    set!(c.reset_text_box.decors, input.inherited.to_vec());
    // Линии, пришедшие в блок, используют метрики его анонимной строчной коробки.
    if input.block {
        let font = DecorFont {
            family: c.inh.font.font_family.clone(),
            monospace: c.inh.font.monospace,
            weight: c.inh.font.font_weight,
            italic: c.inh.font.italic,
            stretch: c.inh.font.font_stretch,
            size: own_px,
        };
        let position = c.inh.text_ink.underline_pos.unwrap_or(0);
        if c.reset_text_box
            .decors
            .iter()
            .any(|d| d.font != font || d.position != position || d.over_lang != over_lang)
        {
            for d in c.reset_text_box.decors.iter_mut() {
                d.font = font.clone();
                d.position = position;
                d.over_lang = over_lang;
            }
        }
    }
    if let Some(lines) = c.reset_text_box.td_lines.filter(|l| *l != 0)
        && input.generates_box
    {
        let family = crate::style::cascade::compute::decor_family(c, vcx);
        let resolve =
            |l| crate::style::cascade::compute::resolve_decor_len(l, &family, own_px, vcx);
        let thickness = match c
            .reset_text_box
            .td_thickness
            .map(resolve)
            .unwrap_or_default()
        {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            t => t,
        };
        let offset = match c.inh.text_ink.underline_offset.unwrap_or_default() {
            DecorLen::Pct(k) => DecorLen::Px(k * own_px),
            DecorLen::Px(v) => DecorLen::Px(v),
            _ => DecorLen::Auto,
        };
        let inset = match c.reset_text_box.td_inset {
            Some(None) => None,
            Some(Some(pair)) => Some(pair),
            None => Some([DecorLen::Px(0.0); 2]),
        };
        let d = Decor {
            lines,
            style: c.reset_text_box.td_style.unwrap_or_default(),
            color: c.used_color(&c.reset_text_box.td_color),
            thickness,
            offset,
            position: c.inh.text_ink.underline_pos.unwrap_or(0),
            inset,
            clone: own.reset_fragmentation.bdb_clone,
            source: own.decor_box,
            rtl: c.wm().rtl(),
            over_lang,
            font: DecorFont {
                family: c.inh.font.font_family.clone(),
                monospace: c.inh.font.monospace,
                weight: c.inh.font.font_weight,
                italic: c.inh.font.italic,
                stretch: c.inh.font.font_stretch,
                size: own_px,
            },
        };
        // Повторное слияние того же стиля (`inherit(merged, own)`) не должно
        // класть линию второй раз.
        if c.reset_text_box.decors.last() != Some(&d) {
            c.reset_text_box.decors.push(d);
        }
    }
    // Отсутствие линии — `None` или `Some(false)`: равные по смыслу значения
    // стиль коробки не копируют.
    let under = c
        .reset_text_box
        .decors
        .iter()
        .any(|d| d.lines & DECOR_UNDER != 0);
    if c.reset_text_box.underline.unwrap_or(false) != under {
        c.reset_text_box.underline = Some(under);
    }
    let through = c
        .reset_text_box
        .decors
        .iter()
        .any(|d| d.lines & DECOR_THROUGH != 0);
    if c.reset_text_box.line_through.unwrap_or(false) != through {
        c.reset_text_box.line_through = Some(through);
    }
}
