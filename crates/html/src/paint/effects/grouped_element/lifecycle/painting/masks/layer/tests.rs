//! Typed clipping geometry retains the source layer's composition settings.
use super::*;
use gpui::IntoElement;

#[test]
fn typed_scene_and_path_definition_have_identical_layer_contracts() {
    let mut group = Grouped::new(gpui::div().into_any_element());
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
