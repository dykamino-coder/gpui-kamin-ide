//! Тесты: transform сохраняет входы размещения float (CSS Transforms 1, раздел 3).

use super::band_float_m;
use crate::dom::Element;
use crate::layout::float::band_flow::Edge;
use crate::layout::float::band_host::band_margins;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[test]
fn transformed_float_keeps_untransformed_margins() {
    let mut style = Computed {
        float: Some(-1),
        width: Some(Len::Px(40.0)),
        height: Some(Len::Px(20.0)),
        ..Default::default()
    };
    style.margin.left = Some(Len::Em(2.0));
    style.margin.right = Some(Len::Pct(0.1));
    let plain = Element::synthetic("div", style.clone(), Default::default(), vec![]);
    style.transform = Some(Default::default());
    style.transform.as_mut().expect("transform").scale = (2.0, 3.0);
    let turned = Element::synthetic("div", style, Default::default(), vec![]);
    assert_eq!(band_float_m(&plain, 16.0), Some(()));
    assert_eq!(band_float_m(&turned, 16.0), Some(()));
    let margins = band_margins(&turned.style.used(), 16.0).expect("resolvable margins");
    assert!(matches!(margins[3], Edge::Px(v) if v == 32.0));
    assert!(matches!(margins[1], Edge::Pct(v) if v == 0.1));
    assert_eq!(turned.style.used().width, plain.style.used().width);
    assert_eq!(turned.style.used().height, plain.style.used().height);
}

#[test]
fn float_host_retains_a_preceding_invisible_inline_run() {
    let metric_doc = crate::document::state::DocState::new(Default::default());
    let vcx = metric_doc.values();
    let mut span = Element::synthetic("span", Computed::default(), Default::default(), vec![]);
    span.inline = true;
    span.children = vec![crate::dom::Node::Text("\n  ".into())].into();
    let tail = span.clone();
    let float = Element::synthetic(
        "div",
        Computed {
            float: Some(-1),
            width: Some(Len::Px(100.)),
            height: Some(Len::Px(100.)),
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    let lead = [crate::dom::Node::Element(span)];
    let nodes = [
        crate::dom::Node::Element(float),
        crate::dom::Node::Element(tail),
    ];
    let (host, next, _) = super::band_host_m(
        &nodes,
        0,
        16.,
        &lead,
        crate::layout::float::FloatCb::of(&Computed::default()),
        vcx,
    )
    .expect("the paragraph owns invisible inline boxes");
    assert_eq!(next, nodes.len());
    assert!(host.children.iter().any(|n| matches!(n,
        crate::dom::Node::Element(c) if c.marks.anon && c.marks.lead.is_none())));
}

#[test]
fn orthogonal_text_float_uses_the_shared_measured_host() {
    let doc = crate::document::state::DocState::new(Default::default());
    use crate::dom::Node;
    use crate::style::computed::writing_mode::WritingModeKw;
    for rl in [false, true] {
        let parent = {
            let mut with_inherited = Computed {
                ..Default::default()
            };
            with_inherited.writing_mode = Some(WritingModeKw::new(true, rl, false));
            with_inherited
        };
        let child = Element::synthetic(
            "div",
            {
                let mut with_inherited = Computed {
                    float: Some(-1),
                    ..Default::default()
                };
                with_inherited.writing_mode = Some(WritingModeKw::new(false, false, false));
                with_inherited
            },
            Default::default(),
            vec![Node::Text("a horizontal floated line".into())],
        );
        let (host, next, _) = super::band_host_m(
            &[Node::Element(child)],
            0,
            16.0,
            &[],
            crate::layout::float::FloatCb::of(&parent),
            doc.values(),
        )
        .expect("orthogonal text keeps the measured float path");
        assert_eq!(next, 1);
        assert!(host.marks.shape_flow.is_some());
    }
}
