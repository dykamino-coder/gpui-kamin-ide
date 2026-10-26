//! Функции времени css-easing-1: разбор и выборка значения по доле прогресса.
//!
//! Вход — текст `animation-timing-function` (`linear`, `ease*`, `cubic-bezier()`,
//! `steps()`); выход — `Easing` и его `sample(progress, before)`. Кривая Безье
//! обращается по `x` бинарным поиском, `y` не зажимается (выброс законен);
//! `steps()` учитывает позицию скачка и сторону `before`. Применяется к
//! КАЖДОМУ интервалу между кадрами, а не к шкале времени целиком: так велит
//! спека, и так считают `frames::frame_at` и `frames::frozen`.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    Cubic([f32; 4]),
    Steps(u32, StepPosition),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepPosition {
    Start,
    End,
    None,
    Both,
}

impl Default for Easing {
    fn default() -> Self {
        Self::Cubic([0.25, 0.1, 0.25, 1.0])
    }
}

impl Easing {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "linear" => return Some(Self::Linear),
            "ease" => return Some(Self::default()),
            "ease-in" => return Some(Self::Cubic([0.42, 0.0, 1.0, 1.0])),
            "ease-out" => return Some(Self::Cubic([0.0, 0.0, 0.58, 1.0])),
            "ease-in-out" => return Some(Self::Cubic([0.42, 0.0, 0.58, 1.0])),
            "step-start" => return Some(Self::Steps(1, StepPosition::Start)),
            "step-end" => return Some(Self::Steps(1, StepPosition::End)),
            _ => {}
        }
        let (name, args) = value.trim().split_once('(')?;
        let args: Vec<_> = args.strip_suffix(')')?.split(',').map(str::trim).collect();
        match (name, args.as_slice()) {
            ("cubic-bezier", [a, b, c, d]) => {
                let number =
                    |s| crate::style::values::value::css_number(s).filter(|v| v.is_finite());
                let points = [number(a)?, number(b)?, number(c)?, number(d)?];
                ((0.0..=1.0).contains(&points[0]) && (0.0..=1.0).contains(&points[2]))
                    .then_some(Self::Cubic(points))
            }
            ("steps", [count] | [count, _]) => {
                let count: u32 = count.parse().ok().filter(|n| *n > 0)?;
                let position = match args.get(1).copied().unwrap_or("end") {
                    "start" | "jump-start" => StepPosition::Start,
                    "end" | "jump-end" => StepPosition::End,
                    "jump-none" if count > 1 => StepPosition::None,
                    "jump-both" => StepPosition::Both,
                    _ => return None,
                };
                Some(Self::Steps(count, position))
            }
            _ => None,
        }
    }

    pub(crate) fn sample(self, progress: f32, before: bool) -> f32 {
        match self {
            Self::Linear => progress,
            Self::Cubic([x1, y1, x2, y2]) => {
                if progress == 0.0 || progress == 1.0 {
                    return progress;
                }
                let curve = |t: f64, a: f32, b: f32| {
                    3.0 * (1.0 - t).powi(2) * t * f64::from(a)
                        + 3.0 * (1.0 - t) * t * t * f64::from(b)
                        + t.powi(3)
                };
                // Invert x monotonically; y may overshoot and must not be clamped.
                let (mut lo, mut hi) = (0.0, 1.0);
                for _ in 0..32 {
                    let mid = (lo + hi) / 2.0;
                    if curve(mid, x1, x2) < f64::from(progress) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                curve((lo + hi) / 2.0, y1, y2) as f32
            }
            Self::Steps(count, position) => {
                let scaled = progress * count as f32;
                let mut step = scaled.floor();
                if matches!(position, StepPosition::Start | StepPosition::Both) {
                    step += 1.0;
                }
                if before && scaled.fract() == 0.0 {
                    step -= 1.0;
                }
                let jumps = match position {
                    StepPosition::None => count as f32 - 1.0,
                    StepPosition::Both => count as f32 + 1.0,
                    _ => count as f32,
                };
                (step / jumps).clamp(0.0, 1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_function_grammar_and_control_points() {
        for value in [
            "cubic-bezier(-1,0,1,1)",
            "steps(0)",
            "steps(1,jump-none)",
            "steps(2,unknown)",
        ] {
            assert!(Easing::parse(value).is_none(), "{value}");
        }
        assert_eq!(
            Easing::parse("steps(4, jump-both)"),
            Some(Easing::Steps(4, StepPosition::Both))
        );
        assert_eq!(
            Easing::parse("cubic-bezier(0, 2, 1, -1)"),
            Some(Easing::Cubic([0.0, 2.0, 1.0, -1.0]))
        );
    }

    #[test]
    fn evaluates_bezier_and_step_boundaries() {
        assert!((Easing::default().sample(0.5, false) - 0.8024034).abs() < 0.000001);
        assert_eq!(Easing::Steps(4, StepPosition::End).sample(0.5, false), 0.5);
        assert_eq!(Easing::Steps(4, StepPosition::End).sample(0.5, true), 0.25);
        assert_eq!(Easing::Steps(1, StepPosition::Start).sample(0.0, true), 0.0);
        assert_eq!(
            Easing::Steps(1, StepPosition::Start).sample(0.0, false),
            1.0
        );
    }
}
