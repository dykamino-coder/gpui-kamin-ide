//! Зависимости размеров и независимые контексты обязаны сохранить общий fallback.

use super::*;

#[gpui::test]
fn numeric_serial_rejects_size_feedback(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let column = "display:flex;flex-direction:column";
    let paragraph =
        "<p style='font:10px/10px Ahem;margin:0'>aa aa aa aa aa aa aa aa aa aa aa aa</p>";
    let cases = [
        (format!("{column};height:50px"), paragraph.to_string()),
        (format!("{column};min-height:50px"), paragraph.to_string()),
        (format!("{column};max-height:50px"), paragraph.to_string()),
        (
            "display:flex;flex-direction:column-reverse".to_string(),
            paragraph.to_string(),
        ),
        (format!("{column};flex-wrap:wrap"), paragraph.to_string()),
        (format!("{column};flex-grow:1"), paragraph.to_string()),
        (format!("{column};justify-content:center"), paragraph.to_string()),
        (format!("{column};row-gap:10%"), paragraph.repeat(2)),
        ("display:grid".to_string(), paragraph.to_string()),
        (
            column.to_string(),
            "<p style='margin:0'><span style='float:left;width:20px;height:20px'></span>aa aa aa aa aa aa</p>".to_string(),
        ),
        (
            column.to_string(),
            format!("<div style='display:grid'>{paragraph}</div>"),
        ),
        (
            column.to_string(),
            paragraph.replace("margin:0", "margin:0;flex-basis:10px"),
        ),
    ];
    for (style, body) in cases {
        let tree = crate::document::parsed::parse(
            &format!("<section><article style='{style}'>{body}</article></section>"),
            "",
        );
        let opts = RenderOpts {
            viewport: (800., 600.),
            normal_line_height: 1.2,
            text: Default::default(),
            cx: Default::default(),
            frag: Default::default(),
        };
        let recipe = Recipe::new(
            section(&tree).unwrap(),
            &BoxStyle::default(),
            &opts,
            4,
            None,
            crate::layout::fragment::types::StackAxis::Horizontal,
            None,
        )
        .unwrap();
        cx.draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(800.), gpui::px(600.)),
            |window, app| {
                let mut stack = ColumnStack::new(
                    recipe.children(80., window, app),
                    4,
                    0.,
                    None,
                    None,
                    None,
                    None,
                    Default::default(),
                );
                for i in 0..stack.children.len() {
                    stack.lay_out_main(i, window, app);
                }
                assert!(
                    stack
                        .children
                        .iter()
                        .all(|c| Model::measure(c, stack.axis, window).is_none()),
                    "unsupported dependency: {style} {body}"
                );
                gpui::div()
            },
        );
    }
}
