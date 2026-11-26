//! Rasterized glyphs must keep their device-pixel phase when boxes move.

use super::reuse_tests::update;
use super::tests::{frame, open};
use gpui::{
    Bounds, DevicePixels, Font, FontId, FontMetrics, FontRun, GlyphId, LineLayout, NoopTextSystem,
    Pixels, PlatformTextSystem, RenderGlyphParams, Size, TestAppContext, TextRenderingMode,
    VisualTestContext, point, size,
};
use std::{borrow::Cow, sync::Arc};

// The default NoopTextSystem has empty raster bounds, so ordinary scene
// assertions never see a glyph. Keep its shaping but emit a one-pixel mask.
struct GlyphTextSystem;
impl PlatformTextSystem for GlyphTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
        NoopTextSystem.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        NoopTextSystem.all_font_names()
    }
    fn font_id(&self, font: &Font) -> gpui::Result<FontId> {
        NoopTextSystem.font_id(font)
    }
    fn font_metrics(&self, id: FontId) -> FontMetrics {
        NoopTextSystem.font_metrics(id)
    }
    fn typographic_bounds(&self, font: FontId, glyph: GlyphId) -> gpui::Result<Bounds<f32>> {
        NoopTextSystem.typographic_bounds(font, glyph)
    }
    fn advance(&self, font: FontId, glyph: GlyphId) -> gpui::Result<Size<f32>> {
        NoopTextSystem.advance(font, glyph)
    }
    fn glyph_for_char(&self, font: FontId, ch: char) -> Option<GlyphId> {
        NoopTextSystem.glyph_for_char(font, ch)
    }
    fn glyph_raster_bounds(&self, _: &RenderGlyphParams) -> gpui::Result<Bounds<DevicePixels>> {
        Ok(Bounds {
            origin: point(DevicePixels(0), DevicePixels(0)),
            size: size(DevicePixels(1), DevicePixels(1)),
        })
    }
    fn rasterize_glyph(
        &self,
        _: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> gpui::Result<(Size<DevicePixels>, Vec<u8>)> {
        Ok((bounds.size, vec![255]))
    }
    fn layout_line(&self, text: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
        NoopTextSystem.layout_line(text, size, runs)
    }
    fn recommended_rendering_mode(&self, font: FontId, size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(font, size)
    }
}

fn glyphs(cx: &mut VisualTestContext) -> Vec<[i32; 4]> {
    cx.update(|window, _| {
        let mut placed: Vec<_> = window
            .painted_sequence()
            .into_iter()
            .filter(|(kind, _)| kind.starts_with("glyph ") || kind.starts_with("subpixel "))
            .map(|(_, ink)| {
                [
                    ink.origin.x.0,
                    ink.origin.y.0,
                    ink.size.width.0,
                    ink.size.height.0,
                ]
                .map(|v| (v * 64.0).round() as i32)
            })
            .collect();
        placed.sort_unstable();
        placed
    })
}

const INLINE: &str = r#"<!doctype html><html><head><style>html,body{margin:0;background:#fff;color:#101820}body{padding:12px;font:20px/1.35 Arial}#flow{width:280px;margin:8px;border:1px solid #777}.band{background:#ffe080;padding:0}.red{background:#e04030}.blue{background:#3060d0}.clear{background:transparent}#flow.narrow{width:170px}.rtl{direction:rtl}.atom{display:inline-block}.small{font-size:17px}</style></head><body><div id="flow" class="" style="">before <span id="band" class="band ">old alpha beta gamma delta epsilon zeta eta theta</span> after trailing text</div><div>sentinel unchanged</div></body></html>"#;

fn ordered_page(step: usize, mixed: bool) -> String {
    let color = match step {
        1 | 3 | 5..=10 => "red",
        2 | 4 | 11 => "blue",
        _ => "",
    };
    let flow = match step {
        7 => "narrow",
        8 => "narrow rtl",
        9 | 10 => "narrow rtl small",
        _ => "",
    };
    let text = if (6..=9).contains(&step) {
        "changed alpha beta LONGER"
    } else {
        "old alpha beta gamma"
    };
    let page = INLINE
        .replace(
            "id=\"flow\" class=\"\"",
            &format!("id=\"flow\" class=\"{flow}\""),
        )
        .replace("class=\"band \"", &format!("class=\"band {color}\""))
        .replace("old alpha beta gamma", text);
    if mixed {
        page.replace(
            "before <span id=",
            "before <span style=\"display:contents\"><span id=",
        ).replace(
            "</span> after trailing",
            "</span></span><i style=\"display:inline-block;width:20px;height:15px;background:#090\"></i> after trailing",
        )
    } else {
        page
    }
}

fn compare_ordered_glyphs(cx: &mut TestAppContext, mixed: bool) {
    let mut app = TestAppContext::build_with_text_system(
        cx.dispatcher.clone(),
        None,
        Arc::new(GlyphTextSystem),
    );
    let (view, vcx) = open(&mut app, &ordered_page(0, mixed));
    frame(vcx);
    let mut incremental = Vec::new();
    for step in 1..=12 {
        update(&view, vcx, &ordered_page(step, mixed));
        incremental.push(glyphs(vcx));
    }
    assert!(incremental.iter().all(|glyphs| !glyphs.is_empty()));
    for (step, after) in (1..=12).zip(incremental) {
        let (_, fresh) = open(&mut app, &ordered_page(step, mixed));
        frame(fresh);
        assert_eq!(
            after,
            glyphs(fresh),
            "ordered edit {step} changed retained glyph placement"
        );
    }
}

/// Nested baseline flex and `min-content` grid items around one span, with
/// or without a background on the outermost item.
fn nested_items(depth: usize, background: bool) -> String {
    let style = "html{background:white}body{margin:0;width:780px;font:16px Arial}\
        *{box-sizing:border-box}.f{display:flex;align-items:baseline;min-width:0}\
        .g{display:grid;grid-template-columns:minmax(min-content,1fr);min-width:0}\
        span{padding:1px;border:1px solid #acd}.bg{background:#eee}";
    let mut body = "<span>min-content and max-content intrinsic measure WWWWWWWWWWW</span>".to_string();
    for level in 0..depth {
        let class = if background && level + 1 == depth { " class=bg" } else { "" };
        body = format!("<div class=f><div class=g>{body}</div><span{class}>side word</span></div>");
    }
    format!("<!doctype html><html><head><style>{style}</style></head><body>{body}</body></html>")
}

#[gpui::test]
fn a_background_does_not_move_the_glyphs_of_a_fresh_frame(cx: &mut TestAppContext) {
    let mut app = TestAppContext::build_with_text_system(
        cx.dispatcher.clone(),
        None,
        Arc::new(GlyphTextSystem),
    );
    for depth in [1, 2, 8, 24, 48] {
        let (_, plain) = open(&mut app, &nested_items(depth, false));
        frame(plain);
        let plain = glyphs(plain);
        let (_, painted) = open(&mut app, &nested_items(depth, true));
        frame(painted);
        assert!(!plain.is_empty());
        assert_eq!(plain, glyphs(painted), "depth {depth}");
    }
}

#[gpui::test]
fn ordered_inline_edits_match_fresh_glyphs(cx: &mut TestAppContext) {
    compare_ordered_glyphs(cx, false);
}

#[gpui::test]
fn ordered_mixed_contents_edits_match_fresh_glyphs(cx: &mut TestAppContext) {
    compare_ordered_glyphs(cx, true);
}
