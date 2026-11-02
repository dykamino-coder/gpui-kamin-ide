//! Типизированные размеры: все компоненты проверяются до изменения полей стиля.

use crate::style::computed::*;
use crate::values::ValueCx;

impl Computed {
    pub(super) fn apply_physical_size(&mut self, i: usize, value: &str, vcx: ValueCx<'_>) {
        let parsed = parsed_value!(self, ParsedSize::parse(value, i >= 4, vcx));
        self.reset_box_layout.stretch_size[[0, 1, 2, 4, 3, 5][i]] = parsed.stretch;
        if let Some(fit_index) = match i {
            0 => Some(0),
            2 => Some(1),
            4 => Some(2),
            _ => None,
        } {
            self.reset_context.fit_arg[fit_index] = parsed.fit;
        }
        let group = &mut *self.reset_box_layout;
        group.calc_size_basis[i] = None;
        let expression = if i < 4 {
            &mut group.calc_size[i]
        } else {
            &mut group.calc_size_max[i - 4]
        };
        *expression = None;
        let slot = match i {
            0 => &mut group.width,
            1 => &mut group.height,
            2 => &mut group.min_width,
            3 => &mut group.min_height,
            4 => &mut group.max_width,
            _ => &mut group.max_height,
        };
        *slot = parsed.length;
        match parsed.calc {
            Some(CalcSize::Fixed(px)) => *slot = Some(Len::Px(px)),
            Some(CalcSize::Over(f, basis)) => {
                *slot = if i < 2 { Some(Len::Auto) } else { None };
                group.calc_size_basis[i] = Some(basis);
                *expression = Some(f);
            }
            None => {}
        }
    }
}

#[derive(Default)]
pub(super) struct ParsedSize {
    pub(super) length: Option<Len>,
    fit: Option<Len>,
    stretch: bool,
    calc: Option<CalcSize>,
}

impl ParsedSize {
    pub(super) fn parse(value: &str, allow_none: bool, vcx: ValueCx<'_>) -> Option<Self> {
        let lower = value.to_ascii_lowercase();
        if let Some(arg) = lower.strip_prefix("fit-content(") {
            size_range::parse_nonnegative_size(arg.strip_suffix(')')?, vcx).filter(|l| {
                !matches!(
                    l,
                    Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                )
            })?;
        }
        let length = size_range::parse_nonnegative_size(value, vcx);
        let fit = fit_content_arg(value, vcx);
        let stretch = stretch_keyword(value);
        let calc = calc_size_arg(value);
        (length.is_some()
            || fit.is_some()
            || stretch
            || calc.is_some()
            || value == "inherit"
            || (allow_none && value.eq_ignore_ascii_case("none")))
        .then_some(Self {
            length,
            fit,
            stretch,
            calc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_basis_and_maximum_survive_cascade() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("width", "calc-size(min-content, size + 10px)", vcx);
        assert_eq!(
            c.reset_box_layout.calc_size_basis[0],
            Some(CalcSizeBasis::MinContent)
        );
        c.apply_one("width", "invalid", vcx);
        assert!(c.reset_box_layout.calc_size[0].is_some());
        c.apply_one("max-width", "calc-size(stretch, size / 2)", vcx);
        assert_eq!(
            c.reset_box_layout.calc_size_basis[4],
            Some(CalcSizeBasis::Stretch)
        );
        assert!(c.reset_box_layout.calc_size_max[0].is_some());
        c.apply_one("width", "20px", vcx);
        assert_eq!(c.reset_box_layout.calc_size_basis[0], None);
        assert_eq!(c.reset_box_layout.calc_size[0], None);
    }
}
