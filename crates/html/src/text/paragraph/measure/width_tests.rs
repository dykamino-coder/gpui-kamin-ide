//! Положения знаков `x_at` на границах кусков: табуляция, мягкий перенос,
//! перевод строки и трекинг по кускам.

use crate::text::paragraph::*;
use gpui::px;

#[test]
fn indexed_positions_keep_tab_hyphen_and_newline_edge_semantics() {
    let text = "ab\tcd\u{ad}ef\ngh\n";
    for align in [Align::Left, Align::Justify] {
        let p = Paragraph::new(
            text.into(),
            vec![],
            px(16.),
            px(20.),
            align,
            Wrap::default(),
        )
        .letter_spans(vec![(0..text.len(), px(0.1)), (0..2, px(5.))])
        .word_spacing(px(0.3));
        let mut start = 0;
        let mut segs = Vec::new();
        for (end, ch) in text
            .char_indices()
            .filter(|(_, c)| matches!(*c, '\t' | '\n' | SOFT_HYPHEN))
            .chain([(text.len(), '\0')])
        {
            segs.push(Seg {
                start,
                end,
                offset: px(start as f32 * 3.7),
                positions: Default::default(),
                layout: std::sync::Arc::new(gpui::LineLayout {
                    len: end - start,
                    width: px((end - start) as f32 * 7.3),
                    ..Default::default()
                }),
            });
            start = end + ch.len_utf8();
        }
        for i in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
            for edge in [Edge::Start, Edge::End] {
                let after = edge == Edge::End && i > 0 && text.as_bytes()[i - 1] == b'\n';
                let seg = segs
                    .iter()
                    .find(|s| if after { s.end + 1 == i } else { i <= s.end })
                    .or_else(|| segs.last())
                    .unwrap();
                let glyph = if i >= seg.end {
                    seg.layout.width
                } else if i <= seg.start {
                    px(0.)
                } else {
                    seg.layout.x_for_index(i - seg.start)
                } + seg.offset;
                // Трекинг кусков — точная сумма, округлённая один раз.
                let mut tracking = 0f64;
                for (at, _) in text[seg.start..i.min(seg.end)].char_indices() {
                    if let Some((_, v)) = p
                        .letter_spans
                        .iter()
                        .find(|(r, _)| r.contains(&(seg.start + at)))
                    {
                        tracking += f64::from(f32::from(*v - p.letter_spacing));
                    }
                }
                let expected = if tracking == 0.0 {
                    glyph
                } else {
                    px((f64::from(f32::from(glyph)) + tracking) as f32)
                };
                assert_eq!(p.x_at(&segs, i, edge), expected, "{i}, after={after}");
            }
        }
    }
}
