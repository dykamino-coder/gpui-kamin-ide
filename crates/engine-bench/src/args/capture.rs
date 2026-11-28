//! Параметры геометрии снимка: конечный масштаб и неотрицательный шаг колеса.

impl super::Args {
    /// Размер поверхности в физических пикселях, с округлением оконного backend.
    pub(crate) fn capture_size(&self) -> (u32, u32) {
        let size = gpui::size(
            gpui::px(self.viewport.0 as f32),
            gpui::px(self.viewport.1 as f32),
        )
        .to_device_pixels(self.device_scale_factor);
        (size.width.0 as u32, size.height.0 as u32)
    }
}

pub(super) fn scroll_step(value: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let step: f32 = value.parse()?;
    if !step.is_finite() || step < 0.0 {
        return Err("scroll step must be finite and nonnegative".into());
    }
    Ok(step)
}

pub(super) fn scale_factor(value: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let scale: f32 = value.parse()?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err("device scale factor must be finite and positive".into());
    }
    Ok(scale)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fractional_scale_rounds_the_physical_surface_without_changing_css_viewport() {
        let mut args = super::super::Args::default();
        args.viewport = (1446, 900);
        assert_eq!(args.capture_size(), (1446, 900));
        args.device_scale_factor = 1.25;
        assert_eq!(args.capture_size(), (1808, 1125));
        assert_eq!(args.viewport, (1446, 900));
    }

    #[test]
    fn capture_scale_is_positive_and_finite() {
        for value in ["0", "-1", "NaN", "inf", "1e99"] {
            assert!(super::scale_factor(value).is_err(), "{value}");
        }
        for (value, expected) in [("1", 1.), ("1.25", 1.25), ("2", 2.)] {
            assert_eq!(super::scale_factor(value).unwrap(), expected);
        }
    }
}
