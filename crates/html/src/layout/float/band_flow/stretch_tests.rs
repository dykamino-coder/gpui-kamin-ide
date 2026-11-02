//! A host the parent stretches lays its children out at the final inline size
//! once: no intrinsic probes, and the measured plan is the one prepaint uses
//! (its width is the final width, its height the measured height). Counted
//! with `crate::work`; run with `--features work-counters`.

use crate::work::{Counter, Session, Snapshot};

fn total(snapshot: &Snapshot, counter: Counter) -> u64 {
    snapshot.0.iter().map(|phase| phase[counter as usize]).sum()
}

/// Probe and intrinsic-probe calls of one drawn frame of `html`.
fn probes(cx: &mut gpui::TestAppContext, html: &str) -> (u64, u64) {
    use gpui::{IntoElement, ParentElement, point, px, size};
    let document = crate::Document::builder(html, crate::BROWSER_CSS)
        .build()
        .expect("test document");
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (400., 300.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let session = Session::begin();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(400.), px(300.)), |_, _| {
            gpui::div()
                .children(document.render(&options))
                .into_any_element()
        });
    let work = session.finish();
    (
        total(&work, Counter::BandProbeCalls),
        total(&work, Counter::BandIntrinsicCalls),
    )
}

const HOST: &str = "<div style='background:rgb(255,0,0)'><span style='float:right;width:20px;height:10px;background:rgb(0,0,255)'></span>some words that fit</div>";

#[gpui::test]
fn a_stretched_host_plans_once_at_its_final_inline_size(cx: &mut gpui::TestAppContext) {
    let stretched = probes(
        cx,
        &format!("<div style='display:flex;flex-direction:column;width:300px'>{HOST}</div>"),
    );
    let definite = probes(cx, &format!("<div style='width:300px'>{HOST}</div>"));
    assert_eq!(stretched.1, 0, "intrinsic probes: {stretched:?}");
    assert!(stretched.0 > 0, "{stretched:?}");
    // The same plan count as a host whose width the block layout gives: the
    // final measure planned at 300px, so prepaint reused that plan.
    assert_eq!(stretched.0, definite.0, "{stretched:?} vs {definite:?}");
}
