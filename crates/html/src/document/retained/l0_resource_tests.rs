//! Ресурс, сменившийся после шага документа: следующий кадр собирает шаг
//! заново (а не повторяет записанный и не собирает внутри `Window::draw`),
//! после чего кадр снова устаивается.

use super::l0_tests::builds;
use super::tests::{frame, open_document};
use crate::Document;
use crate::resources::images::tests_support::{TempDir, png_sized};
use gpui::TestAppContext;

#[gpui::test]
fn a_resource_changed_after_the_step_rebuilds_the_next_frame(cx: &mut TestAppContext) {
    let mut dir = TempDir::new("l0-step-resource");
    dir.write("late.png", &png_sized(4, 4, [9, 9, 9, 255]));
    let page = dir.path().join("page.html");
    let doc = Document::builder("<img src='late.png'><p>text after the image</p>", "")
        .path(&page)
        .build()
        .unwrap();
    let (view, cx) = open_document(cx, doc);
    for _ in 0..3 {
        frame(cx);
    }
    let (settled, inside) = builds(&view, cx);
    frame(cx);
    assert_eq!(builds(&view, cx).0, settled, "a settled frame was rebuilt");
    // A new file revision between frames (another length, whatever the clock).
    dir.write("late.png", &png_sized(8, 6, [9, 9, 9, 255]));
    frame(cx);
    let (rebuilt, inside_after) = builds(&view, cx);
    assert!(
        rebuilt > settled,
        "the frame after the resource changed was a replay"
    );
    assert_eq!(inside_after, inside, "the rebuild ran inside draw");
    for _ in 0..3 {
        frame(cx);
    }
    let (again, inside_again) = builds(&view, cx);
    frame(cx);
    assert_eq!(builds(&view, cx).0, again, "the frame did not settle again");
    assert_eq!(inside_again, inside, "a frame was built inside draw");
}
