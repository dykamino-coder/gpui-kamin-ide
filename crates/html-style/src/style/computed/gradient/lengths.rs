//! Разрешает разобранные длины градиента после наследования шрифта, до отрисовки.

use super::{BgSize, Computed, Gradient, GradientShape, Len, RadialSize};
use crate::style::values::value::calc_get;
use crate::values::ValueCx;

impl Gradient {
    pub fn needs_line_height(&self, vcx: ValueCx<'_>) -> bool {
        let needs_line = |length: &Option<Len>| length.is_some_and(|l| l.needs_line_height(vcx));
        self.stop_lengths.iter().any(needs_line)
            || match &self.shape {
                GradientShape::Radial { position, size, .. } => {
                    needs_line(&position.x)
                        || needs_line(&position.y)
                        || match size {
                            RadialSize::Explicit(first, second) => {
                                needs_line(&Some(*first)) || needs_line(second)
                            }
                            _ => false,
                        }
                }
                _ => false,
            }
    }

    /// Есть ли длины, которые `resolve_lengths` читает: без них обход ничего не пишет.
    pub fn has_lengths(&self) -> bool {
        !self.stop_lengths.is_empty() || matches!(self.shape, GradientShape::Radial { .. })
    }

    /// Изменит ли `resolve_lengths` градиент: `moves` — длины, которые меняет его `fix`. Ответ
    /// по ссылке, без копии: стоп сворачивается или уже приводится к доле и точкам, список
    /// длин стопов пора очистить, у радиального сворачивается позиция или размер.
    pub fn lengths_pending(&self, moves: impl Fn(Len) -> bool, vcx: ValueCx<'_>) -> bool {
        let moves = |length: &Option<Len>| length.is_some_and(&moves);
        let stops = &self.stop_lengths;
        (!stops.is_empty()
            && (stops.iter().all(Option::is_none)
                || stops
                    .iter()
                    .any(|l| moves(l) || stop_terms(*l, vcx).is_some())))
            || match &self.shape {
                GradientShape::Radial { position, size, .. } => {
                    moves(&position.x)
                        || moves(&position.y)
                        || match size {
                            RadialSize::Explicit(first, second) => {
                                moves(&Some(*first)) || moves(second)
                            }
                            _ => false,
                        }
                }
                _ => false,
            }
    }

    pub fn resolve_lengths(&mut self, fix: impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        if !self.stop_lengths.is_empty() {
            for (length, stop) in self.stop_lengths.iter_mut().zip(&mut self.stops_raw) {
                fix(length);
                if let Some((pct, px)) = stop_terms(*length, vcx) {
                    stop.1 = (pct != 0.0 || matches!(length, Some(Len::Pct(_) | Len::Calc(_))))
                        .then_some(pct);
                    stop.2 = (px != 0.0 || pct == 0.0).then_some(px);
                    *length = None;
                }
            }
            self.stops = crate::style::computed::place_stops(
                self.stops_raw
                    .iter()
                    .map(|(color, pct, _)| (*color, *pct))
                    .collect(),
            );
            self.stops_px = if self
                .stops_raw
                .iter()
                .all(|(_, pct, px)| pct.is_none() && px.is_some())
            {
                self.stops_raw
                    .iter()
                    .map(|(color, _, px)| (*color, px.unwrap_or(0.0)))
                    .collect()
            } else {
                vec![]
            };
            if self.stop_lengths.iter().all(Option::is_none) {
                self.stop_lengths.clear();
            }
        }
        if let GradientShape::Radial { position, size, .. } = &mut self.shape {
            fix(&mut position.x);
            fix(&mut position.y);
            if let RadialSize::Explicit(first, second) = size {
                let mut length = Some(*first);
                fix(&mut length);
                if let Some(length) = length {
                    *first = length;
                }
                fix(second);
            }
        }
    }
}

/// Доля и точки стопа, который уже можно нормализовать; `None` — длина ещё не решена.
fn stop_terms(length: Option<Len>, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
    match length? {
        Len::Px(px) => Some((0.0, px)),
        Len::Pct(pct) => Some((pct, 0.0)),
        Len::Calc(id) => calc_get(id, vcx).length_percentage(),
        _ => None,
    }
}

impl Computed {
    pub fn resolve_gradient_lengths(&mut self, fix: impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        let size = match self.reset_background.bg_size {
            BgSize::Fixed(w, h) => [w, h],
            _ => [None, None],
        };
        let uses_line = [
            self.reset_background.bg_pos.x,
            self.reset_background.bg_pos.y,
        ]
        .into_iter()
        .chain(size)
        .flatten()
        .any(|l| l.needs_line_height(vcx))
            || self.reset_background.bg_lists.needs_line_height(vcx)
            || self.image_operands_ref().any(|i| i.needs_line_height(vcx))
            || [
                self.reset_background.gradient.as_ref(),
                self.reset_background.gradient_current.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|g| g.needs_line_height(vcx));
        let line = if uses_line {
            self.image_font_metrics(vcx).3
        } else {
            0.0
        };
        let fix = |length: &mut Option<Len>| {
            fix(length);
            if uses_line && let Some(length) = length {
                length.resolve_line_height(line, vcx);
            }
        };
        // Группа фона пишется, только если в ней есть что сворачивать: общая
        // группа без длин остаётся общей.
        if [
            self.reset_background.gradient.as_ref(),
            self.reset_background.gradient_current.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(Gradient::has_lengths)
        {
            for gradient in {
                let group = &mut *self.reset_background;
                [group.gradient.as_mut(), group.gradient_current.as_mut()]
            }
            .into_iter()
            .flatten()
            {
                gradient.resolve_lengths(&fix, vcx);
            }
        }
        // Позиция и размер вычисляются и без изображения (css-backgrounds-3
        // §2.6, §2.9); процент остаётся для области позиционирования.
        let background = &*self.reset_background;
        let pos = [background.bg_pos.x, background.bg_pos.y].map(|mut v| {
            fix(&mut v);
            v
        });
        let size = match background.bg_size {
            BgSize::Fixed(w, h) => Some([w, h].map(|mut v| {
                fix(&mut v);
                v
            })),
            _ => None,
        };
        if pos != [background.bg_pos.x, background.bg_pos.y] {
            let group = &mut *self.reset_background;
            (group.bg_pos.x, group.bg_pos.y) = (pos[0], pos[1]);
        }
        if let Some([w, h]) = size
            && self.reset_background.bg_size != BgSize::Fixed(w, h)
        {
            self.reset_background.bg_size = BgSize::Fixed(w, h);
        }
        if !self.reset_background.bg_lists.is_empty() {
            self.reset_background.bg_lists.resolve_lengths(&fix, vcx);
        }
        if self
            .image_operands_ref()
            .any(crate::style::computed::ImageSource::has_lengths)
        {
            for image in self.image_operands() {
                image.resolve_lengths(&fix, vcx);
            }
        }
    }
}

#[cfg(test)]
mod tests;
