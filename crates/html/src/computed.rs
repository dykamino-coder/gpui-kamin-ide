//! Вычисленный стиль узла: что получилось после каскада, до применения к GPUI.
//!
//! Промежуточная структура нужна по двум причинам. Во-первых, её видно в
//! тестах без окна и рендера — а `gpui::Style` собрать в тесте нельзя.
//! Во-вторых, ровно она задаёт границу охвата: поле есть — свойство
//! поддержано, поля нет — свойство игнорируется осознанно, а не потеряно.

use crate::css::{Decls, Rule};
use crate::value::{Color, Len};

/// Четыре стороны: `top right bottom left`, как в CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides {
    pub top: Option<Len>,
    pub right: Option<Len>,
    pub bottom: Option<Len>,
    pub left: Option<Len>,
}

impl Sides {
    /// Раскрытие сокращённой записи: 1 значение — все стороны, 2 — верт/гориз,
    /// 3 — верх/гориз/низ, 4 — по часовой.
    fn shorthand(raw: &str) -> Sides {
        let v: Vec<Option<Len>> = raw.split_whitespace().map(Len::parse).collect();
        match v.len() {
            1 => Sides {
                top: v[0],
                right: v[0],
                bottom: v[0],
                left: v[0],
            },
            2 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[0],
                left: v[1],
            },
            3 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[1],
            },
            4 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[3],
            },
            _ => Sides::default(),
        }
    }
}

/// Четыре угла скругления.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: Option<Len>,
    pub tr: Option<Len>,
    pub br: Option<Len>,
    pub bl: Option<Len>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Block,
    /// `inline-block`: коробка со своими размерами, но стоящая В СТРОКЕ.
    /// Отдельный вариант нужен потому, что раньше он схлопывался в `Block` и
    /// два таких элемента вставали друг под друга вместо одной строки.
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexDir {
    Row,
    RowReverse,
    Col,
    ColReverse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Justify {
    Start,
    Center,
    End,
    Between,
    Around,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
}

/// Тень: GPUI умеет несколько внешних теней, поэтому храним список.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

/// Линейный градиент. GPUI берёт ровно два стопа, поэтому крайние цвета —
/// это всё, что доедет; промежуточные отбрасываем осознанно.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub angle_deg: f32,
    pub from: Color,
    pub to: Color,
}

#[derive(Clone, Debug, Default)]
pub struct Computed {
    pub display: Option<Display>,
    pub flex_dir: Option<FlexDir>,
    pub flex_wrap: Option<bool>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub align_items: Option<Align>,
    /// `align-self` — про сам элемент; отдельное поле, иначе он выравнивал
    /// бы своих детей вместо себя.
    pub align_self: Option<Align>,
    pub flex_basis: Option<Len>,
    pub justify_content: Option<Justify>,
    pub gap: Option<(Option<Len>, Option<Len>)>,
    /// `box-sizing`. По умолчанию в CSS — `content-box`: заданная ширина
    /// НЕ включает отступы и рамку. Движок раскладки под нами всегда считает
    /// по `border-box`, поэтому разницу приходится компенсировать вручную.
    pub border_box: Option<bool>,
    pub grid_cols: Option<u16>,
    /// Список дорожек, если он выразим: `auto`, `1fr`, px, `minmax()`.
    pub grid_tracks: Option<Vec<Track>>,

    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<Len>,
    pub min_height: Option<Len>,
    pub max_width: Option<Len>,
    pub max_height: Option<Len>,

    pub padding: Sides,
    pub margin: Sides,
    pub border_width: Sides,
    pub border_color: Option<Color>,
    pub radius: Corners,

    pub position: Option<Position>,
    pub inset: Sides,
    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    pub opacity: Option<f32>,

    pub background: Option<Color>,
    pub gradient: Option<Gradient>,
    pub shadows: Vec<Shadow>,

    pub color: Option<Color>,
    pub font_size: Option<Len>,
    pub font_weight: Option<u16>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub line_through: Option<bool>,
    pub line_height: Option<Len>,
    pub text_align: Option<TextAlign>,
    pub nowrap: Option<bool>,
    /// Переводы строк значимы (`white-space: pre*`).
    pub preserve_newlines: Option<bool>,
    pub monospace: Option<bool>,
    /// `border-color: currentColor` — цвет берётся из `color` того же узла.
    pub border_color_is_current: bool,
    /// Форма курсора: у GPUI набор совпадает с CSS почти буква в букву.
    pub cursor: Option<String>,
    /// `visibility: hidden` — место занимает, но не рисуется.
    pub hidden: Option<bool>,
    pub letter_spacing: Option<Len>,
    pub ellipsis: Option<bool>,
    /// `list-style: none` — навигация, свёрстанная на списках, иначе идёт с
    /// точками.
    pub no_marker: Option<bool>,
    pub object_fit: Option<String>,
}

impl Computed {
    /// Собрать стиль узла: правила таблицы (по специфичности), затем `style=""`.
    pub fn resolve(matched: &mut Vec<&Rule>, inline: &Decls) -> Computed {
        Computed::resolve_with_vars(matched, inline, &Decls::new())
    }

    /// То же с переменными темы.
    pub fn resolve_with_vars(matched: &mut Vec<&Rule>, inline: &Decls, vars: &Decls) -> Computed {
        matched.sort_by_key(|r| (r.sel.specificity(), r.order));
        let mut c = Computed::default();
        for rule in matched.iter() {
            c.apply_decls_with_vars(&rule.decls, vars);
        }
        c.apply_decls_with_vars(inline, vars);
        // `currentColor` в рамке и фоне значит «цвет текста этого элемента» —
        // подставляем уже после того, как цвет стал известен.
        if c.border_color_is_current {
            c.border_color = c.color;
        }
        c
    }

    pub fn apply_decls(&mut self, d: &Decls) {
        for (k, v) in d {
            self.apply_one(k, v);
        }
    }

    /// То же, но со словарём переменных: `var(--x)` подставляется значением.
    ///
    /// Без этого современные темы не работают вовсе — они целиком построены на
    /// переменных, и каждое такое объявление молча терялось.
    pub fn apply_decls_with_vars(&mut self, d: &Decls, vars: &Decls) {
        for (k, v) in d {
            if k.starts_with("--") {
                continue;
            }
            let resolved = resolve_vars(v, vars);
            self.apply_one(k, &resolved);
        }
    }

    fn apply_one(&mut self, key: &str, val: &str) {
        let v = val.trim();
        match key {
            "box-sizing" => self.border_box = Some(v == "border-box"),
            "display" => {
                self.display = match v {
                    "flex" => Some(Display::Flex),
                    "inline-flex" => Some(Display::InlineFlex),
                    "grid" => Some(Display::Grid),
                    "none" => Some(Display::None),
                    "block" => Some(Display::Block),
                    "inline-block" => Some(Display::InlineBlock),
                    "inline" => Some(Display::InlineBlock),
                    _ => self.display,
                }
            }
            "flex-direction" => {
                self.flex_dir = match v {
                    "row" => Some(FlexDir::Row),
                    "row-reverse" => Some(FlexDir::RowReverse),
                    "column" => Some(FlexDir::Col),
                    "column-reverse" => Some(FlexDir::ColReverse),
                    _ => self.flex_dir,
                }
            }
            "flex-wrap" => self.flex_wrap = Some(v == "wrap" || v == "wrap-reverse"),
            "flex-grow" => self.flex_grow = v.parse().ok(),
            "flex-shrink" => self.flex_shrink = v.parse().ok(),
            // `flex: 1` — сокращение для grow/shrink/basis; берём первое число.
            "flex" => {
                if let Some(g) = v
                    .split_whitespace()
                    .next()
                    .and_then(|p| p.parse::<f32>().ok())
                {
                    self.flex_grow = Some(g);
                }
            }
            "flex-basis" => self.flex_basis = Len::parse(v),
            "align-self" => {
                self.align_self = match v {
                    "center" => Some(Align::Center),
                    "flex-start" | "start" => Some(Align::Start),
                    "flex-end" | "end" => Some(Align::End),
                    "stretch" => Some(Align::Stretch),
                    "baseline" => Some(Align::Baseline),
                    _ => self.align_self,
                }
            }
            "align-items" => {
                self.align_items = match v {
                    "center" => Some(Align::Center),
                    "flex-start" | "start" => Some(Align::Start),
                    "flex-end" | "end" => Some(Align::End),
                    "stretch" => Some(Align::Stretch),
                    "baseline" => Some(Align::Baseline),
                    _ => self.align_items,
                }
            }
            "justify-content" => {
                self.justify_content = match v {
                    "center" => Some(Justify::Center),
                    "flex-start" | "start" => Some(Justify::Start),
                    "flex-end" | "end" => Some(Justify::End),
                    "space-between" => Some(Justify::Between),
                    "space-around" | "space-evenly" => Some(Justify::Around),
                    _ => self.justify_content,
                }
            }
            "gap" => {
                let parts: Vec<Option<Len>> = v.split_whitespace().map(Len::parse).collect();
                self.gap = match parts.len() {
                    1 => Some((parts[0], parts[0])),
                    2 => Some((parts[0], parts[1])),
                    _ => self.gap,
                };
            }
            "row-gap" => self.gap = Some((Len::parse(v), self.gap.and_then(|g| g.1))),
            "column-gap" => self.gap = Some((self.gap.and_then(|g| g.0), Len::parse(v))),
            "grid-template-columns" => {
                self.grid_cols = count_tracks(v);
                self.grid_tracks = parse_tracks(v);
            }

            "width" => self.width = Len::parse(v),
            "height" => self.height = Len::parse(v),
            "min-width" => self.min_width = Len::parse(v),
            "min-height" => self.min_height = Len::parse(v),
            "max-width" => self.max_width = Len::parse(v),
            "max-height" => self.max_height = Len::parse(v),

            "padding" => self.padding = Sides::shorthand(v),
            "padding-top" => self.padding.top = Len::parse(v),
            "padding-right" => self.padding.right = Len::parse(v),
            "padding-bottom" => self.padding.bottom = Len::parse(v),
            "padding-left" => self.padding.left = Len::parse(v),
            "margin" => self.margin = Sides::shorthand(v),
            "margin-top" => self.margin.top = Len::parse(v),
            "margin-right" => self.margin.right = Len::parse(v),
            "margin-bottom" => self.margin.bottom = Len::parse(v),
            "margin-left" => self.margin.left = Len::parse(v),

            "border" => self.apply_border_shorthand(v, None),
            "border-top" => self.apply_border_shorthand(v, Some(0)),
            "border-right" => self.apply_border_shorthand(v, Some(1)),
            "border-bottom" => self.apply_border_shorthand(v, Some(2)),
            "border-left" => self.apply_border_shorthand(v, Some(3)),
            "border-width" => self.border_width = Sides::shorthand(v),
            "border-color" => {
                if v.eq_ignore_ascii_case("currentcolor") {
                    self.border_color_is_current = true;
                } else {
                    self.border_color = Color::parse(v);
                }
            }
            "border-radius" => self.radius = radius_shorthand(v),
            "border-top-left-radius" => self.radius.tl = Len::parse(v),
            "border-top-right-radius" => self.radius.tr = Len::parse(v),
            "border-bottom-right-radius" => self.radius.br = Len::parse(v),
            "border-bottom-left-radius" => self.radius.bl = Len::parse(v),

            "position" => {
                self.position = match v {
                    "absolute" => Some(Position::Absolute),
                    "relative" => Some(Position::Relative),
                    "static" => Some(Position::Static),
                    // `fixed` и `sticky` раньше молча подменялись на
                    // `absolute`/`relative`. Это выглядело как поддержка, а
                    // вело себя иначе: фиксированный элемент уезжал вместе с
                    // прокруткой, липкий переставал липнуть. Честнее не
                    // применять ничего — расхождение видно сразу.
                    "fixed" | "sticky" => self.position,
                    _ => self.position,
                }
            }
            "top" => self.inset.top = Len::parse(v),
            "right" => self.inset.right = Len::parse(v),
            "bottom" => self.inset.bottom = Len::parse(v),
            "left" => self.inset.left = Len::parse(v),
            "inset" => self.inset = Sides::shorthand(v),
            "overflow" => {
                let o = parse_overflow(v);
                self.overflow_x = o;
                self.overflow_y = o;
            }
            "overflow-x" => self.overflow_x = parse_overflow(v),
            "overflow-y" => self.overflow_y = parse_overflow(v),
            "opacity" => self.opacity = v.parse().ok(),

            "background" | "background-color" => {
                if v.starts_with("linear-gradient(") {
                    self.gradient = parse_linear_gradient(v);
                } else {
                    self.background = Color::parse(v);
                }
            }
            "box-shadow" => self.shadows = parse_shadows(v),

            "color" => self.color = Color::parse(v),
            "font-size" => self.font_size = Len::parse(v),
            "font-weight" => {
                self.font_weight = match v {
                    "bold" | "bolder" => Some(700),
                    "normal" => Some(400),
                    n => n.parse().ok(),
                }
            }
            "font-style" => self.italic = Some(v == "italic" || v == "oblique"),
            "font-family" => {
                let lower = v.to_ascii_lowercase();
                // Семейство целиком не переносим: шрифты приложения фиксированы.
                // Различаем только моноширинный запрос — он несёт смысл (код).
                self.monospace = Some(lower.contains("mono") || lower.contains("courier"));
            }
            "text-decoration" | "text-decoration-line" => {
                self.underline = Some(v.contains("underline"));
                self.line_through = Some(v.contains("line-through"));
            }
            "line-height" => {
                // Голое число в line-height — множитель, а не пиксели.
                self.line_height = match v.parse::<f32>() {
                    Ok(mult) if !v.ends_with("px") => Some(Len::Pct(mult)),
                    _ => Len::parse(v),
                }
            }
            "text-align" => {
                self.text_align = match v {
                    "center" => Some(TextAlign::Center),
                    "right" | "end" => Some(TextAlign::Right),
                    "left" | "start" => Some(TextAlign::Left),
                    _ => self.text_align,
                }
            }
            // `pre` сохраняет переводы строк — это не то же самое, что запрет
            // переноса: раньше `pre` помечался как `nowrap`, и текст склеивался
            // в одну строку.
            "cursor" => self.cursor = Some(v.to_string()),
            "visibility" => self.hidden = Some(v == "hidden" || v == "collapse"),
            "letter-spacing" => self.letter_spacing = Len::parse(v),
            "text-overflow" => self.ellipsis = Some(v == "ellipsis"),
            "list-style" | "list-style-type" => self.no_marker = Some(v.contains("none")),
            "object-fit" => self.object_fit = Some(v.to_string()),
            "white-space" => {
                self.nowrap = Some(v == "nowrap");
                self.preserve_newlines = Some(matches!(
                    v,
                    "pre" | "pre-wrap" | "pre-line" | "break-spaces"
                ));
            }
            _ => {}
        }
    }

    /// `border: 1px solid #333` — ширина и цвет; стиль линии GPUI различает
    /// только solid/dashed на весь элемент, поэтому его не разбираем.
    fn apply_border_shorthand(&mut self, v: &str, side: Option<usize>) {
        let mut width = None;
        let mut color = None;
        for token in v.split_whitespace() {
            if token == "none" {
                width = Some(Len::Px(0.0));
            } else if let Some(l) = Len::parse(token) {
                width = Some(l);
            } else if let Some(c) = Color::parse(token) {
                color = Some(c);
            }
        }
        if let Some(c) = color {
            self.border_color = Some(c);
        }
        let Some(w) = width else { return };
        match side {
            None => {
                self.border_width = Sides {
                    top: Some(w),
                    right: Some(w),
                    bottom: Some(w),
                    left: Some(w),
                }
            }
            Some(0) => self.border_width.top = Some(w),
            Some(1) => self.border_width.right = Some(w),
            Some(2) => self.border_width.bottom = Some(w),
            Some(3) => self.border_width.left = Some(w),
            _ => {}
        }
    }
}

fn parse_overflow(v: &str) -> Option<Overflow> {
    match v {
        "hidden" | "clip" => Some(Overflow::Hidden),
        "scroll" | "auto" => Some(Overflow::Scroll),
        "visible" => Some(Overflow::Visible),
        _ => None,
    }
}

fn radius_shorthand(raw: &str) -> Corners {
    let v: Vec<Option<Len>> = raw.split_whitespace().map(Len::parse).collect();
    match v.len() {
        1 => Corners {
            tl: v[0],
            tr: v[0],
            br: v[0],
            bl: v[0],
        },
        2 => Corners {
            tl: v[0],
            tr: v[1],
            br: v[0],
            bl: v[1],
        },
        3 => Corners {
            tl: v[0],
            tr: v[1],
            br: v[2],
            bl: v[1],
        },
        4 => Corners {
            tl: v[0],
            tr: v[1],
            br: v[2],
            bl: v[3],
        },
        _ => Corners::default(),
    }
}

/// Подстановка `var(--x)` и `var(--x, запасное)`.
fn resolve_vars(value: &str, vars: &Decls) -> String {
    if !value.contains("var(") {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find("var(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 4..];
        let Some(close) = after.find(')') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let inner = &after[..close];
        let (name, fallback) = match inner.split_once(',') {
            Some((n, f)) => (n.trim(), f.trim()),
            None => (inner.trim(), ""),
        };
        match vars.get(name) {
            Some(v) => out.push_str(v),
            None => out.push_str(fallback),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Одна дорожка сетки в терминах CSS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Px(f32),
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
}

/// Разбор списка дорожек. `repeat(n, X)` разворачивается в n одинаковых;
/// `minmax()` сводится к своей верхней грани — нижняя у нас всегда
/// `min-content`, чего достаточно для разметки документов.
fn parse_tracks(v: &str) -> Option<Vec<Track>> {
    let one = |t: &str| -> Option<Track> {
        let t = t.trim();
        if t == "auto" {
            return Some(Track::Auto);
        }
        if t == "min-content" {
            return Some(Track::MinContent);
        }
        if t == "max-content" {
            return Some(Track::MaxContent);
        }
        if let Some(fr) = t.strip_suffix("fr") {
            return fr.trim().parse().ok().map(Track::Fr);
        }
        if let Some(inner) = t.strip_prefix("minmax(").and_then(|s| s.strip_suffix(')')) {
            let upper = inner.split(',').nth(1)?.trim().to_string();
            return parse_tracks(&upper)?.into_iter().next();
        }
        match Len::parse(t) {
            Some(Len::Px(px)) => Some(Track::Px(px)),
            _ => None,
        }
    };
    if let Some(inner) = v
        .trim()
        .strip_prefix("repeat(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let mut it = inner.splitn(2, ',');
        let n: usize = it.next()?.trim().parse().ok()?;
        let track = one(it.next()?)?;
        return Some(vec![track; n]);
    }
    // Разрезаем по пробелам, не заходя внутрь скобок: `minmax(120px, auto)` —
    // одна дорожка, а не две половинки.
    let list: Vec<Track> = tokenize_shadow(v).iter().filter_map(|t| one(t)).collect();
    (!list.is_empty()).then_some(list)
}

/// Число колонок в `grid-template-columns`: и `repeat(3, 1fr)`, и `1fr 1fr`.
fn count_tracks(v: &str) -> Option<u16> {
    if let Some(inner) = v.strip_prefix("repeat(").and_then(|s| s.strip_suffix(')')) {
        return inner.split(',').next()?.trim().parse().ok();
    }
    let n = v.split_whitespace().count();
    (n > 0).then_some(n as u16)
}

/// `linear-gradient(90deg, #000, #fff)`. Направления словами приводим к углу.
fn parse_linear_gradient(v: &str) -> Option<Gradient> {
    let inner = v.strip_prefix("linear-gradient(")?.strip_suffix(')')?;
    let parts: Vec<&str> = crate::css::split_args(inner);
    if parts.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    let angle = match parts[0].trim() {
        a if a.ends_with("deg") => {
            idx = 1;
            a.trim_end_matches("deg").trim().parse().unwrap_or(180.0)
        }
        "to right" => {
            idx = 1;
            90.0
        }
        "to left" => {
            idx = 1;
            270.0
        }
        "to bottom" => {
            idx = 1;
            180.0
        }
        "to top" => {
            idx = 1;
            0.0
        }
        "to bottom right" | "to right bottom" => {
            idx = 1;
            135.0
        }
        "to top right" | "to right top" => {
            idx = 1;
            45.0
        }
        _ => 180.0,
    };
    // Стоп может нести позицию (`#fff 20%`) — берём только цвет.
    let stops: Vec<Color> = parts[idx..]
        .iter()
        .filter_map(|p| Color::parse(p.split_whitespace().next().unwrap_or("")))
        .collect();
    match stops.len() {
        0 | 1 => None,
        _ => Some(Gradient {
            angle_deg: angle,
            from: stops[0],
            to: *stops.last().unwrap(),
        }),
    }
}

/// `box-shadow: 0 2px 8px rgba(0,0,0,.4), inset 0 0 2px red`.
/// `inset` пропускаем: внутренних теней в GPUI нет, и нарисовать их внешней —
/// значит соврать.
fn parse_shadows(v: &str) -> Vec<Shadow> {
    crate::css::split_args(v)
        .iter()
        .filter(|s| !s.contains("inset"))
        .filter_map(|s| {
            let mut lens = vec![];
            let mut color = None;
            for token in tokenize_shadow(s) {
                if let Some(Len::Px(px)) = Len::parse(&token) {
                    lens.push(px);
                } else if let Some(c) = Color::parse(&token) {
                    color = Some(c);
                }
            }
            if lens.len() < 2 {
                return None;
            }
            Some(Shadow {
                x: lens[0],
                y: lens[1],
                blur: lens.get(2).copied().unwrap_or(0.0),
                spread: lens.get(3).copied().unwrap_or(0.0),
                color: color.unwrap_or(Color {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.35,
                }),
            })
        })
        .collect()
}

/// Разбиение тени на токены: `rgba(0, 0, 0, .4)` — один токен, а не четыре.
fn tokenize_shadow(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch)
            }
            ')' => {
                depth -= 1;
                cur.push(ch)
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::parse_decls;

    fn computed(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn shorthand_sides_expand_like_css() {
        let c = computed("padding: 4px 8px");
        assert_eq!(c.padding.top, Some(Len::Px(4.0)));
        assert_eq!(c.padding.right, Some(Len::Px(8.0)));
        assert_eq!(c.padding.bottom, Some(Len::Px(4.0)));
        assert_eq!(c.padding.left, Some(Len::Px(8.0)));

        let c = computed("margin: 1px 2px 3px 4px");
        assert_eq!(c.margin.bottom, Some(Len::Px(3.0)));
        assert_eq!(c.margin.left, Some(Len::Px(4.0)));
    }

    #[test]
    fn border_shorthand_takes_width_and_color() {
        let c = computed("border: 2px solid #ff0000");
        assert_eq!(c.border_width.top, Some(Len::Px(2.0)));
        assert_eq!(c.border_color.map(|c| c.r), Some(1.0));

        let c = computed("border-left: 3px solid teal");
        assert_eq!(c.border_width.left, Some(Len::Px(3.0)));
        assert_eq!(
            c.border_width.top, None,
            "боковая запись не трогает другие стороны"
        );
    }

    #[test]
    fn line_height_number_is_a_multiplier() {
        assert_eq!(
            computed("line-height: 1.5").line_height,
            Some(Len::Pct(1.5))
        );
        assert_eq!(
            computed("line-height: 20px").line_height,
            Some(Len::Px(20.0))
        );
    }

    #[test]
    fn gradient_direction_words_become_angles() {
        let g = computed("background: linear-gradient(to right, #000, #fff)")
            .gradient
            .unwrap();
        assert_eq!(g.angle_deg, 90.0);
        assert_eq!(g.from.r, 0.0);
        assert_eq!(g.to.r, 1.0);

        let g = computed("background: linear-gradient(45deg, red 10%, blue 90%)")
            .gradient
            .unwrap();
        assert_eq!(g.angle_deg, 45.0);
    }

    #[test]
    fn shadows_split_and_skip_inset() {
        let s = computed("box-shadow: 0 2px 8px rgba(0, 0, 0, .4), inset 0 0 2px red").shadows;
        assert_eq!(s.len(), 1, "inset-тень отбрасывается, её нечем рисовать");
        assert_eq!(s[0].y, 2.0);
        assert_eq!(s[0].blur, 8.0);
        assert!((s[0].color.a - 0.4).abs() < 0.01);
    }

    #[test]
    fn grid_tracks_are_counted_both_ways() {
        assert_eq!(
            computed("grid-template-columns: repeat(3, 1fr)").grid_cols,
            Some(3)
        );
        assert_eq!(
            computed("grid-template-columns: 1fr 1fr").grid_cols,
            Some(2)
        );
    }

    #[test]
    fn track_list_keeps_the_kind_of_each_track() {
        let t = computed("grid-template-columns: 120px auto 1fr")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![Track::Px(120.0), Track::Auto, Track::Fr(1.0)]);
    }

    #[test]
    fn repeat_expands_into_equal_tracks() {
        let t = computed("grid-template-columns: repeat(3, 1fr)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![Track::Fr(1.0); 3]);
    }

    #[test]
    fn minmax_collapses_to_its_upper_bound() {
        // Нижняя грань всегда `min-content` — её ставит слой применения,
        // поэтому из записи достаточно забрать верхнюю.
        let t = computed("grid-template-columns: minmax(120px, max-content)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![Track::MaxContent]);
    }

    #[test]
    fn content_sized_tracks_are_recognised() {
        let t = computed("grid-template-columns: min-content max-content")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![Track::MinContent, Track::MaxContent]);
    }

    #[test]
    fn cascade_order_specificity_then_inline() {
        let rules = crate::css::parse_stylesheet(".a { color: red } div.a { color: blue }");
        let mut matched: Vec<&crate::css::Rule> = rules.iter().collect();
        let c = Computed::resolve(&mut matched, &parse_decls("color: green"));
        assert_eq!(c.color.map(|c| c.g), Some(0.5019608), "инлайн бьёт таблицу");

        let mut matched: Vec<&crate::css::Rule> = rules.iter().collect();
        let c = Computed::resolve(&mut matched, &Decls::new());
        assert_eq!(
            c.color.map(|c| c.b),
            Some(1.0),
            "выше специфичность — тот и выигрывает"
        );
    }
}
