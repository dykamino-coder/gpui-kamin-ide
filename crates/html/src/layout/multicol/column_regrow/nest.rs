//! Внешнее пространство вложенного многоколоночника (css-break-3 §2: вложенный
//! контекст фрагментации; css-multicol-1 §8): ряд его колонок — внешний
//! фрагментаинер, а первый ряд — остаток фрагментаинера от места, где ребёнок
//! начался. Это место решает раскладка предшествующих детей стопки, как
//! `ConstraintSpace::FragmentainerOffset` ребёнка у Blink
//! (`column_layout_algorithm.cc`: `available_outer_space` первого ряда).

use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, Window};

impl ColumnStack {
    /// Собрать и измерить ребёнка `i` в его внешнем пространстве: высота ряда —
    /// заданная высота колонок либо, у единственного вложенного многоколоночника
    /// заданной высоты в балансируемых колонках, его доля на колонку.
    /// Ложь — ребёнок не вложенный ряд, его меряет `lay_out_main`.
    pub(crate) fn resume_nested(&mut self, i: usize, window: &mut Window, cx: &mut App) -> bool {
        let Some(resume) = self.children[i].resume.clone() else {
            return false;
        };
        match fragmentainer_extent(self.fixed_height, self.rows) {
            Some(hh) => {
                let start = self.start_in_fragmentainer(i, hh);
                self.place_nested(i, &resume, Some((hh, start)), window, cx);
                let need = self.children[i].nest_report.as_ref().and_then(|r| r.get());
                if start > COORD_EPS
                    && let Some(need) = need
                {
                    // Ряд в остатке фрагментаинера не уместил своё начало: разрыв
                    // перед многоколоночником лучше (css-break-3 §4.4). Коробка
                    // начинает следующий фрагментаинер, её начало неделимо.
                    self.place_nested(i, &resume, Some((hh, 0.0)), window, cx);
                    self.children[i].solid.push((0.0, need));
                }
            }
            None => {
                self.place_nested(i, &resume, None, window, cx);
                let per = self.children[i].h / self.count as f32;
                let per = self.rows.and_then(|r| r.h).map_or(per, |cap| per.min(cap));
                if per > 1.0 {
                    self.place_nested(i, &resume, Some((per, 0.0)), window, cx);
                }
            }
        }
        true
    }

    /// Ребёнок `i` в пространстве `nest`, измеренный своей копией.
    fn place_nested(
        &mut self,
        i: usize,
        resume: &std::rc::Rc<dyn Fn(Option<(f32, f32)>) -> crate::layout::fragment::types::StackChild>,
        nest: Option<(f32, f32)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if nest != self.children[i].nest {
            let mut child = resume(nest);
            child.resume = Some(resume.clone());
            self.children[i] = child;
        }
        self.lay_out_main(i, window, cx);
    }

    /// Начало ребёнка `i` в его фрагментаинере по укладке предшествующих детей:
    /// пустой щуп с полями и разрывом перед ребёнком встаёт туда же, куда встанет
    /// он сам; щуп на краю фрагментаинера начинает следующий.
    fn start_in_fragmentainer(&self, i: usize, hh: f32) -> f32 {
        let child = &self.children[i];
        let mut kids: Vec<Kid> = self.children[..i].iter().map(|c| c.kid()).collect();
        kids.push(Kid {
            mt: child.mt,
            force_before: child.force_before,
            avoid_before: child.avoid_before,
            par: child.par,
            ..Default::default()
        });
        let (_, _, plan, _) = self.balance(&kids);
        plan.iter()
            .find(|f| f.kid == i)
            .map(|f| f.y)
            .filter(|y| *y < hh - COORD_EPS)
            .unwrap_or(0.0)
    }
}

/// Конец содержимого первого ряда, когда оно не уместилось в ряд: ряд вложенного
/// многоколоночника вышел за остаток внешнего фрагментаинера (`Nested::first_row`).
pub(crate) fn first_row_overflow(
    lines: &[(f32, f32)],
    plan: &crate::layout::fragment::plan::Plan,
    count: usize) -> Option<f32> {
    let row = lines.first()?.1;
    let end = plan
        .iter()
        .filter(|f| f.col < count)
        .map(|f| f.y + f.h)
        .fold(0.0f32, f32::max);
    (end > row + crate::tolerance::COORD_EPS).then_some(end)
}

/// Блочный размер фрагментаинера стопки без балансировки: заданная высота колонок
/// либо ряд внешнего фрагментаинера (`Rows` с переносом без потолка).
pub(crate) fn fragmentainer_extent(
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
) -> Option<f32> {
    match rows {
        None => fixed,
        Some(r) if r.wrap && !r.cap => r.h,
        Some(_) => None,
    }
    .filter(|h| *h > 0.0)
}
