//! Ограниченный проход по вложенным контейнерам и SVG без CSS-коробки.
use super::fixture::*;
use crate::style::values::value::Len;

#[gpui::test]
fn nested_containers_settle_once_per_source_after_resize(cx: &mut gpui::TestAppContext) {
    let mut html =
        String::from("<div id='outer' style='container-type:size;width:100%;height:320px'>");
    for id in 1..=5 {
        html.push_str(&format!(
            "<div id='c{id}' style='container-type:size;width:50cqw;height:50cqh'>"
        ));
    }
    html.push_str(&"</div>".repeat(6));
    // Шесть постоянных источников, затем один окончательный render.
    // Бюджет теста также охватывает resize и последующий стабильный кадр.
    run_bounded(cx, &html, 15, |doc, width, renders, cx| {
        assert_eq!(content(doc, "c5"), (12.5, 10.));
        assert!(renders.get() <= 7, "initial renders: {}", renders.get());
        let before = renders.get();
        width.set(800.);
        cx.update(|window, app| {
            window.refresh();
            window.draw(app).clear(app);
        });
        assert!(renders.get() - before <= 7);
        for id in 1..=5 {
            let divisor = (1 << id) as f32;
            assert_eq!(
                content(doc, &format!("c{id}")),
                (800. / divisor, 320. / divisor)
            );
        }
        let before = renders.get();
        cx.update(|window, app| {
            window.refresh();
            window.draw(app).clear(app);
        });
        assert_eq!(renders.get() - before, 1);
    });
}

#[gpui::test]
fn foreign_object_cannot_create_a_new_style_input_on_each_prepaint(cx: &mut gpui::TestAppContext) {
    run_bounded(
        cx,
        r#"
        <div style="container-type:size;width:300px;height:200px">
          <svg width="200" height="120">
            <foreignObject id="foreign" style="display:block;container-type:size;
              width:100px;height:100px">
              <div id="inner" style="container-type:size;width:50cqw;height:50cqh">
                <div id="leaf" style="width:50cqw;height:50cqh"></div>
              </div>
            </foreignObject>
          </svg>
        </div>"#,
        5,
        |doc, _, renders, _| {
            let foreign = element(doc.nodes(), "foreign").style.computed();
            assert_eq!(foreign.query_container_axes(), (false, false));
            assert_eq!(foreign.reset_context.cq_size, (None, None));
            assert_eq!(content(doc, "inner"), (150., 100.));
            let leaf = element(doc.nodes(), "leaf").style.computed();
            assert_eq!(leaf.reset_box_layout.width, Some(Len::Px(75.)));
            assert_eq!(leaf.reset_box_layout.height, Some(Len::Px(50.)));
            assert!(renders.get() <= 3, "initial renders: {}", renders.get());
        },
    );
}

#[gpui::test]
fn a_container_no_value_reads_lays_out_once_at_any_width(cx: &mut gpui::TestAppContext) {
    // No value of the document holds a container-relative length: the
    // containers' sizes reach nothing, so a resize renders the frame once and
    // keeps every computed style.
    let mut html = String::from("<div id='outer' style='container-type:inline-size;width:100%'>");
    for id in 1..=3 {
        html.push_str(&format!(
            "<div id='c{id}' style='container-type:inline-size;width:50%;padding:1px'>text"
        ));
    }
    html.push_str(&"</div>".repeat(4));
    run_bounded(cx, &html, 4, |doc, width, renders, cx| {
        assert_eq!(renders.get(), 1, "initial renders");
        let held = element(doc.nodes(), "c3").style.computed();
        assert_eq!(held.reset_context.cq_size, (None, None));
        width.set(801.);
        cx.update(|window, app| {
            window.refresh();
            window.draw(app).clear(app);
        });
        assert_eq!(renders.get(), 2, "one render for the new width");
        let kept = element(doc.nodes(), "c3").style.computed();
        assert!(std::rc::Rc::ptr_eq(&held, &kept), "the style was kept");
    });
}
