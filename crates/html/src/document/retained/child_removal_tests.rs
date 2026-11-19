//! A child-list revision preserves the records of the surviving flow blocks.
use super::box_repaint_tests::{first_difference, frames, fresh, quads};
use super::reuse_tests::{reused, settled};
use super::tests::{View, frame, open};
use crate::document::Mutation;
use gpui::{TestAppContext, VisualTestContext, px, size};

#[gpui::test]
fn removing_a_visible_child_matches_fresh_and_the_next_frame_replays(cx: &mut TestAppContext) {
    let style = "<style>body{margin:0}div{height:20px;background:blue}div:first-child{background:red}div:last-child{background:purple}#first{height:35px;background:green}</style>";
    let survivors = "<div></div>".repeat(180);
    let page = |first: &str| format!("{style}<body>{first}{survivors}</body>");
    let first = "<div id=first>b</div>";
    let start = page(first);
    let expected = fresh(cx, &page(""));
    let (view, vcx) = settled(cx, &page("<div id=first>x</div>"), &start);
    let before = quads(vcx);
    assert!(!expected.is_empty());
    assert_ne!(before, expected);
    assert!(
        reused(&view, vcx).0 > 100,
        "the fixture must have recorded the surviving rows"
    );
    let skipped = reused(&view, vcx).1;
    vcx.update(|_, cx| {
        view.update(cx, |v, _| {
            let probe = start.replacen("id=first", "id=first data-probe", 1);
            let records = v.doc.mutations(&probe).unwrap();
            let [Mutation::Attribute { element: child, .. }] = &records[..] else {
                panic!("one attribute on the first row");
            };
            let link = child.parent.take().unwrap();
            let parent = link.upgrade().unwrap();
            child.parent.set(Some(link));
            v.doc
                .apply(
                    &[Mutation::RemoveChild {
                        parent,
                        child: child.clone(),
                    }],
                    "",
                )
                .unwrap();
        })
    });
    frame(vcx);
    assert_eq!(first_difference(&quads(vcx), &expected), None);
    assert!(
        reused(&view, vcx).1 > skipped + 100,
        "removing the first row lost the later block records"
    );
    let (builds, replays) = frames(&view, vcx);
    frame(vcx);
    assert_eq!(frames(&view, vcx), (builds, replays + 1));
    assert_eq!(first_difference(&quads(vcx), &expected), None);
    fn reveal(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> Vec<String> {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4000.));
        cx.simulate_resize(size(px(400.), px(4000.)));
        frame(cx);
        quads(cx)
    }
    let revealed = reveal(&view, vcx);
    let (fresh_view, fresh_cx) = open(cx, &page(""));
    let reference = reveal(&fresh_view, fresh_cx);
    let alternate = page("").replace("background:purple", "background:orange");
    let (other, other_cx) = open(cx, &alternate);
    assert_ne!(
        reference,
        reveal(&other, other_cx),
        "the last row must contribute paint"
    );
    assert_eq!(first_difference(&revealed, &reference), None);
}
