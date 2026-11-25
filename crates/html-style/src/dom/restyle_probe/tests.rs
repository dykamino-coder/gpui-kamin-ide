//! The kept `:has()` marks of a build.

use super::*;
use crate::style::select::has::mark_has;

/// The marks a build leaves to its cache are those a first probe would
/// make; a page without `:has()` leaves none.
#[test]
fn a_build_leaves_its_has_marks_to_the_cache() {
    let page = "<div class=a><p class=hot></p></div><div class=a></div>\
        <style>.a:has(.hot){color:red}</style>";
    for (html, marked) in [(page, true), ("<p class=hot></p>", false)] {
        let input = Admitted::parse(html, false).unwrap();
        let values = DocumentValues::detached(Media::default());
        let mut cache = RestyleCache::default();
        super::super::convert_restyling(
            &input,
            "",
            Media::default(),
            &values,
            None,
            Some(&mut cache),
        )
        .unwrap();
        assert_eq!(cache.has.is_some(), marked, "{html}");
        let (Some(has), Some(probe)) = (cache.has.take(), cache.probe.clone()) else {
            continue;
        };
        let mut kept = TreeIndex::new(values.clone(), false);
        kept.put_has(has);
        let mut fresh = TreeIndex::new(values.clone(), false);
        mark_has(
            &input.dom.document,
            &probe.has_args,
            &mut vec![],
            &mut fresh,
        )
        .unwrap();
        assert!(!fresh.has_marks.is_empty());
        assert!(kept.has_marks == fresh.has_marks);
    }
}
