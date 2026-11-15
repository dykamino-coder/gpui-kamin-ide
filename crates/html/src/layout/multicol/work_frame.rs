//! One frame of a test document with work counters: the counted work of
//! multicol tests (`crate::work`, `--features work-counters`).

use crate::work::{Counter, Session, Snapshot};

/// The sum of `counter` over all phases of `snapshot`.
pub(crate) fn total(snapshot: &Snapshot, counter: Counter) -> u64 {
    snapshot.0.iter().map(|phase| phase[counter as usize]).sum()
}

/// The work of drawing one frame of `html` in an 800×600 window.
pub(crate) fn frame_work(cx: &mut gpui::TestAppContext, html: &str) -> Snapshot {
    use gpui::{IntoElement, ParentElement, point, px, size};
    let document = crate::Document::builder(html, crate::BROWSER_CSS)
        .build()
        .expect("test document");
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let session = Session::begin();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            gpui::div()
                .children(document.render(&options))
                .into_any_element()
        });
    session.finish()
}
