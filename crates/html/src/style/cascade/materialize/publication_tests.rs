//! Контракт публикации: смена входов заменяет handle, used-правки не меняют
//! старую computed-стадию и наблюдаемые операнды геометрии коробки.

use super::tests::element;
use super::*;
use crate::style::values::value::Len;

#[test]
fn cache_hits_reuse_publication_after_used_sizing() {
    let parsed = crate::dom::parse(
        r#"<body style="font-size:20px"><p style="width:50%;height:2em;margin-left:3px;padding-left:4px">x</p></body>"#,
        "",
    );
    let vcx = parsed.values();
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, vcx);
    let p = element(&parsed.nodes, "p");
    let (_, parent) = p.style.slot().with_parent().unwrap();
    let held = p.style.published().unwrap();
    let mut copy = p.clone();
    copy.style.used_mut().width = Some(Len::Px(70.0));
    copy.style.used_mut().height = Some(Len::Px(80.0));
    materialize(&parsed.nodes, &root, vcx);
    let hit = p
        .style
        .slot()
        .get(p.style.cascaded(), &parent, inputs(vcx))
        .unwrap();
    assert!(held.ptr_eq(&hit.handle));
    let mut boxed = box_style(&BoxStyle::shared(parent), &copy.style, vcx);
    assert_eq!(boxed.width, Some(Len::Px(70.0)));
    assert_eq!(boxed.height, Some(Len::Px(80.0)));
    assert_eq!(boxed.margin.left, Some(Len::Px(3.0)));
    assert_eq!(boxed.padding.left, Some(Len::Px(4.0)));
    boxed.width = Some(Len::Px(90.0));
    assert_eq!(held.width, Some(Len::Pct(0.5)));
    assert_eq!(held.height, Some(Len::Px(40.0)));
    assert!(std::ptr::eq::<Computed>(
        &*held,
        &*boxed.element_style.as_ref().unwrap().computed,
    ));
}

#[test]
fn restyle_replaces_publication_and_preserves_retained_reader() {
    let parsed = crate::dom::parse(
        r#"<body style="font-size:20px"><p style="width:2em">x</p></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let (_, parent) = p.style.slot().with_parent().unwrap();
    let held = p.style.published().unwrap();
    let mut copy = p.clone();
    copy.style.used_mut().width = Some(Len::Px(99.0));
    copy.style.cascaded_mut().font_size = Some(Len::Px(30.0));
    assert!(copy.style.slot().with_parent().is_none());
    assert!(copy.style.used_rc().is_none());
    let nodes = [Node::Element(copy)];
    materialize(&nodes, &parent, vcx);
    let updated = element(&nodes, "p").style.published().unwrap();
    assert!(!held.ptr_eq(&updated));
    assert_eq!(held.width, Some(Len::Px(40.0)));
    assert_eq!(updated.width, Some(Len::Px(60.0)));
    assert!(p.style.published().unwrap().ptr_eq(&held));
}

#[test]
fn inherited_explicit_and_environment_inputs_control_cache_misses() {
    let parsed = crate::dom::parse(
        r#"<body style="font-size:20px;padding-left:3px"><p>x</p><span style="padding-left:inherit">x</span></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let p = element(&parsed.nodes, "p");
    let span = element(&parsed.nodes, "span");
    let (_, parent) = p.style.slot().with_parent().unwrap();
    let held = p.style.published().unwrap();
    let mut other = (*parent).clone();
    other.padding.left = Some(Len::Px(9.0));
    let generation = inputs(vcx);
    let get = |style: &SharedStyle, parent: &Computed, generation| {
        style.slot().get(style.cascaded(), parent, generation)
    };
    assert!(
        get(&p.style, &other, generation)
            .unwrap()
            .handle
            .ptr_eq(&held)
    );
    assert!(get(&span.style, &other, generation).is_none());
    other.font_size = Some(Len::Px(30.0));
    assert!(get(&p.style, &other, generation).is_none());
    assert!(get(&p.style, &parent, (generation.0 + 1, generation.1)).is_none());
    let mut viewport = generation.1;
    viewport[0] = (vcx.viewport.0 + 1.0).to_bits();
    assert!(get(&p.style, &parent, (generation.0, viewport)).is_none());
    assert_eq!(held.font_size, Some(Len::Px(20.0)));
    assert_eq!(
        span.style.slot().with_parent().unwrap().0.padding.left,
        Some(Len::Px(3.0))
    );
}
