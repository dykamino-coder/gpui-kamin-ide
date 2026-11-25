//! A restyle in place compares values, not their spelling: a cascaded
//! `font-size-adjust: from-font` equals itself under every restyled element,
//! and a descendant's layer with an explicit `inherit` of a reset property
//! blocks the write only when it reads the value the restyle changes; an
//! element without a box is a mutated element of the restyle like any other.

use super::update;

/// A page whose `[data-deep]` boxes take a background under `html.alt`.
fn page(alt: &str, extra: &str) -> String {
    format!(
        "<!doctype html><html{alt}><head><style>body{{margin:0;color:#000;background:#fff}}\
         {extra}.alt [data-deep]{{background:#eee}}</style></head><body>\
         <div data-deep>a</div><p data-deep><a href=\"#\">b</a></p></body></html>"
    )
}

#[test]
fn a_from_font_size_adjust_restyles_in_place() {
    // piccalilli C: `font-size-adjust: from-font` on `body` reaches every
    // restyled element; equal values keep the batch a paint damage.
    let extra = "body{font-size-adjust:from-font}";
    assert!(update(&page("", extra), &page(" class=\"alt\"", extra), ""));
}

#[test]
fn a_hover_layer_inheriting_another_reset_value_restyles_in_place() {
    // aaronfrancis D: `a{text-decoration:inherit}` with a `:hover` layer under
    // a box whose background changes: the layer reads no changed value.
    let extra = "a{text-decoration:inherit}a:hover{color:red}";
    assert!(update(&page("", extra), &page(" class=\"alt\"", extra), ""));
}

#[test]
fn a_hover_layer_inheriting_the_changed_background_is_built() {
    // The layer's `background-color: inherit` reads the new background of its
    // parent: the frame is built (and equal to a fresh build).
    let extra = "a:hover{background-color:inherit}";
    assert!(!update(&page("", extra), &page(" class=\"alt\"", extra), ""));
}

#[test]
fn an_element_without_a_box_restyles_in_place() {
    // encyclopedia-monobook D: the edited element lies under `display: none`;
    // the build never entered it, and its new background reaches no box.
    let page = |attr: &str| {
        format!(
            "<!doctype html><html><head><style>body{{margin:0}}.x{{display:none}}\
             [data-edit]{{background:#eee}}</style></head><body><p>a</p>\
             <div class=x><span{attr}>b</span></div></body></html>"
        )
    };
    assert!(update(&page(""), &page(" data-edit=\"1\""), ""));
}