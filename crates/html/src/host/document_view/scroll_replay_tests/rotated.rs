//! Rotated, skewed and scaled boxes in a scrolled replay: their matrices are
//! built again from the transform's inputs at the moved origin, by the
//! function a fresh paint runs (`gpui::TransformPivot`), so a replayed step
//! paints the same bits as a tree built fresh at the new offset.

use super::*;

fn page() -> String {
    let mut boxes = String::new();
    for i in 0..24 {
        let (x, y) = (31 + (i * 53) % 560, 45 + i * 115);
        let transform = match i % 6 {
            0 => format!("rotate({}deg)", 7.3 + i as f32 * 3.1),
            1 => "rotate(-23.7deg) scale(1.15)".to_string(),
            2 => "skewY(-8deg)".to_string(),
            3 => "rotate(90deg)".to_string(),
            4 => "scale(0.85, 1.3) translate(4.5px, 3px)".to_string(),
            _ => format!("rotate({}deg)", 41.9 - i as f32),
        };
        boxes.push_str(&format!(
            "<div style='position:absolute;left:{x}px;top:{y}px;width:90px;height:60px;\
             background:linear-gradient(45deg,#c96,#369);border-radius:12px;\
             border:2px solid #234;transform-origin:30% 70%;transform:{transform}'></div>"
        ));
    }
    // Quarter turns whose fills snap both edges (no border, no rounding).
    for i in 0..6 {
        let (x, y) = (57 + i * 97, 80 + i * 410);
        let turn = ["rotate(-90deg)", "rotate(180deg)", "scaleX(-1)", "rotate(270deg) translate(0.5px, 0.25px)"][i % 4];
        boxes.push_str(&format!(
            "<div style='position:absolute;left:{x}.5px;top:{y}.25px;width:61.5px;height:33px;\
             background:#3a6;transform:{turn}'></div>"
        ));
    }
    format!("<body style='margin:0;height:3200px;position:relative;background:#eee'>{boxes}</body>")
}

/// Rotated boxes around their centres, most below the first viewport: they
/// first show from a dormant command, steps after the frame that kept it.
fn below() -> String {
    let mut boxes = String::new();
    for i in 0..60 {
        let (x, y, deg) = (73 + (i * 53) % 583, 45 + i * 45, 7.3 * (i + 1) as f32 % 90.0);
        boxes.push_str(&format!(
            "<div style='position:absolute;left:{x}px;top:{y}px;width:90px;height:60px;\
             background:linear-gradient(45deg,#c96,#369);border-radius:12px;transform:rotate({deg}deg)'></div>"
        ));
    }
    format!("<body style='margin:0;height:3200px;position:relative;background:#eee'>{boxes}</body>")
}

#[gpui::test]
fn wheel_steps_over_rotated_boxes_replay_the_fresh_bits(cx: &mut TestAppContext) {
    for html in [page(), below()] {
        wheel_over(cx, html);
    }
}

fn wheel_over(cx: &mut TestAppContext, html: String) {
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -40., -120., 80., -360., -120., -120.].into_iter().enumerate() {
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        let offset = root(&view, vcx).offset();
        let name = format!("wheel {i} by {dy} (builds {}, replays {})", b1 - b0, r1 - r0);
        // The quads hold their matrices: equal strings are equal bits.
        assert_same(&name, &painted(vcx), &fresh_quads(cx, &html, offset));
        assert_eq!(b1 - b0, 0, "{name}");
    }
}
