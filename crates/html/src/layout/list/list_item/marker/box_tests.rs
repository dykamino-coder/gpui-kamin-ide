//! The `::marker` box: a typed generated box with the CSS Lists 3 UA rules,
//! identical every time a repeated fragment rebuilds it.

use super::*;
use crate::document::state::DocState;
use crate::style::cascade::materialize::box_style;
use crate::style::css::parse_decls;

fn style(css: &str, vcx: ValueCx<'_>) -> Computed {
    let mut c = Computed::default();
    c.apply_decls_with_vars(&parse_decls(css), &Default::default(), vcx);
    c
}

fn marker_box(li: &Element, vcx: ValueCx<'_>) -> Element {
    let merged = inherit(&BoxStyle::default(), &li.style.used(), vcx);
    let marker = resolve(li, &merged, &merged, vcx).expect("a marker");
    let Node::Element(span) = node(li, &merged, marker) else {
        panic!("the marker is an element box")
    };
    span
}

#[test]
fn repeated_marker_boxes_are_identical_generated_boxes() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let li = Element::synthetic(
        "li",
        style(
            "display:list-item;list-style:decimal inside;font-variant-numeric:oldstyle-nums",
            vcx,
        ),
        Default::default(),
        vec![],
    );
    let before = format!("{:?}", li.style.used());
    let first = marker_box(&li, vcx);
    let again = marker_box(&li, vcx);
    for span in [&first, &again] {
        assert_eq!(span.marks.box_origin, Some(BoxOrigin::Marker));
        assert!(span.inline);
        let used = span.style.used();
        assert_eq!(used.bidi_isolate, Some(true));
        assert!(used.first_letter_excluded);
        assert!(used.content.is_none() && used.marker_layer.is_none());
        // CSS Lists 3 §3.1.1: tabular-nums replaces the inherited numeric variant once.
        let numeric: Vec<_> = used
            .font_features
            .iter()
            .filter(|(t, _)| t == "tnum" || t == "onum")
            .collect();
        assert_eq!(numeric, [&("tnum".to_string(), 1)]);
    }
    assert_eq!(*first.style.used().style, *again.style.used().style);
    // Building the marker leaves the list item's style untouched.
    assert_eq!(format!("{:?}", li.style.used()), before);
}

#[test]
fn author_marker_bidi_and_numeric_variant_win_over_ua_rules() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let mut item = style("display:list-item;list-style:decimal inside", vcx);
    item.marker_layer = Some(
        style(
            "unicode-bidi:plaintext;font-variant-numeric:oldstyle-nums",
            vcx,
        )
        .into(),
    );
    let li = Element::synthetic("li", item, Default::default(), vec![]);
    let span = marker_box(&li, vcx);
    let used = span.style.used();
    assert_eq!(span.marks.box_origin, Some(BoxOrigin::Marker));
    assert_ne!(used.bidi_isolate, Some(true));
    assert_eq!(used.bidi_plaintext, Some(true));
    assert!(!used.font_features.iter().any(|(t, _)| t == "tnum"));
}

#[test]
fn marker_box_colors_pass_the_subtree_filter_once() {
    let parsed = crate::dom::parse(
        r#"<ul id="list" style="filter: invert(1)"><li id="item" style="list-style: square inside; color: rgb(0, 128, 0)">x</li></ul>"#,
        "",
    );
    let vcx = parsed.values();
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &std::rc::Rc::new(Computed::default()),
        vcx,
    );
    let find = |id| {
        crate::dom::test_support::find(&parsed.nodes, &|e: &Element| e.attr("id") == Some(id))
            .unwrap()
    };
    let list = box_style(&BoxStyle::default(), &find("list").style, vcx);
    let li = find("item");
    let merged = box_style(&list, &li.style, vcx);
    let marker = resolve(li, &merged, &merged, vcx).expect("a marker");
    let Node::Element(span) = node(li, &merged, marker) else {
        panic!("the marker is an element box")
    };
    let marker_box = box_style(&merged, &span.style, vcx);
    assert_eq!(marker_box.color, merged.color);
}
