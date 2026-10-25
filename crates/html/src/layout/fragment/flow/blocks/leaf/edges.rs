//! Края фрагмента: расход места не меняет consumed content block-size.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Edges {
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) clone: bool,
}

impl Edges {
    pub(crate) fn valid(self) -> bool {
        [self.start, self.end]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0)
    }

    pub(crate) fn sum(self) -> f32 {
        self.start + self.end
    }

    pub(crate) fn start_at(self, first: bool) -> f32 {
        if first || self.clone { self.start } else { 0.0 }
    }

    /// css-break-3 §4.4: при нехватке места сначала усекается клонированный
    /// конец, затем начало; место для продвижения содержимого остаётся всегда.
    pub(crate) fn fit_clone(self, room: f32, content: f32) -> Self {
        if self.sum() < room {
            return self;
        }
        let budget = (room - content.min(1.0)).max(0.0);
        let start = self.start.min(budget);
        Self {
            start,
            end: self.end.min((budget - start).max(0.0)),
            ..self
        }
    }
}
