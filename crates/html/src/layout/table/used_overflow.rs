//! CSS Tables 3 §3.6: table boxes use auto/scroll overflow as visible.

use crate::dom::Element;
use crate::style::computed::{Computed, Display, Overflow};

/// Used values leave the computed declarations available to CSS inheritance.
/// Blink style_adjuster.cc AdjustOverflow and Servo style_ext.rs both make
/// both axes visible if either axis would scroll a table.
pub(crate) fn resolve(e: &Element, c: &Computed) -> Option<Computed> {
    let table = match c.display {
        Some(Display::Table | Display::InlineTable) => true,
        None => e.tag == "table",
        _ => false,
    };
    if !table || ![c.overflow_x, c.overflow_y].contains(&Some(Overflow::Scroll)) {
        return None;
    }
    let mut used = c.clone();
    used.overflow_x = Some(Overflow::Visible);
    used.overflow_y = Some(Overflow::Visible);
    used.scroll.forced = [false; 2];
    used.scroll.used = [0.0; 4];
    used.scroll.native = [0.0; 4];
    Some(used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn html_css_and_inline_tables_ignore_auto_and_forced_scroll_on_either_axis() {
        let doc = crate::document::state::DocState::new(Default::default());
        for (tag, display) in [
            ("table", None),
            ("div", Some(Display::Table)),
            ("span", Some(Display::InlineTable)),
        ] {
            let e = anon_element(tag, vec![]);
            for property in ["overflow", "overflow-x", "overflow-y"] {
                for value in ["auto", "scroll"] {
                    let mut c = Computed {
                        display,
                        ..Default::default()
                    };
                    c.apply_one(property, value, doc.values());
                    let computed = crate::style::cascade::inherit::inherit(
                        &Computed::default(),
                        &c,
                        doc.values(),
                    );
                    let used = resolve(&e, &computed).expect("table overflow override");
                    assert_eq!(used.overflow_x, Some(Overflow::Visible));
                    assert_eq!(used.overflow_y, Some(Overflow::Visible));
                    assert_eq!(used.scroll.forced, [false; 2]);
                    assert!(
                        [computed.overflow_x, computed.overflow_y]
                            .contains(&Some(Overflow::Scroll))
                    );
                    let model = crate::interactive::scrollbar_gutter::Model::new(&used, [false; 2]);
                    assert_eq!(model.total(), [0.0; 2]);
                }
            }
        }
    }

    #[test]
    fn scrolling_axis_disables_both_axes_and_any_cached_gutters() {
        let e = anon_element("table", vec![]);
        let mut c = Computed {
            overflow_x: Some(Overflow::Hidden),
            overflow_y: Some(Overflow::Scroll),
            ..Default::default()
        };
        c.scroll.forced = [false, true];
        c.scroll.used = [0.0, 0.0, 15.0, 0.0];
        c.scroll.native = c.scroll.used;
        let used = resolve(&e, &c).expect("table override");
        assert_eq!(used.overflow_x, Some(Overflow::Visible));
        assert_eq!(used.overflow_y, Some(Overflow::Visible));
        assert_eq!(used.scroll.used, [0.0; 4]);
        assert_eq!(used.scroll.native, [0.0; 4]);
    }

    #[test]
    fn hidden_clip_visible_and_block_scrollers_retain_their_values() {
        let table = anon_element("table", vec![]);
        for overflow in [
            None,
            Some(Overflow::Hidden),
            Some(Overflow::Clip),
            Some(Overflow::Visible),
        ] {
            let c = Computed {
                overflow_x: overflow,
                overflow_y: overflow,
                ..Default::default()
            };
            assert!(resolve(&table, &c).is_none());
        }
        for (tag, display) in [("div", None), ("table", Some(Display::Block))] {
            let e = anon_element(tag, vec![]);
            let c = Computed {
                display,
                overflow_x: Some(Overflow::Scroll),
                ..Default::default()
            };
            assert!(resolve(&e, &c).is_none());
        }
    }
}
