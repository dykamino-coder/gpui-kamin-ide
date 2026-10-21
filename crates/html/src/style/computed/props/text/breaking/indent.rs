//! Полный разбор длины и модификаторов отступа перед присваиванием.
use super::*;
pub(super) fn apply(style: &mut Computed, value: &str, vcx: ValueCx<'_>) {
    if value == "inherit" {
        text_indent::apply(style, value, vcx);
        return;
    }
    let (mut length, mut each, mut hanging) = (None, false, false);
    for word in split_outside_parens(value) {
        match word.as_str() {
            "each-line" if !each => each = true,
            "hanging" if !hanging => hanging = true,
            token if length.is_none() => {
                let Some(parsed) = Len::parse_mixed(token, vcx).filter(|l| {
                    !matches!(
                        l,
                        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                    )
                }) else {
                    return;
                };
                length = Some(parsed);
            }
            _ => return,
        }
    }
    let length = parsed_value!(style, length);
    style.text_indent = Some(length);
    style.text_indent_each_line = each.then_some(true);
    style.text_indent_hanging = hanging.then_some(true);
}
