//! Числовые заявки фрагментации без правок DOM и построения GPUI-элементов.

use super::ColumnStack;
use crate::tolerance::COORD_EPS;

impl ColumnStack {
    /// Заявки роста при неизменной высоте пробного фрагментаинера.
    pub(super) fn growth_requests(&self) -> Vec<Vec<crate::layout::fragment::push::Growth>> {
        let kids = self.kid_geoms();
        let mut grows = self.grow_points(&kids);
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает
        // коробки выше, и разложенные border box прохода остаются верны.
        grows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut local_grows = vec![Vec::new(); self.children.len()];
        let (_, lines, plan, _) = self.balance(&kids);
        // Рост у фрагмента сдвигает всё содержимое потока ниже: точки
        // следующих фрагментов, найденные по прежней раскладке, устарели.
        // Копия с flex растёт по одному фрагменту за проход — первому,
        // которому нужен рост (как последовательная раскладка Blink,
        // фрагмент за фрагментом); остальные — после переукладки.
        let mut tagged: Vec<Vec<(usize, crate::layout::fragment::push::Growth)>> =
            vec![Vec::new(); self.children.len()];
        for f in &plan {
            let c = &self.children[f.kid];
            let (Some(r), Some(w)) = (c.regrow.as_ref(), c.watched.as_ref()) else {
                continue;
            };
            let Some(blocks) = w.blocks() else { continue };
            // Колонки сверх последней линии переполняют её вбок (css-multicol-1 §8.2):
            // их высота — высота этой линии.
            let Some(&(_, height)) = lines.get(f.col / self.count).or(lines.last()) else {
                continue;
            };
            let edge = f.from + height - f.y;
            // Колонка первого ряда, начатого под содержимым внешней колонки.
            let open = f.y <= COORD_EPS && self.row_phase > COORD_EPS && f.col < self.count;
            let mut local = r.fragment_growths(blocks, f.from, f.from + f.h, edge, height, open);
            if plan.iter().any(|g| g.kid == f.kid && g.copy == f.copy + 1) {
                local.extend(crate::layout::fragment::push::monolith_flow::pulls(
                    blocks,
                    &r.laid,
                    f.from,
                    f.from + f.h,
                ));
            }
            tagged[f.kid].extend(local.into_iter().map(|g| (f.copy, g)));
        }
        for &(kid, at, grow, forced) in &grows {
            let c = &self.children[kid];
            let (Some(_r), Some(w)) = (c.regrow.as_ref(), c.watched.as_ref()) else {
                continue;
            };
            let copy = plan
                .iter()
                .filter(|f| {
                    f.kid == kid && f.from - COORD_EPS <= at && at <= f.from + f.h + COORD_EPS
                })
                .map(|f| f.copy)
                .min()
                .unwrap_or(usize::MAX);
            if let Some(blocks) = w.blocks() {
                tagged[kid].extend(
                    crate::layout::fragment::push::Growth::at(blocks, &_r.laid, at, grow, forced)
                        .map(|g| (copy, g)),
                );
            }
        }
        for (kid, list) in tagged.into_iter().enumerate() {
            // Независимые потоки flex и сетки растут по одному фрагменту за проход.
            let flex = self.children[kid]
                .regrow
                .as_ref()
                .is_some_and(|r| !r.laid.flexes.is_empty() || !r.laid.placements.is_empty());
            let first = list.iter().map(|g| g.0).min();
            local_grows[kid].extend(
                list.into_iter()
                    .filter(|g| !flex || Some(g.0) == first)
                    .map(|g| g.1),
            );
        }
        for (kid, grows) in local_grows.iter_mut().enumerate() {
            grows.extend(self.item_growths(kid, &plan, &lines));
        }
        local_grows
    }
}
