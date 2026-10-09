//! Paint emphasis marks centered on character advances before added spacing.

use super::*;

impl Paragraph {
    /// Знаки акцента прогона (css-text-decor-3 §5.3): «drawn exactly as if
    /// each character was assigned the mark as its ruby annotation text …
    /// and the ruby alignment as centered». Как `<rt>` над базой руби
    /// (`render.rs`, рукав контейнера руби): коробка аннотации стоит на краю
    /// строчной коробки базы (её `line-height`, полулидинг от подъёма и
    /// спуска шрифта базы), сама она — строка кегля знака с `line-height: 1`
    /// (UA `rt`), знак — по центру продвижения своего знака базы.
    /// Пропускаются разделители (Z*), управляющие (Cc, Cf, Cn) и пунктуация
    /// (P*), кроме перечисленных в §5.3 знаков.
    pub(super) fn paint_emphasis(
        &self,
        run: &std::ops::Range<usize>,
        shaped: &gpui::ShapedLine,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let src = &self.text[run.clone()];
        // Индексы набора совпадают с байтами абзаца со сдвигом на обёртку
        // направления (`controlled_shape`), только если текст прогона набран
        // как есть (без выброшенных управляющих знаков).
        let Some(lead) = shaped.text.as_ref().find(src) else {
            return;
        };
        if self.run_metrics.len() != self.runs.len() {
            return;
        }
        let baseline =
            at.y + (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        for span in &self.emph_spans {
            let (s, e) = (span.range.start.max(run.start), span.range.end.min(run.end));
            if s >= e {
                continue;
            }
            let mut acc = 0usize;
            let found = self.runs.iter().zip(&self.run_metrics).find_map(|(r, m)| {
                let st = acc;
                acc += r.len;
                (s >= st && s < acc).then_some((r, *m))
            });
            let Some((base_run, (a, d))) = found else {
                continue;
            };
            let half = (span.line_height - a - d) / 2.0;
            let box_top = baseline - px(a + half);
            let mark_lh = px(span.size);
            let top = if span.under {
                box_top + px(span.line_height)
            } else {
                box_top - mark_lh
            };
            let mark_run = TextRun {
                len: span.mark.len(),
                font: base_run.font.clone(),
                font_size: None,
                color: span.color.unwrap_or(base_run.color),
                background_color: None,
                background_pad: Default::default(),
                background_radius: px(0.),
                background_border: None,
                underline: None,
                strikethrough: None,
            };
            let mark = window.text_system().with_ligature_breaking(false).shape_line(
                SharedString::from(span.mark.clone()),
                px(span.size),
                &[mark_run],
                None,
            );
            // Та же привязка базовой линии к точке устройства, что у строки
            // (`text_raster_origin`), но от высоты строки знака: like an
            // `<rt>` paragraph's glyphs, the mark keeps the opaque ancestor
            // fill's device frame and the paragraph's sub-pixel glyph offset
            // that `Window::paint_glyph` adds afterwards.
            let exact = mark
                .runs
                .iter()
                .all(|r| window.text_system().pixel_exact_glyphs(r.font_id));
            let base = (mark_lh - mark.ascent - mark.descent) / 2.0 + mark.ascent;
            let raster = if exact {
                point(at.x, top)
            } else {
                self.raster_origin_for_baseline(point(at.x, top), base, window)
            };
            let (dx, y) = (raster.x - at.x, raster.y);
            for (i, c) in self.text[s..e].char_indices() {
                if !emphasized(c) {
                    continue;
                }
                let off = lead + s - run.start + i;
                let x0 = shaped.x_for_index(off);
                let x1 = shaped.x_for_index(off + c.len_utf8());
                // CSS Text Decoration 3 section 5.3 centers the annotation
                // on its character, not on the extra inter-character spacing.
                let spacing = self.tail_spacing(s + i + c.len_utf8());
                let x = at.x + dx + x0 + (x1 - x0 - spacing - mark.width) / 2.0;
                let _ = mark.paint(point(x, y), mark_lh, gpui::TextAlign::Left, None, window, cx);
            }
        }
    }
}
