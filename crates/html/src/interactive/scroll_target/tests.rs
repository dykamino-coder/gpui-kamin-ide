//! Тесты: нативные цели прокрутки сохраняют CSS-размеры и явную изоляцию владельцев.
use super::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{CssSizingKeyword, Overflow, div, px};

#[test]
fn native_scroll_target_preserves_css_box_and_isolates_nested_context() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let cx = crate::layout::cx::LayoutCx {
        doc: crate::layout::cx::BuildDoc(doc),
        ..Default::default()
    };
    let mut e = crate::layout::table::anon::anon_element("div", vec![]);
    e.node_id = 7;
    let mut c = crate::style::computed::BoxStyle::from(Computed {
        width: Some(Len::FitContent),
        ..Computed::default()
    });
    c.self_node = 7;
    let margin = Sides {
        left: Some(Len::Px(10.0)),
        ..Default::default()
    };
    let handle = ScrollHandle::default();
    let outer = Target::new(7, &handle, true, false, margin);
    {
        let cx = crate::layout::cx::LayoutCx {
            scroll_target: Some(outer.clone()),
            ..cx.clone()
        };
        let child = Target::new(8, &handle, false, true, Sides::default());
        {
            let cx = crate::layout::cx::LayoutCx {
                scroll_target: Some(child.clone()),
                ..cx.clone()
            };
            let mut d = attach(div(), &e, &c, &cx);
            assert_eq!(d.style().overflow.x, None);
        }
        assert!(!child.applied());
        let mut d = attach(div(), &e, &c, &cx);
        assert_eq!(
            gpui::Element::id(&d),
            Some(e.state_id("html-scroll-content", cx.vcx()))
        );
        assert_eq!(d.style().overflow.x, Some(Overflow::Hidden));
        assert_eq!(
            d.style().sizing_keywords,
            Some([Some(CssSizingKeyword::FitContent), None])
        );
        assert_eq!(d.style().margin.left, Some(px(10.0).into()));
        let mut second = attach(div(), &e, &c, &cx);
        assert_eq!(second.style().overflow.x, None);
    }
    assert!(outer.applied());
    let mut after = attach(div(), &e, &c, &cx);
    assert_eq!(after.style().overflow.x, None);
}

#[test]
fn colored_numeric_scrollport_tracks_the_css_box() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let cx = crate::layout::cx::LayoutCx {
        doc: crate::layout::cx::BuildDoc(doc),
        ..Default::default()
    };
    let mut e = crate::layout::table::anon::anon_element("div", vec![]);
    e.node_id = 7;
    let mut c = crate::style::computed::BoxStyle::from(Computed {
        width: Some(Len::Px(100.0)),
        ..Computed::default()
    });
    c.self_node = 7;
    c.apply_one("scrollbar-color", "red blue", cx.vcx());
    let handle = ScrollHandle::default();
    let outer = Target::new(7, &handle, false, true, Sides::default());
    {
        let cx = crate::layout::cx::LayoutCx {
            scroll_target: Some(outer.clone()),
            ..cx.clone()
        };
        let mut d = attach(div(), &e, &c, &cx);
        assert_eq!(d.style().overflow.y, Some(Overflow::Hidden));
        assert_eq!(d.style().sizing_keywords, Some([None, None]));
    }
    assert!(outer.applied());
    let mut d = attach(div(), &e, &c, &cx);
    assert_eq!(d.style().overflow.y, None);
}
