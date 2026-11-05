//! Полный ключ поддерева без рекурсивного форматирования Debug.
use crate::dom::{Element, Node};
use std::hash::{Hash, Hasher};
pub(super) fn hash_tree(root: &Element, hash: &mut impl Hasher) {
    let mut pending = vec![root.children.iter()];
    hash_element(root, hash);
    while let Some(children) = pending.last_mut() {
        match children.next() {
            Some(Node::Element(e)) => {
                hash_element(e, hash);
                pending.push(e.children.iter());
            }
            Some(Node::Text(text)) => {
                1u8.hash(hash);
                format!("{text:?}").hash(hash);
            }
            None => {
                2u8.hash(hash);
                pending.pop();
            }
        }
    }
}
fn hash_element(e: &Element, hash: &mut impl Hasher) {
    0u8.hash(hash);
    let mut header = e.clone();
    header.children = Default::default();
    format!("{header:?}").hash(hash);
    e.children.len().hash(hash);
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(e: &Element) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        hash_tree(e, &mut h);
        h.finish()
    }
    #[test]
    fn nested_content_and_order_change_the_key() {
        let a = Element::synthetic(
            "div",
            Default::default(),
            Default::default(),
            vec![Node::Text("one".into())],
        );
        let mut b = a.clone();
        assert_eq!(key(&a), key(&b));
        b.children = vec![Node::Text("two".into())].into();
        assert_ne!(key(&a), key(&b));
        b.children = vec![Node::Element(a.clone())].into();
        assert_ne!(key(&a), key(&b));
    }
    #[test]
    fn twenty_thousand_levels_hash_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut e =
                    Element::synthetic("div", Default::default(), Default::default(), vec![]);
                for _ in 0..20_000 {
                    e = Element::synthetic(
                        "g",
                        Default::default(),
                        Default::default(),
                        vec![Node::Element(e)],
                    );
                }
                assert_eq!(key(&e), key(&e.clone()));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
