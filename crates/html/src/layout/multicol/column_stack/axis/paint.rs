//! Продолжение краски вертикальных колонок на общем стеке.

use super::*;

impl ColumnStack {
    /// Отрисовка вертикальной стопки (пара к `prepaint_axis`).
    pub(crate) async fn paint_axis(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if self.oof.plan.borrow().failure.is_some()
            || self.record_native_async(window, cx, stack).await.is_err()
        {
            return;
        }
        let axis = self.axis;
        let (_, block_avail) = self.axis_sizes(bounds);
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        let plan = self.plan.borrow().clone();
        let plan_all = plan.clone();
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        let nested_frag = crate::layout::fragment::effect_fragment::active(&self.paint);
        let records = self.copy_records();
        for f in plan {
            let (c, ry) = self.place(f.col);
            let rel = self.children[f.kid].rel;
            let split = parts[f.kid] > 1;
            // Срез поперёк БЛОЧНОЙ оси; вдоль строчной колонка переполнение
            // не режет (css-multicol-1 §8.1 «visibly overflows and is not
            // clipped to the column box») — вылет на высоту окна, как у
            // горизонтальной стопки на его ширину. Кроме вложенного
            // многоколоночника (`nested_cols`).
            let win_h = f32::from(window.viewport_size().height);
            let spill = if self.children[f.kid].nested_cols {
                0.0
            } else {
                win_h.max(f32::from(bounds.size.height))
            };
            // Чернильное переполнение над первым и под последним фрагментом
            // видно — как у горизонтальной стопки (`paint_frags`).
            let continued = plan_all
                .iter()
                .any(|g| g.kid == f.kid && g.copy == f.copy + 1);
            let line_h = self.lines_plan.borrow().first().map_or(0.0, |l| l.1);
            let k0 = &self.children[f.kid];
            // Измерение размера не обрезает чернильное переполнение краёв
            // без разрыва (css-break-3 §5.4). Свой clip задаёт вложенная стопка.
            let keep = spill == 0.0 || nested_frag;
            let up = if !keep && f.copy == 0 && f.from <= COORD_EPS {
                spill
            } else {
                0.0
            };
            let down = if keep || continued || f.from + f.h < k0.over - COORD_EPS {
                0.0
            } else {
                (line_h - f.y - f.h).max(0.0)
            };
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step - spill,
                col_w + spill + spill,
                ry + f.y - up,
                f.h + up + down,
            );
            let mask = fragment_mask::snap(
                Bounds {
                    origin: point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                    size: b.size,
                },
                window.scale_factor(),
            );
            let kid = &mut self.children[f.kid];
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            let segment = gpui::PaintSegmentId(f.col as u64);
            let mut paint = async |window: &mut Window| {
                crate::hooks::with_copy_segment_async(
                    segment,
                    records.as_ref(),
                    window,
                    async |window| el.paint_async(window, cx, stack).await,
                )
                .await
            };
            if split {
                window.with_content_mask_async(Some(mask), paint).await;
            } else {
                paint(window).await;
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint_async(window, cx, stack).await;
        }
        let _ = self.paint_orphans(window);
    }
}
