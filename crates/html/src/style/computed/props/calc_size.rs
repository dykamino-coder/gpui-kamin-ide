//! Разбор `calc-size()` в `width`/`height`/`min-*`/`max-*`: базис и выражение хранятся
//! вместе, чтобы пройти каскад и наследование одной парой.

use crate::style::computed::*;

pub(super) fn apply(
    c: &mut Computed,
    key: &str,
    value: &str,
    vcx: crate::document::state::ValueCx<'_>,
) -> bool {
    let i = match key {
        "width" => 0,
        "height" => 1,
        "min-width" => 2,
        "min-height" => 3,
        "max-width" => 4,
        "max-height" => 5,
        _ => return false,
    };
    let parsed = calc_size_arg(value);
    if parsed.is_none() && value.starts_with("calc-size(") {
        return true;
    }
    let expression = if i < 4 {
        c.calc_size[i]
    } else {
        c.calc_size_max[i - 4]
    };
    if parsed.is_none() && expression.is_none() {
        return false;
    }
    if parsed.is_none()
        && super::super::size_range::parse_nonnegative_size(value, vcx).is_none()
        && fit_content_arg(value, vcx).is_none()
        && !matches!(value, "inherit" | "none")
        && !stretch_keyword(value)
    {
        return true;
    }
    c.calc_size_basis[i] = None;
    if i < 4 {
        c.calc_size[i] = None;
    } else {
        c.calc_size_max[i - 4] = None;
    }
    let Some(parsed) = parsed else { return false };
    match i {
        0 => {
            c.width_inherit = false;
            c.stretch_size[0] = false;
            c.fit_arg[0] = None;
        }
        1 => {
            c.height_inherit = false;
            c.stretch_size[1] = false;
        }
        _ => {
            c.minmax_inherit[i - 2] = false;
            c.stretch_size[[2, 4, 3, 5][i - 2]] = false;
            if i == 2 {
                c.fit_arg[1] = None;
            }
            if i == 4 {
                c.fit_arg[2] = None;
            }
        }
    }
    let slot = match i {
        0 => &mut c.width,
        1 => &mut c.height,
        2 => &mut c.min_width,
        3 => &mut c.min_height,
        4 => &mut c.max_width,
        _ => &mut c.max_height,
    };
    match parsed {
        CalcSize::Fixed(px) => *slot = Some(Len::Px(px)),
        CalcSize::Over(f, basis) => {
            *slot = if i < 2 { Some(Len::Auto) } else { None };
            c.calc_size_basis[i] = Some(basis);
            if i < 4 {
                c.calc_size[i] = Some(f);
            } else {
                c.calc_size_max[i - 4] = Some(f);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_basis_and_maximum_survive_cascade() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("width", "calc-size(min-content, size + 10px)", vcx);
        assert_eq!(c.calc_size_basis[0], Some(CalcSizeBasis::MinContent));
        c.apply_one("width", "invalid", vcx);
        assert!(c.calc_size[0].is_some());
        c.apply_one("max-width", "calc-size(stretch, size / 2)", vcx);
        assert_eq!(c.calc_size_basis[4], Some(CalcSizeBasis::Stretch));
        assert!(c.calc_size_max[0].is_some());
        c.apply_one("width", "20px", vcx);
        assert_eq!(c.calc_size_basis[0], None);
        assert_eq!(c.calc_size[0], None);
    }
}
