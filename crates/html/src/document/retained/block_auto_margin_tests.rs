//! Normal-flow auto margins resolve after min/max-width (CSS 2 §10.3.3,
//! §10.4), including font-relative limits and padding.

use super::tests::{frame, open};
use gpui::{TestAppContext, px, size};

#[gpui::test]
fn board_appearance_auto_margins_follow_the_final_constrained_width(cx: &mut TestAppContext) {
    for width in [375.0_f32, 800.0, 1440.0] {
        for (rule, expected) in [
            ("", width),
            ("max-width:20em", width.min(400.0)),
            ("width:80%;max-width:20em", (width * 0.8).min(400.0)),
            ("width:200px", 200.0),
            ("max-width:20em;padding:0 20px", width.min(440.0)),
            (
                "width:80%;max-width:20em;padding:0 20px",
                (width * 0.8 + 40.0).min(440.0),
            ),
        ] {
            let html = format!(
                "<style>html,body{{margin:0;font-size:20px}}div{{height:20px;\
                 margin:0 auto;background:blue;{rule}}}</style><div></div>"
            );
            let mut app = cx.clone();
            let (view, vcx) = open(&mut app, &html);
            vcx.simulate_resize(size(px(width), px(300.0)));
            vcx.update(|_, cx| view.update(cx, |view, _| view.viewport = (width, 300.0)));
            frame(vcx);
            let boxes = vcx.update(|window, _| {
                let scale = window.scale_factor();
                window
                    .painted_quads()
                    .into_iter()
                    .filter(|quad| quad.background.as_solid() == Some(gpui::blue()))
                    .map(|quad| {
                        (
                            (quad.bounds.origin.x.0 + quad.transformation.translation[0]) / scale,
                            quad.bounds.size.width.0 / scale,
                        )
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(boxes.len(), 1, "{width} {rule}: {boxes:?}");
            let (x, actual) = boxes[0];
            assert!(
                (actual - expected).abs() <= 1.0,
                "{width} {rule}: {boxes:?}"
            );
            assert!(
                (x - (width - expected) / 2.0).abs() <= 1.0,
                "{width} {rule}: {boxes:?}"
            );
        }
    }
}
