//! KaminIDE patch: локальный выбор LCD только для известной непрозрачной поверхности.

use super::{
    FontId, Pixels, TextRenderingMode, TransformationMatrix, Window, WindowBackgroundAppearance,
};

impl Window {
    pub(super) fn should_use_subpixel_rendering(
        &self,
        font_id: FontId,
        font_size: Pixels,
        pixel_exact: bool,
    ) -> bool {
        if !lcd_surface_is_safe(
            self.platform_window.background_appearance(),
            self.next_frame.scene.in_group,
            pixel_exact,
            self.current_transformation(),
        ) || !self.platform_window.is_subpixel_rendering_supported()
        {
            return false;
        }

        let mode = match self.text_rendering_mode.get() {
            TextRenderingMode::PlatformDefault => self
                .text_system()
                .recommended_rendering_mode(font_id, font_size),
            mode => mode,
        };

        mode == TextRenderingMode::Subpixel
    }
}

fn lcd_surface_is_safe(
    appearance: WindowBackgroundAppearance,
    isolated: bool,
    pixel_exact: bool,
    transformation: TransformationMatrix,
) -> bool {
    // LCD blend сохраняет destination alpha: прозрачный intermediate не получает
    // покрытия глифа. Записанная сцена также может позже попасть в такую группу.
    // Alpha кисти на opaque root безопасна: она ослабляет RGB coverage, а не фон.
    appearance == WindowBackgroundAppearance::Opaque
        && !isolated
        && !pixel_exact
        // RGB-полосы должны совпадать с физическими пикселями после композиции.
        && transformation.rotation_scale == TransformationMatrix::unit().rotation_scale
        && transformation.translation.iter().all(|v| v.is_finite() && v.fract() == 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_root_allows_lcd_but_transparent_backdrops_do_not() {
        for appearance in [
            WindowBackgroundAppearance::Opaque,
            WindowBackgroundAppearance::Transparent,
            WindowBackgroundAppearance::Blurred,
            WindowBackgroundAppearance::MicaBackdrop,
            WindowBackgroundAppearance::MicaAltBackdrop,
        ] {
            assert_eq!(
                lcd_surface_is_safe(appearance, false, false, TransformationMatrix::unit()),
                appearance == WindowBackgroundAppearance::Opaque,
            );
        }
    }

    #[test]
    fn isolated_scenes_and_exact_fonts_always_use_grayscale() {
        for (isolated, pixel_exact) in [(true, false), (false, true), (true, true)] {
            assert!(!lcd_surface_is_safe(
                WindowBackgroundAppearance::Opaque,
                isolated,
                pixel_exact,
                TransformationMatrix::unit(),
            ));
        }
    }

    #[test]
    fn only_pixel_aligned_translation_preserves_lcd_stripes() {
        let safe =
            |matrix| lcd_surface_is_safe(WindowBackgroundAppearance::Opaque, false, false, matrix);
        let mut matrix = TransformationMatrix::unit();
        matrix.translation = [-12.0, 34.0];
        assert!(safe(matrix));
        for translation in [
            [0.5, 0.0],
            [0.0, -0.25],
            [f32::NAN, 0.0],
            [0.0, f32::INFINITY],
        ] {
            matrix.translation = translation;
            assert!(!safe(matrix));
        }
        for rotation_scale in [
            [[2.0, 0.0], [0.0, 2.0]],
            [[0.0, -1.0], [1.0, 0.0]],
            [[1.0, 0.25], [0.0, 1.0]],
            [[-1.0, 0.0], [0.0, 1.0]],
        ] {
            matrix = TransformationMatrix::unit();
            matrix.rotation_scale = rotation_scale;
            assert!(!safe(matrix));
        }
    }
}
