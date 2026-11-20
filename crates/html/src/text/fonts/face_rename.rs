//! A private family name for one `@font-face` file of a composite face.
//!
//! Subset files of one family (`unicode-range` faces, css-fonts-4 §4.5)
//! carry the same family and style names, and the platform collection keeps
//! only one face per name and style: the text system could then shape every
//! character with whichever subset it found first. Each such file is
//! installed under its own name instead, so the composite face can list its
//! parts in order (Blink keeps one `FontData` per segment,
//! `CSSSegmentedFontFace`; Gecko one `gfxFontEntry` per rule). Only the
//! `name` table changes: its family, full and PostScript names.

/// `bytes` (an sfnt) with family name `family`; `None` when it is not a
/// plain sfnt (a collection, or a table directory out of bounds).
pub(crate) fn renamed(bytes: &[u8], family: &str) -> Option<Vec<u8>> {
    let u16_at = |at: usize| Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    if bytes.get(0..4)? == b"ttcf" {
        return None;
    }
    let tables = usize::from(u16_at(4)?);
    let record = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&at| bytes.get(at..at + 4) == Some(b"name"))?;
    bytes.get(..12 + 16 * tables)?;
    let mut out = bytes.to_vec();
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let table = name_table(family);
    let offset = u32::try_from(out.len()).ok()?;
    let length = u32::try_from(table.len()).ok()?;
    out.extend_from_slice(&table);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out[record + 4..record + 8].copy_from_slice(&checksum(&table).to_be_bytes());
    out[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
    out[record + 12..record + 16].copy_from_slice(&length.to_be_bytes());
    Some(out)
}

/// A format 0 `name` table with Windows Unicode BMP records (platform 3,
/// encoding 1, US English): family (1), subfamily (2), full (4) and
/// PostScript (6) names.
fn name_table(family: &str) -> Vec<u8> {
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_be_bytes).collect() };
    let names = [
        (1u16, utf16(family)),
        (2, utf16("Regular")),
        (4, utf16(family)),
        (6, utf16(family)),
    ];
    let count = names.len() as u16;
    let storage = 6 + 12 * count;
    let mut out = Vec::new();
    for v in [0u16, count, storage] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    let mut at = 0u16;
    for (id, text) in &names {
        for v in [3u16, 1, 0x409, *id, text.len() as u16, at] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        at += text.len() as u16;
    }
    for (_, text) in &names {
        out.extend_from_slice(text);
    }
    out
}

fn checksum(table: &[u8]) -> u32 {
    table.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// The private family of a subset file: stable for the same bytes.
pub(crate) fn private_family(bytes: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    format!("kamin-face-{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sfnt(tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut out = vec![0, 1, 0, 0];
        out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0; 6]);
        let mut data = Vec::new();
        let start = 12 + 16 * tables.len();
        for (tag, body) in tables {
            out.extend_from_slice(*tag);
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&((start + data.len()) as u32).to_be_bytes());
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            data.extend_from_slice(body);
        }
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn renamed_font_reports_the_private_family() {
        let font = sfnt(&[(b"head", &[7; 8]), (b"name", &name_table("Roboto"))]);
        assert_eq!(
            crate::text::fonts::sfnt_family(&font).as_deref(),
            Some("Roboto")
        );
        let out = renamed(&font, "kamin-face-1").unwrap();
        assert_eq!(out.len() % 4, 0);
        assert_eq!(
            crate::text::fonts::sfnt_family(&out).as_deref(),
            Some("kamin-face-1")
        );
        // Other tables keep their records and bytes.
        assert_eq!(&out[12..28], &font[12..28]);
        assert_eq!(&out[44..52], &font[44..52]);
    }

    #[test]
    fn collections_and_truncated_fonts_are_left_alone() {
        assert!(renamed(b"ttcf\0\0\0\0", "x").is_none());
        assert!(renamed(&[0, 1, 0, 0, 0, 3], "x").is_none());
        let a = private_family(b"a");
        assert_eq!(a, private_family(b"a"));
        assert_ne!(a, private_family(b"b"));
    }
}
