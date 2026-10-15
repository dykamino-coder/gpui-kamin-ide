//! KaminIDE patch (INC-2026-0001): фон окна — очисткой кадра, а не квадом.
//!
//! Корень интерфейса первым рисует сплошной непрозрачный квад на всё окно.
//! Кадр перед этим всё равно очищается, а очистка цели на видеокарте почти
//! бесплатна, тогда как квад проходит пиксельный шейдер и смешивание на каждом
//! пикселе окна. Замер на Intel UHD (окно 1750×1093): квады стоили 4,57 мс GPU
//! на кадр при 4,13 Мпикс площади, из них 1,9 Мпикс — этот фон; без него
//! 3,05 мс и 2,22 Мпикс, кадр целиком 5,40 → 3,89 мс, скриншоты совпадают
//! бит в бит. На WARP цена кадра пропорциональна площади, поэтому выигрыш
//! там того же порядка.
//!
//! Если первый примитив кадра — такой квад, кадр очищается его цветом, а сам
//! квад не рисуется. Цвет переводится тем же `Hsla → Rgba`, что и
//! `hsla_to_rgba` в шейдере, а цель кадра — UNORM без sRGB-преобразования,
//! так что пиксели те же.

use gpui::{PrimitiveBatch, Rgba, Scene};

/// Цвет очистки вместо первого квада сцены, если тот закрывает всё окно
/// `width × height` (физические px) и ничего под собой не прячет.
pub(super) fn opaque_backdrop(scene: &Scene, width: u32, height: u32) -> Option<[f32; 4]> {
    // Квад обязан быть самым первым примитивом: иначе под ним уже что-то
    // нарисовано, и очистка это стёрла бы.
    match scene.batches().next() {
        Some(PrimitiveBatch::Quads(range)) if range.start == 0 && !range.is_empty() => {}
        _ => return None,
    }
    let quad = scene.quads.first()?;
    let color = quad.background.as_solid()?;
    if color.a < 1.0 {
        return None;
    }
    let r = &quad.corner_radii;
    let b = &quad.border_widths;
    let t = &quad.transformation;
    let flat = [r.top_left, r.top_right, r.bottom_left, r.bottom_right]
        .iter()
        .chain([b.top, b.right, b.bottom, b.left].iter())
        .all(|v| v.0 == 0.0);
    let unit = t.rotation_scale == [[1.0, 0.0], [0.0, 1.0]] && t.translation == [0.0, 0.0];
    let (w, h) = (width as f32, height as f32);
    let covers = |b: &gpui::Bounds<gpui::ScaledPixels>| {
        b.origin.x.0 <= 0.0
            && b.origin.y.0 <= 0.0
            && b.origin.x.0 + b.size.width.0 >= w
            && b.origin.y.0 + b.size.height.0 >= h
    };
    if !(flat && unit && covers(&quad.bounds) && covers(&quad.content_mask.bounds)) {
        return None;
    }
    let rgba = Rgba::from(color);
    Some([rgba.r, rgba.g, rgba.b, 1.0])
}

#[cfg(test)]
mod tests {
    use super::opaque_backdrop;
    use gpui::{
        Bounds, ContentMask, Corners, Edges, Hsla, Quad, ScaledPixels, Scene, Shadow,
        TransformationMatrix, point, size,
    };

    const W: u32 = 800;
    const H: u32 = 600;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: point(ScaledPixels(x), ScaledPixels(y)),
            size: size(ScaledPixels(w), ScaledPixels(h)),
        }
    }

    fn quad(color: Hsla) -> Quad {
        let window = rect(0.0, 0.0, W as f32, H as f32);
        Quad {
            order: 0,
            border_style: Default::default(),
            bounds: window,
            content_mask: ContentMask { bounds: window },
            background: color.into(),
            border_color: Hsla::default(),
            corner_radii: Corners::default(),
            border_widths: Edges::default(),
            transformation: TransformationMatrix::unit(),
        }
    }

    fn scene_of(quads: Vec<Quad>) -> Scene {
        let mut scene = Scene::default();
        for q in quads {
            scene.insert_primitive(q);
        }
        scene.finish();
        scene
    }

    fn dark() -> Hsla {
        gpui::hsla(0.6, 0.2, 0.12, 1.0)
    }

    #[test]
    fn full_window_opaque_quad_becomes_the_clear_color() {
        let got = opaque_backdrop(&scene_of(vec![quad(dark())]), W, H).unwrap();
        let want = gpui::Rgba::from(dark());
        assert_eq!(got, [want.r, want.g, want.b, 1.0]);
    }

    #[test]
    fn translucent_rounded_bordered_or_partial_quads_stay_quads() {
        let mut translucent = quad(dark());
        translucent.background = gpui::hsla(0.6, 0.2, 0.12, 0.5).into();
        let mut rounded = quad(dark());
        rounded.corner_radii = Corners::all(ScaledPixels(4.0));
        let mut bordered = quad(dark());
        bordered.border_widths = Edges::all(ScaledPixels(1.0));
        let mut partial = quad(dark());
        partial.bounds = rect(0.0, 0.0, W as f32 - 1.0, H as f32);
        let mut clipped = quad(dark());
        clipped.content_mask.bounds = rect(10.0, 10.0, 100.0, 100.0);
        let mut moved = quad(dark());
        moved.transformation =
            TransformationMatrix::unit().translate(point(ScaledPixels(1.0), ScaledPixels(0.0)));
        for q in [translucent, rounded, bordered, partial, clipped, moved] {
            assert_eq!(opaque_backdrop(&scene_of(vec![q]), W, H), None);
        }
    }

    #[test]
    fn anything_drawn_before_the_quad_keeps_it_a_quad() {
        let mut scene = Scene::default();
        scene.insert_primitive(Shadow {
            order: 0,
            blur_radius: ScaledPixels(4.0),
            bounds: rect(10.0, 10.0, 50.0, 50.0),
            corner_radii: Corners::default(),
            content_mask: ContentMask {
                bounds: rect(0.0, 0.0, W as f32, H as f32),
            },
            color: dark(),
            element_bounds: rect(10.0, 10.0, 50.0, 50.0),
            element_corner_radii: Corners::default(),
            inset: 0,
            pad: 0,
        });
        let mut q = quad(dark());
        q.order = 1;
        scene.insert_primitive(q);
        scene.finish();
        assert_eq!(opaque_backdrop(&scene, W, H), None);
    }
}
