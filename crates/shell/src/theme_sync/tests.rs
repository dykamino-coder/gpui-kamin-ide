//! Тесты resolve-порта contributed-тем: порядок elevation-ramp в dark/light,
//! синтез backdrop при одном нейтрале, исключение полупрозрачных
//! поверхностей и blend muted при отсутствии ключа.

use super::MIN_SEP;
use crate::theme::color_math::lightness;
use crate::theme::resolve::{resolve_palette, theme_hex_opaque};
use kamin_theme::Color;
use serde_json::json;

fn l_hex(c: Color) -> f32 {
    lightness(c)
}

#[test]
fn ramp_dark_theme_orders_surfaces() {
    // 3 нейтрала: sideBar темнее editor, widget светлее (типичный dark)
    let colors = json!({
        "editor.background": "#1e1e2e",
        "sideBar.background": "#181825",
        "editorWidget.background": "#313244",
        "foreground": "#cdd6f4",
        "activityBarBadge.background": "#f38ba8",
    });
    let p = resolve_palette(&colors, true, kamin_theme::DARK);
    // Backdrop = самый тёмный нейтрал (sideBar)
    assert!(l_hex(p.bg_sidebar) <= l_hex(p.bg_mantle));
    // Панель темнее редактора (код-канва остаётся самой яркой)
    assert!(l_hex(p.bg_mantle) < l_hex(p.editor_bg));
    // Карточка светлее панели
    assert!(l_hex(p.bg_surface) > l_hex(p.bg_mantle));
    // Accent = сильнее всех насыщенный кандидат (розовый бейдж)
    assert!((p.accent_primary.r - 0xf3 as f32 / 255.0).abs() < 0.01);
}

#[test]
fn ramp_single_neutral_synthesises_backdrop() {
    // editor == sideBar (один нейтрал) → backdrop сдвигается к чёрному
    let colors = json!({
        "editor.background": "#222233",
        "sideBar.background": "#222233",
        "foreground": "#ccccdd",
    });
    let p = resolve_palette(&colors, true, kamin_theme::DARK);
    assert!(
        (l_hex(p.bg_sidebar) - l_hex(p.bg_mantle)).abs() >= MIN_SEP,
        "backdrop must be nudged apart from panel"
    );
}

#[test]
fn translucent_surface_excluded_from_ramp() {
    // list.activeSelectionBackground с альфой — тинт, не поверхность
    assert!(!theme_hex_opaque("#ffffff80"));
    assert!(theme_hex_opaque("#ffffffff"));
    assert!(theme_hex_opaque("#ffffff"));
}

#[test]
fn muted_blend_when_missing() {
    let colors = json!({
        "editor.background": "#101020",
        "foreground": "#e0e0f0",
    });
    let p = resolve_palette(&colors, true, kamin_theme::DARK);
    // muted между fg и bg
    let lm = l_hex(p.text_muted);
    assert!(lm < l_hex(p.text_primary) && lm > l_hex(p.editor_bg));
}

#[test]
fn light_theme_inverts_ramp() {
    let colors = json!({
        "editor.background": "#ffffff",
        "sideBar.background": "#f3f3f3",
        "editorWidget.background": "#ececec",
        "foreground": "#333333",
    });
    let p = resolve_palette(&colors, false, kamin_theme::LIGHT);
    // Light: backdrop = САМЫЙ СВЕТЛЫЙ нейтрал, панели темнее
    assert!(l_hex(p.bg_sidebar) >= l_hex(p.bg_mantle));
}
