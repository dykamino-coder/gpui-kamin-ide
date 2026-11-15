//! A frame painted into the document's scene layer draws what a frame painted
//! straight into the window draws, and every two primitives whose ink
//! overlaps in the same order: the layer keys a box's own commands, and that
//! key must not reorder them against overlapping neighbours. Primitives that
//! do not overlap may draw in another order (the bounds tree gives them one
//! bucket), which no pixel shows. Each page overlaps N boxes of one kind. The
//! test platform rasterizes no glyphs: text orders through its line layers and
//! decorations here; glyph ink against neighbours is checked by WPT RGB.

use super::tests::{frame, open};
use gpui::{Bounds, ScaledPixels, TestAppContext};

type Sequence = Vec<(String, Bounds<ScaledPixels>)>;

/// The draw sequence of `html` painted anew with or without layers.
fn sequence(cx: &mut TestAppContext, html: &str, layers: bool) -> Sequence {
    let (view, vcx) = open(cx, html);
    vcx.update(|window, _| window.set_scene_layers(layers));
    // A new viewport paints the frame anew, now with or without the layer.
    vcx.update(|_, cx| view.update(cx, |view, _| view.viewport = (400., 290.)));
    frame(vcx);
    frame(vcx);
    vcx.update(|window, _| window.painted_sequence())
}

fn page(body: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff;font-size:20px}}\
         .a{{background:#9cf}} .b{{background:#ff0}} .c{{background:#cfc;border:2px solid #f00}}</style></head>\
         <body>{body}</body></html>"
    )
}

/// Each primitive numbered by its occurrence of equal ones: the same numbers
/// name the same primitives in both sequences.
fn numbered(sequence: &Sequence) -> Vec<(String, usize)> {
    let mut seen = std::collections::HashMap::<&str, usize>::new();
    sequence
        .iter()
        .map(|(p, _)| {
            let n = seen.entry(p.as_str()).or_default();
            *n += 1;
            (p.clone(), *n)
        })
        .collect()
}

/// The layered frame draws the direct one's primitives, every two that
/// overlap in its order.
fn check(cx: &mut TestAppContext, name: &str, body: &str) {
    let html = page(body);
    let mut on = sequence(cx, &html, true);
    let off = sequence(cx, &html, false);
    assert!(
        on.iter().any(|(p, _)| p == "layer"),
        "{name}: the frame drew no scene layer"
    );
    assert!(
        !off.iter().any(|(p, _)| p == "layer"),
        "{name}: the direct frame drew a scene layer"
    );
    on.retain(|(p, _)| p != "layer" && p != "end layer");
    let (a, b) = (numbered(&on), numbered(&off));
    let mut sorted = (a.clone(), b.clone());
    sorted.0.sort();
    sorted.1.sort();
    assert!(
        sorted.0 == sorted.1,
        "{name}: the layered frame draws other primitives"
    );
    let at: std::collections::HashMap<_, _> = a.iter().enumerate().map(|(i, k)| (k, i)).collect();
    for i in 0..b.len() {
        for j in i + 1..b.len() {
            let (bi, bj) = (off[i].1, off[j].1);
            if bi.intersect(&bj).is_empty() {
                continue;
            }
            assert!(
                at[&b[i]] < at[&b[j]],
                "{name}: overlapping primitives swap in the layer:\n {}\n {}",
                off[i].0,
                off[j].0
            );
        }
    }
    assert!(!off.is_empty(), "{name}: nothing painted");
}

const N: usize = 6;

fn repeat(item: &str) -> String {
    item.repeat(N)
}

#[gpui::test]
fn overlapping_blocks_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item = "<div class=a style='height:12px;line-height:8px'>gjpqy</div><div class=b style='height:12px;margin-top:-4px'>x</div>";
    check(cx, "blocks", &repeat(item));
}

#[gpui::test]
fn overlapping_inlines_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item = "<span class=a style='padding:0 6px'>ab</span><span class=b style='margin-left:-8px;padding:0 4px'>cd</span><span class=c>ef</span>";
    check(
        cx,
        "inlines",
        &format!("<p style='width:150px'>{}</p>", repeat(item)),
    );
}

#[gpui::test]
fn overlapping_floats_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item = "<div class=a style='float:left;width:30px;height:20px'>ab</div><div class=b style='float:left;width:30px;height:20px;margin-left:-10px'>cd</div>";
    check(cx, "floats", &repeat(item));
}

#[gpui::test]
fn overlapping_positioned_boxes_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let items: String = (0..N)
        .map(|i| {
            format!(
                "<div class={} style='position:absolute;left:{}px;top:{}px;width:40px;height:30px'>ab</div>",
                ["a", "b", "c"][i % 3],
                i * 15,
                i * 10
            )
        })
        .collect();
    check(cx, "positioned", &items);
}

#[gpui::test]
fn overlapping_grid_items_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item = "<div class=a style='grid-area:1/1'>wide text ab</div><div class=b style='grid-area:1/1;margin-left:20px'>cd</div><div class=c>ef</div>";
    check(
        cx,
        "grid",
        &format!(
            "<div style='display:grid;grid-template-columns:repeat(3,40px)'>{}</div>",
            repeat(item)
        ),
    );
}

#[gpui::test]
fn adjacent_flex_items_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item =
        "<div style='display:flex'><div>I</div><div class=a>a</div><div class=b>b</div></div>";
    check(cx, "flex", &repeat(item));
}

#[gpui::test]
fn flex_items_in_order_modified_order_draw_as_without_a_layer(cx: &mut TestAppContext) {
    let item = "<div style='display:flex'><div class=b style='order:1'>b</div><div style='order:-1'>I</div><div class=a style='order:-1'>a</div></div>";
    check(cx, "flex order", &repeat(item));
}
