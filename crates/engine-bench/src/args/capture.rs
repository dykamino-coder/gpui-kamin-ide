//! Параметры геометрии снимка: конечный масштаб и неотрицательный шаг колеса.

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
    fn capture_scale_is_positive_and_finite() {
        for value in ["0", "-1", "NaN", "inf", "1e99"] {
            assert!(super::scale_factor(value).is_err(), "{value}");
        }
        for (value, expected) in [("1", 1.), ("1.25", 1.25), ("2", 2.)] {
            assert_eq!(super::scale_factor(value).unwrap(), expected);
        }
    }
}
