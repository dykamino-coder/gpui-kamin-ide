//! Large device-aligned positions must not become fractional replay deltas.

use super::*;

fn at_scale(cx: &mut TestAppContext, html: &str, offset: f32) -> Vec<Painted> {
    let mut app = cx.clone();
    let (view, fresh) = open(&mut app, html);
    fresh.simulate_scale_factor_change(1.25);
    root(&view, fresh).set_offset(point(px(0.), px(offset)));
    fresh.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document
                .update(&format!("{html}<!-- fresh -->"), "")
                .unwrap();
            cx.notify();
        });
    });
    frame(fresh);
    frame(fresh);
    painted(fresh)
}

#[gpui::test]
fn device_grid_steps_deep_in_a_document_replay_at_fractional_dpr(cx: &mut TestAppContext) {
    let html = format!(
        "<style>body{{margin:0}}aside{{height:111000px}}.r{{height:20px;background:#ddd}}</style>\
         <aside></aside>{}",
        (0..300)
            .map(|i| format!("<div class=r>row {i}</div>"))
            .collect::<String>()
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    vcx.simulate_scale_factor_change(1.25);
    root(&view, vcx).set_offset(point(px(0.), px(-111953.6)));
    frame(vcx);
    frame(vcx);
    let before = frames(&view, vcx);
    for offset in [-112031.2, -112071.2, -112272.0, -112889.6] {
        root(&view, vcx).set_offset(point(px(0.), px(offset)));
        frame(vcx);
        assert_eq!(frames(&view, vcx).0, before.0, "build at {offset}");
        assert_same("deep scroll", &painted(vcx), &at_scale(cx, &html, offset));
    }
}
