//! The viewport is the screen medium (`Document::set_viewport`): a new size
//! that changes a media query result equals a fresh build at that size, and
//! one that changes none keeps the document.

use super::{state, tree};
use crate::document::Document;
use std::rc::Rc;

fn at(page: &str, size: (f32, f32)) -> Document {
    Document::builder(page, "").viewport(size).build().unwrap()
}

/// Resize `doc` to `size`: restyled or not, and equal to a fresh build there.
fn resize(doc: &mut Document, page: &str, size: (f32, f32)) -> bool {
    let values = doc.state.values.clone();
    let restyled = doc.set_viewport(size).unwrap();
    assert_eq!(restyled, !Rc::ptr_eq(&values, &doc.state.values));
    let fresh = at(page, size);
    assert_eq!(tree(doc), tree(&fresh), "{size:?}");
    assert_eq!(state(doc), state(&fresh), "{size:?}");
    restyled
}

const WIDTH: &str = "<!doctype html><style>p{color:#123}\
    @media (max-width: 500px){p{color:#456}}</style><p>a</p><div>b</div>";

#[test]
fn the_viewport_width_is_the_width_of_the_medium() {
    assert_ne!(
        tree(&at(WIDTH, (400., 300.))),
        tree(&at(WIDTH, (800., 300.)))
    );
    // The default medium is wider than 500px.
    let default = Document::builder(WIDTH, "").build().unwrap();
    assert_eq!(tree(&default), tree(&at(WIDTH, (800., 300.))));
}

#[test]
fn crossing_a_media_boundary_matches_a_fresh_build_at_the_new_size() {
    let mut doc = at(WIDTH, (800., 300.));
    assert!(resize(&mut doc, WIDTH, (400., 300.)));
    assert!(!resize(&mut doc, WIDTH, (450., 300.)));
    assert!(resize(&mut doc, WIDTH, (801., 300.)));
}

#[test]
fn a_resize_within_every_media_range_restyles_nothing() {
    let mut doc = at(WIDTH, (800., 300.));
    for width in [700., 600., 501., 1200.] {
        assert!(!resize(&mut doc, WIDTH, (width, 300.)), "{width}");
    }
    let plain = "<!doctype html><style>p{color:#123}@media print{p{color:#456}}\
        </style><p>a</p>";
    let mut doc = at(plain, (800., 300.));
    assert!(!resize(&mut doc, plain, (300., 900.)));
}

#[test]
fn height_aspect_ratio_and_orientation_follow_the_viewport() {
    let page = "<!doctype html><style>p{color:#123}\
        @media (orientation: portrait){p{color:#456}}\
        @media (min-height: 700px){div{color:#789}}\
        @media (min-aspect-ratio: 2/1){b{color:#abc}}</style><p>a</p><div>b</div><b>c</b>";
    let mut doc = at(page, (800., 600.));
    assert!(resize(&mut doc, page, (500., 600.)));
    assert!(resize(&mut doc, page, (500., 800.)));
    assert!(resize(&mut doc, page, (1700., 800.)));
    assert!(!resize(&mut doc, page, (1800., 800.)));
}

#[test]
fn custom_media_and_negated_queries_follow_the_viewport() {
    let page = "<!doctype html><style>@custom-media --narrow (width < 300px);\
        @media (--narrow){div{color:#789}}\
        @media not all and (min-width: 600px){p{color:#456}}</style><p>a</p><div>b</div>";
    let mut doc = at(page, (800., 300.));
    assert!(resize(&mut doc, page, (400., 300.)));
    assert!(resize(&mut doc, page, (200., 300.)));
    assert!(!resize(&mut doc, page, (250., 300.)));
}

#[test]
fn a_printed_document_keeps_its_page() {
    let mut doc = Document::builder(WIDTH, "")
        .print(true)
        .viewport((400., 300.))
        .build()
        .unwrap();
    let values = doc.state.values.clone();
    assert!(!doc.set_viewport((300., 300.)).unwrap());
    assert!(Rc::ptr_eq(&values, &doc.state.values));
}

const SHEET: &str = "<!doctype html><style>p{color:#123}</style>\
    <style id=s media='(max-width: 500px)'>p{color:#456}</style><p>a</p>";

fn with_media(media: Option<&str>) -> String {
    let media = media.map_or(String::new(), |m| format!(" media='{m}'"));
    format!(
        "<!doctype html><style>p{{color:#123}}</style>\
         <style id=s{media}>p{{color:#456}}</style><p>a</p>"
    )
}

#[test]
fn a_style_element_applies_only_while_its_media_list_matches() {
    let plain = "<!doctype html><style>p{color:#123}</style><p>a</p>";
    let always = with_media(None);
    assert_eq!(
        tree(&at(SHEET, (800., 300.))),
        tree(&at(plain, (800., 300.)))
    );
    assert_eq!(
        tree(&at(SHEET, (400., 300.))),
        tree(&at(&always, (400., 300.)))
    );
    assert_ne!(
        tree(&at(SHEET, (400., 300.))),
        tree(&at(SHEET, (800., 300.)))
    );
    // An empty list is `all` (mediaqueries-4 §2.1); `print` never matches a screen.
    let empty = with_media(Some(""));
    assert_eq!(
        tree(&at(&empty, (800., 300.))),
        tree(&at(&always, (800., 300.)))
    );
    let print = with_media(Some("print"));
    assert_eq!(
        tree(&at(&print, (800., 300.))),
        tree(&at(plain, (800., 300.)))
    );
}

#[test]
fn a_resize_that_flips_a_style_media_list_restyles() {
    let mut doc = at(SHEET, (800., 300.));
    assert!(resize(&mut doc, SHEET, (400., 300.)));
    assert!(!resize(&mut doc, SHEET, (450., 300.)));
    assert!(resize(&mut doc, SHEET, (900., 300.)));
}

#[test]
fn a_new_media_list_of_a_style_element_restyles() {
    use markup5ever_rcdom::{Handle, NodeData};
    fn find(node: &Handle) -> Option<Handle> {
        let hit = matches!(&node.data, NodeData::Element { attrs, .. }
            if attrs.borrow().iter().any(|a| &*a.name.local == "id" && &*a.value == "s"));
        if hit {
            return Some(node.clone());
        }
        node.children.borrow().iter().find_map(find)
    }
    let mut doc = at(SHEET, (800., 300.));
    let style = find(&doc.source_dom()).expect("style#s");
    let media = Some("(min-width: 600px)");
    let record = super::super::Mutation::Attribute {
        element: style,
        name: html5ever::QualName::new(None, html5ever::ns!(), "media".into()),
        value: media.map(Into::into),
    };
    doc.apply(&[record], "").unwrap();
    let fresh = at(&with_media(media), (800., 300.));
    assert_eq!(tree(&doc), tree(&fresh));
    assert_ne!(tree(&doc), tree(&at(SHEET, (800., 300.))));
}
