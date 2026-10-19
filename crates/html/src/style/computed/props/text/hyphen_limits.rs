//! CSS Text character limits for dictionary hyphenation.

pub(super) fn parse_hyphenate_limit_chars(value: &str) -> Option<[Option<usize>; 3]> {
    let mut limits = [None; 3];
    let mut count = 0;
    for token in value.split_whitespace() {
        if count == 3 {
            return None;
        }
        limits[count] = if token == "auto" {
            None
        } else {
            Some(token.parse().ok()?)
        };
        count += 1;
    }
    if count == 0 {
        return None;
    }
    if count == 2 {
        limits[2] = limits[1];
    }
    Some(limits)
}

#[cfg(test)]
mod tests {
    use super::parse_hyphenate_limit_chars;

    #[test]
    fn parses_shorthand_and_rejects_invalid_values() {
        assert_eq!(
            parse_hyphenate_limit_chars("auto 3"),
            Some([None, Some(3), Some(3)])
        );
        assert_eq!(
            parse_hyphenate_limit_chars("8"),
            Some([Some(8), None, None])
        );
        assert_eq!(
            parse_hyphenate_limit_chars("0 2 4"),
            Some([Some(0), Some(2), Some(4)])
        );
        for invalid in ["", "-1", "3px", "auto 2 3 4"] {
            assert_eq!(parse_hyphenate_limit_chars(invalid), None);
        }
    }
}
