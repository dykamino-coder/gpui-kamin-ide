//! Absolute-clock curve with continuous position and velocity on retargeting.

use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Source {
    Wheel,
    Keyboard,
    Programmatic,
}

impl Source {
    fn duration(self, distance: f64) -> f64 {
        match self {
            // Blink's wheel/keyboard timing policy, expressed in seconds rather
            // than frames: the display's refresh rate never changes the curve.
            Self::Wheel => 0.2 - ((distance - 120.) / 360.).clamp(0., 1.) * 0.1,
            Self::Keyboard => 0.15,
            Self::Programmatic => (distance.sqrt() / 60.).clamp(0.1, 0.5),
        }
    }
}

pub(super) struct Sample {
    pub position: [f64; 2],
    pub velocity: [f64; 2],
    pub done: bool,
}

pub(super) struct Motion {
    pub target: [f64; 2],
    start: [f64; 2],
    control: [f64; 2],
    since: Duration,
    duration: f64,
}

impl Motion {
    pub fn delay(&mut self, duration: Duration) {
        self.since += duration;
    }

    /// A cubic stays in its control hull, including velocity on reversal.
    pub fn extent(&self) -> ([f64; 2], [f64; 2]) {
        (
            std::array::from_fn(|a| self.start[a].min(self.control[a]).min(self.target[a])),
            std::array::from_fn(|a| self.start[a].max(self.control[a]).max(self.target[a])),
        )
    }

    pub fn new(
        start: [f64; 2],
        velocity: [f64; 2],
        target: [f64; 2],
        now: Duration,
        source: Source,
    ) -> Self {
        let distance = (target[0] - start[0])
            .abs()
            .max((target[1] - start[1]).abs());
        let mut duration = source.duration(distance);
        for axis in 0..2 {
            let delta = target[axis] - start[axis];
            if delta * velocity[axis] > 0. {
                duration = duration.min(2.5 * delta.abs() / velocity[axis].abs());
            }
        }
        let duration = duration.max(0.001);
        Self {
            target,
            start,
            control: std::array::from_fn(|axis| start[axis] + velocity[axis] * duration * 0.42),
            since: now,
            duration,
        }
    }

    pub fn sample(&self, now: Duration) -> Sample {
        let progress = now.saturating_sub(self.since).as_secs_f64() / self.duration;
        if progress <= 0. {
            return Sample {
                position: self.start,
                velocity: std::array::from_fn(|a| {
                    (self.control[a] - self.start[a]) / (0.42 * self.duration)
                }),
                done: false,
            };
        }
        if progress >= 1. {
            return Sample {
                position: self.target,
                velocity: [0.; 2],
                done: true,
            };
        }
        // Invert the time component of cubic-bezier(.42, 0, .58, 1).
        // Spatial control points encode the old velocity directly, including
        // reversal and an axis whose destination equals its current position.
        let (mut lo, mut hi) = (0., 1.);
        for _ in 0..30 {
            let u = (lo + hi) * 0.5;
            if cubic(0., 0.42, 0.58, 1., u) < progress {
                lo = u
            } else {
                hi = u
            }
        }
        let u = (lo + hi) * 0.5;
        let speed = derivative(0., 0.42, 0.58, 1., u) * self.duration;
        Sample {
            position: std::array::from_fn(|axis| {
                cubic(
                    self.start[axis],
                    self.control[axis],
                    self.target[axis],
                    self.target[axis],
                    u,
                )
            }),
            velocity: std::array::from_fn(|axis| {
                derivative(
                    self.start[axis],
                    self.control[axis],
                    self.target[axis],
                    self.target[axis],
                    u,
                ) / speed
            }),
            done: false,
        }
    }
}

fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let v = 1. - t;
    v * v * v * a + 3. * v * v * t * b + 3. * v * t * t * c + t * t * t * d
}

fn derivative(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    3. * ((1. - t).powi(2) * (b - a) + 2. * (1. - t) * t * (c - b) + t * t * (d - c))
}

#[cfg(test)]
mod tests;
