//! Фильтр необходимых токенов предков: коллизии допускают только лишний матчинг.

use super::Ancestor;
use crate::style::css::Selector;

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Bloom([u64; 4]);

impl Bloom {
    fn insert(&mut self, kind: u8, value: &str) {
        let mut hash = u64::from(kind) + 0xcbf29ce484222325;
        for byte in value.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        for shift in [0, 16] {
            let bit = ((hash >> shift) & 255) as usize;
            self.0[bit / 64] |= 1 << (bit % 64);
        }
    }

    fn relational(&mut self, id: u64) {
        for shift in [0, 16] {
            let bit = ((id >> shift) & 255) as usize;
            self.0[bit / 64] |= 1 << (bit % 64);
        }
    }

    pub fn with_node(mut self, node: &Ancestor) -> Self {
        if let Some(id) = &node.id {
            self.insert(0, id);
        }
        for class in &node.classes {
            self.insert(1, class);
        }
        self.insert(2, &node.tag.to_ascii_lowercase());
        for &id in node.has_marks.iter().chain(&node.shadow_has_marks) {
            self.relational(id);
        }
        for (name, _) in &node.attrs {
            self.insert(3, &name.to_ascii_lowercase());
        }
        self
    }

    pub fn requirements(sel: &Selector) -> Self {
        let mut out = Self::default();
        let mut next = sel.ancestor.as_deref();
        while let Some((ancestor, _)) = next {
            if let Some(id) = &ancestor.id {
                out.insert(0, id);
            }
            for class in &ancestor.classes {
                out.insert(1, class);
            }
            if let Some(tag) = &ancestor.tag {
                out.insert(2, &tag.to_ascii_lowercase());
            }
            for attr in &ancestor.attrs {
                out.insert(3, &attr.name.to_ascii_lowercase());
            }
            for p in ancestor.pseudo.iter() {
                if let Some(arg) = p.strip_prefix("has(").and_then(|s| s.strip_suffix(')')) {
                    out.relational(super::has::has_id(arg));
                }
            }
            // Токены предыдущего брата не обязаны встречаться у предков.
            next = ancestor.ancestor.as_deref();
        }
        out
    }

    pub fn contains(self, required: Self) -> bool {
        self.0
            .iter()
            .zip(required.0)
            .all(|(have, need)| have & need == need)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sibling_tokens_are_not_required_ancestors() {
        let required = Bloom::requirements(&Selector::parse("#sibling + .parent .leaf").unwrap());
        let mut available = Bloom::default();
        available.insert(1, "parent");
        assert!(available.contains(required));
        assert!(!Bloom::default().contains(required));
    }
}
