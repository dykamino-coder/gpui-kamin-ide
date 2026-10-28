//! Числовой план и естественный размер применения до освобождения пробного дерева.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn measure(
    key: u64,
    geometry: &ClampGeometry,
    build: &Build,
    used_box: MeasuredBox,
    border_box: bool,
    known: Size<Option<Pixels>>,
    available: Size<AvailableSpace>,
    fragment: Option<&ClampFlow>,
    window: &mut Window,
    cx: &mut App,
) -> (Size<Pixels>, Rc<ClampPlan>) {
    let inset = used_box.inset();
    let content_top = f32::from(inset.top);
    let mut geometry = geometry.clone();
    if border_box {
        geometry.max_h = geometry
            .max_h
            .map(|h| (h - f32::from(inset.top + inset.bottom)).max(0.));
    }
    window.with_nested_layout(|window| {
        let records = ClampMeasure::default();
        let trial_flow = fragment.map(ClampFlow::scope);
        let mut trial = build(
            ClampInput::Measure(records.clone()),
            known.width,
            used_box,
            false,
        );
        let space = size(
            known
                .width
                .map_or(available.width, AvailableSpace::Definite),
            known
                .height
                .map_or(available.height, AvailableSpace::Definite),
        );
        trial.layout_as_root_unrounded(space, window, cx);
        let batch = records.collect(key, window.element_offset(), window);
        let mut plan = Rc::new(geometry.compute(batch.entries, &batch.rows, content_top));
        drop(trial_flow);
        // Балансировка после выбора точки может увеличить высоту строк
        // (css-text-4 §5.4). Уточнение уменьшает бюджет того же абзаца либо
        // однократно задаёт конечный срез, если строчный путь не смог
        // применить бюджет. Обе ступени конечны в пределах текущей раскладки.
        // Размер берётся из применения плана, а не из n * line-height.
        loop {
            let applied_records = ClampMeasure::default();
            let applied_flow = fragment.map(ClampFlow::scope);
            let mut applied = build(
                ClampInput::ApplyMeasured(plan.clone(), applied_records.clone()),
                known.width,
                used_box,
                false,
            );
            // Размер переносится в родительскую раскладку без округления:
            // округляются только конечные абсолютные края при отрисовке.
            let measured = applied.layout_as_root_unrounded(space, window, cx);
            let batch = applied_records.collect(key, window.element_offset(), window);
            let next = geometry.compute(batch.entries, &batch.rows, content_top);
            if refines(&plan, &next) {
                plan = Rc::new(next);
                continue;
            }
            // Only a layout at the used inline size gives the container's
            // lines; intrinsic-size probes at min/max-content do not.
            let used_width =
                known.width.is_some() || matches!(available.width, AvailableSpace::Definite(_));
            if let Some(fragment) = fragment.filter(|_| used_width) {
                fragment.publish(measured, window);
            }
            drop(applied_flow);
            return (
                size(
                    known.width.unwrap_or(measured.width),
                    known.height.unwrap_or(measured.height),
                ),
                plan,
            );
        }
    })
}

fn refines(plan: &ClampPlan, next: &ClampPlan) -> bool {
    plan.para
        .zip(next.para)
        .is_some_and(|((s, k), (ns, nk))| s == ns && nk < k)
        || (plan.para == next.para && !plan.cut.is_finite() && next.cut.is_finite())
}

#[cfg(test)]
mod tests {
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
        measure.record(move |bounds| ClampEntry {
            bounds,
            line: 20.,
            skip_count: false,
            fixed_height: false,
            bp_after: 0.,
            end_margin: 0.,
            min_end: 0.,
            capped: false,
            node: 0,
            seq: Some(seq),
            clamped: None,
            empty: false,
            span: None,
            bt_before: 0.,
            positioned: false,
            float: false,
        })
    }

    /// A fragment copy is laid out inside its fragmentainer's element offset:
    /// the clamp point of every copy stays where the container's own lines put
    /// it, so no fragment restarts or shifts the line count.
    #[gpui::test]
    fn element_offset_of_a_fragment_copy_does_not_move_the_clamp_point(
        cx: &mut gpui::TestAppContext,
    ) {
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
                    measure(
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
                    )
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
}
