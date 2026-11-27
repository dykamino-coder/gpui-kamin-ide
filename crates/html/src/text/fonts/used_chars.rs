//! The code points a document can set, so a `unicode-range` subset loads only
//! when the page uses a character in it (css-fonts-4 section 4.5; Blink loads a
//! `FontFace` on demand, Gecko a `gfxUserFontEntry`).
//!
//! Faces load before layout, when the rendered text is not known yet, so the
//! set is a superset: every text node and attribute value of the DOM, the style
//! sheets with their escapes decoded (generated `content`, icon code points),
//! the case-mapped forms of those characters (`text-transform`) and the
//! characters the engine inserts itself (list markers, quotes, hyphens, the
//! ellipsis). Every revision rebuilds the document from its DOM, so text a
//! script adds is counted at the next revision and its subset loads then.

use markup5ever_rcdom::{Handle, NodeData};
use std::collections::BTreeSet;

/// Characters the engine can generate whatever the source says.
const GENERATED: &str = "\u{a0}\u{ab}\u{bb}\u{2010}\u{2011}\u{2013}\u{2014}\u{2018}\u{2019}\
    \u{201c}\u{201d}\u{2022}\u{2026}\u{2039}\u{203a}\u{25aa}\u{25b8}\u{25be}\u{25e6} 0123456789";

pub(crate) struct UsedChars {
    bmp: Vec<u64>,
    astral: BTreeSet<u32>,
}

impl UsedChars {
    fn new() -> Self {
        let mut used = Self {
            bmp: vec![0; 0x10000 / 64],
            astral: BTreeSet::new(),
        };
        used.add(GENERATED);
        used
    }

    /// The characters of `document` and its style `sheets`.
    pub(crate) fn of_document(document: &Handle, sheets: &[impl AsRef<str>]) -> Self {
        let mut used = Self::new();
        for sheet in sheets {
            used.add_css(sheet.as_ref());
        }
        // An explicit stack: documents nest deeper than the thread stack allows.
        let mut stack = vec![document.clone()];
        while let Some(node) = stack.pop() {
            match &node.data {
                NodeData::Text { contents } => used.add(&contents.borrow()),
                NodeData::Element {
                    attrs,
                    template_contents,
                    ..
                } => {
                    for attr in attrs.borrow().iter() {
                        used.add(&attr.value);
                    }
                    if let Some(content) = template_contents.borrow().as_ref() {
                        stack.push(content.clone());
                    }
                }
                _ => {}
            }
            stack.extend(node.children.borrow().iter().cloned());
        }
        used
    }

    #[cfg(test)]
    pub(crate) fn of_text(text: &str) -> Self {
        let mut used = Self::new();
        used.add_css(text);
        used
    }

    fn add(&mut self, text: &str) {
        for c in text.chars() {
            self.insert(c);
            if c.is_ascii() {
                self.insert(c.to_ascii_uppercase());
                self.insert(c.to_ascii_lowercase());
            } else {
                for mapped in c.to_uppercase().chain(c.to_lowercase()) {
                    self.insert(mapped);
                }
            }
        }
    }

    /// Style sheet text plus the code points of its escapes (`\f007`).
    fn add_css(&mut self, css: &str) {
        self.add(css);
        let bytes = css.as_bytes();
        let mut i = 0;
        while let Some(at) = css[i..].find('\\') {
            let start = i + at + 1;
            let len = bytes[start..]
                .iter()
                .take(6)
                .take_while(|b| b.is_ascii_hexdigit())
                .count();
            if let Some(c) = u32::from_str_radix(&css[start..start + len], 16)
                .ok()
                .and_then(char::from_u32)
            {
                self.add(&c.to_string());
            }
            // Past the backslash and its hex digits: always a char boundary.
            i = start + len;
        }
    }

    fn insert(&mut self, c: char) {
        let c = c as u32;
        if c < 0x10000 {
            self.bmp[(c >> 6) as usize] |= 1 << (c & 63);
        } else {
            self.astral.insert(c);
        }
    }

    /// Whether any used character lies in `ranges` (empty: every code point).
    pub(crate) fn intersects(&self, ranges: &[(u32, u32)]) -> bool {
        ranges.is_empty() || ranges.iter().any(|&(lo, hi)| self.any_in(lo, hi))
    }

    fn any_in(&self, lo: u32, hi: u32) -> bool {
        if lo < 0x10000 {
            let top = hi.min(0xFFFF);
            for word in (lo >> 6)..=(top >> 6) {
                let mut bits = self.bmp[word as usize];
                if word == lo >> 6 {
                    bits &= !0u64 << (lo & 63);
                }
                if word == top >> 6 {
                    bits &= !0u64 >> (63 - (top & 63));
                }
                if bits != 0 {
                    return true;
                }
            }
        }
        hi >= 0x10000 && self.astral.range(lo.max(0x10000)..=hi).next().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsets_load_only_for_characters_the_page_can_set() {
        let used = UsedChars::of_text("plain text .icon::before { content: \"\\f007\" } \u{1F600}");
        assert!(used.intersects(&[(0x20, 0x7F)]));
        assert!(used.intersects(&[]));
        // Cyrillic and Greek never occur; the escaped icon and the emoji do.
        assert!(!used.intersects(&[(0x400, 0x4FF), (0x370, 0x3FF)]));
        assert!(used.intersects(&[(0xF000, 0xF0FF)]));
        assert!(used.intersects(&[(0x1F600, 0x1F64F)]));
        assert!(!used.intersects(&[(0x1F680, 0x1F6FF)]));
        // Case mapping and generated characters count.
        assert!(UsedChars::of_text("ß").intersects(&[(0x53, 0x53)]));
        assert!(UsedChars::of_text("").intersects(&[(0x2026, 0x2026)]));
    }
}
