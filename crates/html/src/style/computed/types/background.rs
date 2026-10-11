//! Типы фона: background-clip/size/position (с разбором слов положения), background-repeat и укладка.

use super::*;

/// `background-clip`: до какого края коробки красится фон
/// (css-backgrounds-3 §3.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BgClip {
    /// До внешнего края рамки. Умолчание `background-clip`, поэтому там оно
    /// же выражается отсутствием значения; у `background-origin` — нет.
    BorderBox,
    /// До внутреннего края рамки.
    PaddingBox,
    /// До края содержимого — внутрь ещё и на поля.
    ContentBox,
    /// По форме текста (css-backgrounds-4). Сплошную непрозрачную заливку
    /// несёт цвет глифов (`background::text_clip_fill`); узорный фон не
    /// рисуется вовсе — маски глифов нет.
    Text,
    /// По области, которую красит рамка (css-backgrounds-4 `border-area`).
    /// Сплошную заливку несёт краска рамки (`background::border_paint`);
    /// узорный фон красит border-box, как прежде.
    BorderArea,
}

/// `background-size`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BgSize {
    #[default]
    Auto,
    Cover,
    Contain,
    /// Явные размеры; `None` по оси — «тяни пропорционально».
    Fixed(Option<Len>, Option<Len>),
}

/// `background-position`: смещение первой плитки.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BgPos {
    pub x: Option<Len>,
    pub y: Option<Len>,
}

/// Разбор позиции пары слов/длин: ключевые слова НЕСУТ СВОЮ ОСЬ
/// (css-backgrounds-3 §3.6): `bottom center` и `center bottom` — одно и то
/// же. Длины и `center` ложатся по порядку в свободные оси; одно значение
/// задаёт свою ось, вторая — по центру.
pub fn parse_pos_words(v: &str) -> BgPos {
    // Процентная смесь `calc(50px + 50%)` доживает до растра: смещение
    // плитки складывает `доля × свободное место + точки` (`background::origin`,
    // css-values-4 §10.9 — именно `background-position` спека приводит
    // примером «preserves the percentage in a calc()»).
    // ★ НЕ ДЕЛАТЬ: сворачивать `min()`/`max()` из одних процентов
    // при разборе. Сравниваются РАЗРЕШЁННЫЕ длины, а база доли — свободное
    // место, и при картинке больше коробки она отрицательна: `min(0%, 100%)`
    // там равно 100% (`background-position-calc-minmax-001` ловит именно это).
    let length = |t: &str| -> Option<Len> { Len::parse_mixed(t) };
    let is_kw = |t: &str| matches!(t, "left" | "right" | "top" | "bottom" | "center");
    // Список слоёв (`a, b`): здесь — позиция ПЕРВОГО слоя, как и картинка,
    // которую берёт разбор фона.
    let first = crate::style::css::split_args(v)
        .into_iter()
        .next()
        .unwrap_or_default();
    let tokens = split_outside_parens(first.trim());
    // Форма из трёх-четырёх значений (css-backgrounds-3 §3.6): ключевое слово
    // края с СМЕЩЕНИЕМ от него — `right 10px top 20%`. Прежде смещение
    // терялось, и `right 100%` давало правый край вместо левого
    // (`background-position-right-in-body`, `-three-four-values`).
    if tokens.len() >= 3 {
        let mut pairs: Vec<(String, Option<Len>)> = vec![];
        let mut i = 0;
        while i < tokens.len() {
            let t = tokens[i].as_str();
            if !is_kw(t) {
                return BgPos {
                    x: Some(Len::Pct(0.5)),
                    y: Some(Len::Pct(0.5)),
                };
            }
            let off = tokens
                .get(i + 1)
                .filter(|n| !is_kw(n.as_str()))
                .and_then(|n| length(n));
            i += if off.is_some() { 2 } else { 1 };
            pairs.push((t.to_string(), off));
        }
        // Смещение от дальнего края: `100% - смещение`.
        let from_end = |off: Option<Len>| -> Option<Len> {
            match off {
                None => Some(Len::Pct(1.0)),
                Some(Len::Pct(k)) => Some(Len::Pct(1.0 - k)),
                Some(Len::Px(px)) => Len::parse_mixed(&format!("calc(100% - {px}px)")),
                Some(other) => Some(other),
            }
        };
        let (mut x, mut y) = (None, None);
        for (kw, off) in pairs {
            match kw.as_str() {
                "left" => x = Some(off.unwrap_or(Len::Pct(0.0))),
                "right" => x = from_end(off),
                "top" => y = Some(off.unwrap_or(Len::Pct(0.0))),
                "bottom" => y = from_end(off),
                // `center` оставляет свою ось серединой (умолчание ниже).
                _ => {}
            }
        }
        return BgPos {
            x: x.or(Some(Len::Pct(0.5))),
            y: y.or(Some(Len::Pct(0.5))),
        };
    }
    let word = |t: &str| -> Option<Len> {
        match t {
            "left" | "top" => Some(Len::Pct(0.0)),
            "center" => Some(Len::Pct(0.5)),
            "right" | "bottom" => Some(Len::Pct(1.0)),
            other => length(other),
        }
    };
    let mut x: Option<Len> = None;
    let mut y: Option<Len> = None;
    let mut free: Vec<Option<Len>> = vec![];
    // Резка ВНЕ скобок: по пробелам `calc(50px + 50%)` рассыпался на три
    // слова, и позиция падала в центр.
    for t in tokens {
        match t.as_str() {
            "left" | "right" => x = word(&t),
            "top" | "bottom" => y = word(&t),
            other => free.push(word(other)),
        }
    }
    let mut free = free.into_iter();
    if x.is_none() {
        x = free.next().flatten();
    }
    if y.is_none() {
        y = free.next().flatten();
    }
    BgPos {
        x: x.or(Some(Len::Pct(0.5))),
        y: y.or(Some(Len::Pct(0.5))),
    }
}

/// `background-repeat`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BgRepeat {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
    /// Своя укладка по каждой оси: `background-repeat: round space`.
    Axes(Tiling, Tiling),
}

impl BgRepeat {
    pub fn x(self) -> bool {
        self.axis(true) == Tiling::Repeat
    }
    pub fn y(self) -> bool {
        self.axis(false) == Tiling::Repeat
    }

    /// Укладка вдоль оси: `true` — вдоль строки (горизонталь).
    pub fn axis(self, horizontal: bool) -> Tiling {
        match (self, horizontal) {
            (BgRepeat::Axes(x, _), true) => x,
            (BgRepeat::Axes(_, y), false) => y,
            (BgRepeat::Repeat, _) => Tiling::Repeat,
            (BgRepeat::NoRepeat, _) => Tiling::None,
            (BgRepeat::RepeatX, true) | (BgRepeat::RepeatY, false) => Tiling::Repeat,
            (BgRepeat::RepeatX, false) | (BgRepeat::RepeatY, true) => Tiling::None,
        }
    }
}

/// Укладка плиток вдоль ОДНОЙ оси (css-backgrounds-3 §3.4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tiling {
    /// Плитки идут подряд, крайние обрезаются краем коробки.
    #[default]
    Repeat,
    /// Одна плитка.
    None,
    /// Целое число плиток, остаток раздан РАВНЫМИ зазорами между ними.
    Space,
    /// Плитка растянута так, чтобы целое их число заняло коробку без зазоров.
    Round,
}
