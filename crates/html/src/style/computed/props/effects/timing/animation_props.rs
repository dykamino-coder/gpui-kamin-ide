//! Animation shorthand and longhands share parsing; function arguments remain intact.

use crate::animation::easing::Easing;
use crate::style::computed::split_outside_parens;
use crate::style::computed::{AnimSpec, Computed};

fn initial() -> AnimSpec {
    AnimSpec {
        name: String::new(),
        seconds: 0.0,
        infinite: false,
        alternate: false,
        delay: 0.0,
        paused: false,
        names: Vec::new(),
        fill: 0,
        iterations: 1.0,
        reverse: false,
    }
}

fn fill(value: &str) -> Option<u8> {
    match value {
        "none" => Some(0),
        "backwards" => Some(1),
        "forwards" => Some(2),
        "both" => Some(3),
        _ => None,
    }
}

impl Computed {
    pub(super) fn apply_animation(&mut self, key: &str, value: &str) {
        let mut spec = self.animation.clone().unwrap_or_else(initial);
        if key == "animation-fill-mode" {
            if let Some(mode) = fill(value) {
                spec.fill = mode;
            } else {
                return;
            }
        } else {
            if key == "animation" {
                spec = initial();
                self.animation_easing = Some(Easing::default());
            }
            let mut times = 0;
            for token in split_outside_parens(value) {
                let token = token.as_str();
                let time = token
                    .strip_suffix("ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .map(|v| v / 1000.0)
                    .or_else(|| token.strip_suffix('s').and_then(|v| v.parse::<f32>().ok()));
                if let Some(time) = time {
                    match key {
                        "animation-delay" => spec.delay = time,
                        "animation-duration" => spec.seconds = time,
                        _ if times == 0 => spec.seconds = time,
                        _ => spec.delay = time,
                    }
                    times += 1;
                } else if key == "animation-name" {
                    spec.name = token.into();
                } else if let Some(easing) = Easing::parse(token) {
                    self.animation_easing = Some(easing);
                } else if let Some(mode) = fill(token) {
                    spec.fill = mode;
                } else {
                    match token {
                        "paused" => spec.paused = true,
                        "running" => spec.paused = false,
                        "infinite" => spec.infinite = true,
                        "alternate" => spec.alternate = true,
                        "normal" => {
                            spec.alternate = false;
                            spec.reverse = false;
                        }
                        "reverse" => spec.reverse = true,
                        "alternate-reverse" => {
                            spec.reverse = true;
                            spec.alternate = true;
                        }
                        _ if token.parse::<f64>().is_ok() => {
                            spec.iterations = token.parse::<f64>().unwrap().max(0.0);
                            spec.infinite = false;
                        }
                        _ => spec.name = token.into(),
                    }
                }
            }
            if key == "animation-name" {
                let names: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|n| !n.is_empty() && *n != "none")
                    .map(String::from)
                    .collect();
                spec.names = if names.len() > 1 { names } else { vec![] };
            }
        }
        self.animation = accept_parsed!(self, Some(spec));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_and_fill_are_not_animation_names() {
        let mut style = Computed::default();
        style.apply_one(
            "animation",
            "slide 100s -50s steps(4, jump-end) both paused",
        );
        let spec = style.animation.as_ref().unwrap();
        assert_eq!(spec.name, "slide");
        assert_eq!(spec.fill, 3);
        assert_eq!(spec.delay, -50.0);
        assert_eq!(style.animation_easing.unwrap().sample(0.6, false), 0.5);
        style.apply_one("animation-timing-function", "cubic-bezier(0, 0, 1, 1)");
        assert_eq!(style.animation_easing.unwrap().sample(0.5, false), 0.5);
        style.apply_one("animation-fill-mode", "backwards");
        assert_eq!(style.animation.as_ref().unwrap().fill, 1);
        style.apply_one("animation-fill-mode", "invalid");
        assert_eq!(style.animation.as_ref().unwrap().fill, 1);
    }
}
