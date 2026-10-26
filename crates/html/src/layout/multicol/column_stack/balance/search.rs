//! Конечный поиск высоты: shortage сужает нижнюю границу, успешная проба — верхнюю.
//! Границы сходятся до 1/64 точки; CSS-потолок сохраняется точно.

use crate::layout::fragment::token::Error;
use std::collections::HashSet;

// Та же сетка, что у line_shape::nested_kid::layout_unit. На больших
// координатах следующий f32 уже шире сетки: next_up сохраняет прогресс.
fn next_unit(value: f32) -> f32 {
    (((f64::from(value) * 64.).floor() + 1.) / 64.) as f32
}

fn after(value: f32) -> f32 {
    next_unit(value).max(value.next_up())
}

fn before(value: f32) -> f32 {
    ((((f64::from(value) * 64.).ceil() - 1.) / 64.) as f32).min(value.next_down())
}

fn ceil_unit(value: f32) -> f32 {
    let exact = (f64::from(value) * 64.).ceil() / 64.;
    let rounded = exact as f32;
    if f64::from(rounded) < exact {
        rounded.next_up()
    } else {
        rounded
    }
}

pub(crate) struct Trial<T> {
    pub(crate) fits: bool,
    pub(crate) shortage: Option<f32>,
    pub(crate) value: T,
}

pub(crate) fn min_shortage(values: impl IntoIterator<Item = Option<f32>>) -> Option<f32> {
    values
        .into_iter()
        .flatten()
        .filter(|s| s.is_finite() && *s > 0. && *s < f32::MAX)
        .min_by(f32::total_cmp)
}

/// Все вызовы `trial` обязаны начинаться с одного входного курсора и снимка.
/// `ceiling` — CSS-потолок либо высота, вмещающая целый исходный поток.
pub(crate) fn search<T>(
    start: f32,
    ceiling: f32,
    constrained: bool,
    mut trial: impl FnMut(f32) -> Trial<T>,
) -> Result<(f32, T), Error> {
    if !start.is_finite() || !ceiling.is_finite() || start <= 0. || ceiling < start {
        return Err(Error::InvalidGeometry);
    }
    let mut low = before(start);
    let mut high = ceiling;
    let mut at = ceil_unit(start).min(ceiling);
    let mut best = None;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(at.to_bits()) {
            return Err(Error::NoProgress);
        }
        let result = trial(at);
        if result.fits {
            high = at;
            best = Some(result.value);
        } else if at == ceiling {
            return if constrained {
                Ok((at, result.value))
            } else {
                Err(Error::NoProgress)
            };
        } else {
            low = at;
            if let Some(shortage) = result.shortage.filter(|s| s.is_finite() && *s > 0.) {
                // Между at и at+shortage нет следующей допустимой границы.
                // Округлившееся сложение всё равно проходит одну layout unit.
                let next = ceil_unit(at + shortage).max(after(at)).min(high);
                if next == high
                    && let Some(value) = best.take()
                {
                    return Ok((high, value));
                }
                low = before(next);
                at = next;
                continue;
            }
        }
        if best.is_some() && after(low) >= high {
            return best.map(|value| (high, value)).ok_or(Error::NoProgress);
        }
        // Сначала проверяем верхнюю границу. После успешной пробы интервал
        // строго уменьшается на конечной сетке layout units.
        at = if best.is_none() {
            high
        } else {
            let middle = ((f64::from(low) + f64::from(high)) / 2.) as f32;
            ceil_unit(middle).max(after(low)).min(before(high))
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feasible_and_infeasible_bounds_meet_at_adjacent_layout_units() {
        let threshold = 137.25f32;
        let mut heights = HashSet::new();
        let (height, chosen) = search(1., 200., false, |height| {
            assert!(heights.insert(height.to_bits()), "repeated trial");
            Trial {
                fits: height >= threshold,
                shortage: None,
                value: height,
            }
        })
        .unwrap();
        assert_eq!(height, threshold);
        assert_eq!(chosen, height);
        assert_eq!(before(height), threshold - 1. / 64.);
    }

    #[test]
    fn positive_shortage_skips_only_proven_infeasible_heights() {
        let mut heights = Vec::new();
        let (height, _) = search(25., 500., false, |height| {
            heights.push(height);
            Trial {
                fits: height >= 100.,
                shortage: Some(100. - height),
                value: (),
            }
        })
        .unwrap();
        assert_eq!(height, 100.);
        assert_eq!(heights, [25., 100.]);
    }

    #[test]
    fn rounded_shortage_advances_one_layout_unit() {
        let floor = 1024f32;
        let (height, _) = search(floor, floor + 1. / 64., false, |height| Trial {
            fits: height > floor,
            shortage: Some(f32::MIN_POSITIVE),
            value: (),
        })
        .unwrap();
        assert_eq!(height, floor + 1. / 64.);
    }

    #[test]
    fn css_ceiling_between_layout_units_is_preserved() {
        let cap = 10.001;
        let (height, _) = search(10., cap, true, |_| Trial {
            fits: false,
            shortage: Some(1.),
            value: (),
        })
        .unwrap();
        assert_eq!(height, cap);
    }

    #[test]
    fn untested_adjacent_upper_bound_is_tried_without_shortage() {
        let cap = 10.001;
        let (height, _) = search(10., cap, false, |h| Trial {
            fits: h == cap,
            shortage: None,
            value: (),
        })
        .unwrap();
        assert_eq!(height, cap);
        assert_eq!(
            min_shortage([Some(30.), None, Some(10.), Some(50.)]),
            Some(10.)
        );
    }

    #[test]
    fn css_ceiling_can_overflow_but_unproven_unconstrained_bound_is_an_error() {
        let infeasible = |_| Trial {
            fits: false,
            shortage: None,
            value: (),
        };
        assert_eq!(search(10., 20., true, infeasible).unwrap().0, 20.);
        assert_eq!(search(10., 20., false, infeasible), Err(Error::NoProgress));
        assert_eq!(
            search(20., 10., false, infeasible),
            Err(Error::InvalidGeometry)
        );
    }

    #[test]
    fn best_feasible_trial_survives_later_infeasible_trials() {
        let mut sequence = 0;
        let mut accepted = Vec::new();
        let (height, chosen) = search(1., 200., false, |height| {
            sequence += 1;
            if height >= 101. {
                accepted.push((height, sequence));
            }
            Trial {
                fits: height >= 101.,
                shortage: None,
                value: sequence,
            }
        })
        .unwrap();
        assert_eq!((height, chosen), *accepted.last().unwrap());
        assert_eq!(height, 101.);
    }
}
