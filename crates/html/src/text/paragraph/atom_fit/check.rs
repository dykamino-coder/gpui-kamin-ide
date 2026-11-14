//! Tests: prepaint lays the real atoms out at the widths their copies were
//! measured at, and no copy outlives the prepaint of its paragraph.

use super::Fitted;
use crate::{Document, RenderOpts};
use gpui::{Context, IntoElement, Render, TestAppContext, Window};
use std::cell::RefCell;

thread_local! {
    /// Per prepainted paragraph: what its atom copies gave at the prepaint width,
    /// what the real atoms gave, and the copies left after the prepaint.
    static REFITS: RefCell<Vec<(Fitted, Fitted, usize)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn record(copied: Fitted, real: Fitted, copies_left: usize) {
    REFITS.with_borrow_mut(|refits| refits.push((copied, real, copies_left)));
}

struct View(Document);

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (400., 300.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        self.0.render_viewport(&opts)
    }
}

/// The refits of one frame of `html`.
fn refits(cx: &mut TestAppContext, html: &str) -> Vec<(Fitted, Fitted, usize)> {
    REFITS.with_borrow_mut(Vec::clear);
    let doc = Document::builder(html, "").build().unwrap();
    let (_, cx) = cx.add_window_view(|_, _| View(doc));
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    REFITS.with_borrow_mut(std::mem::take)
}

const WORDS: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda";

#[gpui::test]
fn the_real_atom_takes_the_width_and_height_of_its_measured_copy(cx: &mut TestAppContext) {
    // The atom's max-content width is wider than its 120px containing block:
    // it shrinks to fit (CSS 2.1 §10.3.9) and wraps.
    let page = format!(
        "<body style='margin:0'><div style='width:120px'>\
         <p>x <span style='display:inline-block'>{WORDS}</span></p></div></body>"
    );
    let narrowed: Vec<_> = refits(cx, &page)
        .into_iter()
        .filter(|(copied, _, _)| !copied.is_empty())
        .collect();
    assert!(!narrowed.is_empty(), "no atom shrank to fit");
    for (copied, real, copies_left) in narrowed {
        assert_eq!(copies_left, 0, "a copy outlived the prepaint");
        assert_eq!(copied.len(), real.len());
        for (c, r) in copied.iter().zip(&real) {
            assert_eq!((c.0, c.1), (r.0, r.1), "atom index and width");
            assert!(c.1 <= 120.0 + 1e-3, "wider than the containing block");
            assert!((c.2 - r.2).abs() < 0.01, "height {} vs {}", c.2, r.2);
        }
    }
}

#[gpui::test]
fn an_atom_that_fits_keeps_no_copy_after_prepaint(cx: &mut TestAppContext) {
    let page =
        "<body style='margin:0'><p>x <span style='display:inline-block'>a b</span></p></body>";
    let refits = refits(cx, page);
    assert!(!refits.is_empty(), "the atom was not measured");
    for (copied, real, copies_left) in refits {
        assert_eq!((copied.len(), real.len(), copies_left), (0, 0, 0));
    }
}
