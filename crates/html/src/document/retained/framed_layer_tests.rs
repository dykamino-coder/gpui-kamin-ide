//! A paint-only edit in a document whose layer the frame copies as its own
//! paint (`gpui::SceneLayer::framed`: a plane of a 3D context beside a
//! scroll container): the layer takes the edit, the frame replays without a build,
//! and the edited frame equals a fresh document's both ways.

use super::box_repaint_tests::quads;
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::TestAppContext;

/// A scroll container, a turned translucent box and the box `attr` marks.
fn page(attr: &str) -> String {
    let rows: String = (0..12).map(|i| format!("<p>row {i}</p>")).collect();
    format!(
        "<!doctype html><html><head><style>html{{background:white}}body{{margin:0}}\
         p{{margin:0;font:16px/20px sans-serif}}\
         #s{{overflow:auto;height:60px;width:200px}}#p{{perspective:100px}}\
         #o{{transform:rotateY(20deg);opacity:.5;background:#08f;height:20px}}\
         #e{{height:20px;background:#eee}}#e[data-edit]{{background:#0a4}}</style></head>\
         <body><div id=s>{rows}</div><div id=p><div id=o></div></div><div id=e{attr}></div></body></html>"
    )
}

#[gpui::test]
fn a_framed_layer_takes_a_background_edit_without_a_build(cx: &mut TestAppContext) {
    let (none, set) = (page(""), page(" data-edit=\"1\""));
    let (view, vcx) = settled(cx, &none, &set);
    update(&view, vcx, &none);
    let framed = vcx.update(|_, cx| {
        let doc = &view.read(cx).doc;
        doc.reuse
            .scene_layer()
            .map(|layer| layer.content().framed())
    });
    assert_eq!(framed, Some(true), "the page's layer is not framed");
    let builds = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|_, cx| view.read(cx).doc.retained_frames().0)
    };
    let mut edited = Vec::new();
    for (html, name) in [(&set, "set"), (&none, "unset")] {
        let built = builds(vcx);
        update(&view, vcx, html);
        assert_eq!(builds(vcx), built, "{name}: the frame was built");
        edited.push((html.clone(), name, quads(vcx)));
    }
    for (html, name, edited) in edited {
        let (_, fresh) = open(cx, &html);
        frame(fresh);
        assert_eq!(edited, quads(fresh), "{name}: the edited frame differs");
    }
}
