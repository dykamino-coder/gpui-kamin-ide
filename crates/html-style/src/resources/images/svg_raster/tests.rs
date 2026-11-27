//! Exact raster contracts for CSS root ownership and cropped viewports.
use super::*;

fn render(markup: &str, request: RasterRequest) -> Arc<CpuRaster> {
    rasterize(
        markup,
        request,
        Arc::new(usvg::fontdb::Database::new()),
        &|_| None,
        true,
    )
    .unwrap()
}

fn svg(attrs: &str, content: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8" {attrs}>{content}</svg>"#
    )
}

fn owned(opacity: f32) -> RasterRequest {
    RasterRequest {
        root_paint: RootPaint::CssBox {
            opacity: Some(opacity.into()),
        },
        ..RasterRequest::new(12.0, 8.0, 2.0, 0.0)
    }
}

#[test]
fn css_owner_preserves_explicit_root_inheritance_before_compositing() {
    for attrs in [
        r#"opacity=".5""#,
        r#"style="opacity:.5!important""#,
        r#"class="root""#,
    ] {
        for content in [
            r#"<rect width="12" height="8" fill="green" opacity="inherit"/>"#,
            r#"<g opacity="inherit"><rect width="12" height="8" fill="green" opacity="inherit"/></g>"#,
            r##"<defs><rect id="r" width="12" height="8" fill="green" opacity="inherit"/></defs><use href="#r" opacity="inherit"/>"##,
            r#"<style>.root{opacity:.25}.child{opacity:inherit}</style><rect class="child" width="12" height="8" fill="green"/>"#,
        ] {
            let source = svg(attrs, content);
            // The source context has .5, including for local use instances.
            // A fully opaque reference root composites those resolved children.
            let reference = svg("", &content.replace("inherit", ".5"));
            let actual = render(&source, owned(0.5));
            let expected = render(&reference, RasterRequest::new(12.0, 8.0, 2.0, 0.0));
            assert_eq!(actual.bytes, expected.bytes, "{source}");
        }
    }
}

#[test]
fn ownership_only_consumes_the_root_group_and_keys_the_complete_input() {
    let source = svg(
        r#"opacity=".5""#,
        r#"<svg width="12" height="8" opacity=".25"><rect width="12" height="8" fill="green"/></svg>"#,
    );
    let request = RasterRequest::new(12.0, 8.0, 2.0, 0.0);
    let native = render(&source, request);
    let external = render(&source, owned(0.5));
    assert_ne!(native.bytes, external.bytes);
    assert_eq!(
        external.bytes,
        render(&source.replace("opacity=\".5\"", ""), request).bytes
    );
    for other in [request, owned(0.25)] {
        assert_ne!(owned(0.5), other);
        assert_ne!(crate::cache::hint(owned(0.5)), crate::cache::hint(other));
    }
    // Presentation-only roots still own their SVG effect.
    let mut presentation = request;
    presentation.root_paint = RootPaint::CssBox { opacity: None };
    assert_eq!(render(&source, presentation).bytes, native.bytes);
}

#[test]
fn crop_keeps_the_source_root_and_matches_a_viewport_exactly() {
    let content = r#"<rect x="2.5" y="1.25" width="6" height="5" fill="green" opacity="inherit"/>"#;
    let reference_content = content.replace("inherit", ".5");
    let source = svg(r#"style="opacity:.5""#, content);
    for (x, y, w, h) in [(2.0, 1.0, 7.0, 6.0), (2.25, 1.5, 7.25, 5.25)] {
        let wrapper = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}">{}</svg>"#,
            svg("", &reference_content)
        );
        let crop = owned(0.5).cropped(x, y, w, h);
        assert_eq!(
            render(&source, crop).bytes,
            render(&wrapper, RasterRequest::new(w, h, 2.0, 0.0)).bytes
        );
    }
}

#[test]
fn nested_svg_image_keeps_its_own_compositing_context() {
    let source = svg(
        r#"opacity=".5""#,
        r#"<image width="12" height="8" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' opacity='.25'%3E%3Crect width='12' height='8' fill='green'/%3E%3C/svg%3E"/>"#,
    );
    let reference = svg(
        "",
        r#"<rect width="12" height="8" fill="green" opacity=".25"/>"#,
    );
    assert_eq!(
        render(&source, owned(0.5)).bytes,
        render(&reference, RasterRequest::new(12.0, 8.0, 2.0, 0.0)).bytes
    );
}

#[test]
fn requests_canonicalize_nan_density_and_signed_zero() {
    for [a, b] in [[0.0, -0.0], [f32::NAN, f32::from_bits(0xff80_0001)]] {
        let a = RasterRequest::new(10.0, 8.0, a, 0.0);
        let b = RasterRequest::new(10.0, 8.0, b, -0.0);
        assert_eq!(a, b);
        assert_eq!(crate::cache::hint(a), crate::cache::hint(b));
    }
}

#[test]
fn css_box_owns_root_background_without_changing_source_inheritance() {
    let content = r#"<rect width="6" height="8" fill="green" opacity="inherit"/>"#;
    let source = svg(r#"style="background-color:red;opacity:.5""#, content);
    let reference = svg(r#"opacity=".5""#, content);
    let request = RasterRequest::new(12.0, 8.0, 2.0, 0.0);
    assert_ne!(
        render(&source, request).bytes,
        render(&reference, request).bytes
    );
    assert_eq!(
        render(&source, owned(0.5)).bytes,
        render(&reference, owned(0.5)).bytes
    );
}
