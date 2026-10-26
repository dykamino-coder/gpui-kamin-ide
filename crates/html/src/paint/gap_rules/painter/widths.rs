//! Толщина декораций на сетке устройства: CSS Values 4 §lengths, snap as a border width.
//! Одинаковая толщина нужна и полосе, и пересечению с overlap-join (CSS Gaps 1 §inset).

use crate::layout::multicol::gap_rules::GapRuleSpec;

fn snapped_width(width: f32, scale: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    (width * scale).floor().max(1.0) / scale
}

pub(super) fn snapped_spec(spec: &GapRuleSpec, scale: f32) -> GapRuleSpec {
    let scale = scale.max(0.01);
    let mut spec = spec.clone();
    for rule in [&mut spec.col, &mut spec.row].into_iter().flatten() {
        rule.widths = rule.widths.map(|width| snapped_width(*width, scale));
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::snapped_width;

    #[test]
    fn border_width_snapping_preserves_zero_and_rounds_nonzero_device_pixels() {
        for (scale, width, expected) in [
            (1.0, 0.0, 0.0),
            (1.0, 0.25, 1.0),
            (1.0, 1.5, 1.0),
            (1.0, 6.0, 6.0),
            (2.0, 0.25, 0.5),
            (2.0, 1.5, 1.5),
            (2.0, 1.75, 1.5),
            (1.25, 1.5, 0.8),
        ] {
            assert_eq!(snapped_width(width, scale), expected);
        }
    }
}
