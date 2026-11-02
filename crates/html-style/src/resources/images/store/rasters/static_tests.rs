//! Повторное использование статических сцен не меняет RGB, размеры или инвалидирование.

use super::*;

fn drawing(body: &str) -> String {
    format!("<svg xmlns='http://www.w3.org/2000/svg' width='12' height='8'>{body}</svg>")
}

#[test]
fn resource_names_share_rasters_and_preserve_fresh_pixels() {
    let store = ImageStore::default();
    for body in [
        "<defs><linearGradient id='resource'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='12' height='8' fill='url(#resource)'/>",
        "<defs><clipPath id='resource'><circle cx='5' cy='4' r='3'/></clipPath></defs><rect width='12' height='8' fill='red' clip-path='url(#resource)'/>",
        "<defs><mask id='resource'><rect width='6' height='8' fill='white'/></mask></defs><rect width='12' height='8' fill='red' mask='url(#resource)'/>",
        "<defs><filter id='resource'><feTurbulence baseFrequency='.05' numOctaves='3'/><feDisplacementMap in='SourceGraphic' scale='3'/></filter></defs><rect x='1' y='1' width='10' height='6' fill='#397' filter='url(#resource)'/>",
        "<defs><path id='resource' d='M1 1L11 1L6 7Z'/></defs><use href='#resource' fill='red'/>",
        "<defs><rect id='shape' width='6' height='8' fill='red'/><filter id='resource'><feImage href='#shape'/></filter></defs><rect width='12' height='8' filter='url(#resource)'/>",
        "<defs><pattern id='resource' width='3' height='3' patternUnits='userSpaceOnUse'><circle cx='1' cy='1' r='1' fill='blue'/></pattern></defs><rect width='12' height='8' fill='url(#resource)'/>",
    ] {
        let original = drawing(body);
        let renamed = original
            .replace("resource", "different")
            .replace("shape", "target");
        for density in [1.0, 2.0] {
            let req = RasterRequest::new(12.0, 8.0, density, 0.0);
            let a = store.rasterize_svg(&original, req).unwrap();
            let b = store.rasterize_svg(&renamed, req).unwrap();
            assert!(Arc::ptr_eq(&a, &b), "{body}");
            let fresh = ImageStore::default().rasterize_svg(&renamed, req).unwrap();
            assert_eq!(a.bytes, fresh.bytes, "{body}");
            assert_eq!(
                (a.width, a.height, a.stride),
                (fresh.width, fresh.height, fresh.stride)
            );
        }
    }
}

#[test]
fn changed_geometry_paint_and_filter_inputs_are_distinct() {
    let store = ImageStore::default();
    let req = RasterRequest::new(12.0, 8.0, 2.0, 0.0);
    let a = drawing(
        "<defs><filter id='f'><feGaussianBlur stdDeviation='1'/></filter></defs><rect width='12' height='8' fill='red' filter='url(#f)'/>",
    );
    let first = store.rasterize_svg(&a, req).unwrap();
    for changed in [
        a.replace("width='12' height='8' fill", "width='6' height='8' fill"),
        a.replace("fill='red'", "fill='blue'"),
        a.replace("stdDeviation='1'", "stdDeviation='2'"),
    ] {
        let cached = store.rasterize_svg(&changed, req).unwrap();
        let fresh = ImageStore::default().rasterize_svg(&changed, req).unwrap();
        assert!(!Arc::ptr_eq(&first, &cached));
        assert_ne!(first.bytes, cached.bytes);
        assert_eq!(cached.bytes, fresh.bytes);
    }
    assert!(Arc::ptr_eq(&first, &store.rasterize_svg(&a, req).unwrap()));
}

#[test]
fn padding_and_document_generation_remain_part_of_raster_identity() {
    let mut store = ImageStore::default();
    let markup = drawing("<rect id='drawing' width='20' height='20' fill='red'/>");
    let req = RasterRequest::new(12.0, 8.0, 2.0, 0.0);
    let first = store.rasterize_svg(&markup, req).unwrap();
    let padded_req = RasterRequest::new(12.0, 8.0, 2.0, 1.0);
    let padded = store.rasterize_svg(&markup, padded_req).unwrap();
    assert_eq!((first.width, first.height), (24, 16));
    assert_eq!((padded.width, padded.height), (26, 18));
    assert!(!Arc::ptr_eq(&first, &padded));
    let fresh = ImageStore::default()
        .rasterize_svg(&markup, padded_req)
        .unwrap();
    assert_eq!(padded.bytes, fresh.bytes);
    store.begin_generation();
    assert!(!Arc::ptr_eq(
        &first,
        &store.rasterize_svg(&markup, req).unwrap()
    ));
}

#[test]
fn stylesheets_and_missing_references_do_not_alias() {
    let store = ImageStore::default();
    let req = RasterRequest::new(12.0, 8.0, 1.0, 0.0);
    for a in [
        drawing(
            "<style>#x{fill:red}</style><rect id='x' width='12' height='8'/><rect width='1' height='1' fill='blue'/>",
        ),
        drawing(
            "<defs><linearGradient id='x'><stop stop-color='red'/></linearGradient></defs><rect width='12' height='8' fill='url(#missing)'/>",
        ),
    ] {
        let b = a.replace("id='x'", "id='missing'");
        let first = store.rasterize_svg(&a, req).unwrap();
        let changed = store.rasterize_svg(&b, req).unwrap();
        let fresh = ImageStore::default().rasterize_svg(&b, req).unwrap();
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_ne!(first.bytes, changed.bytes);
        assert_eq!(changed.bytes, fresh.bytes);
    }
}
