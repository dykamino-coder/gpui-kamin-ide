//! Separate documents own their deferred descendants and cached invalidation.

use super::*;

struct Pair {
    pages: [Entity<DocumentView>; 2],
    pulse: bool,
}

impl Render for Pair {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(400.))
            .h(px(300.))
            .child(self.pages[0].clone())
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .child(self.pages[1].clone()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(if self.pulse { 80. } else { 81. }))
                    .h(px(60.))
                    .bg(gpui::rgb(0x00ff00)),
            )
    }
}

#[gpui::test]
fn sibling_documents_keep_layers_local_and_invalidate_independently(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let (host, cx) = cx.add_window_view(|window, cx| {
        let pages = [(50, "red", 2147483647), (40, "yellow", 0)].map(|(n, color, z)| {
            let html = format!("<style>body{{margin:0}}header{{position:fixed;top:0;left:0;width:{n}px;height:{n}px;background:{color};z-index:{z}}}</style><header></header>");
            DocumentView::open(&mut Document::builder(&html, ""),
                size(px(400.), px(300.)), window, cx).unwrap()
        });
        Pair { pages, pulse: false }
    });
    frame(cx);
    frame(cx);
    let pages = cx.update(|_, cx| host.read(cx).pages.clone());
    for page in &pages {
        cx.update(|_, cx| page.read(cx).document.take_root_phases());
    }
    assert_host_on_top(cx, 81.);
    cx.update(|window, _| {
        let sequence = window.painted_sequence();
        let order = |width| {
            let quads = window.painted_quads();
            let quad = quads
                .iter()
                .find(|q| {
                    q.bounds.size.width.0 / window.scale_factor() == width
                        && q.bounds.size.height.0 / window.scale_factor() == width
                })
                .unwrap();
            let fill = format!("background: {:?},", quad.background);
            sequence
                .iter()
                .position(|(p, _)| p.starts_with("quad ") && p.contains(&fill))
                .unwrap()
        };
        assert!(
            order(40.) > order(50.),
            "a document escaped above its later sibling"
        );
    });
    cx.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.pulse = true;
            cx.notify();
        })
    });
    draw(cx);
    assert_host_on_top(cx, 80.);
    for page in &pages {
        let sample = cx.update(|_, cx| page.read(cx).document.take_root_phases());
        assert_eq!(sample["work_calls"]["total"], 0, "{sample}");
    }
    cx.update(|_, cx| {
        pages[0].update(cx, |page, cx| {
            page.document.update("<main>changed</main>", "").unwrap();
            cx.notify();
        })
    });
    draw(cx);
    let first = cx.update(|_, cx| pages[0].read(cx).document.take_root_phases());
    let second = cx.update(|_, cx| pages[1].read(cx).document.take_root_phases());
    assert!(first["work_calls"]["total"].as_u64().unwrap() > 0);
    assert_eq!(
        second["work_calls"]["total"], 0,
        "unrelated document: {second}"
    );
}
