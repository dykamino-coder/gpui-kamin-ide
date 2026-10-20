//! Absolute lengths inside a basic-shape value (`shape-outside`) under zoom.
//!
//! The value is kept as its source text and parsed at layout, so the zoom
//! pass multiplies the `px` lengths in the text (css-viewport-1 §zoom:
//! the used value of every length is multiplied by the effective zoom).
//! Percentages, keywords and `url()` images are left alone.

/// `s` with every `<number>px` multiplied by `k`.
pub(super) fn scale_px(s: &str, k: f32) -> String {
    if s.contains("url(") {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let digit_at = |j: usize| j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.');
        let starts = c.is_ascii_digit()
            || (c == b'.' && digit_at(i + 1))
            || ((c == b'-' || c == b'+') && digit_at(i + 1));
        let in_ident = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if !starts || in_ident {
            let ch = s[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let mut j = i + 1;
        while digit_at(j) {
            j += 1;
        }
        let unit_px =
            s[j..].starts_with("px") && !b.get(j + 2).is_some_and(u8::is_ascii_alphabetic);
        match s[i..j].parse::<f32>() {
            Ok(v) if unit_px => {
                out.push_str(&format!("{}px", v * k));
                i = j + 2;
            }
            _ => {
                out.push_str(&s[i..j]);
                i = j;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::scale_px;

    #[test]
    fn scales_px_lengths_only() {
        assert_eq!(
            scale_px("circle(25px at 50px 50px)", 2.0),
            "circle(50px at 100px 100px)"
        );
        assert_eq!(
            scale_px("inset(10% 5px round 2.5px)", 2.0),
            "inset(10% 10px round 5px)"
        );
        assert_eq!(
            scale_px("polygon(-10px 0, 0 1em)", 3.0),
            "polygon(-30px 0, 0 1em)"
        );
        assert_eq!(scale_px("url(a10px.png)", 2.0), "url(a10px.png)");
    }
}
