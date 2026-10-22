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
        trial.layout_as_root(space, window, cx);
        let batch = records.collect(key, point(px(0.), px(0.)), window);
        let mut plan = Rc::new(geometry.compute(batch.entries, &batch.rows, content_top));
        // Балансировка после выбора точки может увеличить высоту строк
        // (css-text-4 §5.4). Уточнение уменьшает бюджет того же абзаца либо
        // однократно задаёт конечный срез, если строчный путь не смог
        // применить бюджет. Обе ступени конечны в пределах текущей раскладки.
        // Размер берётся из применения плана, а не из n * line-height.
        loop {
            let applied_records = ClampMeasure::default();
            let mut applied = build(
                ClampInput::ApplyMeasured(plan.clone(), applied_records.clone()),
                known.width,
                used_box,
                false,
            );
            let measured = applied.layout_as_root(space, window, cx);
            let batch = applied_records.collect(key, point(px(0.), px(0.)), window);
            let next = geometry.compute(batch.entries, &batch.rows, content_top);
            if refines(&plan, &next) {
                plan = Rc::new(next);
                continue;
            }
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
}
