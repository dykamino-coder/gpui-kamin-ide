//! Измеренные возможности разрыва главной копии ребёнка стопки — строки, флоаты и границы
//! блочной стопки по разложенным border box (`Watched::breaks`) — во входе колонок; полосы
//! повтора секций таблицы-ребёнка — по её разложенным секциям (`refresh_repeat`).

use crate::layout::fragment::flow::blocks::table::TableSections;
use crate::layout::fragment::table_bands::repeat_spec;
use crate::layout::fragment::types::{StackAxis, StackChild};
use gpui::{Pixels, Size, Window, point, px};

impl StackChild {
    /// Точки разрыва ребёнка, чью высоту дала раскладка копии (`measure`):
    /// `size` — её размер, поток — до `over`.
    pub(crate) fn measure_breaks(
        &mut self,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Option<TableSections> {
        let watched = self.watched.as_ref()?;
        let end = self.h.max(self.over);
        let m = watched.breaks(point(px(0.0), px(0.0)), size, end, axis, window);
        self.cuts.extend(m.cuts);
        self.forced.extend(m.forced);
        self.solid.extend(m.solid);
        self.avoid.extend(m.avoid);
        self.keep = m.keep;
        self.cuts
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        self.forced.sort_by(f32::total_cmp);
        m.sections
    }

    /// Точки разрыва ребёнка, чью высоту дала мера, а копия разложена в
    /// колонку (`breaks_w`): оценочные `cuts`, `forced` и `solid` заменяются
    /// измеренными.
    pub(crate) fn replace_breaks(
        &mut self,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Option<TableSections> {
        let watched = self.watched.as_ref()?;
        let end = self.h.max(self.over);
        let m = watched.breaks(point(px(0.0), px(0.0)), size, end, axis, window);
        self.cuts = m.cuts;
        self.forced = m.forced;
        self.solid = m.solid;
        self.avoid = m.avoid;
        self.keep = m.keep;
        m.sections
    }

    /// Полосы повтора таблицы-ребёнка по её разложенным секциям `sections`
    /// во фрагментаинере высотой `per` (`table_bands::repeat_spec`); без
    /// повторяемой секции повтора нет вовсе.
    pub(crate) fn refresh_repeat(&mut self, sections: Option<TableSections>, per: Option<f32>) {
        let (Some(sec), Some(per)) = (sections, per) else {
            return;
        };
        if self.repeat.is_none() {
            return;
        }
        match repeat_spec(sec, per) {
            Some((head, foot, geom)) => {
                if let Some(r) = self.repeat.as_mut() {
                    r.head = head;
                    r.foot = foot;
                    r.geom = geom;
                }
            }
            None => self.repeat = None,
        }
    }
}
