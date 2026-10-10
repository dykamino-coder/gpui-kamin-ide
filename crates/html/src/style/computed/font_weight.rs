//! Parse font weights and resolve relative values from the parent's computed weight.

use super::Computed;

pub(super) fn apply(style: &mut Computed, value: &str) {
    let (weight, step) = match value.to_ascii_lowercase().as_str() {
        "inherit" => (None, 0),
        "normal" => (Some(400), 0),
        "bold" => (Some(700), 0),
        "bolder" => (None, 1),
        "lighter" => (None, -1),
        number => {
            // CSS Fonts 4 section 2.2: weights outside [1, 1000] are invalid.
            // Ignoring them must preserve the preceding cascaded declaration.
            let Some(weight) = number
                .parse::<u16>()
                .ok()
                .filter(|w| (1..=1000).contains(w))
            else {
                return;
            };
            (Some(weight), 0)
        }
    };
    style.font_weight = weight;
    style.font_weight_step = step;
}

pub(crate) fn inherit(result: &mut Computed, parent: &Computed, own: &Computed) {
    // CSS Fonts 4 section 2.2.1: relative weights use the parent's computed
    // number, independently of which physical faces are available.
    let weight = parent.font_weight.unwrap_or(400);
    result.font_weight = match own.font_weight_step {
        1 => Some(match weight {
            0..350 => 400,
            350..550 => 700,
            550..900 => 900,
            _ => weight,
        }),
        -1 => Some(match weight {
            0..100 => weight,
            100..550 => 100,
            550..750 => 400,
            _ => 700,
        }),
        _ => own.font_weight.or(parent.font_weight),
    };
    // Descendants inherit the resolved number, not another relative step.
    result.font_weight_step = 0;
}
