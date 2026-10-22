//! Regression tests for bounded external resource sessions.
// KaminIDE patch: bounded file reads and shared resource session regressions.
mod byte_limit_tests {
    use super::super::read_bounded;
    use std::cell::Cell;
    use std::io::{self, Read};

    #[test]
    fn accepts_the_limit_and_rejects_one_extra_byte() {
        assert_eq!(read_bounded(&b"abcd"[..], 4), Some(b"abcd".to_vec()));
        assert!(read_bounded(&b"abcde"[..], 4).is_none());
    }

    #[test]
    fn growing_source_is_never_read_past_limit_plus_one() {
        struct Growing<'a>(&'a Cell<usize>);
        impl Read for Growing<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                buf.fill(b'x');
                self.0.set(self.0.get() + buf.len());
                Ok(buf.len())
            }
        }
        let count = Cell::new(0);
        assert!(read_bounded(Growing(&count), 4).is_none());
        assert_eq!(count.get(), 5);
    }
}

mod cache_regression_tests {
    use super::super::*;
    #[test]
    fn fragments_share_one_parsed_file() {
        let path = std::env::temp_dir().join(format!("usvg-cache-{}.svg", std::process::id()));
        std::fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="a"/><g id="b"/></svg>"#).unwrap();
        let mut uri = Url::from_file_path(&path).unwrap();
        let mut session = Session::default();
        uri.set_fragment(Some("a"));
        let a = session.load(uri.clone(), 1).unwrap();
        let remaining = (session.budget.bytes_left, session.budget.nodes_left);
        // A cached parse survives source changes, including another target fragment.
        std::fs::write(&path, "not XML").unwrap();
        uri.set_fragment(Some("b"));
        let b = session.load(uri, 1).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "fragments must reuse the parsed document");
        assert!(b.definitions.target("b").is_some());
        assert_eq!((session.budget.bytes_left, session.budget.nodes_left), remaining);
    }
}

// KaminIDE patch: exercise session limits with small budgets, without large allocations.
mod session_limits {
    use super::super::*;

    struct File(std::path::PathBuf);
    impl File {
        fn new(name: &str, text: &str) -> Self {
            let path = std::env::temp_dir().join(format!("usvg-{}-{name}.svg", std::process::id()));
            std::fs::write(&path, text).unwrap();
            Self(path)
        }
        fn url(&self) -> Url { Url::from_file_path(&self.0).unwrap() }
    }
    impl Drop for File {
        fn drop(&mut self) { std::fs::remove_file(&self.0).unwrap(); }
    }
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" id="a"/>"#;

    #[test]
    fn byte_budget_is_shared_by_distinct_files_and_failed_fragments() {
        let a = File::new("bytes-a", SVG);
        let b = File::new("bytes-b", SVG);
        let mut session = Session::default();
        let mut probe = Session::default();
        probe.load(a.url(), 1).unwrap();
        session.budget.bytes_left = Budget::default().bytes_left - probe.budget.bytes_left;
        assert!(session.load(a.url(), 1).is_some());
        assert_eq!(session.budget.bytes_left, 0);
        assert!(session.load(b.url(), 1).is_none());
        let nodes = session.budget.nodes_left;
        let mut fragment = b.url();
        fragment.set_fragment(Some("other"));
        assert!(session.load(fragment, 1).is_none());
        assert_eq!(session.budget.nodes_left, nodes, "failed file must not be retried");
        assert!(session.load(a.url(), 1).is_some(), "cache hit needs no source bytes");
    }

    #[test]
    fn invalid_file_is_cached_across_fragments() {
        let file = File::new("invalid", "not XML");
        let mut session = Session::default();
        assert!(session.load(file.url(), 1).is_none());
        let remaining = (session.budget.bytes_left, session.budget.nodes_left);
        std::fs::write(&file.0, SVG).unwrap();
        let mut fragment = file.url();
        fragment.set_fragment(Some("a"));
        assert!(session.load(fragment, 1).is_none());
        assert_eq!((session.budget.bytes_left, session.budget.nodes_left), remaining);
    }

    #[test]
    fn owned_cache_is_reserved_before_capture() {
        let file = File::new("nodes", SVG);
        let mut session = Session::default();
        // One cache slot, two XML nodes, two typed nodes, two owned nodes.
        session.budget.nodes_left = 6;
        assert!(session.load(file.url(), 1).is_none());
        assert!(session.cache.values().all(Option::is_none));
        let mut session = Session::default();
        session.budget.nodes_left = 7;
        assert!(session.load(file.url(), 1).is_some());
        assert_eq!(session.budget.nodes_left, 0);
    }

    #[test]
    fn import_is_atomic_when_copies_do_not_fit() {
        let file = File::new("import", SVG);
        let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="{}#a"/></svg>"#, file.url());
        let xml = roxmltree::Document::parse(&text).unwrap();
        let mut doc = super::super::super::parse_with_limit(&xml, None, false, 20).unwrap();
        let mut session = Session::default();
        session.load(file.url(), 1).unwrap();
        session.budget.nodes_left = 0;
        let before = (doc.nodes.len(), doc.attrs.len());
        session.expand(&mut doc, None, 0);
        assert_eq!((doc.nodes.len(), doc.attrs.len()), before);
    }

    #[test]
    fn copied_attribute_bytes_are_reserved_before_import() {
        let file = File::new("copy-bytes", SVG);
        let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="{}#a"/></svg>"#, file.url());
        let xml = roxmltree::Document::parse(&text).unwrap();
        let mut doc = super::super::super::parse_with_limit(&xml, None, false, 20).unwrap();
        let mut session = Session::default();
        session.load(file.url(), 1).unwrap();
        session.budget.bytes_left = 0;
        let before = (doc.nodes.len(), doc.attrs.len());
        session.expand(&mut doc, None, 0);
        assert_eq!((doc.nodes.len(), doc.attrs.len()), before);
    }

    #[test]
    fn nested_resources_share_the_parent_byte_budget() {
        let child = File::new("nested-child", SVG);
        let parent_text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="{}#a"/></svg>"#, child.url());
        let parent = File::new("nested-parent", &parent_text);
        let mut session = Session::default();
        let mut probe = Session::default();
        probe.load(parent.url(), 1).unwrap();
        session.budget.bytes_left = Budget::default().bytes_left - probe.budget.bytes_left;
        assert!(session.load(parent.url(), 1).is_some());
        let xml = roxmltree::Document::parse(&parent_text).unwrap();
        let mut doc = super::super::super::parse_with_limit(&xml, None, false, 20).unwrap();
        session.expand(&mut doc, Some(&parent.url()), 0);
        assert_eq!(session.budget.bytes_left, 0);
        assert_eq!(session.cache.values().filter(|v| v.is_some()).count(), 1);
        assert!(session.load(child.url(), 1).is_none());
    }

    #[test]
    fn a_different_fragment_in_the_same_file_is_not_a_file_cycle() {
        let file = File::new("same-file", SVG);
        let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="a"><use href="{}#b"/></g><g id="b"><rect width="1" height="1"/></g></svg>"#, file.url());
        std::fs::write(&file.0, text).unwrap();
        let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="{}#a"/></svg>"#, file.url());
        let xml = roxmltree::Document::parse(&text).unwrap();
        let mut doc = super::super::super::parse_with_limit(&xml, None, false, 20).unwrap();
        let instance = doc.descendants().find(|n| n.tag_name() == Some(EId::Use)).unwrap().id;
        let mut session = Session::default();
        session.expand(&mut doc, None, 0);
        assert_eq!(session.cache.len(), 1, "all fragments share one parsed file");
        assert!(doc.get(instance).descendants().any(|n| n.tag_name() == Some(EId::Rect)));
    }

    #[test]
    fn xml_and_typed_text_are_bounded_before_insertion() {
        let mut budget = Budget { nodes_left: 2, ..Default::default() };
        assert!(budget.parse_xml("<svg><g/></svg>").is_none());
        assert_eq!(budget.nodes_left, 0);
        let xml = roxmltree::Document::parse(r#"<svg xmlns="http://www.w3.org/2000/svg"><text>a<tspan>b</tspan>c</text></svg>"#).unwrap();
        let mut budget = Budget { nodes_left: 4, ..Default::default() };
        assert!(budget.parse_tree(&xml).is_none());
        assert_eq!(budget.nodes_left, 0);
    }
}
