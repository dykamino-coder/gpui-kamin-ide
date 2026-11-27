//! A web font file installs under a family name of its own.
//!
//! An `@font-face` rule, not the file's `name` table, names the family
//! (css-fonts-4 section 4.3), and each rule is its own face. The subsets of one
//! family (`unicode-range`) share their internal name: installed as they are,
//! the platform files them as one family and serves a single file for every
//! subset, so text came out in the wrong subset or a fallback font. Blink and
//! Gecko make a typeface straight from each file's data; here every file gets a
//! unique family in a rebuilt `name` table, so every rule addresses its own face.

use std::hash::{Hash, Hasher};

/// Name ids that carry the family (1, 16, 21), the full name (4) and the
/// PostScript name (6); the rest (style names, `fvar`/`STAT` ids) are kept.
const FAMILY_IDS: [u16; 5] = [1, 4, 6, 16, 21];

/// A family name derived from the file bytes: equal files share it.
pub fn unique_family(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("kamin-face-{:016x}", hasher.finish())
}

/// The sfnt with the family records of its `name` table set to `family`;
/// None when the table directory or the `name` table is malformed.
pub fn renamed(bytes: &[u8], family: &str) -> Option<Vec<u8>> {
    let be16 = |at: usize| Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let be32 = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let count = be16(4)? as usize;
    let mut tables = Vec::with_capacity(count);
    for i in 0..count {
        let rec = 12 + i * 16;
        let tag: [u8; 4] = bytes.get(rec..rec + 4)?.try_into().ok()?;
        let (off, len) = (be32(rec + 8)? as usize, be32(rec + 12)? as usize);
        let data = bytes.get(off..off.checked_add(len)?)?;
        tables.push((tag, data.to_vec()));
    }
    let name = tables.iter_mut().find(|(tag, _)| tag == b"name")?;
    name.1 = name_table(&name.1, family)?;
    tables.sort_by_key(|(tag, _)| *tag);
    Some(assemble(be32(0)?, tables))
}

/// The `name` table with the `FAMILY_IDS` strings replaced (format 0; the
/// language-tag records of format 1 are dropped with the tags).
fn name_table(table: &[u8], family: &str) -> Option<Vec<u8>> {
    let be16 = |at: usize| Some(u16::from_be_bytes(table.get(at..at + 2)?.try_into().ok()?));
    let strings = be16(4)? as usize;
    let mut records = Vec::new();
    for i in 0..be16(2)? as usize {
        let rec = 6 + i * 12;
        let field = |k: usize| be16(rec + k);
        let (platform, encoding, language) = (field(0)?, field(2)?, field(4)?);
        let (id, len, off) = (field(6)?, field(8)?, field(10)?);
        if language >= 0x8000 {
            continue;
        }
        let text = if FAMILY_IDS.contains(&id) {
            match platform {
                0 | 3 => family.encode_utf16().flat_map(u16::to_be_bytes).collect(),
                _ => family.as_bytes().to_vec(),
            }
        } else {
            let at = strings + off as usize;
            table.get(at..at + len as usize)?.to_vec()
        };
        records.push(([platform, encoding, language, id], text));
    }
    let mut out = Vec::new();
    let push = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_be_bytes());
    push(&mut out, 0);
    push(&mut out, records.len() as u16);
    push(&mut out, (6 + records.len() * 12) as u16);
    let mut pool: Vec<u8> = Vec::new();
    for (key, text) in &records {
        key.iter().for_each(|v| push(&mut out, *v));
        push(&mut out, text.len() as u16);
        push(&mut out, pool.len() as u16);
        pool.extend_from_slice(text);
    }
    out.extend_from_slice(&pool);
    Some(out)
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, c| {
        let mut word = [0u8; 4];
        word[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// A table directory and 4-byte aligned tables, with the `head` checksum
/// adjustment recomputed (OpenType `head`: 0xB1B0AFBA minus the file sum).
fn assemble(version: u32, mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    if let Some((_, head)) = tables.iter_mut().find(|(tag, _)| tag == b"head")
        && head.len() >= 12
    {
        head[8..12].fill(0);
    }
    let n = tables.len();
    let log = (usize::BITS - 1 - n.max(1).leading_zeros()) as u16;
    let range = 16u16 << log;
    let mut out = Vec::new();
    out.extend_from_slice(&version.to_be_bytes());
    for v in [n as u16, range, log, (n as u16 * 16).saturating_sub(range)] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    let mut offset = 12 + n * 16;
    let mut head_at = None;
    for (tag, data) in &tables {
        if tag == b"head" {
            head_at = Some(offset);
        }
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    if let Some(at) = head_at.filter(|at| out.len() >= at + 12) {
        let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[at + 8..at + 12].copy_from_slice(&adjust.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_record(id: u16, text: &str) -> ([u16; 4], Vec<u8>) {
        (
            [3, 1, 0x409, id],
            text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        )
    }

    fn font(names: &[([u16; 4], Vec<u8>)]) -> Vec<u8> {
        let mut name: Vec<u8> = vec![0, 0];
        name.extend((names.len() as u16).to_be_bytes());
        name.extend(((6 + names.len() * 12) as u16).to_be_bytes());
        let mut pool: Vec<u8> = Vec::new();
        for (key, text) in names {
            key.iter().for_each(|v| name.extend(v.to_be_bytes()));
            name.extend((text.len() as u16).to_be_bytes());
            name.extend((pool.len() as u16).to_be_bytes());
            pool.extend(text);
        }
        name.extend(pool);
        let head = vec![0, 1, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 0x5F, 0x0F, 0x3C, 0xF5];
        assemble(0x0001_0000, vec![(*b"head", head), (*b"name", name)])
    }

    #[test]
    fn family_records_take_the_unique_name_and_style_names_stay() {
        let bytes = font(&[
            name_record(1, "Hanken Grotesk"),
            name_record(2, "Bold"),
            name_record(16, "Hanken Grotesk"),
        ]);
        let unique = unique_family(&bytes);
        assert_eq!(unique, unique_family(&bytes.clone()));
        let out = renamed(&bytes, &unique).unwrap();
        assert_eq!(
            crate::text::fonts::sfnt_family(&out).as_deref(),
            Some(unique.as_str())
        );
        let style: Vec<u8> = "Bold".encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert!(out.windows(style.len()).any(|w| w == style.as_slice()));
        // The whole file sums to the OpenType magic once the adjustment is in.
        assert_eq!(checksum(&out), 0xB1B0_AFBA);
        assert_eq!(renamed(&bytes[..20], &unique), None);
    }
}
