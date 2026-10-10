//! Разбор `position-area`, `position-try-fallbacks`/`-order` и `position-visibility`.

/// Дорожки `position-area` по одной оси (§position-area-syntax).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaKw {
    Start,
    Center,
    End,
    SpanStart,
    SpanEnd,
    SpanAll,
}

/// Ось ключевого слова: физическая, логическая по письму, либо неясная
/// (`start`/`end`/`center`/`span-all` без оси — решается по соседу).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaAxis {
    X,
    Y,
    Block,
    Inline,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaSide {
    pub kw: AreaKw,
    pub axis: AreaAxis,
    /// `left`/`right`/`top`/`bottom` — физические стороны (без переворота
    /// письмом); прочие слова — логические.
    pub logical: bool,
    /// `self-*` — по письму самой коробки, иначе содержащего блока.
    pub self_wm: bool,
}

/// `position-area` как записана: два слова (одно дополняется по §syntax).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionArea(pub AreaSide, pub AreaSide);

/// Разбор `<position-area>` (§position-area-syntax). `none` и негодное → `None`.
pub fn parse_area(v: &str) -> Option<PositionArea> {
    fn one(w: &str) -> Option<AreaSide> {
        let w = w.to_ascii_lowercase();
        let (span, rest) = match w.strip_prefix("span-") {
            Some(r) => (true, r),
            None => (false, w.as_str()),
        };
        let any = |kw| {
            Some(AreaSide {
                kw,
                axis: AreaAxis::Any,
                logical: true,
                self_wm: false,
            })
        };
        match (span, rest) {
            (true, "all") => return any(AreaKw::SpanAll),
            (false, "center") => return any(AreaKw::Center),
            (_, "all") | (_, "center") => return None,
            _ => {}
        }
        let (self_wm, rest) = match rest.strip_prefix("self-") {
            Some(r) => (true, r),
            None => (false, rest),
        };
        let (axis, end, logical) = match rest {
            "left" if !self_wm => (AreaAxis::X, false, false),
            "right" if !self_wm => (AreaAxis::X, true, false),
            "top" if !self_wm => (AreaAxis::Y, false, false),
            "bottom" if !self_wm => (AreaAxis::Y, true, false),
            "x-start" => (AreaAxis::X, false, true),
            "x-end" => (AreaAxis::X, true, true),
            "y-start" => (AreaAxis::Y, false, true),
            "y-end" => (AreaAxis::Y, true, true),
            "block-start" => (AreaAxis::Block, false, true),
            "block-end" => (AreaAxis::Block, true, true),
            "inline-start" => (AreaAxis::Inline, false, true),
            "inline-end" => (AreaAxis::Inline, true, true),
            "start" => (AreaAxis::Any, false, true),
            "end" => (AreaAxis::Any, true, true),
            _ => return None,
        };
        let kw = match (span, end) {
            (false, false) => AreaKw::Start,
            (false, true) => AreaKw::End,
            (true, false) => AreaKw::SpanStart,
            (true, true) => AreaKw::SpanEnd,
        };
        Some(AreaSide {
            kw,
            axis,
            logical,
            self_wm,
        })
    }
    if v.trim() == "none" {
        return None;
    }
    let words: Vec<&str> = v.split_whitespace().collect();
    let (a, b) = match words.as_slice() {
        // Одно слово: с ясной осью второе — `span-all`, иначе повтор.
        [a] => {
            let a = one(a)?;
            let b = if a.axis == AreaAxis::Any {
                a
            } else {
                AreaSide {
                    kw: AreaKw::SpanAll,
                    axis: AreaAxis::Any,
                    logical: true,
                    self_wm: false,
                }
            };
            (a, b)
        }
        [a, b] => (one(a)?, one(b)?),
        _ => return None,
    };
    Some(PositionArea(a, b))
}

/// Один вариант `position-try-fallbacks` (§position-try-fallbacks): имя
/// `@position-try`-правила и/или тактика (биты `FLIP_*`), либо готовая
/// `<position-area>`.
#[derive(Clone, Debug, PartialEq)]
pub struct TryFallback {
    pub name: Option<String>,
    pub tactics: u8,
    pub area: Option<PositionArea>,
}

pub const FLIP_BLOCK: u8 = 1;

pub const FLIP_INLINE: u8 = 2;

pub const FLIP_START: u8 = 4;

/// `none | [ [<dashed-ident> || <try-tactic>] | <position-area> ]#`;
/// негодный элемент списка отбрасывается, `flip-x`/`flip-y` (физические)
/// пока не поддерживаются.
pub fn parse_try_fallbacks(v: &str) -> Vec<TryFallback> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    v.split(',')
        .filter_map(|item| {
            let item = item.trim();
            let mut name = None;
            let mut tactics = 0u8;
            let mut plain = true;
            for w in item.split_whitespace() {
                match w.to_ascii_lowercase().as_str() {
                    "flip-block" => tactics |= FLIP_BLOCK,
                    "flip-inline" => tactics |= FLIP_INLINE,
                    "flip-start" => tactics |= FLIP_START,
                    _ if w.starts_with("--") && name.is_none() => name = Some(w.to_string()),
                    _ => plain = false,
                }
            }
            if plain && (name.is_some() || tactics != 0) {
                return Some(TryFallback {
                    name,
                    tactics,
                    area: None,
                });
            }
            parse_area(item).map(|area| TryFallback {
                name: None,
                tactics: 0,
                area: Some(area),
            })
        })
        .collect()
}

/// `position-try-order`: 0 normal, 1 most-width, 2 most-height,
/// 3 most-block-size, 4 most-inline-size; прочее — негодно.
pub fn parse_try_order(w: &str) -> Option<u8> {
    Some(match w.to_ascii_lowercase().as_str() {
        "normal" => 0,
        "most-width" => 1,
        "most-height" => 2,
        "most-block-size" => 3,
        "most-inline-size" => 4,
        _ => return None,
    })
}

pub const VIS_VALID: u8 = 1;

pub const VIS_VISIBLE: u8 = 2;

pub const VIS_NO_OVERFLOW: u8 = 4;

/// `position-visibility`: `always | [anchor-valid || anchor-visible ||
/// no-overflow]`; легаси `anchors-*` — псевдонимы (§position-visibility).
pub fn parse_visibility(v: &str) -> u8 {
    let mut bits = 0u8;
    for w in v.split_whitespace() {
        bits |= match w.to_ascii_lowercase().as_str() {
            "always" => return 0,
            "anchor-valid" | "anchors-valid" => VIS_VALID,
            "anchor-visible" | "anchors-visible" => VIS_VISIBLE,
            "no-overflow" => VIS_NO_OVERFLOW,
            _ => return 0,
        };
    }
    bits
}
