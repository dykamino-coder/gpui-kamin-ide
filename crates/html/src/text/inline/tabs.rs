//! Resolve each inline tab-size against the block container's font and spacing.

use super::Piece;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::metrics;
use crate::text::paragraph::tabs::TabStops;

pub fn tab_stops(pieces: &[Piece], block: &Computed, base: &gpui::TextStyle) -> TabStops {
    let size = match block.font_size {
        Some(Len::Px(v)) => v,
        _ => f32::from(base.font_size.to_pixels(gpui::px(16.0))),
    };
    let family = block.font_family.as_deref().unwrap_or_else(|| {
        if block.monospace == Some(true) {
            metrics::mono_family_for(block.lang.as_deref())
        } else {
            ""
        }
    });
    let font = super::strut_font(block, base);
    let space = metrics::space_advance(&font, size)
        .unwrap_or_else(|| metrics::ch_ex_px(family, size).0)
        + metrics::spacing_px(block.letter_spacing, family, size)
        + metrics::spacing_px(block.word_spacing, family, size);
    let step = |style: &Computed| match style.tab_size_len {
        Some(Len::Px(v)) if v >= 0.0 => v,
        _ => style.tab_size.unwrap_or(8.0).max(0.0) * space,
    };
    let mut stops = TabStops::uniform(step(block));
    // §4.2: позиция ближе `0.5ch` пропускается — следующая.
    stops.min_gap = 0.5 * metrics::ch_ex_px(family, size).0;
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

    #[test]
    fn numeric_tabs_use_shaped_spaces_with_the_block_font() {
        metrics::install_space_probe(|font, size| {
            assert_eq!(font.weight, gpui::FontWeight(700.0));
            size * 0.25
        });
        let block = Computed {
            font_size: Some(Len::Px(20.0)),
            font_weight: Some(700),
            letter_spacing: Some(Len::Px(2.0)),
            word_spacing: Some(Len::Px(3.0)),
            tab_size: Some(4.0),
            ..Default::default()
        };
        let pieces = vec![Piece::Text {
            text: "\t".into(),
            style: Computed {
                font_size: Some(Len::Px(80.0)),
                tab_size: Some(4.0),
                ..Default::default()
            },
        }];
        let stops = tab_stops(&pieces, &block, &gpui::TextStyle::default());
        assert_eq!(stops.default, 40.0);
        assert_eq!(stops.next(0, 0.0), 40.0);
    }

    #[test]
    fn generic_monospace_uses_the_same_family_as_the_text_run() {
        let expected = metrics::mono_family_for(None).to_string();
        metrics::install_probe(move |family, size| {
            let ch = if family == expected { 0.75 } else { 0.25 };
            (size * ch, size * 0.5, size, size)
        });
        let block = Computed {
            font_size: Some(Len::Px(20.0)),
            monospace: Some(true),
            tab_size: Some(2.0),
            ..Default::default()
        };
        let pieces = vec![Piece::Text {
            text: "\t".into(),
            style: block.clone(),
        }];
        assert_eq!(
            tab_stops(&pieces, &block, &gpui::TextStyle::default()).next(0, 0.0),
            30.0
        );
    }

    #[test]
    fn numeric_override_cancels_an_inherited_length() {
        let parent = Computed {
            tab_size_len: Some(Len::Px(200.0)),
            ..Default::default()
        };
        let own = Computed {
            tab_size: Some(5.0),
            ..Default::default()
        };
        let child = crate::style::cascade::inherit::inherit(&parent, &own);
        assert_eq!(child.tab_size, Some(5.0));
        assert_eq!(child.tab_size_len, None);
    }

    #[test]
    fn inline_font_does_not_replace_the_block_metrics() {
        let block = Computed {
            font_size: Some(Len::Px(20.0)),
            letter_spacing: Some(Len::Px(2.0)),
            word_spacing: Some(Len::Px(3.0)),
            tab_size: Some(2.0),
            ..Default::default()
        };
        let pieces = vec![Piece::Text {
            text: "水\t".into(),
            style: Computed {
                font_size: Some(Len::Px(80.0)),
                tab_size: Some(5.0),
                ..Default::default()
            },
        }];
        let stops = tab_stops(&pieces, &block, &gpui::TextStyle::default());
        assert_eq!(stops.default, 30.0);
        assert_eq!(stops.next(3, 0.0), 75.0);
    }
}
