//! Тесты подбора лица по css-fonts-4 §5.2: приоритет ширины над наклоном и весом, направление
//! поиска веса 400–500, запасные варианты italic/oblique и равенство возможностей при синтезе
//! веса.
use super::*;

fn face(name: &str, width: f32, weight: f32, slope: u8, angle: f32) -> Face {
    Face {
        width: (width, width),
        weight: (weight, weight),
        slope,
        angle: (angle, angle),
        family: name.into(),
        adjust: 1.0,
        features: Vec::new(),
    }
}

fn chosen(faces: Vec<Face>, request: Computed) -> String {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    vcx.doc
        .font_faces
        .faces
        .borrow_mut()
        .insert("selection-test".into(), faces);
    let selected = select("selection-test", &request, vcx).unwrap().family;
    vcx.doc
        .font_faces
        .faces
        .borrow_mut()
        .remove("selection-test");
    selected
}

#[test]
fn non_finite_request_has_a_total_order_and_equal_faces_keep_last() {
    for stretch in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let request = Computed {
            font_stretch: Some(stretch),
            ..Default::default()
        };
        assert_eq!(
            chosen(
                vec![
                    face("first", 100., 400., 0, 0.),
                    face("last", 100., 400., 0, 0.)
                ],
                request
            ),
            "last"
        );
    }
}

#[test]
fn width_precedes_style_and_weight() {
    let request = Computed {
        font_stretch: Some(90.0),
        italic: Some(true),
        font_weight: Some(700),
        ..Default::default()
    };
    assert_eq!(
        chosen(
            vec![
                face("narrow", 80.0, 400.0, 0, 0.0),
                face("wide-italic", 100.0, 700.0, 1, 0.0)
            ],
            request
        ),
        "narrow"
    );
}

#[test]
fn middle_weights_search_up_to_500_before_searching_down() {
    let request = Computed {
        font_weight: Some(450),
        ..Default::default()
    };
    assert_eq!(
        chosen(
            vec![
                face("400", 100.0, 400.0, 0, 0.0),
                face("500", 100.0, 500.0, 0, 0.0),
                face("600", 100.0, 600.0, 0, 0.0)
            ],
            request
        ),
        "500"
    );
}

#[test]
fn italic_oblique_only_uses_upright_fallback() {
    let request = Computed {
        italic: Some(true),
        font_oblique_only: Some(true),
        ..Default::default()
    };
    assert_eq!(
        chosen(
            vec![
                face("upright", 100.0, 400.0, 0, 0.0),
                face("oblique", 100.0, 400.0, 2, 14.0)
            ],
            request
        ),
        "upright"
    );
}

#[test]
fn oblique_search_direction_changes_at_eleven_degrees() {
    for (angle, expected) in [(10.0, "lower"), (12.0, "upper")] {
        let request = Computed {
            italic: Some(true),
            oblique: Some(true),
            font_oblique_angle: Some(angle),
            ..Default::default()
        };
        assert_eq!(
            chosen(
                vec![
                    face("lower", 100.0, 400.0, 2, 5.0),
                    face("upper", 100.0, 400.0, 2, 20.0)
                ],
                request
            ),
            expected
        );
    }
}

#[test]
fn oblique_prefers_upright_to_italic_even_without_synthesis() {
    let request = Computed {
        italic: Some(true),
        oblique: Some(true),
        font_synth: (Some(false), Some(false), Some(false)),
        ..Default::default()
    };
    assert_eq!(
        chosen(
            vec![
                face("upright", 100.0, 400.0, 0, 0.0),
                face("italic", 100.0, 400.0, 1, 0.0)
            ],
            request
        ),
        "upright"
    );
}

#[test]
fn regular_face_keeps_features_equal_across_synthesized_weight_runs() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    vcx.doc.font_faces.faces.borrow_mut().insert(
        "selection-test".into(),
        vec![face("regular", 100.0, 400.0, 0, 0.0)],
    );
    let regular = Computed::default();
    let bold = Computed {
        font_weight: Some(700),
        ..Default::default()
    };
    assert_eq!(
        features("selection-test", &regular, vcx),
        features("selection-test", &bold, vcx)
    );
    vcx.doc
        .font_faces
        .faces
        .borrow_mut()
        .remove("selection-test");
}
