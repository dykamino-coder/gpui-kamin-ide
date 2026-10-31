//! Нормализация HTML-охватов: неотрицательное целое, пределы и граница группы.

use crate::dom::Element;

/// Неотрицательное целое по WHATWG: пробелы ASCII, знак, префикс цифр.
/// Насыщение сохраняет результат HTML-ограничения даже при неограниченном вводе.
fn integer(value: Option<&str>, limit: u16) -> Option<u16> {
    let bytes = value?.as_bytes();
    let mut i = 0;
    while bytes
        .get(i)
        .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
    {
        i += 1;
    }
    let negative = bytes.get(i) == Some(&b'-');
    if negative || bytes.get(i) == Some(&b'+') {
        i += 1;
    }
    if !bytes.get(i).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let limit = u32::from(limit);
    let mut n = 0u32;
    while let Some(b) = bytes.get(i).filter(|b| b.is_ascii_digit()) {
        n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add(u32::from(*b - b'0')))
            .unwrap_or(limit)
            .min(limit);
        i += 1;
    }
    if negative && n != 0 {
        return None;
    }
    u16::try_from(n).ok()
}

pub(super) fn column_span(cell: &Element) -> u16 {
    integer(cell.attr("colspan"), 1000).unwrap_or(1).max(1)
}

pub(super) fn row_span_in_group(cell: &Element, left: usize) -> usize {
    let left = left.max(1);
    match integer(cell.attr("rowspan"), 65534) {
        Some(0) => left,
        Some(n) => usize::from(n).min(left),
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn html_integer_uses_a_signed_ascii_digit_prefix() {
        for (input, expected) in [
            (" +12junk", Some(12)),
            ("\t\n\r\u{c}42", Some(42)),
            ("-0junk", Some(0)),
            ("-000", Some(0)),
            ("-1", None),
            ("+", None),
            ("", None),
            ("junk12", None),
            ("  + 2", None),
            ("\u{b}2", None),
            ("\u{a0}2", None),
            ("１２", None),
        ] {
            assert_eq!(integer(Some(input), 1000), expected, "{input:?}");
        }
        assert_eq!(integer(None, 1000), None);
    }

    #[test]
    fn spans_clamp_before_narrowing_and_keep_zero_rowspan() {
        let mut cell = anon_element("td", vec![]);
        for input in [
            "1000",
            "1001",
            "65535",
            "65536",
            "999999999999999999999999999999999999",
        ] {
            cell.attrs = vec![("colspan".into(), input.into())];
            assert_eq!(column_span(&cell), 1000, "{input}");
        }
        for (input, expected) in [("0", 1), ("-2", 1), ("bad", 1), ("+2tail", 2)] {
            cell.attrs = vec![("colspan".into(), input.into())];
            assert_eq!(column_span(&cell), expected);
        }
        for (input, expected) in [
            ("0", 100_000),
            ("65534", 65534),
            ("65535", 65534),
            ("65536", 65534),
            ("9999999999999999999999999999", 65534),
            ("-2", 1),
        ] {
            cell.attrs = vec![("rowspan".into(), input.into())];
            assert_eq!(row_span_in_group(&cell, 100_000), expected);
            assert_eq!(row_span_in_group(&cell, 3), expected.min(3));
        }
    }
}
