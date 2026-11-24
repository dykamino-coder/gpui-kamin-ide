//! Analytic geometry and stops draw directly in the target without a bitmap tile.
use super::*;
use gpui::{
    AnalyticGradient, Background, ContentMask, GradientQuad, PaintGroup, PaintSurface, Primitive,
    Quad, ScaledPixels, bounds, point, size,
};

fn rect(x: f32, y: f32, w: f32, h: f32) -> gpui::Bounds<ScaledPixels> {
    bounds(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}
fn paint(s: &mut Scene, geometry: [f32; 4], radial: bool, repeating: bool, stops: Vec<[f32; 5]>) {
    let bounds = rect(0., 0., 32., 16.);
    let background = Background::from(gpui::rgba(0xffffffff));
    s.insert_primitive(Primitive::Gradient(GradientQuad {
        quad: Quad {
            bounds,
            content_mask: ContentMask { bounds },
            background,
            ..Default::default()
        },
        gradient: AnalyticGradient::new(geometry, [32., 16.], radial, repeating, stops).unwrap(),
    }));
}
fn device() -> DirectXDevices {
    DirectXDevices::with_instrumentation(
        crate::GpuAdapterPreference::Software,
        gpui::Instrumentation::default(),
    )
    .unwrap()
}
fn read(r: &mut DirectXRenderer, s: &Scene) -> image::RgbaImage {
    r.render_to_image(s, WindowBackgroundAppearance::Opaque)
        .unwrap()
}
#[test]
fn arbitrary_stop_buffer_draws_hard_repeating_and_radial_gradients() {
    let d = device();
    let mut r = offscreen(&d, 32, 16);
    let colors = [
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 1., 0.],
        [1., 0., 1.],
        [0., 1., 1.],
    ];
    let mut stops = Vec::new();
    for (i, c) in colors.iter().enumerate() {
        for t in [i as f32 / 6., (i + 1) as f32 / 6.] {
            stops.push([c[0], c[1], c[2], 1., t]);
        }
    }
    let mut s = Scene::default();
    paint(&mut s, [1., 0., 32., 0.], false, false, stops);
    s.finish();
    let pixels = read(&mut r, &s);
    for x in 0..32 {
        let c = colors[(((x as f32 + 0.5) / 32. * 6.) as usize).min(5)];
        assert_eq!(
            pixels.get_pixel(x, 8).0,
            [
                (c[0] * 255.) as u8,
                (c[1] * 255.) as u8,
                (c[2] * 255.) as u8,
                255
            ],
            "x={x}"
        );
    }
    let stripe = vec![
        [1., 0., 0., 1., 0.],
        [1., 0., 0., 1., 0.125],
        [0., 0., 1., 1., 0.125],
        [0., 0., 1., 1., 0.25],
    ];
    let mut s = Scene::default();
    paint(&mut s, [1., 0., 32., 0.], false, true, stripe);
    s.finish();
    let pixels = read(&mut r, &s);
    for x in 0..32 {
        assert_eq!(
            pixels.get_pixel(x, 8).0,
            if x % 8 < 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        );
    }
    let mut s = Scene::default();
    paint(
        &mut s,
        [5., 7., 4., 2.],
        true,
        false,
        vec![
            [1., 0., 0., 1., 0.],
            [1., 0., 0., 1., 1.],
            [0., 0., 1., 1., 1.],
        ],
    );
    s.finish();
    let pixels = read(&mut r, &s);
    assert_eq!(pixels.get_pixel(5, 7).0, [255, 0, 0, 255]);
    assert_eq!(pixels.get_pixel(11, 7).0, [0, 0, 255, 255]);
}
#[test]
fn stop_alpha_and_changed_content_survive_retained_upload_and_group_batches() {
    let d = device();
    let mut r = offscreen(&d, 32, 16);
    for green in [false, true, false] {
        let c = if green {
            [0., 1., 0., 0.5]
        } else {
            [1., 0., 0., 0.5]
        };
        let stops = vec![[c[0], c[1], c[2], c[3], 0.], [c[0], c[1], c[2], c[3], 1.]];
        let mut inner = Scene::default();
        paint(&mut inner, [1., 0., 32., 0.], false, false, stops);
        inner.finish();
        let plain = read(&mut r, &inner);
        let mut outer = Scene::default();
        let b = rect(0., 0., 32., 16.);
        outer.groups.push(PaintGroup {
            scene: std::rc::Rc::new(inner),
            bounds: b,
            ..Default::default()
        });
        outer.surfaces.push(PaintSurface {
            order: 1,
            bounds: b,
            content_mask: ContentMask { bounds: b },
            group: 1,
            opacity: 1.,
            corner_radii: Default::default(),
            blur_radius: 0.,
            color_matrix: None,
            fragment: 0,
        });
        let grouped = read(&mut r, &outer);
        assert_eq!(plain, grouped);
        let got = plain.get_pixel(8, 8).0;
        let expected = if green {
            [127, 255, 127, 255]
        } else {
            [255, 127, 127, 255]
        };
        for (a, b) in got.into_iter().zip(expected) {
            assert!(a.abs_diff(b) <= 1, "{got:?}");
        }
    }
    read(&mut r, &Scene::default());
    assert!(r.pipelines.quad_pipeline.gradients.is_none());
}

#[test]
fn translated_hard_stops_keep_the_same_pixels_on_an_iso_line() {
    let d = device();
    let mut r = offscreen(&d, 100, 280);
    let clip = rect(50., 50., 38., 218.);
    let mut expected = None;
    for origin in [38., 42., 50.] {
        let mut scene = Scene::default();
        let bounds = rect(origin, origin, 50., 230.);
        scene.insert_primitive(Primitive::Gradient(GradientQuad {
            quad: Quad {
                bounds,
                content_mask: ContentMask { bounds: clip },
                background: Background::from(gpui::rgba(0xffffffff)),
                ..Default::default()
            },
            gradient: AnalyticGradient::new(
                [
                    std::f32::consts::FRAC_1_SQRT_2,
                    -std::f32::consts::FRAC_1_SQRT_2,
                    280. * std::f32::consts::FRAC_1_SQRT_2,
                    0.,
                ],
                [50., 230.],
                false,
                false,
                vec![
                    [1., 0., 0., 1., 0.],
                    [1., 0., 0., 1., 0.5],
                    [0., 0., 1., 1., 0.5],
                    [0., 0., 1., 1., 0.75],
                    [0., 1., 0., 1., 0.75],
                    [0., 1., 0., 1., 1.],
                ],
            )
            .unwrap(),
        }));
        scene.finish();
        let pixels = read(&mut r, &scene);
        if let Some(expected) = &expected {
            assert_eq!(&pixels, expected, "origin={origin}");
        } else {
            expected = Some(pixels);
        }
    }
}

fn plain(s: &mut Scene, x: f32, w: f32, color: u32) {
    let bounds = rect(x, 0., w, 16.);
    s.insert_primitive(Primitive::Quad(Quad {
        bounds,
        content_mask: ContentMask { bounds },
        background: Background::from(gpui::rgba(color)),
        ..Default::default()
    }));
}

#[test]
fn gradient_runs_keep_paint_order_and_compile_their_program_on_first_use() {
    let d = device();
    let mut r = offscreen(&d, 32, 16);
    let mut s = Scene::default();
    plain(&mut s, 0., 32., 0x00ff00ff);
    s.finish();
    assert_eq!(read(&mut r, &s).get_pixel(4, 8).0, [0, 255, 0, 255]);
    assert!(!r.pipelines.quad_gradient.is_created(), "no analytic quad was drawn");
    // plain, analytic, plain: the second plain quad must stay above the gradient.
    let mut s = Scene::default();
    plain(&mut s, 0., 32., 0x00ff00ff);
    paint(&mut s, [1., 0., 32., 0.], false, false, vec![[1., 0., 0., 1., 0.], [1., 0., 0., 1., 1.]]);
    plain(&mut s, 8., 8., 0x0000ffff);
    s.finish();
    let pixels = read(&mut r, &s);
    assert_eq!(pixels.get_pixel(2, 8).0, [255, 0, 0, 255]);
    assert_eq!(pixels.get_pixel(12, 8).0, [0, 0, 255, 255]);
    assert_eq!(pixels.get_pixel(20, 8).0, [255, 0, 0, 255]);
    assert!(r.pipelines.quad_gradient.is_created());
}