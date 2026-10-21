//! Вычисленный стиль: `tab-size` (CSS Text 3 §4.2) с различием числа и длины —
//! число наследуется множителем ширины пробела, длина хранится отдельным полем
//! и наследуется абсолютной величиной; отрицательные и неконечные значения
//! негодны и не затирают прежнее объявление.

use super::Computed;
use crate::document::state::ValueCx;
use crate::style::values::value::{Len, css_number};

pub(super) fn apply(style: &mut Computed, value: &str, vcx: ValueCx<'_>) {
    if let Some(number) = css_number(value) {
        if number.is_finite() && number >= 0.0 {
            style.accept_declaration();
            style.tab_size = Some(number);
            style.tab_size_len = None;
        }
    } else if let Some(length) = Len::parse(value, vcx) {
        let valid = match length {
            Len::Px(v)
            | Len::Em(v)
            | Len::Ch(v)
            | Len::Ex(v)
            | Len::Ic(v)
            | Len::Lh(v)
            | Len::Vh(v)
            | Len::Vw(v) => v.is_finite() && v >= 0.0,
            Len::Calc(_) | Len::EmPx(_, _) | Len::LhPx(_, _) => true,
            _ => false,
        };
        if valid {
            style.accept_declaration();
            style.tab_size_len = Some(length);
            style.tab_size = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_numbers_and_zero_keep_their_numeric_type() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        for number in ["2.5", ".5", "0", "+4", "2e1"] {
            apply(&mut style, number, vcx);
            assert_eq!(style.tab_size, css_number(number));
            assert_eq!(style.tab_size_len, None);
        }
    }

    #[test]
    fn invalid_values_do_not_replace_a_valid_declaration() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        apply(&mut style, "2.5", vcx);
        for invalid in ["-1", "-1px", "10%", "auto", "NaN", "inf", "6.", "1e99"] {
            apply(&mut style, invalid, vcx);
            assert_eq!(style.tab_size, Some(2.5), "{invalid}");
            assert_eq!(style.tab_size_len, None, "{invalid}");
        }
    }

    #[test]
    fn successive_number_and_length_declarations_cancel_each_other() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        apply(&mut style, "40px", vcx);
        assert_eq!(style.tab_size_len, Some(Len::Px(40.0)));
        assert_eq!(style.tab_size, None);
        apply(&mut style, "2.5", vcx);
        assert_eq!(style.tab_size, Some(2.5));
        assert_eq!(style.tab_size_len, None);
    }
}
