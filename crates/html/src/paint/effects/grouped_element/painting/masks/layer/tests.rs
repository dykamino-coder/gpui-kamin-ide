//! Тесты: типизированная геометрия обрезки сохраняет настройки композиции исходного слоя.
use super::*;
use gpui::IntoElement;

#[test]
fn typed_scene_and_path_definition_have_identical_layer_contracts() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut group = Grouped::new(gpui::div().into_any_element(), vcx);
    group.mask_repeat_modes = vec![(1, 2)];
    group.mask_composite = vec![1, 3];
    group.mask_luminance = true;
    let scene = coverage(
        30.0,
        25.0,
        Geometry::parse("M1.3 2.7H19.4V17.6H1.3Z"),
        FillRule::EvenOdd,
    );
    let source = "pathdef:evenodd:M1.3 2.7H19.4V17.6H1.3Z";
    for sf in [1.0, 1.25, 2.0] {
        let build = |input| {
            mask_layer(
                1,
                input,
                &group,
                27.5,
                21.2,
                sf,
                &|_, _, _| (3.5, 4.5),
                &|_| (true, false),
                vcx,
            )
            .unwrap()
        };
        let direct = build(MaskInput::Scene(&scene));
        let parsed = build(MaskInput::Source(source));
        assert_eq!(direct.image.as_bytes(0), parsed.image.as_bytes(0));
        assert_eq!(direct.tile, parsed.tile);
        assert_eq!(direct.no_repeat, parsed.no_repeat);
        assert_eq!(direct.gap, parsed.gap);
        assert_eq!(direct.snap, parsed.snap);
        assert_eq!(direct.op, parsed.op);
        assert_eq!(direct.luminance, parsed.luminance);
    }
}

#[test]
fn none_image_retains_a_transparent_composition_layer() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut group = Grouped::new(gpui::div().into_any_element(), vcx);
    group.mask_composite = vec![2, 3];
    let image = crate::style::computed::ImageSource::Invalid;
    let layer = mask_layer(
        1,
        MaskInput::Image(&image),
        &group,
        10.0,
        10.0,
        1.0,
        &|_, _, _| (0.0, 0.0),
        &|_| (false, false),
        vcx,
    )
    .unwrap();
    assert_eq!(layer.op, 3);
    assert!(layer.image.as_bytes(0).unwrap().iter().all(|&b| b == 0));
}
