//! URL decoding takes precedence over a dimensionless image() color fallback.

use super::{Source, key, source};
use crate::style::{computed::Computed, values::value::Color};

#[test]
fn decode_url_before_using_fallback() {
    let image = source("image('data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%222%22 height=%223%22/%3E', red)").unwrap();
    assert!(matches!(image, Source::Vector { .. }));
    let size = image.intrinsic();
    assert_eq!((size.w, size.h), (Some(2.0), Some(3.0)));
    for url in [
        "data:text/plain,not-an-image",
        "missing-image-fallback-unit.png",
    ] {
        let image = source(&format!("image('{url}', green)")).unwrap();
        assert!(matches!(image, Source::Gradient { .. }));
        let size = image.intrinsic();
        assert_eq!((size.w, size.h, size.ratio), (None, None, None));
        assert!(source(&format!("image('{url}')")).is_none());
    }
}

#[test]
fn fallback_current_color_resolves_after_inheritance_with_alpha() {
    let c = Computed {
        color: Color::parse("rgba(0, 128, 0, 0.5)"),
        ..Computed::default()
    };
    let src = key("image('data:text/plain,invalid', currentColor)", &c);
    let Some(Source::Gradient { raw }) = source(&src) else {
        panic!("fallback missing")
    };
    assert_eq!(
        crate::style::computed::parse_image_color(&raw).and_then(Color::parse),
        c.color
    );
}
