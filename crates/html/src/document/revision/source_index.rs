//! The current values binding map replaces the ordinal source-address index.
#[cfg(test)]
mod tests {
    #[test]
    fn current_bindings_agree_with_the_address_scan_and_reject_foreign_nodes() {
        let doc =
            crate::document::Document::builder("<div><p>a</p><p>b<span>c</span></p></div>", "")
                .build()
                .unwrap();
        let bindings = doc.state.values.traversal.bindings();
        let list: Vec<_> = bindings.elements().collect();
        let mut pending = vec![doc.source_dom()];
        let mut seen = 0;
        while let Some(node) = pending.pop() {
            let address = std::rc::Rc::as_ptr(&node) as usize;
            let scanned = list
                .iter()
                .find(|(key, ptr)| *key == node.key() && *ptr == address)
                .map(|(key, _)| key.packed());
            assert_eq!(bindings.id(&node), scanned);
            seen += usize::from(scanned.is_some());
            pending.extend(node.children.borrow().iter().cloned());
        }
        assert!(seen >= 4, "{seen}");
        let foreign = crate::document::Document::builder("<div>x</div>", "")
            .build()
            .unwrap();
        let mut pending = vec![foreign.source_dom()];
        while let Some(node) = pending.pop() {
            assert_eq!(bindings.id(&node), None);
            pending.extend(node.children.borrow().iter().cloned());
        }
    }
}
