//! Инвалидация по прочитанным входам, независимые владельцы и ленивые запросы.

use super::*;
use crate::text::paragraph::{Align, Wrap};
use gpui::{Context, IntoElement, Render, TestAppContext, TextRun, px};

fn paragraph(text: &str, owner: &Rc<Retained>) -> Paragraph {
    Paragraph::new(
        text.to_owned().into(),
        vec![TextRun {
            len: text.len(),
            ..Default::default()
        }],
        px(16.),
        px(20.),
        Align::Left,
        Wrap::default(),
    )
    .retained(Some(owner.clone()))
}

#[test]
fn keys_separate_shape_breaks_and_paint_without_hash_collisions() {
    let owner = Rc::default();
    let mut p = paragraph("AV office", &owner);
    let shape = Inputs::shape(&p);
    let lines = Inputs::lines(&p);
    p.runs[0].color = gpui::red();
    p.indent_basis = Some(px(123.));
    assert!(shape.matches_shape(&p));
    assert!(lines.matches_lines(&p)); // Constraint stores the basis separately.
    p.wrap.nowrap = true;
    assert!(shape.matches_shape(&p));
    assert!(!lines.matches_lines(&p));
    p.runs[0].font.stretch = gpui::FontStretch::Condensed;
    assert!(!shape.matches_shape(&p));
    let shape = Inputs::shape(&p);
    p.runs[0].font.family = "another family".into();
    assert!(!shape.matches_shape(&p));
    let shape = Inputs::shape(&p);
    p.letter_spacing = px(1.);
    assert!(!shape.matches_shape(&p));
}

struct View;
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::div()
    }
}

#[gpui::test]
fn a_requested_box_keeps_glyphs_and_lines_after_frame_eviction(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| View);
    cx.update(|window, _| {
        let owner = Rc::default();
        let untouched = Rc::default();
        let p = paragraph("one two three four five", &owner);
        let _unrequested = paragraph("must stay lazy", &untouched);
        let first = p.measure(window);
        let lines = p.split(Some(px(90.)), window);
        let misses = p.measures.split_misses.get();
        for _ in 0..4 {
            p.measures.begin_frame();
        }
        let again = p.measure(window);
        assert!(Arc::ptr_eq(&first[0].layout, &again[0].layout));
        assert!(Rc::ptr_eq(&first[0].positions, &again[0].positions));
        assert_eq!(p.split(Some(px(90.)), window).len(), lines.len());
        assert_eq!(p.measures.split_misses.get(), misses);
        p.split(Some(px(70.)), window);
        assert_eq!(p.measures.split_misses.get(), misses + 1);
        assert!(Arc::ptr_eq(&first[0].layout, &p.measure(window)[0].layout));
        assert!(
            untouched.state.borrow().shape.is_none(),
            "no document-wide flush"
        );
        let other = paragraph("unrelated text", &untouched);
        let unchanged = other.measure(window);
        paragraph("edited text", &owner).measure(window);
        assert!(Arc::ptr_eq(
            &unchanged[0].layout,
            &other.measure(window)[0].layout
        ));
        assert!(owner.measured(&p, window).is_none());
    });
}

#[gpui::test]
fn font_installation_invalidates_all_retained_text_stages(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| View);
    cx.update(|window, _| {
        let owner = Rc::default();
        let p = paragraph("abc", &owner);
        p.split(Some(px(90.)), window);
        let shaped = p
            .shape_piece(&(0..3), p.runs.clone(), false, "", window)
            .unwrap();
        assert!(owner.painted(&p, &(0..3), &p.runs, window).is_some());
        let generation = window.text_system().font_generation();
        window.text_system().add_fonts(vec![]).unwrap();
        assert_ne!(window.text_system().font_generation(), generation);
        assert!(owner.measured(&p, window).is_none());
        assert!(owner.lines(&p, Some(px(90.)), window).is_none());
        assert!(owner.painted(&p, &(0..3), &p.runs, window).is_none());
        assert_eq!(shaped.text.as_ref(), "\u{202d}abc\u{202c}");
    });
}

#[gpui::test]
fn paint_reuses_controlled_geometry_with_fresh_colors_and_invisible_runs(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| View);
    cx.update(|window, _| {
        let owner = Rc::default();
        let mut p = paragraph("a\u{2060}b", &owner);
        let range = 0..p.text.len();
        p.measure(window);
        let first = p
            .shape_piece(&range, p.runs.clone(), false, "", window)
            .unwrap();
        p.runs[0].color = gpui::red();
        let again = p
            .shape_piece(&range, p.runs.clone(), false, "", window)
            .unwrap();
        assert!(Arc::ptr_eq(
            &first.retained_layout(),
            &again.retained_layout()
        ));
        assert_eq!(again.text.as_ref(), "\u{202d}ab\u{202c}");
        let fresh = p.retained(None);
        let fresh = fresh
            .shape_piece(&range, fresh.runs.clone(), false, "", window)
            .unwrap();
        assert_eq!(format!("{again:?}"), format!("{fresh:?}"));
    });
}
