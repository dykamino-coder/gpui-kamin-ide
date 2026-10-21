//! Тесты: идентичность документа, удержание снимка и отложенная геометрия маски.

use super::*;

#[test]
fn matching_ids_and_retained_definitions_stay_document_local() {
    let first = crate::dom::parse(
        r#"<svg><mask id="same"><rect width="10" height="10" fill="red"/></mask></svg>"#,
        "",
    );
    let second = crate::dom::parse(
        r#"<svg><mask id="same"><circle r="8" fill="blue"/></mask></svg>"#,
        "",
    );
    collect_mask_defs(&first.nodes, first.doc.values());
    collect_mask_defs(&second.nodes, second.doc.values());
    let first_defs = first.doc.masks.definitions();
    let second_defs = second.doc.masks.definitions();
    assert!(first_defs.get("same").unwrap().markup.contains("<rect"));
    assert!(second_defs.get("same").unwrap().markup.contains("<circle"));
    drop(first);
    drop(second);
    assert!(first_defs.get("same").unwrap().markup.contains("<rect"));
    assert!(second_defs.get("same").unwrap().markup.contains("<circle"));
    assert!(first_defs.get("missing").is_none());
}

#[test]
fn alpha_and_clip_definitions_share_the_document_owner() {
    let parsed = crate::dom::parse(
        r#"<svg><mask id="a" style="mask-type:alpha"><rect width="10" height="10"/></mask><clipPath id="c"><circle r="2"/></clipPath></svg>"#,
        "",
    );
    let vcx = parsed.doc.values();
    collect_mask_defs(&parsed.nodes, vcx);
    let retained = parsed.doc.masks.definitions();
    assert!(retained.get("a").unwrap().alpha);
    assert!(retained.get("c").unwrap().markup.contains("<circle"));
    assert!(retained.get("absent").is_none());
    collect_mask_defs(&parsed.nodes, vcx);
    assert!(std::sync::Arc::ptr_eq(
        &retained,
        &parsed.doc.masks.definitions()
    ));
}

#[test]
fn retained_viewport_survives_nested_layout_and_later_builds() {
    let opts = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let outer = opts.inner(|cx| cx.paint_viewport = opts.viewport);
    let retained = outer.cx.vcx().keep();
    let inner = outer.inner(|cx| cx.paint_viewport = (320.0, 240.0));
    assert_eq!(inner.cx.vcx().viewport, (320.0, 240.0));
    drop(inner);
    drop(outer);
    assert_eq!(retained.vcx().viewport, (800.0, 600.0));
    assert_eq!(opts.cx.vcx().viewport, (0.0, 0.0));
}
