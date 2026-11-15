//! In-place restyle after a removal and an insertion before existing nodes: the
//! inserted node's key is new and sorts after every later node, so node keys stop
//! following document order, yet each edit equals a fresh document.

use super::tests::{View, frame, open};
use crate::dom::Node;
use gpui::{TestAppContext, VisualTestContext};
use html5ever::{
    QualName, ns,
    tree_builder::{ElementFlags, NodeOrText, TreeSink},
};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

const STYLE: &str = "html,body{margin:0;color:#000;background:#fff}span,i,em,b{text-decoration:underline}\
    .hot{color:#c00}.cool{color:#0a0}";

/// The page after the edits: `b` removed, `em` inserted first in `main`.
fn page(root: &str, new: &str) -> String {
    format!(
        "<!doctype html><html><head><style>{STYLE}</style></head><body>\
         <main id=root class=\"{root}\"><em id=n class=\"{new}\">N</em><p id=a>A<span id=s>s</span></p><p id=c>C</p>\
         <div id=d><i id=i>z</i></div></main></body></html>"
    )
}

const ORIGINAL: &str = "<!doctype html><html><head><style>\
    html,body{margin:0;color:#000;background:#fff}span,i,em,b{text-decoration:underline}\
    .hot{color:#c00}.cool{color:#0a0}</style></head><body>\
    <main id=root class=\"\"><p id=a>A<span id=s>s</span></p>\
    <p id=b>B<b id=bb>b</b></p><p id=c>C</p><div id=d><i id=i>z</i></div></main></body></html>";

fn named(root: &Handle, id: &str) -> Handle {
    let mut pending = vec![root.clone()];
    while let Some(node) = pending.pop() {
        if let NodeData::Element { attrs, .. } = &node.data
            && attrs
                .borrow()
                .iter()
                .any(|a| &*a.name.local == "id" && &*a.value == id)
        {
            return node;
        }
        pending.extend(node.children.borrow().iter().cloned());
    }
    panic!("missing {id}")
}

fn computed(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if let Node::Element(e) = node {
            out.push(format!("{} {:?}", e.tag, e.style.computed()));
            computed(&e.children, out);
        }
    }
}

fn same(got: &[String], want: &[String], what: &str) {
    let tags = |v: &[String]| {
        v.iter()
            .map(|s| s.split(' ').next().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(tags(got), tags(want), "{what}: boxes");
    for (g, w) in got.iter().zip(want) {
        if g != w {
            let at = g.bytes().zip(w.bytes()).take_while(|(a, b)| a == b).count();
            let lo = at.saturating_sub(120);
            panic!(
                "{what}: got ..{}.. want ..{}..",
                &g[lo..(at + 120).min(g.len())],
                &w[lo..(at + 120).min(w.len())]
            );
        }
    }
}

/// A fully transparent colour paints nothing whatever its hue.
fn clear_hues(quad: String) -> String {
    let mut out = String::new();
    let mut rest = quad.as_str();
    while let Some(at) = rest.find("Hsla { ") {
        out.push_str(&rest[..at]);
        let end = rest[at..].find(" }").map_or(rest.len(), |e| at + e + 2);
        let colour = &rest[at..end];
        out.push_str(if colour.ends_with("a: 0.0 }") {
            "Hsla(clear)"
        } else {
            colour
        });
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn scene(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window
            .painted_quads()
            .into_iter()
            .map(|q| clear_hues(format!("{q:?}")));
        let lines = window
            .painted_underlines()
            .into_iter()
            .map(|u| format!("{u:?}"));
        quads.chain(lines).collect()
    })
}

fn styles(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        let mut out = Vec::new();
        computed(view.read(cx).doc.nodes(), &mut out);
        out
    })
}

#[gpui::test]
fn restyle_in_place_after_key_reuse_matches_a_fresh_document(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, ORIGINAL);
    frame(vcx);
    // Remove `b` from the middle: its subtree's keys die with it.
    let freed = vcx.update(|_, cx| {
        view.update(cx, |v, _| {
            let input = v.doc.state.values.traversal.take_source().unwrap();
            let b = named(&input.dom.document, "b");
            let mut freed = vec![b.key()];
            let mut pending = b.children.borrow().clone();
            while let Some(n) = pending.pop() {
                freed.push(n.key());
                pending.extend(n.children.borrow().iter().cloned());
            }
            input.dom.remove_from_parent(&b);
            drop(b);
            v.doc.state.values.traversal.retain(input);
            v.doc.apply(&[], "/* rebuild */").unwrap();
            freed
        })
    });
    frame(vcx);
    // Insert `em` first in `main`: it gets a key never issued before, so its
    // key sorts after the keys of every later node.
    let new_key = vcx.update(|_, cx| {
        view.update(cx, |v, _| {
            let input = v.doc.state.values.traversal.take_source().unwrap();
            let a = named(&input.dom.document, "a");
            let em = input.dom.create_element(
                QualName::new(None, ns!(html), "em".into()),
                vec![
                    html5ever::Attribute {
                        name: QualName::new(None, ns!(), "id".into()),
                        value: "n".into(),
                    },
                    html5ever::Attribute {
                        name: QualName::new(None, ns!(), "class".into()),
                        value: "".into(),
                    },
                ],
                ElementFlags::default(),
            );
            input.dom.append(&em, NodeOrText::AppendText("N".into()));
            let key = em.key();
            input
                .dom
                .append_before_sibling(&a, NodeOrText::AppendNode(em));
            v.doc.state.values.traversal.retain(input);
            v.doc.apply(&[], "").unwrap();
            key
        })
    });
    frame(vcx);
    assert!(
        !freed.contains(&new_key),
        "a key of a dropped node was reissued"
    );
    let d_key = vcx.update(|_, cx| named(&view.read(cx).doc.source_dom(), "d").key());
    assert!(
        new_key.packed() > d_key.packed(),
        "the reused key must break document order for this test"
    );
    let got = styles(&view, vcx);
    let (fresh, fcx) = open(vcx, &page("", ""));
    frame(fcx);
    same(&got, &styles(&fresh, fcx), "styles after reuse");
    let mut in_place = 0;
    for (root, new) in [
        ("hot", ""),
        ("hot", "cool"),
        ("", "cool"),
        ("", ""),
        ("cool", "hot"),
    ] {
        let markup = page(root, new);
        let values = vcx.update(|_, cx| view.read(cx).doc.state.values.clone());
        vcx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&markup, "").unwrap()));
        frame(vcx);
        in_place +=
            usize::from(vcx.update(|_, cx| Rc::ptr_eq(&values, &view.read(cx).doc.state.values)));
        let (got_styles, got_scene) = (styles(&view, vcx), scene(vcx));
        let (fresh, fcx) = open(vcx, &markup);
        frame(fcx);
        same(
            &got_styles,
            &styles(&fresh, fcx),
            &format!("styles {root}/{new}"),
        );
        same(&got_scene, &scene(fcx), &format!("pixels {root}/{new}"));
    }
    assert!(
        in_place == 5,
        "only {in_place} of 5 edits restyled in place"
    );
}
