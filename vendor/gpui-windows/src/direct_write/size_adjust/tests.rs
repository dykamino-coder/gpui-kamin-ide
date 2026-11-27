//! Native DirectWrite shaping with different primary and fallback metrics.

use super::super::DirectWriteTextSystem;
use super::aspect;
use anyhow::Result;
use gpui::{
    FontFallbacks, FontId, FontRun, FontSizeAdjustment, FontSizeMetric, GlyphId, LineLayout,
    PlatformTextSystem, px,
};
use std::borrow::Cow;

fn system() -> Result<DirectWriteTextSystem> {
    let system = DirectWriteTextSystem::new(None)?;
    system.add_fonts(vec![
        Cow::Borrowed(include_bytes!(
            "../../../../gpui/tests/fonts/IBMPlexSans-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../../gpui/tests/fonts/Lilex-Regular.ttf"
        )),
    ])?;
    Ok(system)
}

fn run(id: FontId, text: &str, size: f32) -> FontRun {
    FontRun {
        len: text.len(),
        font_id: id,
        font_size: px(size),
        break_ligatures: false,
    }
}

#[test]
fn each_fallback_uses_its_own_metric_before_shaping() -> Result<()> {
    let system = system()?;
    let primary = system.font_id(&gpui::font("IBM Plex Sans"))?;
    let fallback = system.font_id(&gpui::font("Lilex"))?;
    // U+01EA exists in Lilex but not IBM Plex Sans: selection is independent
    // of the installed system families.
    let text = "Ǫ";
    assert_eq!(system.glyph_for_char(primary, 'Ǫ'), Some(GlyphId(0)));
    assert_ne!(system.glyph_for_char(fallback, 'Ǫ'), Some(GlyphId(0)));
    for metric in [
        FontSizeMetric::XHeight,
        FontSizeMetric::CapHeight,
        FontSizeMetric::ZeroAdvance,
        FontSizeMetric::IdeographicWidth,
        FontSizeMetric::IdeographicHeight,
    ] {
        let raw = |id: FontId| -> Result<f32> {
            let state = system.state.read();
            let info = &state.fonts[id.0];
            let font = unsafe { info.font_collection.GetFontFromFontFace(&info.font_face)? };
            Ok(aspect(&font, metric)?)
        };
        let primary_aspect = raw(primary)?;
        let fallback_aspect = raw(fallback)?;
        let used_size = 20.0 * (0.5 / primary_aspect);
        let mut font = gpui::font("IBM Plex Sans");
        font.fallbacks = Some(
            FontFallbacks::from_fonts(vec!["Lilex".into()]).with_size_adjustment(
                FontSizeAdjustment {
                    metric,
                    aspect: primary_aspect,
                },
            ),
        );
        let adjusted = system.font_id(&font)?;
        let actual = system.layout_line(text, px(used_size), &[run(adjusted, text, used_size)]);
        let size = used_size * (primary_aspect / fallback_aspect);
        let expected = system.layout_line(text, px(size), &[run(fallback, text, size)]);
        assert!(!actual.runs.is_empty());
        assert_eq!(actual.runs[0].font_size, px(size), "{metric:?}");
        assert_eq!(actual.width, expected.width, "{metric:?}");
        let glyphs = |line: &LineLayout| {
            line.runs[0]
                .glyphs
                .iter()
                .map(|glyph| (glyph.id, glyph.position, glyph.index, glyph.is_emoji))
                .collect::<Vec<_>>()
        };
        assert_eq!(glyphs(&actual), glyphs(&expected), "{metric:?}");
    }
    Ok(())
}

#[test]
fn normalization_stays_in_its_own_run() -> Result<()> {
    let system = system()?;
    let mut plain = gpui::font("IBM Plex Sans");
    plain.fallbacks = Some(FontFallbacks::from_fonts(vec!["Lilex".into()]));
    let mut adjusted = plain.clone();
    adjusted.fallbacks = Some(
        plain
            .fallbacks
            .as_ref()
            .unwrap()
            .clone()
            .with_size_adjustment(FontSizeAdjustment {
                metric: FontSizeMetric::XHeight,
                aspect: 1.0,
            }),
    );
    let plain = system.font_id(&plain)?;
    let adjusted = system.font_id(&adjusted)?;
    let first = system.layout_line("Ǫ", px(20.), &[run(plain, "Ǫ", 20.)]);
    let second = system.layout_line("Ǫ", px(20.), &[run(adjusted, "Ǫ", 20.)]);
    let mixed = system.layout_line(
        "ǪǪǪ",
        px(20.),
        &[
            run(plain, "Ǫ", 20.),
            run(adjusted, "Ǫ", 20.),
            run(plain, "Ǫ", 20.),
        ],
    );
    let sizes: Vec<_> = mixed.runs.iter().map(|run| run.font_size).collect();
    assert_eq!(
        sizes,
        vec![
            first.runs[0].font_size,
            second.runs[0].font_size,
            first.runs[0].font_size
        ]
    );
    assert_ne!(sizes[0], sizes[1]);
    Ok(())
}
