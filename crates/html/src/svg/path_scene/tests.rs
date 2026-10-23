//! Тесты: побайтовое сравнение типизированных сцен с эквивалентным разобранным SVG.
use super::*;
use crate::style::values::basic_shape::commands::Sink;
use crate::svg::path_commands::Geometry;

fn assert_pixels(scene: &Scene, body: &str) {
    let text = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">{body}</svg>"#,
        scene.width, scene.height
    );
    let xml = crate::svg::raster::geometry(&text, scene.width, scene.height).unwrap();
    let typed = scene.rasterize().unwrap();
    let (actual, expected) = (typed.as_bytes(0).unwrap(), xml.as_bytes(0).unwrap());
    assert_eq!(actual.len(), expected.len(), "{text}");
    let different = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert_eq!(different, 0, "different bytes: {text}");
}

#[test]
fn ordered_luminance_masks_preserve_antialiasing_and_colors() {
    for offset in [-3.7, 0.0, 5.3] {
        let white = rectangle(1.3, 2.7, 23.6, 17.1, gray(true)).unwrap();
        let black = rectangle(15.1 + offset, 8.3, 20.7, 22.3, gray(false)).unwrap();
        let color = paint(CssColor {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            a: 0.7,
        });
        let root = masked(40.0, 35.0, color, group(vec![white, black])).unwrap();
        assert_pixels(
            &Scene::new(40.0, 35.0, root),
            &format!(
                r##"<defs><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="40" height="35"><rect x="1.3" y="2.7" width="23.6" height="17.1" fill="white"/><rect x="{}" y="8.3" width="20.7" height="22.3" fill="black"/></mask></defs><rect width="40" height="35" fill="rgb(51,102,204)" fill-opacity="0.7" mask="url(#m)"/>"##,
                15.1 + offset
            ),
        );
    }
}

#[test]
fn blur_and_compound_clip_match_svg_compositing() {
    for sigma in [0.5, 2.0, 4.5] {
        let mut caster = group(vec![rectangle(9.3, 7.7, 13.4, 12.3, gray(true)).unwrap()]);
        caster = translated(caster, 2.3, -1.7);
        let mut blurred = group(vec![element(caster)]);
        blurred.blur = sigma;
        let mut d = Geometry::default();
        d.raw("M0 0H40V35H0Z M15 12H25V23H15Z");
        let mut root = masked(40.0, 35.0, gray(true), blurred).unwrap();
        root.clip = Some(PathShape {
            data: d.finish().unwrap(),
            fill: Some(gray(false)),
            rule: FillRule::EvenOdd,
            stroke: None,
        });
        assert_pixels(
            &Scene::new(40.0, 35.0, root),
            &format!(
                r##"<defs><filter id="b" filterUnits="userSpaceOnUse" x="0" y="0" width="40" height="35"><feGaussianBlur stdDeviation="{sigma}"/></filter><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="40" height="35"><g filter="url(#b)"><g transform="translate(2.3 -1.7)"><rect x="9.3" y="7.7" width="13.4" height="12.3" fill="white"/></g></g></mask><clipPath id="c"><path clip-rule="evenodd" d="M0 0H40V35H0Z M15 12H25V23H15Z"/></clipPath></defs><rect width="40" height="35" fill="white" mask="url(#m)" clip-path="url(#c)"/>"##
            ),
        );
    }
}

#[test]
fn scene_cache_keys_include_geometry_and_compositing() {
    let scene = Scene::new(
        20.0,
        20.0,
        group(vec![rectangle(1.0, 2.0, 9.0, 10.0, gray(true)).unwrap()]),
    );
    let same = scene.clone();
    assert!(Arc::ptr_eq(
        &scene.rasterize().unwrap(),
        &same.rasterize().unwrap()
    ));
    let mut altered = scene.clone();
    altered.root.transform = Transform::from_translate(1.0, 0.0);
    assert_ne!(fingerprint::scene(&scene), fingerprint::scene(&altered));
    altered = scene.clone();
    altered.root.mask = Some(Box::new(group(Vec::new())));
    assert_ne!(fingerprint::scene(&scene), fingerprint::scene(&altered));
}

#[test]
fn translated_scene_keeps_its_reference_box_offset() {
    let root = group(vec![rectangle(1.0, 2.0, 9.0, 10.0, gray(true)).unwrap()]);
    let root = translated(translated(root, 7.3, 8.7), 2.1, -1.9);
    assert_pixels(
        &Scene::new(40.0, 35.0, root),
        r#"<g transform="translate(2.1 -1.9)"><g transform="translate(7.3 8.7)"><rect x="1" y="2" width="9" height="10" fill="white"/></g></g>"#,
    );
}

#[test]
fn empty_and_degenerate_paths_keep_transparent_coverage() {
    for raw in ["", "M1 2", "M1 2L1 2Z"] {
        assert_pixels(
            &coverage(20.0, 15.0, Geometry::parse(raw), FillRule::NonZero),
            &format!(r#"<path d="{raw}" fill="white"/>"#),
        );
    }
}
