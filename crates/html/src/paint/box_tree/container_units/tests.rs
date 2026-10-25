//! Реальная геометрия HTML и вычисленные длины в единицах контейнера.
use super::fixture::*;
use crate::style::values::value::Len;

#[gpui::test]
fn percentages_min_max_and_border_box_use_solved_content(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="width:400px;height:300px">
        <div id="container" style="container-type:size;box-sizing:border-box;
          width:75%;min-width:350px;max-width:380px;height:80%;max-height:180px;
          padding:20px;border:5px solid">
          <div id="child" style="container-type:size;width:50cqw;height:50cqh"></div>
        </div>
      </div>"#,
        |doc, _, renders, cx| {
            assert_eq!(content(doc, "container"), (300., 130.));
            assert_eq!(content(doc, "child"), (150., 65.));
            let before = renders.get();
            cx.update(|window, app| {
                window.refresh();
                window.draw(app).clear(app);
            });
            assert_eq!(renders.get() - before, 1, "stable frame must not retry");
        },
    );
}

#[gpui::test]
fn nearest_supported_axis_and_consumer_writing_mode(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div id="outer" style="container-type:size;width:300px;height:200px">
        <div style="contain:size;container-type:inline-size;width:120px;height:100px">
          <div id="physical" style="container-type:size;width:100cqw;height:100cqh"></div>
          <div id="logical" style="container-type:size;writing-mode:vertical-rl;
            width:100cqi;height:100cqb"></div>
          <div id="minmax" style="container-type:size;width:100cqmin;height:100cqmax"></div>
        </div>
      </div>"#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "physical"), (120., 200.));
            assert_eq!(content(doc, "logical"), (200., 120.));
            assert_eq!(content(doc, "minmax"), (120., 200.));
        },
    );
}

#[gpui::test]
fn inherited_computed_lengths_and_resize(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div id="outer" style="container-type:size;width:100%;height:200px">
        <div id="inner" style="container-type:size;width:50%;height:100px;
          line-height:10cqw;border-spacing:10cqw 20cqh">
          <div id="child" style="container-type:size;width:50cqw;height:20px"></div>
        </div>
      </div>"#,
        |doc, width, _, cx| {
            assert_eq!(content(doc, "child"), (100., 20.));
            assert_eq!(
                element(doc.nodes(), "child").style.computed().line_height,
                Some(Len::Px(40.))
            );
            assert_eq!(
                element(doc.nodes(), "child")
                    .style
                    .computed()
                    .border_spacing,
                Some((Some(Len::Px(40.)), Some(Len::Px(40.))))
            );
            width.set(600.);
            cx.update(|window, app| {
                window.refresh();
                window.draw(app).clear(app);
            });
            assert_eq!(content(doc, "child"), (150., 20.));
            assert_eq!(
                element(doc.nodes(), "child").style.computed().line_height,
                Some(Len::Px(60.))
            );
            assert_eq!(
                element(doc.nodes(), "child")
                    .style
                    .computed()
                    .border_spacing,
                Some((Some(Len::Px(60.)), Some(Len::Px(40.))))
            );
        },
    );
}

#[gpui::test]
fn vertical_container_supports_its_inline_axis_only(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="container-type:size;width:300px;height:200px">
        <div style="container-type:inline-size;writing-mode:vertical-rl;width:120px;height:100px">
          <div id="child" style="container-type:size;writing-mode:horizontal-tb;
            width:100cqw;height:100cqh"></div>
        </div>
      </div>"#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "child"), (300., 100.));
        },
    );
}
