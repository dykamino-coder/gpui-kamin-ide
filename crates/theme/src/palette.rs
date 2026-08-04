//! Палитры Dark/Light — ДОСЛОВНО из plan/20 §1 (dark-theme.css / light-theme.css).
//! Contributed-темы (VSIX) переопределяют поля рантайм-мапой поверх (plan/20 §8) — позже.

use crate::color::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    // Фоны (elevation ramp)
    pub bg_primary: Color,
    pub bg_base: Color,
    pub bg_mantle: Color,
    pub bg_sidebar: Color,
    pub bg_surface: Color,
    pub bg_overlay: Color,
    // Редактор
    pub editor_bg: Color,
    pub editor_fg: Color,
    pub editor_cursor: Color,
    // Текст
    pub text_primary: Color,
    pub text_subtext: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_disabled: Color,
    pub text_muted_2: Color,
    pub text_muted_light: Color,
    // Акценты
    pub accent_blue: Color,
    pub accent_sapphire: Color,
    pub accent_red: Color,
    pub accent_maroon: Color,
    pub accent_green: Color,
    pub accent_yellow: Color,
    pub accent_pink: Color,
    pub accent_purple: Color,
    pub accent_orange: Color,
    pub accent_teal: Color,
    pub accent_rosewater: Color,
    // Семантика действия (dark=синий, light=ОРАНЖЕВЫЙ — plan/20)
    pub accent_action: Color,
    pub accent_action_hover: Color,
    pub accent_action_fg: Color,
    pub accent_primary: Color,
    // Состояния
    pub bg_surface_hover: Color,
    pub bg_overlay_hover: Color,
    // Glint-бордер: угловые стопы 0%/100% и средние 22%/78% (plan/20 §2)
    pub glint_edge: Color,
    pub glint_mid: Color,
}

pub const DARK: Palette = Palette {
    bg_primary: Color::hex(0x313240),
    bg_base: Color::hex(0x313240),
    bg_mantle: Color::hex(0x262533),
    bg_sidebar: Color::hex(0x1d1d28),
    bg_surface: Color::hex(0x3d3f51),
    bg_overlay: Color::hex(0x515567),
    editor_bg: Color::hex(0x1d1c25),
    editor_fg: Color::hex(0xdcdce4),
    editor_cursor: Color::hex(0xa0a0d0),
    text_primary: Color::hex(0xcfd4e2),
    text_subtext: Color::hex(0xafb6ca),
    text_secondary: Color::hex(0xadb3c7),
    text_muted: Color::hex(0x838aa0),
    text_disabled: Color::hex(0x60667b),
    text_muted_2: Color::hex(0x7f849c),
    text_muted_light: Color::hex(0xacb2d2),
    accent_blue: Color::hex(0x89b4fa),
    accent_sapphire: Color::hex(0x74c7ec),
    accent_red: Color::hex(0xf38ba8),
    accent_maroon: Color::hex(0xeba0ac),
    accent_green: Color::hex(0xa6e3a1),
    accent_yellow: Color::hex(0xf9e2af),
    accent_pink: Color::hex(0xf5c2e7),
    accent_purple: Color::hex(0xcba6f7),
    accent_orange: Color::hex(0xfab387),
    accent_teal: Color::hex(0x94e2d5),
    accent_rosewater: Color::hex(0xf5e0dc),
    accent_action: Color::hex(0x89b4fa),
    accent_action_hover: Color::hex(0x74c7ec),
    accent_action_fg: Color::hex(0x313240),
    accent_primary: Color::hex(0x89b4fa),
    bg_surface_hover: Color::hex(0x3b3b52),
    bg_overlay_hover: Color::hex(0x3e3e56),
    glint_edge: Color::hex_a(0xffffff, 0.18),
    glint_mid: Color::hex(0x262533), // = bg_mantle
};

pub const LIGHT: Palette = Palette {
    bg_primary: Color::hex(0xf6efeb),
    bg_base: Color::hex(0xfbf8f1),
    bg_mantle: Color::hex(0xfbf7f4),
    bg_sidebar: Color::hex(0xf4f1ea),
    bg_surface: Color::hex(0xe6e1d4),
    bg_overlay: Color::hex(0xd6d0c0),
    editor_bg: Color::hex(0xfcfaf6),
    editor_fg: Color::hex(0x48433c),
    editor_cursor: Color::hex(0x48433c),
    text_primary: Color::hex(0x322e28),
    text_subtext: Color::hex(0x463f37),
    text_secondary: Color::hex(0x524c43),
    text_muted: Color::hex(0x6e685d),
    text_disabled: Color::hex(0x938e82),
    text_muted_2: Color::hex(0x524c43),
    text_muted_light: Color::hex(0x524c43),
    accent_blue: Color::hex(0x3b6fc4),
    accent_sapphire: Color::hex(0x3a8aa3),
    accent_red: Color::hex(0xca3939),
    accent_maroon: Color::hex(0xd35a5a),
    accent_green: Color::hex(0x5e9855),
    accent_yellow: Color::hex(0xc89a3f),
    accent_pink: Color::hex(0xc46598),
    accent_purple: Color::hex(0x8a5fc8),
    accent_orange: Color::hex(0xda8343),
    accent_teal: Color::hex(0x4a9999),
    accent_rosewater: Color::hex(0xc08571),
    accent_action: Color::hex(0xda8343),
    accent_action_hover: Color::hex(0xb16527),
    accent_action_fg: Color::hex(0xffffff),
    accent_primary: Color::hex(0xda8343),
    bg_surface_hover: Color::hex(0xd8d4c4),
    bg_overlay_hover: Color::hex(0xc2bcab),
    glint_edge: Color::hex_a(0x3c2814, 0.18), // rgba(60,40,20,.18)
    glint_mid: Color::hex(0xe6e1d4),          // = bg_surface (light glint mid)
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_semantics_flip() {
        // Ключевой инвариант plan/20: dark action = blue, light action = orange.
        assert_eq!(DARK.accent_action, DARK.accent_blue);
        assert_eq!(LIGHT.accent_action, LIGHT.accent_orange);
    }

    #[test]
    fn glint_mid_matches_panel_fill() {
        assert_eq!(DARK.glint_mid, DARK.bg_mantle);
        assert_eq!(LIGHT.glint_mid, LIGHT.bg_surface);
    }
}
