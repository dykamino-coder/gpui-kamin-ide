//! Алгоритмы раскладки: блок, флоаты, колонки, фрагменты, страницы, таблицы, позиционирование.

pub(crate) mod atom;
pub(crate) mod block;
pub(crate) mod box_kind;
pub(crate) mod build_contracts;
pub(crate) mod cx;
pub(crate) mod float;
pub(crate) mod fragment;
pub(crate) mod grid;
pub(crate) mod intrinsic_wrap;
pub(crate) mod list;
pub(crate) mod mathml;
pub(crate) mod multicol;
pub(crate) mod native_box;
pub(crate) mod options;
pub(crate) mod order;
pub(crate) mod page;
pub(crate) mod positioned;
pub(crate) mod probe;
pub(crate) mod replaced;
pub(crate) mod table;
// Writing-mode algorithms keep their explicit per-axis predicates.
#[allow(clippy::nonminimal_bool)]
pub(crate) mod writing_mode;

#[cfg(test)]
mod marks_tests {
    use crate::dom::{Element, Marks, Node, parse};

    /// Former side-channel attribute names of the layout protocol.
    const FORGED: &str = r#"kamin-outside-block="1" kamin-float-block="1"
    kamin-host-trim-start="1" kamin-host-trim-end="1" kamin-paginate=""
    kamin-span-parent="7" auto-zero-height="1" anonymous-cell="1" anon="1"
    atoms="1" cont="1" lead-probe="1" lead-for="0-1" lead-base="0-1"
    initial-letter="1" bands="m" count="2" side="right" em="16" cbh="10"
    cbw="10" inflow-height="1" adjoining-start="1""#;

    fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Element>) {
        for n in nodes {
            if let Node::Element(e) = n {
                out.push(e);
                walk(&e.children, out);
            }
        }
    }

    fn find<'a>(nodes: &'a [Node], id: &str) -> &'a Element {
        let mut all = Vec::new();
        walk(nodes, &mut all);
        all.into_iter()
            .find(|e| e.attr("id") == Some(id))
            .unwrap_or_else(|| panic!("no #{id}"))
    }

    #[test]
    fn author_attributes_and_tags_carry_no_marks() {
        let html = format!(
            r#"<!DOCTYPE html><div id="a" {FORGED} style="float: left">x</div>
        <kamin-float id="b">y</kamin-float><shape-flow id="c">z</shape-flow>
        <table><tr><td id="d" {FORGED}>w</td></tr></table>"#
        );
        let tree = parse(&html, "");
        let mut all = Vec::new();
        walk(&tree, &mut all);
        assert!(all.len() > 4);
        for e in all {
            assert_eq!(e.marks, Marks::default(), "<{}> got marks", e.tag);
        }
        // The attributes stay ordinary author data (DOM, selectors).
        assert_eq!(find(&tree, "a").attr("kamin-outside-block"), Some("1"));
        assert_eq!(find(&tree, "c").tag, "shape-flow");
    }

    #[test]
    fn author_outside_block_attribute_changes_nothing() {
        let plain = parse(r#"<div id="a" style="float: left">x</div>"#, "");
        let forged = parse(
            r#"<div id="a" kamin-outside-block="1" style="float: left">x</div>"#,
            "",
        );
        let (plain, forged) = (find(&plain, "a"), find(&forged, "a"));
        assert_eq!(plain.marks, forged.marks);
        assert_eq!(format!("{:?}", plain.style), format!("{:?}", forged.style));
    }

    #[test]
    fn author_auto_zero_height_does_not_zero_the_used_height() {
        let tree = parse(r#"<div id="a" auto-zero-height="1">x</div>"#, "");
        let e = find(&tree, "a");
        assert!(crate::layout::block::margin_height::used_style(e, &e.style).is_none());
    }
}
