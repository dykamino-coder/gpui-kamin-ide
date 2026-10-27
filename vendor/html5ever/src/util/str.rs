// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;

/// Streams the same escaped Debug representation without intermediate strings.
pub(crate) struct EscapedDebug<'a, T>(pub &'a T);
impl<T: fmt::Debug> fmt::Display for EscapedDebug<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Escape<'a, 'b>(&'a mut fmt::Formatter<'b>);
        impl fmt::Write for Escape<'_, '_> {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                for c in text.chars().flat_map(char::escape_default) {
                    fmt::Write::write_char(self.0, c)?;
                }
                Ok(())
            }
        }
        fmt::write(&mut Escape(f), format_args!("{:?}", self.0))
    }
}

/// If `c` is an ASCII letter, return the corresponding lowercase
/// letter, otherwise None.
pub(crate) fn lower_ascii_letter(c: char) -> Option<char> {
    match c {
        'a'..='z' => Some(c),
        'A'..='Z' => Some((c as u8 - b'A' + b'a') as char),
        _ => None,
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::lower_ascii_letter;

    #[test]
    fn lower_letter_a_is_a() {
        assert_eq!(lower_ascii_letter('a'), Some('a'));
    }

    #[test]
    fn lower_letter_A_is_a() {
        assert_eq!(lower_ascii_letter('A'), Some('a'));
    }

    #[test]
    fn lower_letter_symbol_is_None() {
        assert_eq!(lower_ascii_letter('!'), None);
    }

    #[test]
    fn lower_letter_nonascii_is_None() {
        assert_eq!(lower_ascii_letter('\u{a66e}'), None);
    }
}
