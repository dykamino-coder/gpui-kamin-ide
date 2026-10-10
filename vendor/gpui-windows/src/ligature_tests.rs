//! Regression coverage for IDE style boundaries and caller-local browser shaping.

use super::direct_write::DirectWriteTextSystem;
use anyhow::Result;
use gpui::{
    FontFeatures, FontId, GlyphId, Pixels, PlatformTextSystem, Point, ShapedRun, TextRun,
    TextSystem, WindowTextSystem, font, px, rgb,
};
use std::{borrow::Cow, sync::Arc};

#[test]
fn ligature_policy_is_local_and_separates_cached_layouts() -> Result<()> {
    let platform = Arc::new(DirectWriteTextSystem::new(None)?);
    // Exercise the programming ligatures in the font actually shipped by the IDE.
    platform.add_fonts(vec![Cow::Borrowed(include_bytes!(
        "../../../crates/shell/assets/fonts/JetBrainsMono-Variable.ttf"
    ))])?;
    let text_system = WindowTextSystem::new(Arc::new(TextSystem::new(platform)));
    let browser = text_system.with_ligature_breaking(false);
    let mut font = font("JetBrains Mono");
    font.features = FontFeatures(Arc::new(vec![("calt".into(), 1)]));
    let base = TextRun {
        font,
        color: rgb(0x000000).into(),
        ..Default::default()
    };

    for text in ["->", "=>", "!="] {
        let runs = [
            TextRun {
                len: 1,
                ..base.clone()
            },
            TextRun {
                len: 1,
                color: rgb(0xff0000).into(),
                ..base.clone()
            },
        ];
        let whole = [TextRun {
            len: text.len(),
            ..base.clone()
        }];
        let reference = browser.layout_line(text, px(16.), &whole, None);
        let joined = browser.layout_line(text, px(16.), &runs, None);
        assert_eq!(
            glyphs(&joined.runs),
            glyphs(&reference.runs),
            "browser: {text}"
        );
        let same_style = [
            TextRun {
                len: 1,
                ..base.clone()
            },
            TextRun {
                len: 1,
                ..base.clone()
            },
        ];
        assert_eq!(
            glyphs(
                &text_system
                    .layout_line(text, px(16.), &same_style, None)
                    .runs
            ),
            glyphs(&reference.runs),
            "identical styles must retain ligatures: {text}",
        );
        let broken = text_system.layout_line(text, px(16.), &runs, None);
        assert_ne!(ids(&broken.runs), ids(&joined.runs), "IDE: {text}");
        assert!(
            broken
                .runs
                .iter()
                .any(|run| run.font_size == px(16_f32.next_up()))
        );
        assert!(Arc::ptr_eq(
            &joined,
            &browser.layout_line(text, px(16.), &runs, None)
        ));
        assert!(Arc::ptr_eq(
            &broken,
            &text_system.layout_line(text, px(16.), &runs, None)
        ));

        // The editor's hashed path must use the same boundaries and cache policy.
        let hash = text
            .as_bytes()
            .iter()
            .fold(0u64, |hash, byte| hash * 257 + u64::from(*byte));
        let hashed =
            text_system.layout_line_by_hash(hash, text.len(), px(16.), &runs, None, || text.into());
        assert_eq!(glyphs(&hashed.runs), glyphs(&broken.runs));
        assert!(
            browser
                .try_layout_line_by_hash(hash, text.len(), px(16.), &runs, None)
                .is_none()
        );
        let joined_hash =
            browser.layout_line_by_hash(hash, text.len(), px(16.), &runs, None, || text.into());
        assert_eq!(glyphs(&joined_hash.runs), glyphs(&joined.runs));

        let wrapped = text_system.shape_text(text.into(), px(16.), &runs, None, None)?;
        assert_eq!(
            glyphs(&wrapped[0].unwrapped_layout.runs),
            glyphs(&broken.runs)
        );
        let wrapped_browser = browser.shape_text(text.into(), px(16.), &runs, None, None)?;
        assert_eq!(
            glyphs(&wrapped_browser[0].unwrapped_layout.runs),
            glyphs(&joined.runs)
        );
    }
    // Different sizes force the SAME run boundaries in both modes. Only the
    // policy in the cache key distinguishes these two layouts.
    let size_runs = [
        TextRun {
            len: 1,
            ..base.clone()
        },
        TextRun {
            len: 1,
            font_size: Some(px(24.)),
            ..base.clone()
        },
    ];
    let joined_sizes = browser.layout_line("ab", px(16.), &size_runs, None);
    let broken_sizes = text_system.layout_line("ab", px(16.), &size_runs, None);
    assert!(!Arc::ptr_eq(&joined_sizes, &broken_sizes));
    assert!(joined_sizes.runs.iter().any(|run| run.font_size == px(24.)));
    assert!(
        broken_sizes
            .runs
            .iter()
            .any(|run| run.font_size == px(24_f32.next_up()))
    );

    // Run sizes stay independent of the line size, and the nudge alternates.
    let runs = [
        TextRun {
            len: 2,
            ..base.clone()
        },
        TextRun {
            len: 1,
            font_size: Some(px(24.)),
            color: rgb(0xff0000).into(),
            ..base.clone()
        },
        TextRun {
            len: 1,
            font_size: Some(px(24.)),
            ..base
        },
    ];
    let mixed = text_system.layout_line("éab", px(16.), &runs, None);
    for (index, expected) in [(0, px(16.)), (2, px(24_f32.next_up())), (3, px(24.))] {
        assert!(
            mixed.runs.iter().any(|run| run.font_size == expected
                && run.glyphs.iter().any(|glyph| glyph.index == index))
        );
    }
    Ok(())
}

fn glyphs(runs: &[ShapedRun]) -> Vec<(FontId, Pixels, GlyphId, Point<Pixels>, usize)> {
    runs.iter()
        .flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                (
                    run.font_id,
                    run.font_size,
                    glyph.id,
                    glyph.position,
                    glyph.index,
                )
            })
        })
        .collect()
}

fn ids(runs: &[ShapedRun]) -> Vec<GlyphId> {
    runs.iter()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.id))
        .collect()
}
