//! Применение к GPUI: проекция потока сетки на физические оси после обмена
//! шаблонов дорожек при вертикальном письме. По `writing-mode`, `direction`
//! и `text-orientation` считается, какая физическая ось идёт в обратную
//! сторону; прямостоящий вертикальный текст даёт используемое направление LTR,
//! боковое письмо ориентацию не учитывает.
use crate::style::computed::Computed;

pub(super) fn reversed(c: &Computed) -> [bool; 2] {
    c.wm().reversed_axes()
}

pub(super) fn block(c: &Computed) -> [bool; 3] {
    let vertical = c.wm().vertical();
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
                writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
                    vertical, rl, sideways,
                )),
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
            writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
                true, false, false,
            )),
            rtl: Some(true),
            text_orientation: Some(crate::style::computed::writing_mode::TextOrientation::Upright),
            ..Computed::default()
        };
        assert_eq!(reversed(&c), [false, false]);
        c.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            false,
            c.wm().block_rl(),
            c.wm().sideways(),
        ));
        assert_eq!(reversed(&c), [true, false]);
        c.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, true, true,
        ));
        assert_eq!(reversed(&c), [true, true]);
    }
}
