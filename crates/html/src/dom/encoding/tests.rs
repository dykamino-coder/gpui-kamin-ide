//! Tests for encoding; split out to keep the owning module within 250 lines.

use encoding_rs::{UTF_8, WINDOWS_1252};

use super::*;

#[test]
fn bom_wins_over_meta() {
    assert_eq!(sniff(b"\xEF\xBB\xBF<meta charset=windows-1251>"), UTF_8);
}

#[test]
fn meta_charset_is_read() {
    assert_eq!(
        sniff(b"<!DOCTYPE html><meta charset='iso8859-2'>"),
        encoding_rs::ISO_8859_2
    );
}

#[test]
fn charset_inside_comment_is_ignored() {
    assert_eq!(
        sniff(b"<!-- <meta charset=iso8859-2> --><meta charset=utf-8>"),
        UTF_8
    );
}

#[test]
fn charset_inside_other_attribute_is_ignored() {
    assert_eq!(
        sniff(b"<meta test=\" charset=iso8859-2>\n<p>\"</p>"),
        WINDOWS_1252
    );
}

#[test]
fn unterminated_tag_gives_nothing() {
    assert_eq!(sniff(b"<meta charset=euc-jp"), WINDOWS_1252);
}

#[test]
fn default_is_windows_1252() {
    assert_eq!(sniff(b"<!DOCTYPE html><p>text"), WINDOWS_1252);
}
