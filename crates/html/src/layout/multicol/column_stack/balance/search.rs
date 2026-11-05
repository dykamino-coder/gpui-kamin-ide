//! Конечный поиск высоты: минимальный shortage растягивает пробу, без него границы
//! сходятся делением до 1/64 точки; CSS-потолок сохраняется точно.

use crate::layout::fragment::token::Error;
use std::collections::HashSet;

// Та же сетка, что у line_shape::nested_kid::layout_unit. На больших
// координатах следующий f32 уже шире сетки: next_up сохраняет прогресс.
fn next_unit(value: f32) -> f32 {
    (((f64::from(value) * 64.).floor() + 1.) / 64.) as f32
}

fn after(value: f32) -> f32 {
    next_unit(value).max(value.next_up())
}

fn before(value: f32) -> f32 {
    ((((f64::from(value) * 64.).ceil() - 1.) / 64.) as f32).min(value.next_down())
}

pub(crate) fn ceil_unit(value: f32) -> f32 {
    let exact = (f64::from(value) * 64.).ceil() / 64.;
    let rounded = exact as f32;
    if f64::from(rounded) < exact {
        rounded.next_up()
    } else {
        rounded
    }
}

pub(crate) struct Trial<T> {
    pub(crate) fits: bool,
    pub(crate) shortage: Option<f32>,
    pub(crate) value: T,
}

pub(crate) fn min_shortage(values: impl IntoIterator<Item = Option<f32>>) -> Option<f32> {
    values
        .into_iter()
        .flatten()
        .filter(|s| s.is_finite() && *s > 0. && *s < f32::MAX)
        .min_by(f32::total_cmp)
}

/// Все пробы начинаются с одного входного курсора и снимка.
struct Search<T> {
    low: f32,
    high: f32,
    at: f32,
    ceiling: f32,
    constrained: bool,
    best: Option<T>,
    seen: HashSet<u32>,
}
impl<T> Search<T> {
    fn new(start: f32, ceiling: f32, constrained: bool) -> Result<Self, Error> {
        if !start.is_finite() || !ceiling.is_finite() || start <= 0. || ceiling < start {
            return Err(Error::InvalidGeometry);
        }
        Ok(Self {
            low: before(start),
            high: ceiling,
            at: ceil_unit(start).min(ceiling),
            ceiling,
            constrained,
            best: None,
            seen: HashSet::new(),
        })
    }
    fn next(&mut self) -> Result<f32, Error> {
        if !self.seen.insert(self.at.to_bits()) {
            return Err(Error::NoProgress);
        }
        crate::work::add!(MulticolProbes, 1);
        Ok(self.at)
    }
    fn accept(&mut self, result: Trial<T>) -> Result<Option<(f32, T)>, Error> {
        if result.fits {
            self.high = self.at;
            self.best = Some(result.value);
        } else if self.at == self.ceiling {
            return if self.constrained {
                Ok(Some((self.at, result.value)))
            } else {
                Err(Error::NoProgress)
            };
        } else {
            self.low = self.at;
            if let Some(shortage) = result.shortage.filter(|s| s.is_finite() && *s > 0.) {
                // Минимальный недолаз — наименьший прирост, меняющий хоть один
                // разрыв пробы (Blink `MinimalSpaceShortage` и растяжение в
                // `ColumnLayoutAlgorithm::LayoutRow`). Более низкие высоты
                // повторяют те же разрывы, поэтому они отвергнуты вместе с пробой.
                let next = ceil_unit(self.at + shortage)
                    .max(after(self.at))
                    .min(self.high);
                self.low = before(next).max(self.at);
                if self.best.is_none() || next < self.high {
                    self.at = next;
                    return Ok(None);
                }
            }
        }
        if self.best.is_some() && after(self.low) >= self.high {
            return self
                .best
                .take()
                .map(|value| Some((self.high, value)))
                .ok_or(Error::NoProgress);
        }
        // Сначала проверяем верхнюю границу. После успешной пробы интервал
        // строго уменьшается на конечной сетке layout units.
        self.at = if self.best.is_none() {
            self.high
        } else {
            let middle = ((f64::from(self.low) + f64::from(self.high)) / 2.) as f32;
            ceil_unit(middle)
                .max(after(self.low))
                .min(before(self.high))
        };

        Ok(None)
    }
}
pub(crate) fn search<T>(
    start: f32,
    ceiling: f32,
    constrained: bool,
    mut trial: impl FnMut(f32) -> Trial<T>,
) -> Result<(f32, T), Error> {
    let mut state = Search::new(start, ceiling, constrained)?;
    loop {
        let result = trial(state.next()?);
        if let Some(chosen) = state.accept(result)? {
            return Ok(chosen);
        }
    }
}
pub(crate) async fn search_async<T>(
    start: f32,
    ceiling: f32,
    constrained: bool,
    mut trial: impl AsyncFnMut(f32) -> Trial<T>,
) -> Result<(f32, T), Error> {
    let mut state = Search::new(start, ceiling, constrained)?;
    loop {
        let result = trial(state.next()?).await;
        if let Some(chosen) = state.accept(result)? {
            return Ok(chosen);
        }
    }
}

#[cfg(test)]
mod tests;
