//! Цветовая арифметика темы: смешение, светлота, хрома, шаги.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::theme_sync::BACKDROP_NUDGE;
use crate::theme_sync::BLACK;
use crate::theme_sync::MID_L;
use crate::theme_sync::MIN_SEP;
use crate::theme_sync::OVERLAY_MAX_STEP;
use crate::theme_sync::PANEL_NUDGE;
use crate::theme_sync::SURFACE_MAX_STEP;
use crate::theme_sync::WHITE;
use kamin_theme::Color;

pub fn mixc(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}
/// HSL lightness (0..1).
pub fn lightness(c: Color) -> f32 {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    (max + min) / 2.0
}
/// Chroma (0..1) = max−min — «нейтральность» независимо от светлоты.
pub fn chroma_of(c: Color) -> f32 {
    c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
}
/// HSL saturation (0..1).
pub fn saturation(c: Color) -> f32 {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    if max == min {
        return 0.0;
    }
    let l = (max + min) / 2.0;
    let d = max - min;
    if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    }
}
/// Подтянуть target к anchor так, чтобы разница светлоты ≤ max_step.
pub fn cap_step(anchor: Color, target: Color, max_step: f32) -> Color {
    let d = (lightness(target) - lightness(anchor)).abs();
    if d > max_step {
        mixc(anchor, target, max_step / d)
    } else {
        target
    }
}
pub fn close(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.002 && (a.g - b.g).abs() < 0.002 && (a.b - b.b).abs() < 0.002
}
/// Ramp фоновых слоёв из авторских нейтралов (bgSurfaces оригинала 1:1).
pub struct Surfaces {
    pub backdrop: Option<Color>,
    pub panel: Option<Color>,
    pub surface: Option<Color>,
    pub surface_hover: Option<Color>,
    pub overlay: Option<Color>,
}
pub fn bg_surfaces(stops: &[Color], dark: bool, editor_bg: Option<Color>) -> Surfaces {
    if stops.is_empty() {
        return Surfaces {
            backdrop: None,
            panel: None,
            surface: None,
            surface_hover: None,
            overlay: None,
        };
    }
    let lo = stops[0];
    let hi = *stops.last().unwrap();
    let le = editor_bg.map(lightness).unwrap_or(MID_L);
    let darker: Vec<Color> = stops
        .iter()
        .copied()
        .filter(|s| lightness(*s) < le)
        .collect();
    let lighter: Vec<Color> = stops
        .iter()
        .copied()
        .filter(|s| lightness(*s) > le)
        .collect();

    let (mut backdrop, panel, mut surface, mut overlay);
    if dark {
        backdrop = lo;
        panel = darker
            .last()
            .or(lighter.first())
            .copied()
            .unwrap_or_else(|| mixc(editor_bg.unwrap_or(lo), BLACK, PANEL_NUDGE));
        surface = lighter
            .iter()
            .find(|s| !close(**s, panel))
            .or(lighter.first())
            .copied()
            .or(editor_bg)
            .unwrap_or(panel);
        overlay = lighter
            .iter()
            .rev()
            .find(|s| !close(**s, panel))
            .copied()
            .unwrap_or(surface);
    } else {
        backdrop = hi;
        panel = darker
            .last()
            .or(lighter.first())
            .copied()
            .unwrap_or_else(|| mixc(editor_bg.unwrap_or(hi), WHITE, PANEL_NUDGE));
        surface = darker
            .iter()
            .find(|s| !close(**s, panel))
            .or(darker.first())
            .copied()
            .or(editor_bg)
            .unwrap_or(panel);
        overlay = darker.first().copied().unwrap_or(surface);
    }
    // Backdrop ОБЯЗАН отличаться от панелей (editor==sideBar у многих тем)
    if (lightness(backdrop) - lightness(panel)).abs() < MIN_SEP {
        backdrop = mixc(panel, if dark { BLACK } else { WHITE }, BACKDROP_NUDGE);
    }
    // Кап подъёма карточек/поповеров над редактором (HC-темы)
    let anchor = editor_bg.unwrap_or(panel);
    let mut surface_hover = if dark {
        lighter.get(1).copied().unwrap_or(surface)
    } else {
        surface
    };
    surface = cap_step(anchor, surface, SURFACE_MAX_STEP);
    overlay = cap_step(anchor, overlay, OVERLAY_MAX_STEP);
    surface_hover = cap_step(anchor, surface_hover, OVERLAY_MAX_STEP);
    Surfaces {
        backdrop: Some(backdrop),
        panel: Some(panel),
        surface: Some(surface),
        surface_hover: Some(surface_hover),
        overlay: Some(overlay),
    }
}
