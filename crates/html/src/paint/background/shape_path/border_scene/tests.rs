//! Тесты: точное покрытие расширения и сжатия контура границы.
use super::*;

#[test]
fn border_offsets_match_ordered_svg_fill_and_stroke() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let raw = "polygon(0px 0px, 20px 0px, 20px 15px, 0px 15px)";
    let contour = Contour::place((raw, [0.0; 4]), 20.0, 15.0, 7.3, 8.7, vcx).unwrap();
    for white in [false, true] {
        for distance in [-2.3f32, 0.0, 3.7] {
            let fill = if white { "white" } else { "black" };
            let opposite = if white { "black" } else { "white" };
            let width = distance.abs() * 2.0;
            let path = "M0 0L20 0L20 15L0 15Z";
            let body = if distance < 0.0 {
                format!(
                    r#"<path d="{path}" fill="{fill}"/><path d="{path}" fill="none" stroke="{opposite}" stroke-width="{width}" stroke-miterlimit="1000"/>"#
                )
            } else {
                format!(
                    r#"<path d="{path}" fill="{fill}" stroke="{fill}" stroke-width="{width}" stroke-miterlimit="1000"/>"#
                )
            };
            let xml = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="35"><g transform="translate(7.3 8.7)">{body}</g></svg>"#
            );
            let expected = crate::svg::raster::geometry(&xml, 40.0, 35.0).unwrap();
            let scene = Scene::new(40.0, 35.0, contour.offset(white, distance));
            assert_eq!(
                scene.rasterize().unwrap().as_bytes(0),
                expected.as_bytes(0),
                "{xml}"
            );
        }
    }
}
