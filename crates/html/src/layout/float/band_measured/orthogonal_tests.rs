//! Orthogonal float roots retain native layout ownership in measured hosts.

use super::construction_tests::{default_host, find};
use super::*;
use crate::dom::parse;
use crate::style::values::value::Len;

#[test]
fn orthogonal_block_tail_keeps_its_independent_contents_in_the_measured_host() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    for mode in ["vertical-lr", "vertical-rl"] {
        let markup = format!(
            r#"<div id="source"><div style="float:right;width:40px;height:120px"></div><div id="tail" style="writing-mode:{mode};width:40px;height:120px"><ruby>base<rt>note</rt></ruby></div></div>"#
        );
        let tree = parse(&markup, "");
        let nodes = &find(&tree, "source").expect("source").children;
        let (host, end, lifted) =
            default_host(nodes, vcx).expect("orthogonal block owns its line layout");
        assert_eq!(end, nodes.len());
        assert!(lifted.is_empty());
        assert!(host.marks.shape_flow.is_some());
        let tail = find(&host.children, "tail").expect("tail");
        assert!(tail.children.iter().any(has_ruby));
        assert_eq!(tail.style.height, Some(Len::Px(120.)));
    }
}

#[test]
fn orthogonal_float_roots_keep_native_layout_ownership() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    for mode in ["vertical-lr", "vertical-rl", "sideways-lr"] {
        for (layout, content, admitted) in [
            ("", "<span>first second</span>", true),
            ("", "<div>first</div><div>second</div>", true),
            ("", r#"<span style="float:left">nested</span>"#, true),
            ("width:100px", "<div>first</div><div>second</div>", true),
            ("height:100px", "<div>first</div><div>second</div>", true),
            ("display:grid", "<div>first</div><div>second</div>", true),
            ("display:flex", "<div>first</div><div>second</div>", true),
        ] {
            let markup = format!(
                r#"<div id="source"><div style="float:left;writing-mode:{mode};{layout}">{content}</div><div style="float:right;writing-mode:{mode};{layout}">{content}</div></div>"#
            );
            let tree = parse(&markup, "");
            let nodes = &find(&tree, "source").expect("source").children;
            let measured = default_host(nodes, vcx);
            if !admitted {
                assert!(measured.is_none(), "block children require root sizing");
            } else {
                let (host, end, lifted) = measured.expect("root owns its intrinsic layout");
                assert_eq!(end, nodes.len());
                assert!(lifted.is_empty());
                assert!(host.marks.shape_flow.is_some());
            }
        }
    }
}
