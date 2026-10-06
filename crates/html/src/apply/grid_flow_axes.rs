//! Project CSS grid flow onto physical axes after swapping vertical track templates.
use crate::computed::Computed;

pub(super) fn reversed(c: &Computed) -> [bool; 2] {
    let vertical = c.vertical == Some(true);
    let sideways = c.sideways == Some(true);
    // Upright vertical text forces the used direction to LTR; sideways writing ignores orientation.
    let rtl = c.rtl == Some(true) && !(vertical && !sideways && c.upright == Some(true));
    if vertical {
        [
            c.vertical_rl == Some(true),
            rtl != (sideways && c.vertical_rl != Some(true)),
        ]
    } else {
        [rtl, false]
    }
}

pub(super) fn block(c: &Computed) -> [bool; 3] {
    let vertical = c.vertical == Some(true);
    let axes = reversed(c);
    [vertical, vertical && axes[0], axes[usize::from(vertical)]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_modes_project_grid_flow_to_physical_axes() {
        for (vertical, rl, sideways, rtl, expected) in [
            (false, false, false, false, [false, false]),
            (false, false, false, true, [true, false]),
            (true, false, false, false, [false, false]),
            (true, false, false, true, [false, true]),
            (true, true, false, false, [true, false]),
            (true, true, false, true, [true, true]),
            (true, false, true, false, [false, true]),
            (true, false, true, true, [false, false]),
            (true, true, true, false, [true, false]),
        ] {
            let c = Computed {
                vertical: Some(vertical),
                vertical_rl: Some(rl),
                sideways: Some(sideways),
                rtl: Some(rtl),
                ..Computed::default()
            };
            assert_eq!(reversed(&c), expected);
            assert_eq!(
                block(&c),
                [
                    vertical,
                    vertical && expected[0],
                    expected[usize::from(vertical)]
                ]
            );
        }
    }

    #[test]
    fn upright_uses_ltr_only_in_vertical_writing() {
        let mut c = Computed {
            vertical: Some(true),
            vertical_rl: Some(false),
            rtl: Some(true),
            upright: Some(true),
            ..Computed::default()
        };
        assert_eq!(reversed(&c), [false, false]);
        c.vertical = Some(false);
        assert_eq!(reversed(&c), [true, false]);
        c.vertical = Some(true);
        c.sideways = Some(true);
        c.vertical_rl = Some(true);
        assert_eq!(reversed(&c), [true, true]);
    }
}
