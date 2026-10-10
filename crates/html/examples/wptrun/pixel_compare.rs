//! Exact RGB comparison for reftests, independent of the layout diagnostic threshold.
//!
//! Screenshot buffers are BGRA; WPT compares the three colour channels. Alpha
//! is not part of the RGB screenshot oracle. Dimensions must match even when
//! two differently shaped images happen to contain the same number of pixels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difference {
    pub maximum_channel: u8,
    pub pixels: usize,
}

pub fn compare(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>)) -> Option<Difference> {
    let expected = usize::try_from(a.0)
        .ok()?
        .checked_mul(usize::try_from(a.1).ok()?)?
        .checked_mul(4)?;
    if a.0 != b.0 || a.1 != b.1 || expected == 0 || a.2.len() != expected || b.2.len() != expected {
        return None;
    }
    let mut result = Difference {
        maximum_channel: 0,
        pixels: 0,
    };
    for (left, right) in a.2.chunks_exact(4).zip(b.2.chunks_exact(4)) {
        let delta = (0..3)
            .map(|channel| left[channel].abs_diff(right[channel]))
            .max()
            .unwrap_or(0);
        result.maximum_channel = result.maximum_channel.max(delta);
        result.pixels += usize::from(delta != 0);
    }
    Some(result)
}

/// An exact pass is represented by zero only. A small rounded percentage must
/// never turn a one-pixel difference into a pass in the existing report tools.
pub fn verdict(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>)) -> String {
    match compare(a, b) {
        Some(Difference { pixels: 0, .. }) => "0.00".into(),
        Some(d) => format!(
            "pixel mismatch max={} total={}",
            d.maximum_channel, d.pixels
        ),
        None => "invalid screenshot dimensions".into(),
    }
}

/// Exact equality is the completion oracle; the old threshold is opt-in.
pub fn exact_mode() -> bool {
    let mode = std::env::var("WPT_COMPARE").unwrap_or_else(|_| "exact".into());
    let exact = match mode.as_str() {
        "exact" => true,
        "legacy-layout" => false,
        other => {
            eprintln!("unknown WPT_COMPARE mode: {other}");
            std::process::exit(2);
        }
    };
    eprintln!(
        "WPT comparison: {}",
        if exact {
            "exact RGB (no fuzzy allowances)"
        } else {
            "legacy layout diagnostic; NOT a WPT pass count"
        }
    );
    exact
}

pub fn passed(exact: bool, verdict: &str) -> bool {
    if exact {
        verdict == "0.00"
    } else {
        verdict.parse::<f32>().is_ok_and(|d| d <= 0.5)
    }
}

/// Допуск, объявленный САМИМ тестом: `<meta name="fuzzy"
/// content="maxDifference=0-2;totalPixels=0-1200">`. Это часть протокола
/// reftest WPT, а не поблажка стенда: тест знает, что расходится с эталоном
/// на антиалиасинге, и называет верхнюю границу расхождения. Возвращается
/// наибольшее допустимое ЧИСЛО разошедшихся точек.
pub fn legacy_fuzzy_pixels(source: &str) -> usize {
    let lower = source.to_ascii_lowercase();
    let Some(at) = lower
        .find("name=\"fuzzy\"")
        .or_else(|| lower.find("name=fuzzy"))
    else {
        return 0;
    };
    let tail = &lower[at..];
    let Some(c) = tail.find("totalpixels=") else {
        return 0;
    };
    let value = &tail[c + "totalpixels=".len()..];
    let value = &value[..value
        .find(|ch: char| !ch.is_ascii_digit() && ch != '-')
        .unwrap_or(value.len())];
    // Запись — диапазон `0-1200` или число; допуск — верхняя граница.
    value
        .rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(width: u32, height: u32, pixels: Vec<u8>) -> (u32, u32, Vec<u8>) {
        (width, height, pixels)
    }

    #[test]
    fn one_unit_in_one_channel_is_a_failure() {
        let a = shot(1, 1, vec![0, 0, 0, 255]);
        let b = shot(1, 1, vec![0, 1, 0, 255]);
        assert_eq!(
            compare(&a, &b),
            Some(Difference {
                maximum_channel: 1,
                pixels: 1
            })
        );
        assert_eq!(verdict(&a, &b), "pixel mismatch max=1 total=1");
    }

    #[test]
    fn channels_count_once_per_pixel_and_alpha_is_ignored() {
        let a = shot(2, 1, vec![0, 1, 2, 255, 4, 5, 6, 255]);
        let b = shot(2, 1, vec![40, 41, 42, 0, 4, 5, 6, 0]);
        assert_eq!(
            compare(&a, &b),
            Some(Difference {
                maximum_channel: 40,
                pixels: 1
            })
        );
        assert_eq!(
            verdict(&a, &shot(2, 1, vec![0, 1, 2, 0, 4, 5, 6, 0])),
            "0.00"
        );
    }

    #[test]
    fn equal_buffer_lengths_do_not_hide_different_dimensions() {
        let a = shot(2, 1, vec![0; 8]);
        let b = shot(1, 2, vec![0; 8]);
        assert_eq!(compare(&a, &b), None);
    }

    #[test]
    fn empty_truncated_or_excess_buffers_are_not_passes() {
        for a in [
            shot(0, 0, vec![]),
            shot(1, 1, vec![0; 3]),
            shot(1, 1, vec![0; 8]),
        ] {
            assert_eq!(compare(&a, &a), None);
            assert_ne!(verdict(&a, &a), "0.00");
        }
    }

    #[test]
    fn layout_diagnostic_cannot_be_mistaken_for_an_exact_pass() {
        assert!(!passed(true, "0.01"));
        assert!(passed(false, "0.01"));
        assert!(!passed(true, "pixel mismatch max=1 total=1"));
        assert!(!passed(false, "pixel mismatch max=1 total=1"));
    }
}
