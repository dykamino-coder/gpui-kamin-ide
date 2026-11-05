//! Проверки плана ограничения строк и смещения фрагмента.

use super::*;
use crate::text::clamp::ClampEntry;
use std::collections::HashMap;

#[test]
fn atomic_inline_paragraph_retains_geometric_cut_after_budget_application() {
    let geometry = ClampGeometry {
        limit: Some(4),
        max_h: None,
        trim_end: 0.,
    };
    // Word-row с атомарным inline сохраняет пять строк, получив бюджет 4.
    let mut paragraph = ClampEntry {
        bounds: Bounds::new(point(px(0.), px(0.)), size(px(100.), px(160.))),
        line: 32.,
        skip_count: false,
        fixed_height: false,
        bp_after: 0.,
        end_margin: 0.,
        min_end: 0.,
        capped: false,
        node: 0,
        seq: Some(0),
        clamped: None,
        empty: false,
        span: None,
        bt_before: 0.,
        positioned: false,
        float: false,
    };
    let trial = geometry.compute(vec![paragraph.clone()], &HashMap::new(), 0.);
    assert_eq!(trial.para, Some((0, 4)));
    assert_eq!(trial.cut, f32::INFINITY);
    paragraph.clamped = Some(4);
    let applied = geometry.compute(vec![paragraph], &HashMap::new(), 0.);
    assert_eq!(applied.para, trial.para);
    assert_eq!(applied.cut, 128.);
    assert!(refines(&trial, &applied));
    assert!(!refines(&applied, &applied));
    assert!(!refines(&applied, &trial));
}

fn line_probe(measure: &ClampMeasure, seq: u32) -> AnyElement {
    measure.record(move |bounds| ClampEntry::paragraph(bounds, 20., seq))
}

/// A fragment copy is laid out inside its fragmentainer's element offset:
/// the clamp point of every copy stays where the container's own lines put
/// it, so no fragment restarts or shifts the line count.
#[gpui::test]
fn element_offset_of_a_fragment_copy_does_not_move_the_clamp_point(cx: &mut gpui::TestAppContext) {
    use gpui::{IntoElement, ParentElement, Styled, div};
    let build: Build = Rc::new(|input, _, _, _| {
        let measure = match &input {
            ClampInput::Measure(m) | ClampInput::ApplyMeasured(_, m) => Some(m.clone()),
            ClampInput::Apply(_) => None,
        };
        let block = |h: f32, seq| {
            div()
                .relative()
                .h(px(h))
                .children(measure.as_ref().map(|m| line_probe(m, seq)))
        };
        div()
            .w(px(100.))
            .child(block(60., 0))
            .child(block(20., 1))
            .into_any_element()
    });
    let geometry = ClampGeometry {
        limit: Some(2),
        max_h: None,
        trim_end: 0.,
    };
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(200.), px(200.)),
        |window, app| {
            let mut plan_at = |window: &mut Window| {
                reblessive::Stack::new()
                    .enter(|stack| {
                        measure_async(
                            1,
                            &geometry,
                            &build,
                            MeasuredBox::default(),
                            false,
                            size(Some(px(100.)), None),
                            size(
                                AvailableSpace::Definite(px(100.)),
                                AvailableSpace::MaxContent,
                            ),
                            None,
                            window,
                            app,
                            stack,
                        )
                    })
                    .finish()
                    .1
            };
            let unshifted = plan_at(window);
            let shifted =
                window.with_absolute_element_offset(point(px(30.), px(-100.)), &mut plan_at);
            assert_eq!(unshifted.para, Some((0, 2)));
            assert_eq!(unshifted.cut, 40.);
            assert_eq!(shifted, unshifted);
            div()
        },
    );
}
