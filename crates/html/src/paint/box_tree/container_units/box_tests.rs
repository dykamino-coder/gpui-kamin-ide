//! Content box с прокруткой, атомарными inline, zoom и шрифтовыми длинами.
use super::fixture::*;
use crate::style::values::value::Len;

#[gpui::test]
fn scrollbars_and_stable_gutters_are_outside_the_query_box(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="container-type:size;box-sizing:border-box;width:200px;height:150px;
        padding:10px;border:5px solid;overflow:scroll">
        <div id="bars" style="container-type:size;width:100cqw;height:100cqh"></div>
      </div>
      <div style="container-type:size;box-sizing:border-box;width:200px;height:150px;
        padding:10px;border:5px solid;overflow:scroll;scrollbar-gutter:stable both-edges">
        <div id="gutters" style="container-type:size;width:100cqw;height:100cqh"></div>
      </div>"#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "bars"), (155., 105.));
            assert_eq!(content(doc, "gutters"), (140., 105.));
        },
    );
}

#[gpui::test]
fn atomic_inline_and_ineligible_boxes_select_the_real_ancestor(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="container-type:size;width:300px;height:200px">
        <span style="display:inline-block;container-type:inline-size;width:50%;
          box-sizing:border-box;padding:10px">
          <div id="atomic" style="container-type:size;width:100cqw;height:100cqh"></div>
        </span>
        <div style="display:contents;container-type:size;width:10px;height:10px">
          <span style="container-type:size;width:20px;height:20px">
            <span id="skipped" style="display:inline-block;container-type:size;
              width:100cqw;height:100cqh"></span>
          </span>
        </div>
      </div>
      <div id="fallback" style="container-type:size;width:10cqw;height:10cqh"></div>
      "#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "atomic"), (130., 200.));
            assert_eq!(content(doc, "skipped"), (300., 200.));
            assert_eq!(content(doc, "fallback"), (40., 60.));
        },
    );
}

#[gpui::test]
fn container_css_size_is_scaled_by_the_consumers_zoom(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="container-type:size;zoom:2;width:200px;height:100px">
        <div id="child" style="container-type:size;zoom:2;width:50cqw;height:50cqh"></div>
      </div>"#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "child"), (100., 50.));
            assert_eq!(
                element(doc.nodes(), "child").style.computed().width,
                Some(Len::Px(400.))
            );
        },
    );
}

#[gpui::test]
fn container_font_inputs_precede_font_relative_lengths(cx: &mut gpui::TestAppContext) {
    run(
        cx,
        r#"
      <div style="container-type:size;width:300px;height:200px;writing-mode:vertical-rl">
        <div id="font" style="container-type:size;font-size:10cqi;width:2em;height:20px"></div>
        <div id="line" style="container-type:size;line-height:10cqw;width:20px;height:2lh"></div>
        <div id="mixed" style="container-type:size;width:calc(50% + 10cqw);height:20px"></div>
      </div>"#,
        |doc, _, _, _| {
            assert_eq!(content(doc, "font"), (40., 20.));
            assert_eq!(content(doc, "line"), (20., 60.));
            assert_eq!(content(doc, "mixed"), (180., 20.));
        },
    );
}
