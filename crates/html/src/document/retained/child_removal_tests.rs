//! A child-list revision preserves the records of the surviving flow blocks.
use super::box_repaint_tests::{first_difference, frames};
use super::reuse_tests::{reused, settled};
use super::tests::{View, frame, open};
use crate::document::Mutation;
use gpui::{TestAppContext, VisualTestContext, px, size};

/// Compare these translated solid fills at their final device coordinates.
/// The effective clip on each fill includes all four edges of its content mask.
fn visible_quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let mut sequence = Vec::new();
        let mut previous = Vec::new();
        for mut quad in window.painted_quads() {
            assert_eq!(quad.transformation.rotation_scale, [[1., 0.], [0., 1.]]);
            let [x, y] = quad.transformation.translation;
            quad.bounds.origin.x.0 += x;
            quad.bounds.origin.y.0 += y;
            quad.transformation.translation = [0., 0.];
            let ink = quad.content_mask.bounds.intersect(&quad.bounds);
            quad.content_mask.bounds = ink;
            quad.order = 0;
            if !ink.is_empty() {
                let value = format!("{quad:?}");
                for (area, earlier) in &previous {
                    if !ink.intersect(area).is_empty() {
                        sequence.push(format!("overlap {earlier} before {value}"));
                    }
                }
                sequence.push(value.clone());
                previous.push((ink, value));
            }
        }
        sequence.sort();
        sequence
    })
}

fn reveal(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4000.));
    cx.simulate_resize(size(px(400.), px(4000.)));
    frame(cx);
    visible_quads(cx)
}

#[gpui::test]
fn removing_a_visible_child_matches_fresh_and_the_next_frame_replays(cx: &mut TestAppContext) {
    let style = "<style>body{margin:0}div{height:20px;background:blue}div:first-child{background:red}div:last-child{background:purple}#first{height:35px;background:green}</style>";
    let survivors = "<div></div>".repeat(180);
    let page = |first: &str| format!("{style}<body>{first}{survivors}</body>");
    let first = "<div id=first>b</div>";
    let start = page(first);
    let (_, expected_cx) = open(cx, &page(""));
    frame(expected_cx);
    let expected = visible_quads(expected_cx);
    let (view, vcx) = settled(cx, &page("<div id=first>x</div>"), &start);
    let before = visible_quads(vcx);
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
    assert_eq!(first_difference(&visible_quads(vcx), &expected), None);
    assert!(
        reused(&view, vcx).1 > skipped + 100,
        "removing the first row lost the later block records"
    );
    let (builds, replays) = frames(&view, vcx);
    frame(vcx);
    assert_eq!(frames(&view, vcx), (builds, replays + 1));
    assert_eq!(first_difference(&visible_quads(vcx), &expected), None);
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

#[gpui::test]
fn inserting_and_moving_a_row_matches_fresh_and_reuses_survivors(cx: &mut TestAppContext) {
    let css = "<style>body{margin:0}div{height:20px;background:blue}div:first-child{background:red}#new{height:35px;background:green}</style>";
    let rows = "<div></div>".repeat(180);
    let page = |first: &str, last: &str| {
        format!("{css}<body>{first}<div id=probe>b</div>{rows}{last}</body>")
    };
    let inserted = "<div id=new></div>";
    let references = [page(inserted, ""), page("", inserted)].map(|html| {
        let (_, fresh) = open(cx, &html);
        frame(fresh);
        visible_quads(fresh)
    });
    assert_ne!(references[0], references[1]);
    let original = page("", "");
    let (view, vcx) = settled(cx, &original.replace(">b<", ">a<"), &original);
    let (body, probe, new) = vcx.update(|_, cx| {
        let doc = &view.read(cx).doc;
        let records = doc
            .mutations(&original.replace("id=probe", "id=probe data-probe"))
            .unwrap();
        let [Mutation::Attribute { element, .. }] = &records[..] else {
            panic!("expected the probe");
        };
        let link = element.parent.take().unwrap();
        let body = link.upgrade().unwrap();
        element.parent.set(Some(link));
        let new = doc
            .create_element(html5ever::QualName::new(
                None,
                html5ever::ns!(html),
                "div".into(),
            ))
            .unwrap();
        (doc.source_node(&body).unwrap(), element.clone(), new)
    });
    for (index, reference) in references.iter().enumerate() {
        let skipped = reused(&view, vcx).1;
        let replayed = vcx.update(|_, cx| view.read(cx).doc.replayed_blocks());
        vcx.update(|_, cx| {
            view.update(cx, |v, _| {
                let mut records = vec![Mutation::InsertBefore {
                    parent: body.clone(),
                    child: new.clone(),
                    before: (index == 0).then(|| probe.clone()),
                }];
                if index == 0 {
                    records.push(Mutation::Attribute {
                        element: new.handle().clone(),
                        name: html5ever::QualName::new(None, html5ever::ns!(), "id".into()),
                        value: Some("new".into()),
                    });
                }
                v.doc.apply(&records, "").unwrap();
            })
        });
        frame(vcx);
        assert!(!reference.is_empty());
        assert_eq!(first_difference(&visible_quads(vcx), reference), None);
        assert!(
            reused(&view, vcx).1 > skipped + 100,
            "step {index} lost surviving blocks"
        );
        assert!(vcx.update(|_, cx| view.read(cx).doc.replayed_blocks()) > replayed);
        let (builds, replays) = frames(&view, vcx);
        frame(vcx);
        assert_eq!(frames(&view, vcx), (builds, replays + 1));
    }
    let observed = reveal(&view, vcx);
    let (other, fresh) = open(cx, &page("", inserted));
    let reference = reveal(&other, fresh);
    assert!(reference.len() > 180);
    assert_eq!(first_difference(&observed, &reference), None);
}
