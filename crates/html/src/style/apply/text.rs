//! Текст: шрифт, цвет, выравнивание (apply_text).

use crate::style::computed::{Computed, TextAlign};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px, relative};

/// Текстовые свойства коробки: шрифт, кегль, цвет, начертание.
///
/// Открыта наружу для маркера списка: он рисуется отдельной коробкой и
/// свойства пункта сам не получает.
pub fn apply_text(mut d: Div, c: &Computed) -> Div {
    // Оформление текста нужно и на блоке: в ветке «строка из кусков» текст
    // рисуется обычными `div`, и подчёркивание, живущее только в прогонах,
    // там пропадало.
    if c.underline == Some(true) {
        d.style()
            .text
            .underline = Some(gpui::UnderlineStyle {
            thickness: px(1.),
            color: c.color.map(|col| col.to_hsla()),
            wavy: false,
        });
    }
    if c.line_through == Some(true) {
        d.style()
            .text
            .strikethrough = Some(gpui::StrikethroughStyle {
            thickness: px(1.),
            color: c.color.map(|col| col.to_hsla()),
        });
    }
    // Возможности шрифта: капитель, старостильные цифры, ширина начертания —
    // всё это таблицы OpenType, и GPUI умеет их включать.
    if let Some(family) = c.font_family.as_ref().filter(|f| !f.is_empty()) {
        // Имя из разметки — придуманное (`@font-face`): в набор обязано уйти
        // имя, под которым файл знает система шрифтов. Без подмены весь
        // текст, идущий гpui-раскладкой (не резчиком), набирался подменным
        // системным шрифтом.
        d = d.font_family(
            crate::text::fonts::alias_stretch(family, c.font_stretch)
                .unwrap_or_else(|| family.clone()),
        );
    }
    if let Some(pct) = c.font_stretch {
        d.style()
            .text
            .font_stretch = Some(gpui::FontStretch::from_percent(pct));
    }
    let features = c.used_features();
    if !features.is_empty() {
        d.style()
            .text
            .font_features = Some(gpui::FontFeatures(std::sync::Arc::new(features)));
    }
    if let Some(col) = c.color {
        d = d.text_color(col.to_hsla());
    }
    if let Some(Len::Px(size)) = c.font_size {
        d = d.text_size(px(size));
    }
    if let Some(w) = c.font_weight {
        d = d.font_weight(gpui::FontWeight(w as f32));
    }
    if c.italic == Some(true) {
        d = d.italic();
    }
    if let Some(lh) = c.line_height {
        d = match lh {
            Len::Px(v) => d.line_height(px(v)),
            Len::Pct(mult) => d.line_height(relative(mult)),
            Len::Em(k) => d.line_height(px(k * 16.0)),
            l @ (Len::EmPx(..) | Len::Ch(_) | Len::Ic(_) | Len::Ex(_)) => d.line_height(px(
                crate::text::metrics::fallback_len_px(l, "", 16.0).unwrap_or(16.0),
            )),
            Len::Lh(k) | Len::LhPx(k, _) => d.line_height(relative(k)),
            Len::Vw(k) | Len::Vh(k) => d.line_height(relative(k)),
            // `anchor()` в `line-height` не бывает (css-anchor-position-1 §anchor-fn:
            // только вставки) — как незнакомая длина, без сдвига.
            Len::Calc(_)
            | Len::Auto
            | Len::MinContent
            | Len::MaxContent
            | Len::FitContent
            | Len::Anchor(_) => d,
        };
    }
    if c.nowrap == Some(true) {
        d = d.whitespace_nowrap();
    }
    // Выравнивание текста разбиралось, но до элемента не доходило — поле
    // оставалось мёртвым, и `text-align: center` не делал ничего.
    match c.text_align {
        Some(TextAlign::Center) => d = d.text_center(),
        Some(TextAlign::Right) => d = d.text_right(),
        Some(TextAlign::Left) => d = d.text_left(),
        Some(TextAlign::Justify) => d = d.text_align(gpui::TextAlign::Justify),
        // Логические края — по НАПРАВЛЕНИЮ ПИСЬМА: `end` при rtl — левый
        // край (text-align-end-001: текст уходил вправо).
        Some(TextAlign::Start) => {
            d = if c.rtl == Some(true) {
                d.text_right()
            } else {
                d.text_left()
            }
        }
        Some(TextAlign::End) => {
            d = if c.rtl == Some(true) {
                d.text_left()
            } else {
                d.text_right()
            }
        }
        None => {}
    }
    if c.monospace == Some(true) {
        d = d.font_family(crate::text::metrics::mono_family_for(c.lang.as_deref()));
    }
    if let Some(Len::Px(v)) = c.letter_spacing {
        d = d.letter_spacing(px(v));
    }
    // `text-overflow: ellipsis` действует на БЛОК-КОНТЕЙНЕРЕ с overflow,
    // отличным от visible (css-overflow-3 §text-overflow) — слитый стиль
    // растаскивал его на текстовые куски, и «…» дорисовывался даже там,
    // где текст помещался.
    if c.ellipsis == Some(true)
        && c.overflow_x
            .is_some_and(|o| o != crate::style::computed::Overflow::Visible)
    {
        // Строковый маркер `text-overflow: "…текст…"` рисуется вместо
        // многоточия (css-overflow-4): gpui умеет любой текст усечения.
        d = match &c.overflow_marker {
            Some(m) => d.text_overflow(gpui::TextOverflow::Truncate(m.clone().into())),
            None => d.text_ellipsis(),
        };
    }
    d
}
