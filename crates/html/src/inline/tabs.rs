//! Resolve each inline tab-size against the block container's font and spacing.

use super::Piece;
use crate::{computed::Computed, lines::tabs::TabStops, metrics, value::Len};

pub fn tab_stops(pieces: &[Piece], block: &Computed, base_size: f32) -> TabStops {
    let size = match block.font_size {
        Some(Len::Px(v)) => v,
        _ => base_size,
    };
    let family = block.font_family.as_deref().unwrap_or_else(|| {
        if block.monospace == Some(true) {
            metrics::mono_family_for(block.lang.as_deref())
        } else {
            ""
        }
    });
    let space = metrics::ch_ex_px(family, size).0
        + metrics::spacing_px(block.letter_spacing, family, size)
        + metrics::spacing_px(block.word_spacing, family, size);
    let step = |style: &Computed| match style.tab_size_len {
        Some(Len::Px(v)) if v >= 0.0 => v,
        _ => style.tab_size.unwrap_or(8.0).max(0.0) * space,
    };
    let mut stops = TabStops::uniform(step(block));
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
        assert_eq!(tab_stops(&pieces, &block, 20.0).next(0, 0.0), 30.0);
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
        let child = super::super::inherit(&parent, &own);
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
        let stops = tab_stops(&pieces, &block, 80.0);
        assert_eq!(stops.default, 30.0);
        assert_eq!(stops.next(3, 0.0), 75.0);
    }
}
