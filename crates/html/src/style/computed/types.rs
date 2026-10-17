//! Значения свойств: перечисления и записи, из которых собран Computed.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod background;
mod boxes;
mod decor;
mod image;
mod layout;
mod text;
pub use background::{BgClip, BgPos, BgRepeat, BgSize, Tiling, parse_pos_words};
pub use boxes::{
    BorderShape, Logical, LogicalSides, Outline, Overflow, Position, PositionAnchor, SideSeq,
    TextEdge,
};
pub use decor::{
    DECOR_OVER, DECOR_THROUGH, DECOR_UNDER, Decor, DecorFont, DecorLen, DecorStyle, UPOS_FROM_FONT,
    UPOS_LEFT, UPOS_RIGHT, UPOS_UNDER,
};
pub(super) use decor::{
    parse_decor_inset, parse_decor_length, parse_decor_style, parse_decor_thickness,
};
pub use image::{BorderImage, BorderImageSlice, BorderImageWidth, GradSpace, Gradient, Shadow};
pub use layout::{Align, AutoFlow, Display, FlexDir, GapInset, GapList, Justify, Placement};
pub use text::{
    Hanging, RubyAlign, RubyOverhang, RubyRole, TT_FULL_WIDTH, TT_KANA, TT_MATH, TextAlign,
    TextFit, TextTransform,
};

/// `animation`: имя набора кадров и как его проигрывать.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimSpec {
    pub name: String,
    pub seconds: f32,
    pub infinite: bool,
    /// `alternate` — обратный ход через раз.
    pub alternate: bool,
    /// `animation-delay`: отрицательная — старт с середины.
    pub delay: f32,
    /// `animation-play-state: paused` — живой анимации нет, рисуется один
    /// кадр на месте `(-delay)/duration` (reftest'ы иначе недетерминированы).
    pub paused: bool,
    /// `animation-name: a, b` — все имена списка по порядку; пусто, когда имя
    /// одно (тогда работает `name`).
    pub names: Vec<String>,
    /// CSS Animations 1 fill modes: bit 1 backwards, bit 2 forwards.
    pub fill: u8,
    pub iterations: f64,
    pub reverse: bool,
}

impl AnimSpec {
    /// Анимация, которая за жизнь страницы не сдвинется ни на точку, по сути
    /// остановлена: пауза или `animation: a 2000000s; animation-delay:
    /// -1000000s`. Один предикат на разрешение кадров (`dom.rs`) и отрисовку.
    pub fn frozen(&self) -> bool {
        self.paused
            || (self.seconds == 0.0 && self.delay <= 0.0)
            || (!self.infinite && self.seconds >= 3600.0 && self.delay <= 0.0)
    }

    /// CSS Animations 1: fill phases, iteration boundaries and direction.
    pub fn progress_at(&self, elapsed: f64) -> Option<f32> {
        let active = elapsed - f64::from(self.delay);
        let duration = f64::from(self.seconds).max(0.0);
        let end = if self.infinite {
            f64::INFINITY
        } else {
            duration * self.iterations
        };
        if (active < 0.0 && self.fill & 1 == 0) || (active >= end && self.fill & 2 == 0) {
            return None;
        }
        let total = if duration > 0.0 {
            active.max(0.0).min(end) / duration
        } else {
            self.iterations
        };
        let finished = active >= end && total > 0.0;
        let iteration = if finished {
            total.ceil() - 1.0
        } else {
            total.floor()
        };
        let mut progress = if finished && total.fract() == 0.0 {
            1.0
        } else {
            total.fract()
        };
        if self.reverse ^ (self.alternate && iteration.rem_euclid(2.0) == 1.0) {
            progress = 1.0 - progress;
        }
        Some(progress as f32)
    }

    pub fn frozen_t(&self) -> f32 {
        self.progress_at(0.0).unwrap_or(0.0)
    }
}

/// Одна составляющая `content` (css-content-3 §2 `<content-list>`).
#[derive(Clone, Debug, PartialEq)]
pub enum ContentItem {
    /// Литеральная строка.
    Str(String),
    /// `counter(имя, стиль)`.
    Counter(String, String),
    /// `counters(имя, разделитель, стиль)`.
    Counters(String, String, String),
    /// Attribute name and serialized fallback; None is guaranteed-invalid.
    Attr(String, Option<String>),
    /// `open-quote`/`close-quote` (`emit`) и `no-open-quote`/`no-close-quote`
    /// (только сдвиг глубины) — css-content-3 §4.2.
    Quote { open: bool, emit: bool },
    /// `url(…)` — картинка-атом в `::before`/`::after` (css-content-3 §2).
    /// Строится настоящим `<img>`-ребёнком псевдоэлемента: природный размер
    /// меряет обычный путь картинок. В маркере и тексте не печатается.
    Image(String),
}
