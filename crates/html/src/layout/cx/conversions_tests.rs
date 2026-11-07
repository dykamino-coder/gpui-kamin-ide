//! Память переводов стиля: ключ по значениям коробки, кандидаты, пауза на промахах.

use super::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn state() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

#[cfg(test)]
fn sized(width: f32) -> (BoxStyle, BoxDimensions) {
    let mut style = Computed::default();
    style.reset_box_layout.width = Some(Len::Px(width));
    let input = BoxStyle::new(style);
    let dimensions = BoxDimensions::from_computed(&input);
    (input, dimensions)
}

#[cfg(test)]
fn refinement(width: f32) -> Rc<StyleRefinement> {
    let mut style = StyleRefinement::default();
    style.size.width = Some(gpui::px(width).into());
    Rc::new(style)
}

#[test]
fn an_equal_box_reuses_the_conversion_whatever_its_registry_keys() {
    let doc = state();
    let vcx = doc.frame_values().with_viewport((800.0, 600.0));
    let memo = StyleConversions::default();
    let (input, dimensions) = sized(10.0);
    let converted = refinement(10.0);
    memo.record(&input, dimensions, vcx, converted.clone());
    let mut twin = input;
    (twin.self_node, twin.anchor_seq, twin.anchor_key) = (41, 5, 99);
    let found = memo.find(&twin, &dimensions, vcx).unwrap();
    assert!(Rc::ptr_eq(&found, &converted));
}

#[test]
fn another_value_size_or_viewport_converts_anew() {
    let doc = state();
    let vcx = doc.frame_values().with_viewport((800.0, 600.0));
    let memo = StyleConversions::default();
    let (input, dimensions) = sized(10.0);
    memo.record(&input, dimensions, vcx, refinement(10.0));
    let (other, other_dimensions) = sized(11.0);
    assert!(memo.find(&other, &other_dimensions, vcx).is_none());
    assert!(memo.find(&input, &other_dimensions, vcx).is_none());
    let wider = doc.frame_values().with_viewport((1024.0, 600.0));
    assert!(memo.find(&input, &dimensions, wider).is_none());
    let mut stretched = input;
    stretched.stretched = true;
    assert!(memo.find(&stretched, &dimensions, vcx).is_none());
}

#[test]
fn the_newest_distinct_inputs_stay_candidates() {
    let doc = state();
    let vcx = doc.frame_values();
    let memo = StyleConversions::default();
    for width in 0..=CANDIDATES {
        let (input, dimensions) = sized(width as f32);
        memo.record(&input, dimensions, vcx, refinement(width as f32));
    }
    let (oldest, oldest_dimensions) = sized(0.0);
    assert!(memo.find(&oldest, &oldest_dimensions, vcx).is_none());
    for width in 1..=CANDIDATES {
        let (input, dimensions) = sized(width as f32);
        let found = memo.find(&input, &dimensions, vcx).unwrap();
        assert_eq!(found.size.width, Some(gpui::px(width as f32).into()));
    }
}

#[test]
fn a_run_of_distinct_styles_pauses_lookups_until_a_probe_hits() {
    let doc = state();
    let vcx = doc.frame_values();
    let memo = StyleConversions::default();
    let miss = |memo: &StyleConversions, width: f32| {
        let (input, dimensions) = sized(width);
        let lookup = memo.lookup(&input, &dimensions, vcx);
        if matches!(lookup, Lookup::Miss) {
            memo.record(&input, dimensions, vcx, refinement(width));
        }
        lookup
    };
    for width in 0..-PAUSE_AT {
        assert!(matches!(miss(&memo, width as f32), Lookup::Miss));
    }
    let (repeated, dimensions) = sized(1000.0);
    memo.record(&repeated, dimensions, vcx, refinement(1000.0));
    let looks: Vec<_> = (0..PROBE_EVERY)
        .map(|_| memo.lookup(&repeated, &dimensions, vcx))
        .collect();
    assert!(
        looks[..looks.len() - 1]
            .iter()
            .all(|l| matches!(l, Lookup::Pass))
    );
    assert!(matches!(looks.last(), Some(Lookup::Hit(_))));
    assert!(matches!(
        memo.lookup(&repeated, &dimensions, vcx),
        Lookup::Hit(_)
    ));
}

#[test]
fn the_memory_works_only_inside_a_build_of_its_document_and_is_emptied_by_it() {
    let doc = state();
    let vcx = doc.frame_values();
    let memo = StyleConversions::default();
    let (input, dimensions) = sized(10.0);
    assert!(memo.active().is_none());
    {
        let _outer = memo.build();
        memo.active()
            .unwrap()
            .record(&input, dimensions, vcx, refinement(10.0));
        assert!(memo.find(&input, &dimensions, vcx).is_some());
        {
            // A nested build starts and ends empty.
            let _inner = memo.build();
            assert!(memo.find(&input, &dimensions, vcx).is_none());
        }
        assert!(memo.active().is_some());
    }
    assert!(memo.active().is_none());
    let _again = memo.build();
    assert!(memo.find(&input, &dimensions, vcx).is_none());
}
