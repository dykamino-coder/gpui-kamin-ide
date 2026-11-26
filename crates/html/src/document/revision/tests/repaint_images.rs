//! `class` and `data-*` edits that swap a background image (`revision::repaint`):
//! a generated image (a conic gradient) is paint damage, restyled in place and
//! equal to a fresh build; an image with a URL loads as a resource of the build
//! and rebuilds.

use super::update;

/// Rows with `background: <image>`; the second row carries `attr`, which a
/// rule turns into a plain colour.
fn page(image: &str, attr: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         .row{{height:20px;background:{image}}}[data-edit]{{background:#eee}}\
         [data-edit=\"2\"]{{background:conic-gradient(red,blue)}}</style></head><body>\
         <div class=row>a</div><div class=row{attr}>b</div></body></html>"
    )
}

#[test]
fn a_conic_gradient_replaced_by_a_colour_is_restyled_in_place() {
    // effects-480 D: `background: #eee` replaces a layer's conic gradient.
    let image = "repeating-conic-gradient(from 13deg,#e457,#37e8 10deg,#fc98 30deg)";
    let (old, new) = (page(image, ""), page(image, " data-edit=\"1\""));
    assert!(update(&old, &new, ""), "conic -> colour");
    assert!(update(&new, &old, ""), "colour -> conic");
    // One generated image for another.
    let other = page(image, " data-edit=\"2\"");
    assert!(update(&old, &other, ""), "conic -> conic");
}

#[test]
fn an_image_with_a_url_replaced_by_a_colour_rebuilds_its_boxes() {
    let image = "url(data:image/gif;base64,R0lGODlhAQABAAAAACw=)";
    let (old, new) = (page(image, ""), page(image, " data-edit=\"1\""));
    assert!(update(&old, &new, ""), "url -> colour");
    assert!(update(&new, &old, ""), "colour -> url");
}
