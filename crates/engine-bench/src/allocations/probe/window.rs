//! Окно пробника: фаза и кадры, в которых пишутся строки, и конец окна по
//! выходу последнего кадра из заданной фазы.

use super::OUTSIDE;

/// Какие аллокации записывать: фаза (`None` — любая) и кадры замера.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Filter {
    pub(super) phase: Option<usize>,
    pub(super) first: u64,
    pub(super) last: u64,
}
impl Filter {
    pub(super) fn parse(spec: &str, names: &[&str]) -> Option<Self> {
        let (phase, frames) = match spec.split_once('@') {
            Some((phase, frames)) => (phase, Some(frames)),
            None => (spec, None),
        };
        let phase = match phase {
            "*" => None,
            "outside" => Some(OUTSIDE),
            name => Some(names.iter().position(|known| *known == name)?),
        };
        let (first, last) = match frames.map(|frames| frames.split_once('-').ok_or(frames)) {
            None => (0, u64::MAX),
            Some(Ok((first, last))) => (first.parse().ok()?, last.parse().ok()?),
            Some(Err(one)) => {
                let one = one.parse().ok()?;
                (one, one)
            }
        };
        (first <= last).then_some(Self { phase, first, last })
    }
    pub(super) fn matches(self, phase: usize, frame: u64) -> bool {
        self.phase.is_none_or(|wanted| wanted == phase) && (self.first..=self.last).contains(&frame)
    }
}

/// Конец окна: выход последнего кадра из фазы `phase`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Until {
    #[default]
    Open,
    /// Ещё не вошли в фазу.
    Before,
    /// Последний кадр сейчас в фазе.
    Inside,
    /// Окно закрыто навсегда.
    Closed,
}
impl Until {
    /// Следующее состояние при входе в `phase` кадра `frame`.
    pub(super) fn step(self, until: usize, last: u64, phase: usize, frame: u64) -> Self {
        match self {
            Self::Before if frame == last && phase == until => Self::Inside,
            Self::Inside if phase != until => Self::Closed,
            other => other,
        }
    }
}
