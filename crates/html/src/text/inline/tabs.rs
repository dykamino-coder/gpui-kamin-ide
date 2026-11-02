//! Позиции табуляции абзаца (`tab-size`, css-text-3 §4.2): шаг по умолчанию от шрифта,
//! `letter-spacing` и `word-spacing` блочного контейнера (ширина пробела струта, не куска),
//! свой шаг для кусков с табуляцией по их `tab-size`, минимальный зазор `0.5ch`.
//! На выходе `TabStops` для раскладки строк. Без знака табуляции в кусках абзаца
//! позиции не читаются, и метрики шрифта (шейпинг пробела) не запрашиваются вовсе.

use super::Piece;
use crate::defaults::MEDIUM_FONT_SIZE_PX;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::metrics;
use crate::text::paragraph::tabs::TabStops;

pub fn tab_stops(
    pieces: &[Piece],
    block: &Computed,
    base: &gpui::TextStyle,
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: crate::values::ValueCx<'_>,
) -> TabStops {
    // Позиции читает только сам знак табуляции (Blink `Font::TabWidth` — лишь
    // при встрече `\t`); текст абзаца — это его текстовые куски.
    if !pieces
        .iter()
        .any(|p| matches!(p, Piece::Text { text, .. } if text.contains('\t')))
    {
        return TabStops::uniform(0.0);
    }
    let size = block
        .used_font(
            f32::from(base.font_size.to_pixels(gpui::px(MEDIUM_FONT_SIZE_PX))),
            vcx,
        )
        .size_px;
    let family = block.inh.font.font_family.as_deref().unwrap_or_else(|| {
        if block.inh.font.monospace == Some(true) {
            metrics::mono_family_for(block.inh.text_flow.lang.as_deref(), vcx)
        } else {
            ""
        }
    });
    let font = super::strut_font(block, base, vcx);
    let space = metrics::space_advance(&font, size, fonts)
        .unwrap_or_else(|| metrics::ch_ex_px(family, size, fonts, vcx).0)
        + metrics::spacing_px(block.inh.text_flow.letter_spacing, family, size, vcx)
        + metrics::spacing_px(block.inh.text_flow.word_spacing, family, size, vcx);
    let step = |style: &Computed| match style.inh.text_flow.tab_size_len {
        Some(Len::Px(v)) if v >= 0.0 => v,
        _ => style.inh.text_flow.tab_size.unwrap_or(8.0).max(0.0) * space,
    };
    let mut stops = TabStops::uniform(step(block));
    // §4.2: позиция ближе `0.5ch` пропускается — следующая.
    stops.min_gap = 0.5 * metrics::ch_ex_px(family, size, fonts, vcx).0;
    let mut byte = 0;
    for piece in pieces {
        if let Piece::Text { text, style } = piece {
            let end = byte + text.len();
            if text.contains('\t') {
                stops.spans.push((byte..end, step(style)));
            }
            byte = end;
        }
    }
    stops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::BoxStyle;

    #[test]
    fn numeric_tabs_use_shaped_spaces_with_the_block_font() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        metrics::install_space_probe(
            |font, size| {
                assert_eq!(font.weight, gpui::FontWeight(700.0));
                size * 0.25
            },
            &doc.text,
            vcx,
        );
        let block = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.inh.font.font_size = Some(Len::Px(20.0));
            with_inherited.inh.font.font_weight = Some(700);
            with_inherited.inh.text_flow.letter_spacing = Some(Len::Px(2.0));
            with_inherited.inh.text_flow.word_spacing = Some(Len::Px(3.0));
            with_inherited.inh.text_flow.tab_size = Some(4.0);
            with_inherited
        };
        let pieces = vec![Piece::Text {
            text: "\t".into(),
            style: {
                let mut with_inherited = Computed {
                    ..Default::default()
                };
                with_inherited.inh.font.font_size = Some(Len::Px(80.0));
                with_inherited.inh.text_flow.tab_size = Some(4.0);
                BoxStyle::from(with_inherited)
            },
        }];
        let stops = tab_stops(&pieces, &block, &gpui::TextStyle::default(), &doc.text, vcx);
        assert_eq!(stops.default, 40.0);
        assert_eq!(stops.next(0, 0.0), 40.0);
    }

    #[test]
    fn text_without_tabs_does_not_measure_the_font() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        metrics::install_space_probe(|_, _| panic!("space shaped without a tab"), &doc.text, vcx);
        let block = Computed::default();
        let pieces = vec![Piece::Text {
            text: "no tabs here".into(),
            style: BoxStyle::from(block.clone()),
        }];
        let stops = tab_stops(&pieces, &block, &gpui::TextStyle::default(), &doc.text, vcx);
        assert!(stops.spans.is_empty());
    }

    #[test]
    fn generic_monospace_uses_the_same_family_as_the_text_run() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        let expected = metrics::mono_family_for(None, vcx).to_string();
        metrics::install_probe(
            move |family, size| {
                let ch = if family == expected { 0.75 } else { 0.25 };
                (size * ch, size * 0.5, size, size)
            },
            &doc.text,
            vcx,
        );
        let block = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.inh.font.font_size = Some(Len::Px(20.0));
            with_inherited.inh.font.monospace = Some(true);
            with_inherited.inh.text_flow.tab_size = Some(2.0);
            with_inherited
        };
        let pieces = vec![Piece::Text {
            text: "\t".into(),
            style: BoxStyle::from(block.clone()),
        }];
        assert_eq!(
            tab_stops(&pieces, &block, &gpui::TextStyle::default(), &doc.text, vcx).next(0, 0.0),
            30.0
        );
    }

    #[test]
    fn numeric_override_cancels_an_inherited_length() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.inh.text_flow.tab_size_len = Some(Len::Px(200.0));
            with_inherited
        };
        let own = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.inh.text_flow.tab_size = Some(5.0);
            with_inherited
        };
        let child = crate::style::cascade::inherit::inherit(&BoxStyle::from(parent), &own, vcx);
        assert_eq!(child.inh.text_flow.tab_size, Some(5.0));
        assert_eq!(child.inh.text_flow.tab_size_len, None);
    }

    #[test]
    fn inline_font_does_not_replace_the_block_metrics() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        let block = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.inh.font.font_size = Some(Len::Px(20.0));
            with_inherited.inh.text_flow.letter_spacing = Some(Len::Px(2.0));
            with_inherited.inh.text_flow.word_spacing = Some(Len::Px(3.0));
            with_inherited.inh.text_flow.tab_size = Some(2.0);
            with_inherited
        };
        let pieces = vec![Piece::Text {
            text: "水\t".into(),
            style: {
                let mut with_inherited = Computed {
                    ..Default::default()
                };
                with_inherited.inh.font.font_size = Some(Len::Px(80.0));
                with_inherited.inh.text_flow.tab_size = Some(5.0);
                BoxStyle::from(with_inherited)
            },
        }];
        let stops = tab_stops(&pieces, &block, &gpui::TextStyle::default(), &doc.text, vcx);
        assert_eq!(stops.default, 30.0);
        assert_eq!(stops.next(3, 0.0), 75.0);
    }
}
