//! Измеренные возможности разрыва главной копии ребёнка стопки — строки, флоаты и границы
//! блочной стопки по разложенным border box (`Watched::breaks`) — во входе колонок.

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
    ) {
        let Some(watched) = &self.watched else {
            return;
        };
        let end = self.h.max(self.over);
        let m = watched.breaks(point(px(0.0), px(0.0)), size, end, axis, window);
        self.cuts.extend(m.cuts);
        self.forced.extend(m.forced);
        self.solid.extend(m.solid);
        self.avoid.extend(m.avoid);
        self.cuts
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        self.forced.sort_by(f32::total_cmp);
    }

    /// Точки разрыва ребёнка, чью высоту дала мера, а копия разложена в
    /// колонку (`breaks_w`): оценочные `cuts`, `forced` и `solid` заменяются
    /// измеренными.
    pub(crate) fn replace_breaks(
        &mut self,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) {
        let Some(watched) = &self.watched else {
            return;
        };
        let end = self.h.max(self.over);
        let m = watched.breaks(point(px(0.0), px(0.0)), size, end, axis, window);
        self.cuts = m.cuts;
        self.forced = m.forced;
        self.solid = m.solid;
        self.avoid = m.avoid;
    }
}
