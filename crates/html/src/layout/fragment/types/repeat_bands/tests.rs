//! Копии фрагмента и полосы повтора: одно правило для колонок и страниц.

use super::frag_copy;
use crate::layout::fragment::types::{Frag, Repeat, RepeatGeom};
use gpui::{AnyElement, IntoElement};

fn el() -> AnyElement {
    gpui::div().into_any_element()
}

fn frag(copy: usize, head: f32, foot: f32) -> Frag {
    Frag {
        kid: 0,
        copy,
        col: copy,
        y: 30.0,
        from: 0.0,
        h: 100.0,
        head,
        foot,
    }
}

#[test]
fn copy_zero_is_the_element_and_missing_copies_are_none() {
    let (mut e, mut frags) = (el(), vec![el()]);
    assert!(frag_copy(&mut e, &mut frags, 0).is_some());
    assert!(frag_copy(&mut e, &mut frags, 1).is_some());
    assert!(frag_copy(&mut e, &mut frags, 2).is_none());
}

#[test]
fn head_band_only_on_continuations_and_foot_band_below_content() {
    let mut r = Repeat {
        head: Some((5.0, 20.0)),
        foot: Some((200.0, 15.0)),
        geom: RepeatGeom::default(),
        row_ends: Vec::new(),
        head_els: vec![el(), el()],
        foot_els: vec![el(), el(), el()],
    };
    let mut placed = Vec::new();
    reblessive::Stack::new()
        .enter(async |_| {
            r.place_bands_async(&frag(0, 20.0, 15.0), async |_, at| placed.push(at))
                .await;
        })
        .finish();
    // First fragment: no repeated head, foot copy lifted onto `h + foot − bh`.
    assert_eq!(placed, vec![100.0 + 15.0 - 15.0 - 200.0]);
    placed.clear();
    reblessive::Stack::new()
        .enter(async |_| {
            r.place_bands_async(&frag(1, 20.0, 0.0), async |_, at| placed.push(at))
                .await;
        })
        .finish();
    assert_eq!(placed, vec![-20.0 - 5.0]);
    let mut painted = Vec::new();
    reblessive::Stack::new()
        .enter(async |_| {
            r.paint_bands_async(&frag(1, 20.0, 15.0), async |_, top, h| {
                painted.push((top, h))
            })
            .await;
        })
        .finish();
    assert_eq!(painted, vec![(-20.0, 20.0), (100.0, 15.0)]);
}
