//! A recoloured paragraph keeps its measured layout leaf (`Paragraph::layout_key`
//! leaves the run colours out): a class that only recolours the text lays out
//! nothing again, while a class that changes a font weight measures anew.

use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// Paragraphs on the screen that wrap; `html.paint` recolours them through an
/// inherited `color`, `html.bold` changes their font weight.
fn page(class: &str, first: &str) -> String {
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do";
    let rows: String = (0..6)
        .map(|i| {
            let text = if i == 0 { first } else { words };
            format!("<p>{text} {i}</p>")
        })
        .collect();
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;padding:0;background:#fff;color:#000}} p{{margin:0}} \
         html.paint body{{color:#123456}} html.bold p{{font-weight:bold}}\
         </style></head><body>{rows}</body></html>"
    )
}

/// Layout nodes the frame tree kept over the frame of `html`.
fn claimed(view: &gpui::Entity<View>, cx: &mut VisualTestContext, html: &str) -> u64 {
    let before = cx.update(|window, _| window.claimed_layout_nodes());
    update(view, cx, html);
    cx.update(|window, _| window.claimed_layout_nodes()) - before
}

#[gpui::test]
fn a_recoloured_paragraph_keeps_its_layout_and_a_new_weight_does_not(cx: &mut TestAppContext) {
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do";
    let (view, cx) = open(cx, &page("", words));
    frame(cx);
    // One paragraph's new text: the others and their parent are kept.
    let edited = claimed(&view, cx, &page("", "edited"));
    claimed(&view, cx, &page("", words));
    let recoloured = claimed(&view, cx, &page("paint", words));
    assert!(
        recoloured > edited,
        "a recolour kept {recoloured} nodes, a one-paragraph text edit {edited}"
    );
    let back = claimed(&view, cx, &page("", words));
    assert_eq!(back, recoloured, "the recolour back keeps the same nodes");
    let bold = claimed(&view, cx, &page("bold", words));
    assert!(
        bold < edited,
        "a new font weight kept {bold} nodes, a one-paragraph text edit {edited}"
    );
}
