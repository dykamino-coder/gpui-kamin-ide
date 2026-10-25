//! Глубокий документ и реальные фазы кадра на фиксированном стеке 8 МиБ.

use super::*;

pub(super) fn on_eight_mib(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn parse_and_cascade_keep_the_leaf_at_browser_depth() {
    on_eight_mib(|| {
        let document = Document::builder(&markup(1026), "").build().unwrap();
        assert!(document.traversal_limit().is_none());
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "leaf");
    });
}

#[test]
fn box_builder_keeps_2052_levels_on_eight_mib() {
    on_eight_mib(|| {
        let nodes = tree(2052);
        let boxes = crate::render::render(&nodes, &options());
        assert!(!boxes.is_empty());
        drop(boxes);
        drop(nodes);
    });
}

fn frame(stage: &str) {
    let mut cx = gpui::TestAppContext::single();
    let document = Document::builder(&markup(1026), "").build().unwrap();
    assert!(document.traversal_limit().is_none());
    let boxes = document.render(&options());
    cx.add_empty_window().draw(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(100.), gpui::px(100.)),
        |_, _| FrameProbe(gpui::div().children(boxes).into_any_element(), stage.into()),
    );
}

#[test]
fn request_layout_1026_levels_on_eight_mib() {
    on_eight_mib(|| frame("request_layout"));
}

#[test]
fn layout_1026_levels_on_eight_mib() {
    on_eight_mib(|| frame("layout"));
}

#[test]
fn prepaint_1026_levels_on_eight_mib() {
    on_eight_mib(|| frame("prepaint"));
}

#[test]
fn paint_1026_levels_on_eight_mib() {
    on_eight_mib(|| frame("paint"));
}
