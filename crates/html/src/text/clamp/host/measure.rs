//! Числовой план и естественный размер применения до освобождения пробного дерева.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn measure_async(
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
    stack: &mut gpui::PaintStack,
) -> (Size<Pixels>, Rc<ClampPlan>) {
    let inset = used_box.inset();
    let content_top = f32::from(inset.top);
    let mut geometry = geometry.clone();
    if border_box {
        geometry.max_h = geometry
            .max_h
            .map(|h| (h - f32::from(inset.top + inset.bottom)).max(0.));
    }
    window
        .with_nested_layout_async(async |window| {
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
            trial
                .layout_as_root_unrounded_async(space, window, cx, stack)
                .await;
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
                let measured = applied
                    .layout_as_root_unrounded_async(space, window, cx, stack)
                    .await;
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
        .await
}

fn refines(plan: &ClampPlan, next: &ClampPlan) -> bool {
    plan.para
        .zip(next.para)
        .is_some_and(|((s, k), (ns, nk))| s == ns && nk < k)
        || (plan.para == next.para && !plan.cut.is_finite() && next.cut.is_finite())
}

#[cfg(test)]
mod tests;
