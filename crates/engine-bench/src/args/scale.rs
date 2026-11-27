//! Keep CSS constraints independent of the rounded device-pixel capture size.

use super::Args;

pub(super) fn parse(value: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let scale: f32 = value.parse()?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err("scale factor must be finite and positive".into());
    }
    Ok(scale)
}

pub(super) fn validate_capture(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let size = args.capture_size();
    if !(1..=8192).contains(&size.0) || !(1..=8192).contains(&size.1) {
        return Err("scaled viewport dimensions must be 1..8192 device pixels".into());
    }
    Ok(())
}

impl Args {
    pub(crate) fn capture_size(&self) -> (u32, u32) {
        let size = gpui::size(
            gpui::px(self.viewport.0 as f32),
            gpui::px(self.viewport.1 as f32),
        )
        .to_device_pixels(self.scale_factor);
        (size.width.into(), size.height.into())
    }

    pub(crate) fn uses_root_scroller(&self) -> bool {
        self.root_scroller || self.mode == super::Mode::Scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_scale_rounds_capture_without_changing_css_constraints() {
        let page = std::env::temp_dir().join("engine-bench-scale-test.html");
        std::fs::write(&page, "<p>scaled</p>").unwrap();
        let args = Args::parse(
            [
                page.to_str().unwrap(),
                "--viewport",
                "1446x804",
                "--scale-factor",
                "1.25",
                "--root-scroller",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        assert_eq!(args.viewport, (1446, 804));
        assert_eq!(args.capture_size(), (1808, 1005));
        assert!(args.uses_root_scroller());
        std::fs::remove_file(page).unwrap();
    }

    #[test]
    fn invalid_scales_and_unbounded_targets_are_rejected_before_opening_input() {
        for scale in ["NaN", "inf", "-1", "0", "0.00001", "100000"] {
            assert!(
                Args::parse(
                    ["missing.html", "--scale-factor", scale]
                        .into_iter()
                        .map(str::to_owned)
                )
                .is_err()
            );
        }
    }
}
