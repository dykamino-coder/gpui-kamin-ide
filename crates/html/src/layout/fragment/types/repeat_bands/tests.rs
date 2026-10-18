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
        head_els: vec![el(), el()],
        foot_els: vec![el(), el(), el()],
    };
    let mut placed = Vec::new();
    r.place_bands(&frag(0, 20.0, 15.0), |_, at| placed.push(at));
    // First fragment: no repeated head, foot copy lifted onto `h + foot − bh`.
    assert_eq!(placed, vec![100.0 + 15.0 - 15.0 - 200.0]);
    placed.clear();
    r.place_bands(&frag(1, 20.0, 0.0), |_, at| placed.push(at));
    assert_eq!(placed, vec![-20.0 - 5.0]);
    let mut painted = Vec::new();
    r.paint_bands(&frag(1, 20.0, 15.0), |_, top, h| painted.push((top, h)));
    assert_eq!(painted, vec![(-20.0, 20.0), (100.0, 15.0)]);
}
