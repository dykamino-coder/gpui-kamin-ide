//! Дорожки сетки: имена линий, размещение, repeat(auto-fill), разбор дорожек.

use crate::style::computed::*;
use crate::style::values::value::Len;

mod auto_fill;
mod line_names;
mod track_list;
pub(super) use auto_fill::{
    auto_fill_body, auto_fill_body_tracks, auto_fill_fit_px, auto_fill_intrinsic, auto_fill_max,
    auto_fill_min, auto_fill_pct, auto_fill_tracks, auto_repeat_outside_intrinsic,
};
use line_names::line_name_tokens;
pub(super) use line_names::{
    parse_line_names, parse_named_pair, parse_named_placement, parse_placement, parse_span,
};
use track_list::tokenize_tracks;
pub(super) use track_list::{count_tracks, parse_tracks};

/// Одна дорожка сетки в терминах CSS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Px(f32),
    Fr(f32),
    /// Доля ШИРИНЫ СЕТКИ (`25%`) — не путать с долей остатка (`fr`).
    Pct(f32),
    /// Длина в единицах ШРИФТА (`2ch`, `1em`, `8rem`): кегль и метрики на
    /// разборе ещё неизвестны, величина считается вместе с прочими `em`.
    Font(Len),
    Auto,
    MinContent,
    MaxContent,
    /// Предел `fit-content(N)` — только как ВЕРХНЯЯ грань `minmax(auto, …)`:
    /// дорожка по содержимому, зажатая N (css-grid-2 §7.2.4,
    /// `fit-content( <length-percentage> )`), а не фиксированный максимум.
    FitPx(f32),
    FitPct(f32),
}

impl Track {
    /// Перевести отложенную длину в точки: единицы шрифта известны только
    /// после разрешения кегля узла.
    pub(crate) fn resolve_font_one(&mut self, family: &str, size_px: f32) {
        if let Track::Font(l) = *self {
            *self = Track::Px(crate::text::metrics::spacing_px(Some(l), family, size_px));
        }
    }
}

impl TrackSize {
    /// То же для обеих граней записи.
    pub(crate) fn resolve_font(&mut self, family: &str, size_px: f32) {
        match self {
            TrackSize::Single(t) => t.resolve_font_one(family, size_px),
            TrackSize::MinMax(a, b) => {
                a.resolve_font_one(family, size_px);
                b.resolve_font_one(family, size_px);
            }
            TrackSize::AutoRepeat { tracks, .. } => {
                for t in tracks.iter_mut() {
                    t.resolve_font(family, size_px);
                }
            }
        }
    }
}

/// Дорожка целиком: одиночная либо пара граней `minmax(a, b)`.
///
/// Обе грани нужны по-настоящему: `minmax(120px, 1fr)` — это «не уже 120, а
/// дальше забирай остаток». Сведение к одной грани меняет ширину колонки.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackSize {
    Single(Track),
    MinMax(Track, Track),
    /// `repeat(auto-fill | auto-fit, …)` — сколько дорожек влезет; при
    /// `fit` пустые схлопываются (css-grid-2 §auto-repeat). Считает это
    /// раскладка: на разборе ширины контейнера ещё нет.
    AutoRepeat {
        fit: bool,
        tracks: Vec<TrackSize>,
    },
}

/// Разрезать короткую запись сетки по косой черте ВНЕ скобок.
///
/// `grid: repeat(4, auto) / 1fr` — черта внутри `repeat()` не разделитель, и
/// резать по первой попавшейся нельзя.
pub(super) fn split_slash(v: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return (v[..i].trim(), v[i + 1..].trim()),
            _ => {}
        }
    }
    (v.trim(), "")
}

/// Убрать слово `auto-flow` (и `dense`) из стороны короткой записи сетки.
pub(super) fn strip_auto_flow(v: &str) -> &str {
    v.trim()
        .trim_start_matches("auto-flow")
        .trim()
        .trim_start_matches("dense")
        .trim()
        .trim_end_matches("dense")
        .trim()
        .trim_end_matches("auto-flow")
        .trim()
}

/// Разбор списка дорожек. `repeat(n, X)` разворачивается в n одинаковых;
/// `minmax()` сводится к своей верхней грани — нижняя у нас всегда
/// `min-content`, чего достаточно для разметки документов.
/// Повтор «сколько влезет»: `repeat(auto-fill | auto-fit, <дорожка>)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AutoRepeat {
    /// `auto-fit` — пустые дорожки схлопываются, остаток делят непустые.
    pub fit: bool,
    /// Размер дорожки в точках; `None` — дорожка по содержимому (`auto`).
    pub track: Option<f32>,
    /// Доля контейнера, если дорожка задана процентом: `repeat(auto-fill,
    /// 25%)` — четыре дорожки в трёхстах точках. В точки её переводит
    /// раскладка: на разборе ширины контейнера ещё нет
    /// (`column-auto-repeat-002`).
    pub track_pct: Option<f32>,
    /// Дорожка ПО СОДЕРЖИМОМУ (`max-content`/`min-content`/`fit-content`):
    /// число повторов задают сами элементы — по дорожке на каждого
    /// (row-auto-repeat-max-content-001).
    pub intrinsic: bool,
    /// Дорожка названа `min-content`: меряется САМЫМ УЗКИМ местом
    /// содержимого, а не самым широким.
    pub intrinsic_min: bool,
    /// Потолок `fit-content(N)`: дорожка по содержимому, но не шире N.
    pub fit_px: Option<f32>,
    /// Максимум `minmax(N, auto)`: дорожка растягивается остатком
    /// (css-grid-2 §12.8 «Stretch auto Tracks»); прежде терялся, и живые
    /// дорожки `auto-fit` оставались минимумом
    /// (`grid-content-distribution-with-collapsed-tracks-004`).
    pub max_auto: bool,
    /// Максимум `minmax(N, k fr)`: доля остатка.
    pub max_fr: Option<f32>,
    /// Сколько дорожек в ТЕЛЕ повтора: `repeat(auto-fill, fit-content(100px)
    /// fit-content(100px))` — две. Число повторов делит место на ВСЁ тело
    /// (css-grid-2 §7.2.3.2; Blink `CalculateAutomaticRepetitions`,
    /// `repeater_size`), а скалярная ветка раскладки видела одну дорожку.
    pub body: usize,
}
