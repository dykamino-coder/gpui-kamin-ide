//! Document identity, snapshot retention and deferred mask geometry.

use super::*;

#[test]
fn matching_ids_and_snapshot_keys_stay_document_local() {
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
    let first_markup = mask_def("same", first.doc.values()).unwrap();
    let second_markup = mask_def("same", second.doc.values()).unwrap();
    assert!(first_markup.contains("<rect"));
    assert!(second_markup.contains("<circle"));
    let first_key = snapshot_mask_def("same", first.doc.values()).unwrap();
    let second_key = snapshot_mask_def("same", second.doc.values()).unwrap();
    // Identical local keys cannot select another document's definition.
    assert_eq!(first_key, second_key);
    let retained = first.doc.values().keep();
    drop(first);
    assert_eq!(
        mask_snapshot(&first_key, retained.vcx()),
        Some(first_markup)
    );
    assert_eq!(
        mask_snapshot(&second_key, second.doc.values()),
        Some(second_markup)
    );
    assert!(mask_def("missing", retained.vcx()).is_none());
}

#[test]
fn alpha_snapshots_and_reference_forms_share_the_document_owner() {
    let parsed = crate::dom::parse(
        r#"<svg><mask id="a" style="mask-type:alpha"><rect width="10" height="10"/></mask><clipPath id="c"><circle r="2"/></clipPath></svg>"#,
        "",
    );
    let vcx = parsed.doc.values();
    collect_mask_defs(&parsed.nodes, vcx);
    let alpha = resolve_mask_refs("url('#a')", vcx);
    assert!(alpha.starts_with("url(svgsnap:k"));
    assert!(alpha.ends_with("A)"));
    let clip = resolve_mask_refs("clipref:c", vcx);
    let key = clip.strip_prefix("clipsnap:").unwrap();
    assert!(mask_snapshot(key, vcx).unwrap().contains("<circle"));
    assert_eq!(resolve_mask_refs("url(#absent)", vcx), "url(#absent)");
    let before = parsed.doc.masks.next.get();
    collect_mask_defs(&parsed.nodes, vcx);
    assert_eq!(parsed.doc.masks.next.get(), before);
}

#[test]
fn retained_viewport_survives_nested_layout_and_later_builds() {
    let opts = crate::render::RenderOpts {
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
