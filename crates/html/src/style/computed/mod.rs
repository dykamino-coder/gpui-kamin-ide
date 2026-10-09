//! Вычисленный стиль узла: что получилось после каскада, до применения к GPUI.
//!
//! Промежуточная структура нужна по двум причинам. Во-первых, её видно в
//! тестах без окна и рендера — а `gpui::Style` собрать в тесте нельзя.
//! Во-вторых, ровно она задаёт границу охвата: поле есть — свойство
//! поддержано, поля нет — свойство игнорируется осознанно, а не потеряно.

mod gradient_paint;
mod font_kerning;
mod font_members;
pub(crate) mod font_family;
mod white_space;
mod font_shorthand;
pub(crate) mod font_weight;
mod text_indent;
mod bidi_properties;
mod image_color;
mod radius_mask;
mod border_color;
mod radius_parse;
pub(crate) use image_color::parse as parse_image_color;
mod mask_size;
mod mask_shorthand;
pub(crate) mod orthogonal;
mod tab_size;
mod quotes;
mod counters;
mod list_style_string;
mod list_style;
mod size_range;
mod content_functions;
pub(crate) use content_functions::parse_content;
mod outline_style;
use outline_style::parse as outline_style_of;
pub(crate) use outline_style::DOUBLE as OUTLINE_DOUBLE;
pub(crate) mod props;
#[cfg(test)]
mod snapshot_tests;

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
        // Разрез — по пробелам ВНЕ скобок: `calc(10px + 1%) 0 0 0` — четыре
        // значения, а не шесть обрывков (`calc-margin-block-1`). Смесь с долей
        // доживает индексом (`parse_mixed`) — раскладка складывает её сама.
        let v: Vec<Option<Len>> = split_outside_parens(raw)
            .iter()
            .map(|t| Len::parse_mixed(t))
            .collect();
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

/// Разряды `inherit_bits`: ненаследуемые свойства, у которых слово `inherit`
/// обязано скопировать вычисленное значение родителя (§6.2.1).
pub(crate) mod ainh {
    pub(crate) const ALIGN_ITEMS: u8 = 1 << 0;
    pub(crate) const JUSTIFY_ITEMS: u8 = 1 << 1;
    pub(crate) const ALIGN_CONTENT: u8 = 1 << 2;
    pub(crate) const JUSTIFY_CONTENT: u8 = 1 << 3;
    pub(crate) const JUSTIFY_SELF: u8 = 1 << 4;
}

pub(crate) mod inh {
    pub(crate) const BG_REPEAT: u32 = 1 << 0;
    pub(crate) const Z_INDEX: u32 = 1 << 1;
    pub(crate) const OUTLINE_W: u32 = 1 << 2;
    pub(crate) const DISPLAY: u32 = 1 << 3;
    pub(crate) const BG_IMAGE: u32 = 1 << 4;
    pub(crate) const BG_POS: u32 = 1 << 5;
    pub(crate) const CLIP: u32 = 1 << 6;
    pub(crate) const BG_ORIGIN: u32 = 1 << 7;
    pub(crate) const BG_CLIP: u32 = 1 << 8;
    pub(crate) const BG_SIZE: u32 = 1 << 9;
    pub(crate) const TRANSFORM: u32 = 1 << 10;
    pub(crate) const TRANSFORM_ORIGIN: u32 = 1 << 11;
    pub(crate) const OUTLINE_C: u32 = 1 << 12;
    pub(crate) const OUTLINE_S: u32 = 1 << 13;
    pub(crate) const OUTLINE_O: u32 = 1 << 14;
    /// `overflow-clip-margin: inherit` — коробка отсчёта и поле родителя.
    pub(crate) const CLIP_MARGIN: u32 = 1 << 15;
    /// `column-rule-color: inherit` — скалярный цвет и список линеек родителя.
    pub(crate) const COLUMN_RULE_C: u32 = 1 << 16;
    /// `row-rule-color: inherit`.
    pub(crate) const ROW_RULE_C: u32 = 1 << 17;
}

/// Разряды `will_change` (css-will-change-1 §2.1): чего ждать от коробки,
/// которая свойство только ОБЕЩАЕТ. «If any non-initial value of a property
/// would create a stacking context on the element, specifying that property
/// in will-change must create a stacking context on the element» — и то же
/// дословно про содержащий блок для `absolute` и для `fixed`.
pub(crate) mod wc {
    /// Содержащий блок для `position: absolute`.
    pub(crate) const CB_ABS: u8 = 1 << 0;
    /// Содержащий блок для `position: fixed`.
    pub(crate) const CB_FIXED: u8 = 1 << 1;
    /// Контекст наложения.
    pub(crate) const STACK: u8 = 1 << 2;
    /// `z-index`: контекст только там, где `z-index` действует
    /// (позиционированная коробка, элемент flex/grid) — решает
    /// `inline::inherit`, где известен вид родителя.
    pub(crate) const STACK_Z: u8 = 1 << 3;
    /// Обещано свойство семьи `transform` или `contain`: к строчной
    /// НЕатомарной коробке они не применяются (css-transforms-1
    /// «transformable element»), поэтому три разряда выше ставит `dom::walk`,
    /// когда вид коробки уже известен (`will-change-transform-inline`).
    pub(crate) const BOX: u8 = 1 << 4;
}

/// Функция картинки в начале слоя и хвост за её закрывающей скобкой.
fn split_image_func(v: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (&v[..=i], &v[i + 1..]);
                }
            }
            _ => {}
        }
    }
    (v, "")
}

/// Есть ли в записи длины в единицах шрифта (`em`, `rem`, `ex`, `ch`).
fn has_font_units(v: &str) -> bool {
    v.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .any(|t| {
            let unit = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-');
            unit.len() < t.len()
                && matches!(unit.to_ascii_lowercase().as_str(), "em" | "rem" | "ex" | "ch")
        })
}

/// Заменить длины в единицах шрифта на пиксели: `1em` → `16px`.
pub(crate) fn font_lengths_to_px(v: &str, em: f32, rem: f32, ex: f32, ch: f32) -> String {
    let mut out = String::with_capacity(v.len() + 8);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.is_empty() {
            return;
        }
        let unit_at = token
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .unwrap_or(token.len());
        let (num, unit) = token.split_at(unit_at);
        let k = match unit.to_ascii_lowercase().as_str() {
            "em" => Some(em),
            "rem" => Some(rem),
            "ex" => Some(ex),
            "ch" => Some(ch),
            _ => None,
        };
        match (k, num.parse::<f32>()) {
            (Some(k), Ok(n)) if unit_at > 0 => out.push_str(&format!("{}px", n * k)),
            _ => out.push_str(token),
        }
        token.clear();
    };
    for c in v.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            token.push(c);
        } else {
            flush(&mut token, &mut out);
            out.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Block,
    /// `display: grid-lanes` — раскладка лунками (CSS Grid 3).
    GridLanes,
    /// `display: contents` — коробки нет, дети идут в поток родителя.
    Contents,
    /// `display: list-item` — блок с маркером.
    ListItem,
    /// `display: table-row` — ряд ячеек.
    TableRow,
    /// `display: table-row-group` и родня: обёртка над рядами. По раскладке
    /// это блок, но сборщик таблицы обязан заглянуть внутрь за рядами — от
    /// обычного блока его тем и надо отличать.
    TableRowGroup,
    /// `display: table` — контейнер табличной раскладки.
    Table,
    /// `display: inline-table` — та же таблица, но стоящая В СТРОКЕ.
    InlineTable,
    /// `display: table-cell` — ячейка.
    TableCell,
    /// `display: inline-grid`.
    InlineGrid,
    /// `inline-block`: коробка со своими размерами, но стоящая В СТРОКЕ.
    /// Отдельный вариант нужен потому, что раньше он схлопывался в `Block` и
    /// два таких элемента вставали друг под друга вместо одной строки.
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    None,
}

/// Список значений линейки промежутков (css-gaps-1 §lists): ведущие значения,
/// тело `repeat(auto, …)` и хвостовые. Без авто-повтора список ЦИКЛИТСЯ по
/// промежуткам («repeat beginning from the first item in values»); с ним
/// ведущие идут от первого промежутка, хвостовые — от последнего, а тело
/// заполняет середину по кругу.
#[derive(Clone, Debug, PartialEq)]
pub struct GapList<T> {
    pub lead: Vec<T>,
    pub auto: Vec<T>,
    pub tail: Vec<T>,
}

impl<T: Copy> GapList<T> {
    pub fn single(v: T) -> Self {
        GapList { lead: vec![v], auto: vec![], tail: vec![] }
    }

    /// Первое значение — им живёт многоколонник, знающий одну линейку.
    pub fn first(&self) -> Option<T> {
        self.lead
            .first()
            .or(self.auto.first())
            .or(self.tail.first())
            .copied()
    }

    /// Больше одного значения или авто-повтор: скаляра недостаточно.
    pub fn is_plural(&self) -> bool {
        !self.auto.is_empty() || self.lead.len() + self.tail.len() > 1
    }

    pub fn any(&self, f: impl Fn(&T) -> bool) -> bool {
        self.lead.iter().chain(&self.auto).chain(&self.tail).any(f)
    }

    pub fn map<U: Copy>(&self, f: impl Fn(&T) -> U) -> GapList<U> {
        GapList {
            lead: self.lead.iter().map(&f).collect(),
            auto: self.auto.iter().map(&f).collect(),
            tail: self.tail.iter().map(&f).collect(),
        }
    }

    /// Значение промежутка `k` из `n` (§value-assignment).
    pub fn at(&self, k: usize, n: usize) -> Option<T> {
        if self.auto.is_empty() {
            let m = self.lead.len() + self.tail.len();
            if m == 0 {
                return None;
            }
            let i = k % m;
            return Some(if i < self.lead.len() {
                self.lead[i]
            } else {
                self.tail[i - self.lead.len()]
            });
        }
        if k < self.lead.len() {
            return Some(self.lead[k]);
        }
        let tail_from = n.saturating_sub(self.tail.len()).max(self.lead.len());
        if k >= tail_from {
            return self
                .tail
                .get(k - tail_from)
                .copied()
                .or(self.auto.first().copied());
        }
        Some(self.auto[(k - self.lead.len()) % self.auto.len()])
    }
}

/// Втяжка конца линейки (css-gaps-1 §inset): длина, доля ширины
/// пересекающего зазора или `overlap-join` — дотянуть до дальнего края
/// поперечной линейки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GapInset {
    Len(Len),
    OverlapJoin,
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
    /// css-anchor-position-1 §anchor-center: центр по якорю по умолчанию в
    /// пределах inset-modified containing block; без якоря или не у
    /// абсолюта — как `center` (так его и видит раскладка, `apply::to_items`).
    AnchorCenter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Justify {
    Start,
    Center,
    End,
    /// `left`/`right` — физические стороны; поперёк строчной оси (гибкая
    /// колонка) ведут себя как `start` (css-align-3 §5.2).
    Left,
    Right,
    /// `start`/`end` — оси ПИСЬМА, а не гибкой раскладки: при `row-reverse`
    /// они смотрят в другую сторону, чем `flex-start`/`flex-end`.
    WmStart,
    WmEnd,
    Between,
    Around,
    /// `space-evenly` — равные промежутки И по краям; у `space-around` края
    /// вдвое уже, поэтому свести их в одно значение нельзя.
    Evenly,
    Stretch,
}

/// Куда класть элемент в сетке: номер линии, число дорожек или «сама реши».
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Auto,
    Line(i16),
    Span(u16),
}

/// `grid-auto-flow`: в какую сторону раскладываются неразмещённые элементы.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoFlow {
    Row,
    Col,
    RowDense,
    ColDense,
}

/// `text-transform: full-width` (см. `Computed::text_transform_flags`).
pub const TT_FULL_WIDTH: u8 = 1;
/// `text-transform: full-size-kana`.
pub const TT_KANA: u8 = 2;
/// `text-transform: math-auto`.
pub const TT_MATH: u8 = 4;

/// `text-transform`: регистр меняется при отрисовке текста, не в шрифте.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextTransform {
    None,
    Upper,
    Lower,
    Capitalize,
    /// `full-width` — знак меняется на полноширинного двойника: латиница и
    /// цифры набираются в клетку, как иероглифы.
    FullWidth,
}

/// Стороны и размеры, заданные ЛОГИЧЕСКИ.
///
/// Какая сторона логического начала физическая — знает только письмо, а оно
/// наследуется. Поэтому значения доживают до отдельного прохода по дереву
/// (`doc::resolve_logical`) и лишь там ложатся на физические поля. Перевод
/// при разборе, как было раньше, молча считал письмо горизонтальным: в
/// вертикальном тексте `margin-block` разворачивал отступ не по той оси.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Logical {
    pub inline_size: Option<Len>,
    pub block_size: Option<Len>,
    /// `contain-intrinsic-inline-size` / `-block-size`: подменная величина
    /// по логическим осям — раскладывается по физическим после наследования
    /// письма, как и остальные логические свойства.
    pub ci_inline: Option<f32>,
    pub ci_block: Option<f32>,
    pub min_inline: Option<Len>,
    pub min_block: Option<Len>,
    pub max_inline: Option<Len>,
    pub max_block: Option<Len>,
    pub padding: LogicalSides,
    pub margin: LogicalSides,
    pub inset: LogicalSides,
    /// Логические кромки: сырые значения `border-block/inline-start/end` —
    /// раскладываются по физическим сторонам ПОСЛЕ наследования письма
    /// (у `vertical-rl` inline-start это ВЕРХ, а разбор писал влево).
    /// Порядок: [block-start, inline-end, block-end, inline-start].
    pub border: [Option<String>; 4],
}

/// Четыре логические стороны коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogicalSides {
    pub inline_start: Option<Len>,
    pub inline_end: Option<Len>,
    pub block_start: Option<Len>,
    pub block_end: Option<Len>,
    /// Порядковый номер объявления каждой стороны (inline_start, inline_end,
    /// block_start, block_end) — спор с физической стороной решает более
    /// позднее объявление, а на какую физическую сторону ляжет логическая,
    /// известно только после наследования письма.
    pub seq: [u32; 4],
}

/// `outline`: рамка ВНЕ коробки, не влияющая на раскладку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outline {
    pub width: Option<Len>,
    pub color: Option<Color>,
    pub offset: Option<Len>,
    /// `outline-offset: inset` (css-ui-4; WPT `outline-offset-inset-*`):
    /// сдвиг равен минус толщине. Толщина известна только к отрисовке,
    /// поэтому здесь метка, а не длина.
    pub inset: bool,
    /// 0 — none/hidden, 1 — solid/groove/…, 2 — auto, 3 — dotted,
    /// 4 — dashed, 5 — double (two rings with a transparent gap).
    pub style: Option<u8>,
}

/// `text-fit` (css-text-5): кегль подбирается так, чтобы строка заполняла
/// коробку. Свойство не про перенос, а про РАЗМЕР — поэтому применяется
/// после раскладки строк и меняет кегль абзаца.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFit {
    /// Разрешено увеличивать кегль.
    pub grow: bool,
    /// Разрешено уменьшать.
    pub shrink: bool,
    /// Каждой строке — свой кегль (`per-line`), иначе один на все
    /// (`consistent`).
    pub per_line: bool,
    /// `per-line-all`: подбор идёт и для ПОСЛЕДНЕЙ строки тоже.
    pub all: bool,
    /// Процент — ЗАЖИМ множителя, а не доля заполнения (css-text-5
    /// §text-fit): при `grow` и значении ≥ 100% это максимум, при `shrink` и
    /// значении ≤ 100% — минимум; иначе, и когда не задан, предела нет.
    pub target: Option<f32>,
}

/// Какая пунктуация свисает за край строки (`hanging-punctuation`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hanging {
    /// Открывающий знак в начале ПЕРВОЙ строки.
    pub first: bool,
    /// Закрывающий знак в конце ПОСЛЕДНЕЙ строки.
    pub last: bool,
    /// Точка или запятая в конце любой строки — свисает всегда.
    pub force_end: bool,
    /// То же, но только если иначе строка не влезает.
    pub allow_end: bool,
}

/// Роль коробки в руби по `display` (css-ruby-1 §2.1, `ruby | ruby-base |
/// ruby-text | ruby-base-container | ruby-text-container`, а также `block
/// ruby`). Роль по ТЕГУ (`ruby/rb/rt/rbc/rtc`) сюда не пишется — её даёт
/// `render::ruby_role`, чтобы авторский `display: block` на `<rt>` роль
/// снимал, а не дописывал.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RubyRole {
    Container,
    Base,
    Text,
    BaseContainer,
    TextContainer,
}

/// `ruby-align` (css-ruby-1 §4.3): выключка содержимого руби-коробки, когда
/// оно уже своей колонки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RubyAlign {
    Start,
    Center,
    SpaceBetween,
    /// Начальное значение: как `space-between`, плюс по половине зазора с
    /// краёв; без точек выключки (латиница) — по центру.
    SpaceAround,
}

/// `ruby-overhang` (css-ruby-1 §4.4): may an annotation wider than its base
/// overhang the adjacent content? `Auto` (initial): over adjacent text by at
/// most half the annotation's font size; `Spaces`: only over adjacent space
/// separators; `None`: never.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RubyOverhang {
    Auto,
    None,
    Spaces,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    /// Выключка по ширине: остаток строки раздаётся её пробелам.
    Justify,
    /// `start`/`end` — края СТРОКИ, а не листа: при письме справа налево они
    /// меняются местами. Направление письма приходит с наследованием и в
    /// момент разбора ещё неизвестно, поэтому значения доживают до него.
    Start,
    End,
}

impl TextAlign {
    /// Развернуть логические края в физические по направлению письма.
    pub fn physical(self, rtl: bool) -> Self {
        match (self, rtl) {
            (TextAlign::Start, false) | (TextAlign::End, true) => TextAlign::Left,
            (TextAlign::Start, true) | (TextAlign::End, false) => TextAlign::Right,
            (other, _) => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    /// `fixed` — отсчёт от окна, прокрутка его не двигает.
    Fixed,
    /// `sticky` — в потоке, пока лента не увезла элемент за край; дальше он
    /// стоит у края видимой части, но не покидает родителя.
    Sticky,
}

/// `position-anchor` (css-anchor-position-1 §position-anchor): якорь по
/// умолчанию для `anchor()` без имени. `normal` без `position-area` ведёт
/// себя как `none`; `match-parent` пока не решается (нет пар).
#[derive(Clone, Debug, PartialEq)]
pub enum PositionAnchor {
    Normal,
    None,
    Auto,
    Named(String),
    MatchParent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    /// Обрезка БЕЗ скролл-контейнера: авто-минимум flex/grid-элемента
    /// остаётся по содержимому (CSSWG #7714; min-size-auto-overflow-clip).
    Clip,
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

/// Градиент: линейный под углом либо радиальный от центра.
///
/// Один слой GPUI несёт два стопа. Промежуточные не выбрасываются: если
/// градиент идёт по оси (сверху вниз или слева направо), он рисуется
/// полосами — по слою на пару соседних стопов, и картинка совпадает с
/// браузером. Для наклонного градиента с тремя и более стопами полосами не
/// обойтись, там доезжают крайние цвета.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Gradient {
    pub angle_deg: f32,
    /// Радиальный: угол не участвует, цвет идёт от центра к краям.
    pub radial: bool,
    /// `circle` — окружность вместо эллипса по краям коробки.
    pub circle: bool,
    pub from: Color,
    pub to: Color,
    /// Все стопы с позициями в долях: `[(цвет, доля)]`, включая крайние.
    pub stops: Vec<(Color, f32)>,
    /// Стопы, заданные в ТОЧКАХ (`yellow 0px 28px`): доля от них не считается
    /// без длины оси, поэтому рисуются полосами в точках. Пусто, если хоть у
    /// одного стопа позиция не в точках.
    pub stops_px: Vec<(Color, f32)>,
    /// Сырые стопы для растра: (цвет, доля, точки). Смешение точек с долями
    /// разрешается только при отрисовке, когда длина оси известна — иначе
    /// точечная позиция теряется и стоп встаёт «поровну» (blue 170px в
    /// 50px-градиенте красил край синим вместо интерполяции).
    pub stops_raw: Vec<(Color, Option<f32>, Option<f32>)>,
    /// Пространство, в котором смешиваются цвета (css-color-4 §12.2).
    pub space: GradSpace,
    /// Дуга тона для полярных пространств: 0 shorter (умолчание), 1 longer,
    /// 2 increasing, 3 decreasing (css-color-4 §12.4).
    pub hue: u8,
}

/// Пространство интерполяции цвета градиента (css-color-4 §12.2).
///
/// Умолчание решается СОСТАВОМ стопов, а не записью: пока все цвета заданы
/// устаревшими формами sRGB (имя, `#hex`, `rgb()`, `rgba()`, `hsl()`,
/// `hsla()`, `hwb()`), смешение обязано идти в гамма-кодированном sRGB — этим
/// спека держит совместимость с вебом. Стоит хоть одному цвету быть записанным
/// современной формой — умолчанием становится OKLab.
///
/// `Linear` — любое пространство, линейное по свету (`srgb-linear`, `xyz`,
/// `xyz-d50`, `xyz-d65`, `display-p3-linear`, `rec2020-linear`): они связаны
/// ЛИНЕЙНЫМ преобразованием, а линейная интерполяция с ним коммутирует, так
/// что результат у них общий (набор и держит на них один эталон).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GradSpace {
    #[default]
    Srgb,
    Linear,
    Oklab,
    Oklch,
    Lab,
    Lch,
    Hsl,
    Hwb,
}

/// `border-image`: картинка вместо рамки (css-backgrounds-3 §6).
///
/// Рисуется девятью кусками: углы, четыре края и середина. Всё, что нужно для
/// этого разбора, лежит здесь, а рисование — в `border_image`.
#[derive(Clone, Debug, PartialEq)]
pub struct BorderImage {
    pub src: String,
    /// Срезы образа: верх, право, низ, лево.
    pub slice: [BorderImageSlice; 4],
    /// `fill` — рисовать и середину.
    pub fill: bool,
    /// Ширина кусков рамки на экране: верх, право, низ, лево.
    pub width: [BorderImageWidth; 4],
    /// Насколько рамка выходит за коробку: верх, право, низ, лево.
    pub outset: [f32; 4],
    /// Как мостятся края: вдоль строки и вдоль колонки.
    pub repeat: (Tiling, Tiling),
}

/// Срез образа: своё число точек образа или доля его стороны.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderImageSlice {
    Px(f32),
    Pct(f32),
}

/// Ширина куска рамки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderImageWidth {
    /// Число — во столько раз толще самой рамки.
    Times(f32),
    Px(f32),
    /// Доля стороны коробки.
    Pct(f32),
    Auto,
}

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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BgSize {
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
    let first = crate::css::split_args(v).into_iter().next().unwrap_or_default();
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
                return BgPos { x: Some(Len::Pct(0.5)), y: Some(Len::Pct(0.5)) };
            }
            let off = tokens.get(i + 1).filter(|n| !is_kw(n.as_str())).and_then(|n| length(n));
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

/// `transform`: поворот, масштаб и сдвиг при отрисовке.
///
/// Сдвиг хранится вместе с поворотом: в CSS `translate()` внутри `transform`
/// и отдельное свойство `translate` складываются.
#[derive(Clone, Copy, Debug, PartialEq)]
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09): шаг 1 объёмных трансформаций
// (css-transforms-2) — полная накопленная 4x4 `m4`/`m4_pct`/`has_3d` рядом с
// плоской 2x3, `perspective`/`perspective-origin`/`transform-style`,
// `translateZ`/`scaleZ`/`rotateX|Y|3d`/`matrix3d` целиком, `preserve-3d`
// через потоко-локальный стек накопленных матриц и сплющивание плоскости
// z=0 на отрисовке (`interact::Transformed`), обёртка `transformed()` и без
// собственного `transform`. Срез 3029 пар (transforms/contain/overflow/
// masking/position/backgrounds): 2102 -> 2053, **+11/-60**; из потерь
// одиннадцать — 99.00 (`css-rotate-2d-3d-001`, `rotate3d-Z-*`,
// `css3-transform-rotateY`, `perspective-children-only-*`,
// `preserve3d-and-flattening-z-order-001/002`): страница разъезжается
// целиком, а не сдвигается. Возвращаться по одному рукаву: сначала
// `matrix3d`/`perspective()` внутри ОДНОГО элемента без стека, затем стек.
// План и патч — `target/scout-3d-2026-09.md` §7.
// Корень провала нашёл второй заход (`scout-3d-2026-09b.md`): хунк 7.18
// домножал на масштаб устройства весь столбец сдвига, включая m44
// (1 -> 1.25), а сплющивание делило на него всю матрицу — каждая коробка
// на объёмном пути сжималась в 0.8 вокруг transform-origin (0.75 % = ровно
// 125² − 100²). Узкий шаг 1' — 4x4 внутри ОДНОГО элемента, без стека и без
// обёртки элементов без `transform`, свёртка `S·M·S⁻¹` с нетронутым m44 —
// на том же срезе дал +14/-1 и внесён ниже.
pub struct Transform {
    pub rotate_rad: f32,
    /// Скос по осям в радианах (`skew`, `skewX`, `skewY`).
    pub skew_rad: (f32, f32),
    pub scale: (f32, f32),
    pub translate: (f32, f32),
    /// Сдвиг, заданный долями СОБСТВЕННОГО размера: `translate(-50%, -50%)`.
    /// Разрешается при отрисовке, когда размер известен.
    pub translate_pct: (f32, f32),
    /// Аффинная матрица всех функций В ПОРЯДКЕ ЗАПИСИ (css-transforms-1
    /// §transform-rendering: «multiply … from left to right»): линейная
    /// часть и сдвиг. Разложение выше складывает функции покомпонентно и
    /// порядок теряет (`translate(200px) rotate(180deg)` уводило коробку за
    /// экран) — рисует отрисовка по матрице, разложение остаётся для SVG и
    /// сдвига клипа.
    pub lin: [[f32; 2]; 2],
    /// Сдвиг по осям: пиксели, доля СОБСТВЕННОЙ ширины, доля высоты —
    /// проценты внутри цепочки складываются линейно и разрешаются при
    /// отрисовке.
    pub tr: [[f32; 3]; 2],
    /// Элемент m33 накопленной 4x4-матрицы. Плоская отрисовка его не видит,
    /// но `backface-visibility: hidden` прячет элемент ровно при m33 < 0
    /// (css-transforms-2 §backface-visibility). У плоских функций m33 = 1,
    /// поэтому множители перемножаются без потери точности.
    pub m33: f32,
    /// Полная 4×4 ОДНОГО элемента (css-transforms-2 §3d-transform-rendering),
    /// `m4[строка][столбец]`, столбец 3 — сдвиг в css-точках. Плоские функции
    /// вкладываются как есть, объёмные (`rotateX/Y/3d`, `translateZ`,
    /// `scaleZ`, `perspective()`, `matrix3d`) живут только здесь;
    /// `lin`/`tr` остаются для SVG, клипа и плоского пути отрисовки.
    pub m4: [[f32; 4]; 4],
    /// Доли СОБСТВЕННОГО размера в столбце сдвига: `m4_pct[строка] =
    /// [доля ширины, доля высоты]` (как `tr[i][1..3]`).
    pub m4_pct: [[f32; 2]; 4],
    /// Встретилась действительно объёмная функция: отрисовка идёт по `m4`,
    /// иначе — прежний плоский путь по `lin`/`tr`.
    pub has_3d: bool,
}

/// Ячейка матрицы перспективы элемента в точках устройства
/// (css-transforms-2 §perspective-matrix-computation): заводится при
/// разборе `perspective`, наполняется его `Transformed::paint`, читается
/// объёмным путём ПРЯМЫХ детей. Разделяемая ячейка, а не стек кадра:
/// абсолютный ребёнок с `z-index`/`fixed` рисуется отложенным слоем
/// (`defers`), когда `paint` родителя уже вышел; ячейка переживает кадр.
pub type PerspectiveFrame = std::rc::Rc<std::cell::Cell<Option<[[f32; 4]; 4]>>>;

/// Ячейка объёмного контекста `transform-style: preserve-3d`
/// (css-transforms-2 §accumulated-3d-transformation-matrix): накопленная
/// 4×4 в точках устройства И собственная аффинная доля
/// `[[a, b, tx], [c, d, ty]]`, которую владелец уже втолкнул в gpui.
/// Ребёнок кладёт себя по `flatten(A · C)`, а родительскую долю обязан
/// снять сам: `with_transformation` складывает вложения как `inner∘outer`
/// (`vendor/gpui/src/window.rs:2789`; обратный порядок ЗАМЕРЕН И ОТКАЧЕН —
/// css-writing-modes −9). Ячейка, а не стек кадра, — по той же причине,
/// что у перспективы: абсолютный ребёнок с `z-index`/`fixed` рисуется
/// отложенным слоем, когда `paint` владельца уже вышел.
pub type Frame3d = std::rc::Rc<std::cell::Cell<Option<([[f32; 4]; 4], [[f32; 3]; 2])>>>;

/// Единичная 4×4.
pub const IDENTITY4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Произведение 4×4: `a · b`.
pub fn mul4(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// Определитель 4×4 (разложение по первой строке через миноры 3×3).
pub fn det4(m: &[[f32; 4]; 4]) -> f32 {
    let minor = |r: [usize; 3], c: [usize; 3]| -> f32 {
        let a = |i: usize, j: usize| m[r[i]][c[j]];
        a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
            - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
            + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0))
    };
    m[0][0] * minor([1, 2, 3], [1, 2, 3]) - m[0][1] * minor([1, 2, 3], [0, 2, 3])
        + m[0][2] * minor([1, 2, 3], [0, 1, 3])
        - m[0][3] * minor([1, 2, 3], [0, 1, 2])
}

/// Гомография плоскости z=0 → экран: строки/столбцы 0,1,3 полной матрицы.
/// Её вырождение — плоскость видна ребром (`rotateX(90deg)`), даже когда
/// сама 4×4 обратима.
pub fn det3_plane(m: &[[f32; 4]; 4]) -> f32 {
    let idx = [0usize, 1, 3];
    let a = |i: usize, j: usize| m[idx[i]][idx[j]];
    a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
        - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0))
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            rotate_rad: 0.0,
            skew_rad: (0.0, 0.0),
            scale: (1.0, 1.0),
            translate: (0.0, 0.0),
            translate_pct: (0.0, 0.0),
            lin: [[1.0, 0.0], [0.0, 1.0]],
            tr: [[0.0; 3]; 2],
            m33: 1.0,
            m4: IDENTITY4,
            m4_pct: [[0.0; 2]; 4],
            has_3d: false,
        }
    }
}

impl Transform {
    /// Поворот вокруг произвольной оси, СПЛЮЩЕННЫЙ на плоскость экрана.
    ///
    /// Сплющивание (css-transforms-2 §3d-transform-rendering) — это
    /// вычёркивание третьей строки и третьего столбца 4x4-матрицы, поэтому
    /// плоская часть — ровно верхний 2x2 блок матрицы поворота Родрига, а
    /// m33 = z²(1-cos a) + cos a нужен для `backface-visibility`.
    /// Проверка: ось Z даёт обычный поворот, ось Y — diag(cos a, 1), ось X —
    /// diag(1, cos a), то есть ровно `rotateZ`/`rotateY`/`rotateX`.
    pub fn axis_rot(x: f32, y: f32, z: f32, a: f32) -> ([[f32; 2]; 2], f32) {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return ([[1.0, 0.0], [0.0, 1.0]], 1.0);
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        (
            [
                [x * x * k + c, x * y * k - z * s],
                [y * x * k + z * s, y * y * k + c],
            ],
            z * z * k + c,
        )
    }
    /// Дописать плоскую функцию справа в ОБЕ матрицы: `M := M · [l | v]`.
    fn push(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        self.push2(l, v);
        let mut f = IDENTITY4;
        f[0][0] = l[0][0];
        f[0][1] = l[0][1];
        f[1][0] = l[1][0];
        f[1][1] = l[1][1];
        f[0][3] = v[0][0];
        f[1][3] = v[1][0];
        let pct = [[v[0][1], v[0][2]], [v[1][1], v[1][2]], [0.0, 0.0], [0.0, 0.0]];
        self.push4(f, pct);
    }

    /// Только плоская 2×3 (`lin`/`tr`) — для объёмных функций, чья
    /// сплющенная тень нужна SVG и клипу.
    fn push2(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        let m = self.lin;
        self.lin = [
            [
                m[0][0] * l[0][0] + m[0][1] * l[1][0],
                m[0][0] * l[0][1] + m[0][1] * l[1][1],
            ],
            [
                m[1][0] * l[0][0] + m[1][1] * l[1][0],
                m[1][0] * l[0][1] + m[1][1] * l[1][1],
            ],
        ];
        for i in 0..2 {
            for k in 0..3 {
                self.tr[i][k] += m[i][0] * v[0][k] + m[i][1] * v[1][k];
            }
        }
    }

    /// Дописать 4×4 справа: `M4 := M4 · f`. Столбец сдвига несёт доли размера:
    /// `(M·F)[i][3] = Σ_k M[i][k]·F[k][3]`, где `F[k][3]` для k<3 — «точки +
    /// доля», а `F[3][3]` домножает уже накопленные доли самой `M`.
    fn push4(&mut self, f: [[f32; 4]; 4], pct: [[f32; 2]; 4]) {
        let m = self.m4;
        let mut p = [[0.0f32; 2]; 4];
        for i in 0..4 {
            for a in 0..2 {
                p[i][a] = self.m4_pct[i][a] * f[3][3]
                    + (0..3).map(|k| m[i][k] * pct[k][a]).sum::<f32>();
            }
        }
        self.m4 = mul4(m, f);
        self.m4_pct = p;
    }

    /// `translate3d(x, y, z)` в точках.
    pub fn translate4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][3] = x;
        m[1][3] = y;
        m[2][3] = z;
        m
    }

    /// `scale3d(x, y, z)`.
    pub fn scale4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][0] = x;
        m[1][1] = y;
        m[2][2] = z;
        m
    }

    /// `perspective(d)`: m34 = −1/d (css-transforms-2 §perspective()); d уже
    /// не меньше 1px — clamp делает вызывающий.
    pub fn perspective4(d: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[3][2] = -1.0 / d;
        m
    }

    /// `rotate3d(x, y, z, a)`: R = cos·I + sin·[u]× + (1−cos)·u·uᵀ — верхний
    /// 2×2 блок и m33 ровно те же, что в `axis_rot`.
    pub fn rot4(x: f32, y: f32, z: f32, a: f32) -> [[f32; 4]; 4] {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return IDENTITY4;
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        [
            [x * x * k + c, x * y * k - z * s, x * z * k + y * s, 0.0],
            [y * x * k + z * s, y * y * k + c, y * z * k - x * s, 0.0],
            [z * x * k - y * s, z * y * k + x * s, z * z * k + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// Список — чистый плоский сдвиг в css-точках (`translate*()`/`matrix`
    /// с единичной линейной частью, без долей размера и без объёма):
    /// `Some((x, y))`.
    pub fn pure_px_shift(&self) -> Option<(f32, f32)> {
        let id2 = self.lin == [[1.0, 0.0], [0.0, 1.0]];
        let no_pct = self.tr[0][1] == 0.0
            && self.tr[0][2] == 0.0
            && self.tr[1][1] == 0.0
            && self.tr[1][2] == 0.0
            && self.m4_pct.iter().all(|r| r[0] == 0.0 && r[1] == 0.0);
        let mut m = self.m4;
        m[0][3] = 0.0;
        m[1][3] = 0.0;
        let (x, y) = (self.tr[0][0], self.tr[1][0]);
        (id2 && no_pct
            && !self.has_3d
            && m == IDENTITY4
            && self.m4[0][3] == x
            && self.m4[1][3] == y
            && x.is_finite()
            && y.is_finite())
        .then_some((x, y))
    }

    /// Домножить СПРАВА на уже накопленную матрицу другого объявления.
    ///
    /// Нужно слоению motion-1: offset-трансформ идёт ПЕРЕД авторским
    /// `transform`, а разбор авторского уже сложил свою матрицу — её
    /// приходится приставлять целиком, а не по одной функции.
    pub fn then(mut self, other: &Transform) -> Transform {
        self.push(other.lin, other.tr);
        self.rotate_rad += other.rotate_rad;
        self.skew_rad.0 += other.skew_rad.0;
        self.skew_rad.1 += other.skew_rad.1;
        self.scale.0 *= other.scale.0;
        self.scale.1 *= other.scale.1;
        self.translate.0 += other.translate.0;
        self.translate.1 += other.translate.1;
        self.translate_pct.0 += other.translate_pct.0;
        self.translate_pct.1 += other.translate_pct.1;
        self.m33 *= other.m33;
        self
    }

    /// Отдельные `rotate`/`scale` СЛЕВА от этого списка (css-transforms-2
    /// §ctm: п.4 — `rotate`, п.5 — `scale`, п.7 — функции `transform`; Blink
    /// `ComputedStyle::ApplyTransform`, style/computed_style.cc:1464-1487 —
    /// `Rotate()`, `Scale()`, затем `Transform().Operations()`). Обе матрицы —
    /// плоская и 4×4 — получают одну и ту же свёртку; разложение, m33 и
    /// `has_3d` остаются от самого списка (SVG, клип, изнанка).
    pub fn after_individual(self, rotate: Option<f32>, scale: Option<(f32, f32)>) -> Transform {
        if rotate.is_none() && scale.is_none() {
            return self;
        }
        let mut out = Transform::default();
        if let Some(a) = rotate {
            out.push(Self::rot(a), NO_SHIFT);
        }
        if let Some((x, y)) = scale {
            out.push(Self::diag(x, y), NO_SHIFT);
        }
        out.push2(self.lin, self.tr);
        out.push4(self.m4, self.m4_pct);
        Transform {
            lin: out.lin,
            tr: out.tr,
            m4: out.m4,
            m4_pct: out.m4_pct,
            ..self
        }
    }

    /// Промежуточная ПЛОСКАЯ матрица между `self` и `other` на доле `k`
    /// (css-transforms-1 §matrix-interpolation): обе раскладываются на
    /// масштаб, угол и остаток 2×2 (§decomposing-a-2d-matrix, «unmatrix»),
    /// компоненты смешиваются линейно — со сменой флипа и без «длинного пути»
    /// (§interpolation-of-decomposed-2d-matrix-values) — и собираются обратно:
    /// `lin = K·R(угол)·diag(sx, sy)`. `none` приходит сюда тождеством
    /// (§interpolation-of-transforms). Сдвиг вместе с долями размера —
    /// линейно. Необратимая сторона — `None`: анимация дискретна.
    pub fn lerp_2d(&self, other: &Transform, k: f32) -> Option<Transform> {
        use std::f32::consts::{PI, TAU};
        // Столбцы `lin` — образы осей: `row0` псевдокода = (a, b) записи
        // `matrix(a, b, c, d, e, f)`, `row1` = (c, d).
        let unmatrix = |l: [[f32; 2]; 2]| -> Option<((f32, f32), f32, [f32; 4])> {
            let (r0x, r0y, r1x, r1y) = (l[0][0], l[1][0], l[0][1], l[1][1]);
            let det = r0x * r1y - r0y * r1x;
            if det.abs() < 1e-9 {
                return None;
            }
            let mut sx = (r0x * r0x + r0y * r0y).sqrt();
            let mut sy = (r1x * r1x + r1y * r1y).sqrt();
            if det < 0.0 {
                if r0x < r1y {
                    sx = -sx;
                } else {
                    sy = -sy;
                }
            }
            let (r0x, r0y, r1x, r1y) = (r0x / sx, r0y / sx, r1x / sy, r1y / sy);
            let angle = r0y.atan2(r0x);
            let (sn, cs) = (-r0y, r0x);
            let m = [
                cs * r0x + sn * r1x,
                cs * r0y + sn * r1y,
                -sn * r0x + cs * r1x,
                -sn * r0y + cs * r1y,
            ];
            Some(((sx, sy), angle, m))
        };
        let (mut sa, mut aa, ma) = unmatrix(self.lin)?;
        let (sb, mut ab, mb) = unmatrix(other.lin)?;
        if (sa.0 < 0.0 && sb.1 < 0.0) || (sa.1 < 0.0 && sb.0 < 0.0) {
            sa = (-sa.0, -sa.1);
            aa += if aa < 0.0 { PI } else { -PI };
        }
        if aa == 0.0 {
            aa = TAU;
        }
        if ab == 0.0 {
            ab = TAU;
        }
        if (aa - ab).abs() > PI {
            if aa > ab {
                aa -= TAU;
            } else {
                ab -= TAU;
            }
        }
        let mix = |x: f32, y: f32| x + (y - x) * k;
        let (sx, sy) = (mix(sa.0, sb.0), mix(sa.1, sb.1));
        let m: [f32; 4] = std::array::from_fn(|i| mix(ma[i], mb[i]));
        let angle = mix(aa, ab);
        let r = Self::rot(angle);
        // K·R — остаток столбцами (m11, m12) и (m21, m22), как в псевдокоде.
        let kr = [
            [m[0] * r[0][0] + m[2] * r[1][0], m[0] * r[0][1] + m[2] * r[1][1]],
            [m[1] * r[0][0] + m[3] * r[1][0], m[1] * r[0][1] + m[3] * r[1][1]],
        ];
        let lin = [[kr[0][0] * sx, kr[0][1] * sy], [kr[1][0] * sx, kr[1][1] * sy]];
        let tr: [[f32; 3]; 2] =
            std::array::from_fn(|i| std::array::from_fn(|j| mix(self.tr[i][j], other.tr[i][j])));
        let mut out = Transform::default();
        out.push(lin, tr);
        Some(Transform {
            rotate_rad: angle,
            scale: (sx, sy),
            translate: (tr[0][0], tr[1][0]),
            translate_pct: (tr[0][1], tr[1][2]),
            ..out
        })
    }

    fn rot(a: f32) -> [[f32; 2]; 2] {
        [[a.cos(), -a.sin()], [a.sin(), a.cos()]]
    }

    fn diag(x: f32, y: f32) -> [[f32; 2]; 2] {
        [[x, 0.0], [0.0, y]]
    }
}

const NO_SHIFT: [[f32; 3]; 2] = [[0.0; 3]; 2];

/// `filter`: цветовое преобразование элемента.
///
/// Размытие поддерева требует отрисовки в отдельную текстуру, которой в GPUI
/// нет; всё остальное — арифметика над цветом, и её можно применить прямо к
/// собственным цветам элемента, не трогая конвейер отрисовки.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Filter {
    /// Доля обесцвечивания.
    pub grayscale: f32,
    /// Множитель яркости.
    pub brightness: f32,
    /// Множитель насыщенности.
    pub saturate: f32,
    /// Доля инверсии.
    pub invert: f32,
    /// Доля сепии.
    pub sepia: f32,
    /// Множитель непрозрачности.
    pub opacity: f32,
    /// Поворот тона в градусах.
    pub hue_rotate: f32,
    /// Множитель контраста.
    pub contrast: f32,
    /// Радиус размытия поддерева в точках: `filter: blur(N)`.
    ///
    /// Цветовые функции считаются по цвету каждого примитива, а размытию
    /// нужна готовая картинка поддерева — поэтому оно живёт отдельно от
    /// остальных полей и включает отрисовку в свой буфер.
    pub blur: f32,
}

impl Filter {
    /// Нейтральный фильтр: ничего не меняет.
    pub fn neutral() -> Filter {
        Filter {
            grayscale: 0.0,
            brightness: 1.0,
            saturate: 1.0,
            invert: 0.0,
            sepia: 0.0,
            opacity: 1.0,
            hue_rotate: 0.0,
            contrast: 1.0,
            blur: 0.0,
        }
    }

    /// Цветовые функции одной аффинной матрицей 4×5 над НЕумноженным RGBA:
    /// строки R, G, B, A по пять чисел (четыре множителя и сдвиг), порядок
    /// функций — как в `apply`. None — матрица единичная (остаётся разве что
    /// размытие). filter-effects-1 §«Supported filter functions»: каждая
    /// цветовая функция — `feColorMatrix`/`feComponentTransfer` с линейной
    /// формулой, их цепочка — произведение матриц.
    pub fn color_matrix(&self) -> Option<[f32; 20]> {
        // Три строки RGB: множители при r, g, b и сдвиг.
        type M = [[f32; 4]; 3];
        const ID: M = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        // `a` поверх `b`: a·(b·x + tb) + ta.
        fn then(a: &M, b: &M) -> M {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let s: f32 = (0..3).map(|k| a[i][k] * b[k][j]).sum();
                    if j == 3 { s + a[i][3] } else { s }
                })
            })
        }
        // (1 − k)·I + k·T — доля `k` пути к матрице `t` (без сдвига).
        fn toward(t: [[f32; 3]; 3], k: f32) -> M {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    if j == 3 {
                        return 0.0;
                    }
                    let id = if i == j { 1.0 } else { 0.0 };
                    id + (t[i][j] - id) * k
                })
            })
        }
        let diag = |k: f32, shift: f32| -> M {
            [
                [k, 0.0, 0.0, shift],
                [0.0, k, 0.0, shift],
                [0.0, 0.0, k, shift],
            ]
        };
        let mut m = ID;
        if self.sepia > 0.0 {
            let t = [
                [0.393, 0.769, 0.189],
                [0.349, 0.686, 0.168],
                [0.272, 0.534, 0.131],
            ];
            m = then(&toward(t, self.sepia.min(1.0)), &m);
        }
        if self.grayscale > 0.0 || self.saturate != 1.0 {
            let l = [0.2126f32, 0.7152, 0.0722];
            m = then(&toward([l, l, l], self.grayscale.min(1.0)), &m);
            // lum + (c − lum)·s = I + (L − I)·(1 − s)
            m = then(&toward([l, l, l], 1.0 - self.saturate), &m);
        }
        if self.invert > 0.0 {
            let k = self.invert.min(1.0);
            m = then(&diag(1.0 - 2.0 * k, k), &m);
        }
        if self.contrast != 1.0 {
            m = then(&diag(self.contrast, 0.5 - 0.5 * self.contrast), &m);
        }
        if self.hue_rotate != 0.0 {
            let a = self.hue_rotate.to_radians();
            let (cos, sin) = (a.cos(), a.sin());
            let h = [
                [
                    0.213 + cos * 0.787 - sin * 0.213,
                    0.715 - cos * 0.715 - sin * 0.715,
                    0.072 - cos * 0.072 + sin * 0.928,
                ],
                [
                    0.213 - cos * 0.213 + sin * 0.143,
                    0.715 + cos * 0.285 + sin * 0.140,
                    0.072 - cos * 0.072 - sin * 0.283,
                ],
                [
                    0.213 - cos * 0.213 - sin * 0.787,
                    0.715 - cos * 0.715 + sin * 0.715,
                    0.072 + cos * 0.928 + sin * 0.072,
                ],
            ];
            m = then(&toward(h, 1.0), &m);
        }
        if self.brightness != 1.0 {
            m = then(&diag(self.brightness, 0.0), &m);
        }
        if m == ID && self.opacity >= 1.0 {
            return None;
        }
        Some([
            m[0][0], m[0][1], m[0][2], 0.0, m[0][3], //
            m[1][0], m[1][1], m[1][2], 0.0, m[1][3], //
            m[2][0], m[2][1], m[2][2], 0.0, m[2][3], //
            0.0, 0.0, 0.0, self.opacity.min(1.0), 0.0,
        ])
    }

    /// Применить к цвету.
    pub fn apply(&self, c: Color) -> Color {
        // Filter Effects 1 §6.1 caps conversion amounts at one when used.
        // Keep the specified value intact for animation interpolation.
        let (mut r, mut g, mut b) = (c.r, c.g, c.b);
        // Порядок как в CSS: функции применяются слева направо, а записаны
        // они у нас в фиксированном порядке — для набора без повторов это то
        // же самое.
        if self.sepia > 0.0 {
            let k = self.sepia.min(1.0);
            let (sr, sg, sb) = (
                0.393 * r + 0.769 * g + 0.189 * b,
                0.349 * r + 0.686 * g + 0.168 * b,
                0.272 * r + 0.534 * g + 0.131 * b,
            );
            r += (sr - r) * k;
            g += (sg - g) * k;
            b += (sb - b) * k;
        }
        if self.grayscale > 0.0 || self.saturate != 1.0 {
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            // Обесцвечивание тянет к яркости, насыщение — от неё.
            let k = self.grayscale.min(1.0);
            r += (lum - r) * k;
            g += (lum - g) * k;
            b += (lum - b) * k;
            let sat = self.saturate;
            r = lum + (r - lum) * sat;
            g = lum + (g - lum) * sat;
            b = lum + (b - lum) * sat;
        }
        if self.invert > 0.0 {
            let k = self.invert.min(1.0);
            r += (1.0 - r - r) * k;
            g += (1.0 - g - g) * k;
            b += (1.0 - b - b) * k;
        }
        if self.contrast != 1.0 {
            // Аффинный контраст: растяжение вокруг середины (filter-effects-1
            // §contrast: c*k + 0.5 - 0.5k).
            let k = self.contrast;
            r = (r - 0.5) * k + 0.5;
            g = (g - 0.5) * k + 0.5;
            b = (b - 0.5) * k + 0.5;
        }
        if self.hue_rotate != 0.0 {
            // Матрица поворота тона из спецификации фильтров.
            let a = self.hue_rotate.to_radians();
            let (cos, sin) = (a.cos(), a.sin());
            let (r0, g0, b0) = (r, g, b);
            r = (0.213 + cos * 0.787 - sin * 0.213) * r0
                + (0.715 - cos * 0.715 - sin * 0.715) * g0
                + (0.072 - cos * 0.072 + sin * 0.928) * b0;
            g = (0.213 - cos * 0.213 + sin * 0.143) * r0
                + (0.715 + cos * 0.285 + sin * 0.140) * g0
                + (0.072 - cos * 0.072 - sin * 0.283) * b0;
            b = (0.213 - cos * 0.213 - sin * 0.787) * r0
                + (0.715 - cos * 0.715 + sin * 0.715) * g0
                + (0.072 + cos * 0.928 + sin * 0.072) * b0;
        }
        if self.brightness != 1.0 {
            r *= self.brightness;
            g *= self.brightness;
            b *= self.brightness;
        }
        Color {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: (c.a * self.opacity.min(1.0)).clamp(0.0, 1.0),
        }
    }
}

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
}

impl AnimSpec {
    /// Анимация, которая за жизнь страницы не сдвинется ни на точку, по сути
    /// остановлена: пауза или `animation: a 2000000s; animation-delay:
    /// -1000000s`. Один предикат на разрешение кадров (`dom.rs`) и отрисовку.
    pub fn frozen(&self) -> bool {
        self.paused || (!self.infinite && self.seconds >= 3600.0 && self.delay <= 0.0)
    }

    /// Доля пути остановленной анимации: `(-delay)/duration`.
    pub fn frozen_t(&self) -> f32 {
        if self.seconds > 0.0 {
            ((-self.delay) / self.seconds).clamp(0.0, 1.0)
        } else {
            0.0
        }
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

/// Порядковые номера объявлений физических сторон (top, right, bottom, left)
/// для полей, отступов и краёв — см. `LogicalSides::seq`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SideSeq {
    pub padding: [u32; 4],
    pub margin: [u32; 4],
    pub inset: [u32; 4],
}

/// Метрика края текста (`<text-edge>`, css-inline-3 §4.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextEdge {
    /// Подъём/спуск шрифта — начальное значение.
    #[default]
    Text,
    /// Высота прописной (только верхний край).
    Cap,
    /// Высота строчной (только верхний край).
    Ex,
    /// Алфавитная базовая линия (только нижний край).
    Alphabetic,
}

/// `border-shape` (css-borders-4 §border-shape): одна фигура — рамка
/// обводкой по её контуру толщиной «relevant side»; две — заливка между
/// внешней и внутренней. Текст фигуры хранится как есть, доли резолвит
/// отрисовка от опорной коробки (`geometry-box`: 0 border, 1 margin,
/// 2 padding, 3 content, 4 half-border-box).
#[derive(Clone, Debug, PartialEq)]
pub struct BorderShape {
    pub outer: String,
    pub outer_box: u8,
    pub inner: Option<(String, u8)>,
}

/// Линии украшения (css-text-decor-3 §2.1 `text-decoration-line`).
pub const DECOR_UNDER: u8 = 1;
pub const DECOR_OVER: u8 = 2;
pub const DECOR_THROUGH: u8 = 4;
/// `text-underline-position` (css-text-decor-4 §5.2).
pub const UPOS_UNDER: u8 = 1;
pub const UPOS_LEFT: u8 = 2;
pub const UPOS_RIGHT: u8 = 4;
pub const UPOS_FROM_FONT: u8 = 8;

/// `text-decoration-style` (css-text-decor-3 §2.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DecorStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Длина украшения: толщина, смещение подчёркивания, отступ концов.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DecorLen {
    #[default]
    Auto,
    FromFont,
    /// Абсолютная длина в точках CSS.
    Px(f32),
    /// Доля (1.0 = 100%): у толщины и смещения — от кегля, у отступа — от
    /// ширины украшаемого прогона.
    Pct(f32),
    /// Как задано (единицы шрифта решаются при наследовании).
    Raw(Len),
    /// `calc(доля + точки)` отступа концов: доля от ширины прогона.
    Mix(f32, f32),
}

/// Шрифт украшающей коробки: от него берутся метрики линий
/// (css-text-decor-3 §2.1 «decorating box»).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecorFont {
    pub family: Option<String>,
    pub monospace: Option<bool>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub stretch: Option<f32>,
    pub size: f32,
}

/// Украшение, наложенное украшающей коробкой (Blink `AppliedTextDecoration`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decor {
    pub lines: u8,
    pub style: DecorStyle,
    pub color: Color,
    /// Толщина: `Auto`, `FromFont` или `Px`.
    pub thickness: DecorLen,
    /// `text-underline-offset`: `Auto` или `Px`.
    pub offset: DecorLen,
    pub position: u8,
    /// `None` — `auto`; иначе (начало, конец): `Px` или `Pct`.
    pub inset: Option<[DecorLen; 2]>,
    pub clone: bool,
    pub font: DecorFont,
    /// Язык украшающей коробки — японский, корейский или монгольский: в вертикальном
    /// письме подчёркивание по умолчанию справа (Blink
    /// `ResolveUnderlinePosition`, css-text-decor-3 §default-stylesheet).
    pub over_lang: bool,
}

fn parse_decor_style(t: &str) -> Option<DecorStyle> {
    Some(match t {
        "solid" => DecorStyle::Solid,
        "double" => DecorStyle::Double,
        "dotted" => DecorStyle::Dotted,
        "dashed" => DecorStyle::Dashed,
        "wavy" => DecorStyle::Wavy,
        _ => return None,
    })
}

/// `<length-percentage>` украшения (толщина, смещение, отступ концов).
fn parse_decor_length(t: &str) -> Option<DecorLen> {
    if t == "auto" || t == "normal" {
        return None;
    }
    let l = Len::parse_spacing(t)?;
    Some(match l {
        Len::Px(v) => DecorLen::Px(v),
        Len::Pct(k) => DecorLen::Pct(k),
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => return None,
        other => DecorLen::Raw(other),
    })
}

/// `text-decoration-thickness`: `auto | from-font | <length-percentage> |
/// <line-width>` (css-text-decor-4 §2.4).
fn parse_decor_thickness(t: &str) -> Option<DecorLen> {
    Some(match t {
        "auto" => DecorLen::Auto,
        "from-font" => DecorLen::FromFont,
        "thin" => DecorLen::Px(1.0),
        "medium" => DecorLen::Px(3.0),
        "thick" => DecorLen::Px(5.0),
        _ => parse_decor_length(t)?,
    })
}

#[derive(Clone, Debug, Default)]
pub struct Computed {
    /// Счётчик объявлений этого узла: порядок каскада между логическими и
    /// физическими сторонами (UA `padding-inline-start: 40px` против
    /// авторского `padding-top: 0` в вертикальном письме —
    /// `line-box-direction-vrl-019`).
    pub decl_seq: u32,
    pub side_seq: SideSeq,
    pub display: Option<Display>,
    pub flex_dir: Option<FlexDir>,
    pub flex_wrap: Option<bool>,
    /// `wrap-reverse` — строки укладываются с противоположного края.
    pub flex_wrap_reverse: Option<bool>,
    /// `flex-wrap: balance` (css-flexbox-2 §5.2) — строки режет
    /// балансировщик; ортогонально `wrap`/`wrap-reverse`.
    pub flex_balance: Option<bool>,
    /// `flex-line-count` (css-flexbox-2 §5.3) — минимум строк у balance;
    /// умолчание 1.
    pub flex_line_count: Option<u16>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    /// `flex-basis: content` — основа берётся ПО СОДЕРЖИМОМУ, и заданный
    /// главный размер при ней игнорируется. Ключевого значения у раскладки
    /// под нами нет, поэтому признак хранится отдельно, а основа ставится в
    /// `auto`: этого достаточно, если снять главный размер.
    /// Свой пиксель ЗАМЕЩАЕМОГО элемента: размер из атрибутов `width`/`height`
    /// (у холста — умолчание 300×150). Хранится отдельно от `width`, потому
    /// что `flex-basis: content` снимает ЗАДАННЫЙ главный размер, но свой
    /// пиксель оставляет.
    pub attr_width: Option<Len>,
    pub attr_height: Option<Len>,
    /// Размер по оси пришёл из АТРИБУТА (`<canvas width>`), а не из CSS:
    /// у холста атрибуты — природный размер (HTML §4.12.5), не `width`
    /// (§15.3.10 их к нему не относит), и в сетке ось может растянуться.
    pub attr_sized: (bool, bool),
    pub basis_content: Option<bool>,
    pub align_items: Option<Align>,
    /// `align-self` — про сам элемент; отдельное поле, иначе он выравнивал
    /// бы своих детей вместо себя.
    pub align_self: Option<Align>,
    /// Авторское `align-self: normal` (css-align-3 §6.2): у элемента гибкого
    /// контейнера оно ведёт себя как `stretch`, а не «взять у родителя».
    pub align_self_normal: bool,
    /// Элемент гибкого РЯДА, поперечный размер которого тянет строка
    /// (`stretch` при `height: auto`). Взводит сборщик детей гибкого
    /// контейнера (`render.rs`, ветка `flex_context`), где виден родитель;
    /// читает обособление размера в `apply::apply_box`.
    pub(crate) cross_stretched: bool,
    pub flex_basis: Option<Len>,
    pub justify_content: Option<Justify>,
    pub gap: Option<(Option<Len>, Option<Len>)>,
    /// `box-sizing`. По умолчанию в CSS — `content-box`: заданная ширина
    /// НЕ включает отступы и рамку. Движок раскладки под нами всегда считает
    /// по `border-box`, поэтому разницу приходится компенсировать вручную.
    pub border_box: Option<bool>,
    pub grid_cols: Option<u16>,
    /// Список дорожек, если он выразим: `auto`, `1fr`, px, `%`, `minmax()`.
    pub grid_tracks: Option<Vec<TrackSize>>,
    /// `grid-template-areas`: сетка имён по строкам.
    ///
    /// Именованных областей нет ни в GPUI, ни в taffy, поэтому имена
    /// разворачиваются в номера линий при сборке дерева — там, где известны и
    /// контейнер, и его дети.
    pub grid_areas: Option<Vec<Vec<String>>>,
    /// `grid-template-areas: inherit` (css-cascade-4 §7.3 «explicit
    /// inheritance»): свойство не наследуемое, запись родителя переносит
    /// `doc::settle_explicit_inherit`.
    pub grid_areas_inherit: bool,
    /// Имя области у ребёнка: `grid-area: header`.
    pub grid_area_name: Option<String>,
    /// Имена линий `grid-template-columns` (логические колонки): у
    /// подсеточной оси — её `<line-name-list>` (`subgrid [a] [b]`). Разрешает
    /// их раскладка (taffy `NamedLineResolver`), в том числе через подсетки
    /// (css-grid-2 §9 (d)); прежде имена выбрасывались при разборе.
    pub grid_col_line_names: Option<gpui::GridAxisLineNames>,
    /// То же для `grid-template-rows`.
    pub grid_row_line_names: Option<gpui::GridAxisLineNames>,
    /// Именованные грани `grid-column-start`/`-end` (css-grid-2 §8.3):
    /// `Placement` у такой грани — `Auto`, имя разрешает раскладка.
    pub grid_col_named: [Option<gpui::GridNamedLine>; 2],
    /// То же для рядов.
    pub grid_row_named: [Option<gpui::GridNamedLine>; 2],
    /// `repeat(auto-fill, minmax(N, 1fr))` — сколько влезет колонок шириной
    /// не меньше N. Число колонок здесь считает раскладка, а не разметка.
    pub grid_auto_fill_min: Option<f32>,
    /// Тело повтора из НЕСКОЛЬКИХ дорожек: `repeat(auto-fill, 50px 50px)`
    /// повторяет пару, а не одну дорожку. Пусто — тело из одной дорожки, её
    /// размер лежит в `grid_auto_fill_min`.
    pub grid_auto_fill_tracks: Vec<f32>,
    /// То же по рядам: `grid-template-rows: repeat(auto-fill, 100px)`.
    pub grid_auto_fill_row: Option<f32>,
    /// Повтор «сколько влезет» по колонкам и по рядам целиком: нужен и вид
    /// повтора (`auto-fit` схлопывает пустые дорожки), и размер дорожки
    /// (`None` — дорожка по содержимому).
    /// `grid-template-*: subgrid` — дорожки берутся у родительской сетки.
    /// Своей раскладки подсетки нет, но знать о ней надо: абсолютных потомков
    /// она размещает по СВОИМ линиям, а не по линиям внешней сетки.
    pub subgrid: bool,
    /// Подсеточность ПООСЕВАЯ: `grid-template-columns: subgrid` и
    /// `grid-template-rows: subgrid` — разные объявления, и правило
    /// css-grid-2 §subgrid-box-alignment («в подсеточной оси свой размер и
    /// self-выравнивание игнорируются») действует ровно в СВОЕЙ оси.
    /// Скалярный `subgrid` этого не выражает: у пяти зелёных
    /// `standalone-axis-size-*` подсеточны РЯДЫ, а размер задан по КОЛОНКАМ,
    /// и гасить его нельзя.
    ///
    /// ВАЖНО: маршрут СРЕЗА дорожек эти поля не меняют — срез по-прежнему на
    /// скалярном `subgrid`. Перевод среза на поосевые признаки замерен и
    /// откачен (шапка `dom.rs: subgrid_takes_parent_tracks`, +2/−3).
    pub subgrid_cols: bool,
    pub subgrid_rows: bool,
    /// `container-type: size | inline-size` — элемент стал контейнером
    /// запросов размера и подсеткой быть не может (css-grid-2
    /// §subgrid-listing). Отдельным полем, а НЕ через `contain_size`: голое
    /// `contain: size` подсетку не отменяет — это отдельно проверяют случаи
    /// 8 и 9 `independent-formatting-context.html`.
    pub container_size_query: bool,
    pub auto_repeat_cols: Option<AutoRepeat>,
    pub auto_repeat_rows: Option<AutoRepeat>,
    /// Тело авто-повтора ДОРОЖКА ЗА ДОРОЖКОЙ: `repeat(auto-fill, max-content
    /// min-content)` — это два РАЗНЫХ размера, а не два одинаковых.
    /// css-grid-3 §7.2.1 («The hypothetical size of each track in the repeat()
    /// listing is given by the largest track corresponding to that entry (by
    /// index)», `csswg-drafts/css-grid-3/Overview.bs:469-471`) требует считать
    /// каждую запись тела по ЕЁ функции; скалярные `track`/`intrinsic_min`
    /// этого не выражают. Пишется РЯДОМ с `AutoRepeat` и `grid_tracks` не
    /// трогает: тот путь замерен и откачен (патч II 07.09, v143).
    pub auto_repeat_body_cols: Option<Vec<TrackSize>>,
    pub auto_repeat_body_rows: Option<Vec<TrackSize>>,

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
    /// `anchor-name` (css-anchor-position-1 §anchor-name): имена якоря,
    /// `none` — отсутствие. Коробка с именем получает пробу
    /// (`anchor::probe_for`), пишущую её рамку в реестр кадра.
    pub anchor_name: Option<Vec<String>>,
    /// `position-anchor`; начальное `normal`.
    pub position_anchor: Option<PositionAnchor>,
    /// Неявный якорь псевдоэлемента — `node_id` порождающего элемента
    /// (§implicit: «The implicit anchor element of a pseudo-element is its
    /// originating element»). Ставит `dom::walk` после обхода детей.
    pub implicit_anchor: Option<u64>,
    /// `position-area` (§position-area): два слова области, разбор и смысл —
    /// `anchor::parse_area`. `none`/не задано — `None`.
    pub position_area: Option<crate::anchor::PositionArea>,
    /// `position-try-fallbacks` (§position-try-fallbacks): варианты позиции —
    /// имя `@position-try`-правила и/или тактика, либо `<position-area>`.
    /// Перебор — `anchor::AnchorPlace` на подготовке кадра, выбранный
    /// вариант накладывается на стиль следующей сборки (`anchor::apply_chosen`).
    pub position_try_fallbacks: Vec<crate::anchor::TryFallback>,
    /// `position-try-order` (§position-try-order-property): 0 normal,
    /// 1 most-width, 2 most-height, 3 most-block-size, 4 most-inline-size.
    pub position_try_order: u8,
    /// `position-visibility` (§position-visibility), биты `anchor::VIS_*`:
    /// 1 anchor-valid, 2 anchor-visible, 4 no-overflow; 0 — `always`
    /// (начальное значение Blink; спека просит `anchor-visible`, но без
    /// явного свойства гасить коробки по обрезке якоря слишком дорого).
    pub position_visibility: u8,
    /// Стиль ДО наложения выбранного варианта `position-try` — из него
    /// `anchor::place` строит остальные кандидаты. Ставит `anchor::apply_chosen`.
    pub try_base: Option<std::rc::Rc<Computed>>,
    /// Служебные поля якорного шага, ставит `render::element`: свой `node_id`
    /// (ключ реестра содержащих блоков `anchor::CB`), `node_id` ближайшего
    /// содержащего блока абсолюта (`inline::inherit`; 0 — начальный, окно),
    /// порядковый номер сборки в кадре («последний якорь раньше по дереву» в
    /// реестре прошлого кадра) и ключ коробки в реестре размеров клетки.
    pub self_node: u64,
    pub cb_node: u64,
    pub anchor_seq: u32,
    pub anchor_key: u64,
    pub overflow_x: Option<Overflow>,
    /// Внутренняя копия прокручиваемой коробки (`render` снимает с неё
    /// `overflow`, чтобы обёртка `ScrollArea` резала сама): по спеке она
    /// остаётся scroll container — автоминимум по `aspect-ratio` к ней не
    /// применяется (css-sizing-4 §5.2; `block-aspect-ratio-011/012`).
    pub scroller: bool,
    pub overflow_y: Option<Overflow>,
    pub opacity: Option<f32>,

    pub background: Option<Color>,
    /// Фон ЗАЯВЛЕН автором (`background` или `background-color`), пусть даже
    /// прозрачным. Умолчание UA у полей ввода ставится только тогда, когда
    /// автор не сказал ничего: сокращение `background: linear-gradient(...)`
    /// сбрасывает цвет в прозрачный (§2.1), и подставлять поверх него
    /// служебную заливку нельзя.
    pub bg_explicit: bool,
    /// `border: inherit` / `padding: inherit`: свойства не наследуемые, слово
    /// копирует вычисленное значение родителя — оно известно только при
    /// слиянии стилей.
    pub(crate) border_inherit: bool,
    /// То же по СТОРОНАМ и по частям рамки: `border-width: inherit`,
    /// `border-bottom: inherit`, `border-left-color: inherit`. Порядок сторон
    /// всюду один: верх, право, низ, лево.
    pub(crate) border_inherit_w: [bool; 4],
    pub(crate) border_inherit_s: [bool; 4],
    pub(crate) border_inherit_c: [bool; 4],
    pub(crate) padding_inherit: bool,
    /// `inherit` по СТОРОНАМ у полей и отступов, порядок [верх, право, низ,
    /// лево]. Сокращение ставит все четыре.
    pub(crate) margin_inherit: [bool; 4],
    /// `inherit` у ненаследуемых свойств, которым своей ветки не было:
    /// повтор мостовой (`background-repeat`), слой (`z-index`), обводка,
    /// вид (`display`), плитка и её место (`background-image`,
    /// `background-position`), обрезка (`clip`), сокращение шрифта,
    /// преобразование регистра.
    pub(crate) inherit_bits: u32,
    pub(crate) padding_inherit_side: [bool; 4],
    /// `box-shadow: inherit`.
    pub(crate) shadow_inherit: bool,
    /// Относительный цвет фона (css-color-5): функция с `from currentColor`
    /// не решается при разборе — она наследуется КАК ФУНКЦИЯ и считается от
    /// цвета каждого элемента заново.
    pub background_rcs: Option<String>,
    /// `background-color: inherit`: фон не наследуемый, слово переносит
    /// вычисленное значение родителя (включая нерешённую функцию).
    pub(crate) background_inherit: bool,
    /// `background: inherit` — сокращение, значит наследуется весь фон, а не
    /// только цвет.
    pub(crate) background_all_inherit: bool,
    /// Явное `inherit` на ненаследуемых размерах и краях: значение берётся
    /// от родителя при слиянии (`inline::inherit`), как у padding/border.
    pub(crate) width_inherit: bool,
    pub(crate) height_inherit: bool,
    /// То же у пределов: `min-width`, `min-height`, `max-width`, `max-height`.
    pub(crate) minmax_inherit: [bool; 4],
    /// Сторона письма СОДЕРЖАЩЕГО БЛОКА: избыточный край выбирается по ней,
    /// а не по своей (§9.4.3). Ставится при наследовании.
    pub(crate) cb_rtl: bool,
    /// Письмо СОДЕРЖАЩЕГО БЛОКА: вертикальное ли (в том числе повёрнутый
    /// абзац, чей стиль собран горизонтальным клоном с `rotated_line`), идут ли
    /// блоки справа налево, `sideways-*` ли. По ним у переопределённой оси
    /// абсолюта выбирается отбрасываемый край (css-writing-modes-4 §7.1).
    /// Ставится при наследовании, как `cb_rtl`.
    pub(crate) cb_vertical: bool,
    pub(crate) cb_vertical_rl: bool,
    pub(crate) cb_sideways: bool,
    /// top/right/bottom/left.
    pub(crate) inset_inherit: [bool; 4],
    pub gradient: Option<Gradient>,
    pub shadows: Vec<Shadow>,
    /// Внутренние тени (`box-shadow: inset`) — отдельным списком: рисуются
    /// поверх заливки, а не под фигурой.
    pub inset_shadows: Vec<Shadow>,

    pub color: Option<Color>,
    pub font_size: Option<Len>,
    pub font_weight: Option<u16>,
    pub(crate) font_weight_step: i8,
    pub italic: Option<bool>,
    /// `font-style: oblique` отдельно от `italic`: набору наклон один
    /// (`italic` держит оба), а подбору лица это РАЗНЫЕ запросы (css-fonts-4
    /// §font-style-matching) — от выбранного лица зависит `size-adjust`
    /// (`fonts::size_adjust`, `italic-oblique-fallback`).
    pub oblique: Option<bool>,
    pub underline: Option<bool>,
    pub line_through: Option<bool>,
    pub line_height: Option<Len>,
    /// `orphans`/`widows` (css-break-3 §4.4 «Breaks Between Lines»): сколько
    /// строк блока обязано остаться до/после разрыва внутри него. Наследуются,
    /// начальное значение 2 (`None` = 2).
    pub orphans: Option<u16>,
    pub widows: Option<u16>,
    pub text_align: Option<TextAlign>,
    /// `text-align-last` — выключка ПОСЛЕДНЕЙ строки абзаца. Отдельное
    /// свойство, потому что по умолчанию последняя строка не растягивается:
    /// иначе абзац из одного слова разъехался бы во всю ширину.
    pub text_align_last: Option<TextAlign>,
    /// `text-justify: none` — выключка запрещена, строка идёт как `start`.
    pub no_justify: Option<bool>,
    /// CSS Text 4: ruby annotation justification excludes word spaces.
    pub ruby_justify: Option<bool>,
    /// `text-justify` expansion opportunities (css-text-3 §7.3): `Some(0)`
    /// `inter-word` (word separators only), `Some(2)` `inter-character` /
    /// `distribute` (between typographic character units), `None`/`Some(1)`
    /// `auto` (word separators plus CJK ideographs, as Blink).
    pub justify_chars: Option<u8>,
    /// Internal ruby unit promoted to a technical block for layout.
    pub ruby_unit: bool,
    /// `hanging-punctuation` — какая пунктуация выходит за край строки.
    /// `text-box-trim` — срезать полулидинг первой/последней строки блока.
    pub text_box_trim_start: bool,
    pub text_box_trim_end: bool,
    /// `text-box-edge` — метрики верхнего и нижнего краёв среза.
    pub text_box_over: TextEdge,
    pub text_box_under: TextEdge,
    /// `text-box-edge` задан явно (в т.ч. `auto`/`text`): свойство
    /// наследуемое, и явное значение перекрывает унаследованное, а начальное
    /// `Text` от него неотличимо без флага
    /// (`text-box-trim-not-ignore-nested-text-box-edge`).
    pub text_box_edge_set: bool,
    pub hanging: Option<Hanging>,
    pub nowrap: Option<bool>,
    /// Переводы строк значимы (`white-space: pre*`).
    pub preserve_newlines: Option<bool>,
    /// Пробелы значимы. У `pre-line` переводы строк значимы, а пробелы нет —
    /// без отдельного поля он схлопывал и то и другое.
    pub keep_spaces: Option<bool>,
    pub monospace: Option<bool>,
    /// First available family supplies font-relative metrics.
    pub font_family: Option<String>,
    pub(crate) font_families: Option<Vec<String>>,
    /// Есть ли у рамки ВИДИМЫЙ рисунок. Толщина без рисунка не считается:
    /// начальное значение `border-style` — `none`, и по CSS такая рамка
    /// вычисляется в ноль (`descendant-static-position-001`: коробка выходила
    /// шире на заданную, но не нарисованную рамку).
    pub border_visible: [Option<bool>; 4],
    /// `border-color: currentColor` — цвет берётся из `color` того же узла.
    ///
    /// Внутренний флаг каскада, а не свойство отрисовки: к моменту выхода из
    /// `resolve` он уже подставлен в `border_color`.
    pub(crate) border_color_is_current: bool,
    /// Сторона получила `currentColor` из БОКОВОГО сокращения без цвета
    /// (`border-top: solid 1em`) поверх общего `border-color`, пришедшего
    /// раньше. Внутренний флаг каскада: цвет текста в `border_colors`
    /// подставляет `inline::inherit`, когда тот известен.
    pub(crate) border_side_current: [bool; 4],
    /// `border-collapse: collapse` — зазор между ячейками пропадает.
    pub border_collapse: Option<bool>,
    /// `empty-cells: hide` — у ПУСТОЙ ячейки не рисуются ни фон, ни рамка
    /// (CSS 2.1 §17.6.1.1). Свойство наследуемое.
    pub empty_cells_hide: Option<bool>,
    /// Форма курсора: у GPUI набор совпадает с CSS почти буква в букву.
    pub cursor: Option<String>,
    /// `visibility: hidden` — место занимает, но не рисуется.
    pub hidden: Option<bool>,
    /// `visibility: collapse` — не «невидимый», а ВЫБРОШЕННЫЙ из строки
    /// гибкого контейнера: перенос и размеры считаются без него.
    pub collapsed: Option<bool>,
    /// Опорная коробка clip-path: 0 border, 1 margin, 2 padding, 3 content.
    pub clip_ref: Option<u8>,
    /// `clip-path` задан ОДНИМ словом коробки (`margin-box`, `padding-box`,
    /// …): обрезка краями этой коробки (css-masking-1 §5.1 «If specified by
    /// itself, uses the edges of the specified box … as clipping path»).
    pub clip_bare_box: bool,
    /// Маска-изображение (`mask-image: url(...)|<gradient>`): источник
    /// строкой до растра при сборке группы.
    pub mask_image: Option<String>,
    pub letter_spacing: Option<Len>,
    pub ellipsis: Option<bool>,
    /// Маркер обрезки `text-overflow: <string>` (css-overflow-4 §5);
    /// None при ellipsis — многоточие по умолчанию.
    pub overflow_marker: Option<String>,
    /// `text-overflow: inherit` (css-cascade-4 §7.3): свойство не
    /// наследуемое, значение родителя переносит `doc::settle_explicit_inherit`
    /// (`text-overflow-004`).
    pub text_overflow_inherit: bool,
    /// `block-ellipsis` (css-overflow-4 §block-ellipsis): None — `auto`
    /// (U+2026), пустая строка — `no-ellipsis`, иначе строка-знак. Отдельно
    /// от `overflow_marker`: `text-overflow` относится к строчной оси и
    /// знака на строке обрыва `line-clamp` не задаёт.
    pub clamp_mark: Option<String>,
    /// `list-style: none` — навигация, свёрстанная на списках, иначе идёт с
    /// точками.
    pub no_marker: Option<bool>,
    /// Вид маркера, если документ его задал.
    pub list_style_type: Option<String>,
    /// `list-style-position: inside` — маркер идёт первым куском содержимого
    /// пункта, а не отдельной колонкой снаружи (css-lists-3 §4).
    pub list_style_inside: Option<bool>,
    /// Строковый маркер: `list-style-type: "→ "` (css-lists-3 §3).
    pub marker_text: Option<String>,
    /// Слой `::marker` (css-lists-3 §marker-properties): ТОЛЬКО объявления
    /// самих правил `::marker` поверх таблицы агента, без копии стиля
    /// хозяина — при отрисовке накладывается на стиль пункта через
    /// `inline::inherit`. Копия стиля пункта сюда не годится: она утянула бы
    /// в маркер рамку, поля и размеры самого `<li>`. Blink делает то же —
    /// текст маркера набирается стилем САМОГО `::marker`
    /// (`CreateAnonymousStyleWithDisplay(marker.StyleRef(), …)`,
    /// `list_marker.cc:267-336`). Не наследуется.
    pub marker_layer: Option<Box<Computed>>,
    /// `content: none` против `content: normal`. У `::before`/`::after`
    /// разницы нет — коробки нет в обоих случаях, — а у `::marker` `none`
    /// гасит маркер, `normal` возвращает к `list-style-*` (css-lists-3
    /// §content-property). Оба сбрасывают `content` в None, поэтому нужна
    /// отдельная метка.
    pub content_none: Option<bool>,
    pub object_fit: Option<String>,
    /// `image-orientation: none` (css-images-3 §5.4) — НЕ разворачивать растр
    /// по метке EXIF. Начальное значение свойства — `from-image`, поэтому
    /// хранится именно отказ, а не разрешение.
    pub image_orient_none: Option<bool>,

    /// `aspect-ratio` — отношение ширины к высоте.
    pub aspect_ratio: Option<f32>,
    /// Отношение из записи `auto <ratio>` (css-sizing-4 §5.1): у замещаемого
    /// оно ЗАПАСНОЕ — природное сильнее. Отдельным полем, потому что
    /// НЕзамещаемой коробке отношение из этой записи наша раскладка пока не
    /// выражает (★ ЗАМЕРЕНО: в общем поле `block-aspect-ratio-002/015/016/018/
    /// 043/047`, `grid-aspect-ratio-005/008` уходили с 0.00 в 14.25).
    pub aspect_ratio_auto: Option<f32>,
    /// Коробка с `aspect-ratio` — элемент ГИБКОГО контейнера (ставит
    /// `render.rs` при раскладке детей ряда/колонки). Её автоминимум по
    /// соотношению считает раскладка (css-flexbox-1 §4.5: подсказка по
    /// содержимому), а не явный минимум `apply::ratio_as_auto_min`
    /// (`flex-aspect-ratio-002/004`).
    pub flex_item_ratio: bool,
    /// Коробка АБСОЛЮТНА, но позиционирование с неё снято ради статической
    /// позиции (`render.rs`). Само `position` там обнуляется, а знать о нём
    /// нужно: размер по свободной строчной оси у абсолюта считается по
    /// содержимому, и без этой пометки вертикальный абзац снова растягивался
    /// бы на весь предел ортогонального потока.
    pub abs_static: bool,
    /// Ортогональный элемент СЕТКИ с невытягивающим выравниванием
    /// (`place-items: start` и родня): по строчной оси он размером в
    /// содержимое, а не в область сетки (css-grid-1 §6.6, css-align-3 §6.1).
    /// Ставится сборщиком детей сетки, глубже не наследуется.
    pub hug_inline: bool,
    /// Родитель — ВЕРТИКАЛЬНАЯ сетка с невытягивающим `justify-*` (или блок
    /// по цепочке под ней, `render.rs: vertical_hug_children`): повёрнутый
    /// абзац заявляет высотой длину своей строки. Отдельно от `hug_inline`:
    /// ортогональным элементам горизонтальной сетки (и лункам) такая заявка
    /// противопоказана (`grid-lanes/.../column-explicit-placement-002`
    /// 0.00 -> 10.02 при общем флаге).
    pub hug_claim: bool,
    /// `order`: визуальный порядок в гибкой строке. Раскладка под нами его не
    /// знает, поэтому детей переставляет сам сборщик дерева.
    pub order: Option<i32>,
    pub align_content: Option<Justify>,
    /// Задано ли `align-content` значением, ОТЛИЧНЫМ от `normal`
    /// (css-align-3 §align-block). Отдельно от `align_content`, потому что
    /// `parse_justify` роняет в `None` два разных случая: `normal`
    /// (выравнивания нет — и контекста тоже) и `baseline`/`first`/`last`
    /// (выравнивание есть, раскладка его пока не знает, но КОНТЕКСТ по спеке
    /// заводится). На блочном контейнере флаг делает коробку корнем блочного
    /// контекста форматирования; у флекса и сетки он безразличен — они и так
    /// заводят свой контекст первой же веткой `own_context`.
    pub align_content_block: bool,
    /// `justify-items`/`justify-self` — поперечная ось В СЕТКЕ.
    pub justify_items: Option<Align>,
    /// Модификатор `safe` у выравниваний (css-align §5.3): при переполнении
    /// области выравнивание падает в `start`, чтобы содержимое не обрезалось.
    /// Без него позиция сохраняется и элемент вылезает (unsafe/дефолт).
    pub justify_self_safe: bool,
    pub justify_items_safe: bool,
    pub align_self_safe: bool,
    /// `align-self: self-start`/`self-end` — начало и конец берутся по письму
    /// САМОГО элемента, а не контейнера (css-align-3 §6.2). Значение при этом
    /// остаётся физическим, а «мерить по себе» помнится здесь: зеркалит его
    /// `inline::inherit`, где известны письмо элемента И письмо родителя.
    pub align_self_own_axis: bool,
    /// Ключевое слово `align-self` — ГИБКОЕ (`flex-start`/`flex-end`): его
    /// концы следуют `wrap-reverse` строки, а `start`/`end`/`self-*` — нет
    /// (css-align-3 §6.1). `Align` различия не несёт, его зеркалит
    /// `inline::inherit` (`self-align-start-end-flex-001`).
    pub align_self_flex_kw: bool,
    /// `justify-self: self-start`/`self-end` — по письму САМОГО элемента, как
    /// `align_self_own_axis` (`align-self-static-position-006`).
    pub justify_self_own_axis: bool,
    /// Preserve line-left/line-right separately from flow-relative start/end.
    pub justify_self_physical: Option<bool>,
    /// `last baseline`: запасное выравнивание — `end`, а не `start`
    /// (css-align-3 §9.3; `align-self-static-position-008`,
    /// `justify-self-static-position-001`). `Align::Baseline` его не различает.
    pub align_self_last: bool,
    /// Авторское объявление `align-self` (внешний `Some`), с его значением.
    /// Нужно, чтобы отличать значение автора от приёмов сборки, которые
    /// пишут в то же поле `align_self` (блок собран колонкой flex): только
    /// авторское значение гасится у коробки вне гибкого контейнера и сетки
    /// (`inline::inherit`), и только оно же переходит по `inherit` к детям —
    /// вычисленное значение родителя от гашения не меняется (css-align-3
    /// §6.1 «Applies to: flex items, grid items, and absolutely-positioned
    /// boxes»; css-cascade-4 §7.3).
    pub(crate) align_self_decl: Option<Option<Align>>,
    /// `align-self: inherit` — значение берёт `inline::inherit` у родителя
    /// (свойство ненаследуемое, слово копирует вычисленное значение).
    pub(crate) align_self_inherit: bool,
    /// `inherit` у `align-items`/`justify-items`/`align-content`/
    /// `justify-content`/`justify-self` (разряды `ainh::*`): свойства
    /// ненаследуемые, слово копирует вычисленное значение родителя
    /// (css-cascade-4 §7.3.1) — его берёт `inline::inherit`.
    pub(crate) align_inherit: u8,
    pub justify_self_last: bool,
    /// `align-items: last baseline` — то же для умолчания детей: раскладка
    /// получает `LastBaseline` (css-align-3 §4.2), а не первую базовую.
    pub align_items_last: bool,
    /// `justify-items: last baseline` — для лунок-колонок (поперёк лунки).
    pub justify_items_last: bool,
    pub align_items_safe: bool,
    pub justify_content_safe: bool,
    pub align_content_safe: bool,
    /// Порог «равных» лунок (`flow-tolerance`): None = `normal`/не задано (= 1em).
    pub lanes_tolerance: Option<Len>,
    /// `grid-lanes-direction: row` — лунки идут РЯДАМИ, элементы укладываются
    /// вдоль строки, а не вдоль колонки.
    pub lanes_row: Option<bool>,
    /// `fill-reverse` — лунки заполняются С ДРУГОГО КОНЦА: первым выбирается
    /// самое правое (нижнее) свободное место, а не левое.
    pub lanes_fill_reverse: bool,
    /// `track-reverse` — сами лунки перечислены в обратном порядке: первая
    /// дорожка списка встаёт последней.
    pub lanes_track_reverse: bool,
    /// `grid-lanes-pack: dense` — элемент встаёт в САМОЕ ВЕРХНЕЕ свободное
    /// место, а не под всё уже уложенное: дыры, оставленные многолуночными
    /// соседями, заполняются следующими элементами.
    pub lanes_dense: bool,
    /// `display: inline grid-lanes` — контейнер лунок строчного уровня:
    /// ширина по дорожкам, не на всю строку.
    pub lanes_inline: bool,
    /// Контейнер лунок, переведённый на путь СЕТКИ: `display` уже
    /// `Grid`/`InlineGrid`, а раскладку лунками делает taffy
    /// (`vendor/taffy/src/compute/grid/lanes.rs`). Ставит
    /// `dom::lanes_as_grid`.
    pub lanes_taffy: bool,
    pub justify_self: Option<Align>,
    /// Explicit normal must not take the parent's justify-items value.
    pub justify_self_normal: bool,

    pub grid_rows: Option<Vec<TrackSize>>,
    pub grid_auto_cols: Option<TrackSize>,
    /// Список неявных дорожек, когда их больше одной: `grid-auto-columns: A B C`.
    /// Пусто — дорожка одна, она в `grid_auto_cols`.
    pub grid_auto_cols_list: Vec<TrackSize>,
    /// То же для неявных РЯДОВ.
    pub grid_auto_rows_list: Vec<TrackSize>,
    pub grid_auto_rows: Option<TrackSize>,
    pub grid_auto_flow: Option<AutoFlow>,
    pub grid_col: Option<(Placement, Placement)>,
    pub grid_row: Option<(Placement, Placement)>,

    /// `z-index`: порядок наложения. У GPUI слоёв нет — вместо них отложенная
    /// отрисовка с приоритетом.
    pub z_index: Option<i32>,
    /// Цвета рамки по сторонам. У GPUI цвет рамки один на элемент, поэтому
    /// разные цвета сторон дорисовываются полосами.
    pub border_colors: [Option<Color>; 4],
    /// `font-size: larger` (+1) / `smaller` (−1): шаг по таблице ключевых
    /// кеглей от кегля родителя (§15.7). Разрешает `inline::inherit`.
    pub(crate) font_size_step: i8,
    /// Ранги стилей кромок по сторонам [верх, право, низ, лево] для
    /// разбора конфликтов сросшихся рамок (CSS 2.1 §17.6.2.1):
    /// 0 none, 1 hidden, 3 inset, 4 groove, 5 outset, 6 ridge, 7 dotted,
    /// 8 dashed, 9 solid, 10 double. `None` — стиль не задавался.
    pub border_side_styles: [Option<u8>; 4],
    /// `caption-side: bottom` — заголовок таблицы под сеткой.
    pub caption_bottom: Option<bool>,
    pub border_dashed: Option<bool>,
    /// `border-style: dotted` — точечный узор.
    pub border_dotted: Option<bool>,
    /// Сырая запись градиента фона: источник для слоя-картинки там, где
    /// градиент рисуется плиткой (фон ряда таблицы).
    pub gradient_raw: Option<String>,
    /// Градиент фона с длинами в единицах шрифта (`green 4em`): позиция
    /// стопа в `em` разбором не читается и терялась (стоп становился «без
    /// позиции»). Ждёт своего кегля и переразбирается в `resolve_em`
    /// (`white-space-intrinsic-size-017/018`).
    pub gradient_em: Option<String>,
    /// `border-spacing` таблицы: горизонтальный и вертикальный зазор.
    pub border_spacing: Option<(Option<Len>, Option<Len>)>,
    pub outline: Option<Outline>,
    /// `backdrop-filter: blur(N)` — размытие того, что под элементом.
    pub backdrop_blur: Option<f32>,
    /// `backdrop-filter`: цветовые функции списка (без размытия). Рисуются
    /// матрицей 4×5 тем же проходом подложки (`Filter::color_matrix`).
    pub backdrop_color: Option<Filter>,

    pub word_spacing: Option<Len>,
    /// Межбуквенный интервал ПОСЛЕ последнего знака этого куска.
    ///
    /// На границе двух элементов зазор задаёт не сам кусок, а ближайший общий
    /// предок обоих знаков (css-text-3 §8.2: зазор «задаётся и рисуется внутри
    /// самого внутреннего элемента, который содержит эту границу»). Видно эту
    /// границу только сборке кусков, она поле и ставит; из стилей документа
    /// оно не приходит и по дереву не наследуется.
    pub letter_spacing_after: Option<Len>,
    pub text_transform: Option<TextTransform>,
    /// Добавки `text-transform` к регистру (css-text-3 §2.1: значение —
    /// `[ case ] || full-width || full-size-kana`, плюс `math-auto`
    /// MathML Core §2.1.5): `TT_FULL_WIDTH` | `TT_KANA` | `TT_MATH`. Живут
    /// вместе с `text_transform` и наследуются вместе с ним.
    pub text_transform_flags: u8,
    pub text_indent: Option<Len>,
    /// `text-indent: … each-line` — отступ повторяется после жёстких разрывов.
    pub text_indent_each_line: Option<bool>,
    /// `text-indent: … hanging` — отступ получают все строки, КРОМЕ первой.
    pub text_indent_hanging: Option<bool>,
    /// `word-break: break-all` и родня — рвать слово, а не переносить целиком.
    pub break_anywhere: Option<bool>,
    /// `overflow-wrap: break-word|anywhere` — рвать слово, ТОЛЬКО если иначе
    /// оно не влезает в строку целиком.
    pub break_word: Option<bool>,
    /// `text-wrap: balance` — строки абзаца одной длины.
    pub balance_lines: Option<bool>,
    /// `unicode-bidi: bidi-override` (и тег `<bdo>`) — порядок знаков задан
    /// силой, разбор двунаправленности внутри куска не работает.
    pub bidi_override: Option<bool>,
    /// Explicit inheritance of the otherwise non-inherited unicode-bidi property.
    pub(crate) bidi_inherit: bool,
    /// `unicode-bidi: isolate` — кусок не влияет на порядок соседей.
    pub bidi_isolate: Option<bool>,
    /// `unicode-bidi: embed` — свой уровень встраивания (RLE/LRE … PDF). Без
    /// него (`normal`) `direction` строчного элемента порядка знаков НЕ
    /// меняет (css-writing-modes-3 §2.2: «normal — the element does not open
    /// an additional level of embedding»). Атрибут `dir` ставит его сам
    /// (`dom.rs: apply_direction`).
    pub bidi_embed: Option<bool>,
    /// `unicode-bidi: plaintext` — сторона письма решается для каждого абзаца
    /// между жёсткими разрывами. HTML ставит это правило на `dir="auto"`.
    pub bidi_plaintext: Option<bool>,
    /// `line-break: anywhere` — разрыв разрешён В ЛЮБОМ месте, включая
    /// соседство с пробелом. Это НЕ то же самое, что `word-break: break-all`:
    /// тот рвёт только внутри слова.
    pub break_anywhere_strict: Option<bool>,
    /// `line-break: normal` (1) / `loose` (2): уровень строгости переноса
    /// CJK (css-text-3 §5.2); `auto`/`strict`/`anywhere` — `None`.
    pub line_break_loose: Option<u8>,
    /// `word-break: keep-all` — иероглифическое письмо переносится ТОЛЬКО по
    /// пробелам, между знаками разрыв запрещён.
    pub keep_all: Option<bool>,
    /// `white-space: break-spaces` — сохранённый пробел НЕ свисает за край:
    /// он занимает место в строке, и после каждого такого пробела разрешён
    /// перенос. Отличие от `pre-wrap`, где хвостовые пробелы висят снаружи.
    pub break_after_spaces: Option<bool>,
    /// `-webkit-line-clamp`: сколько строк оставить.
    pub line_clamp: Option<u32>,
    /// `-webkit-line-clamp`: действует ТОЛЬКО в паре с
    /// `display: -webkit-box` и `-webkit-box-orient: vertical`
    /// (css-overflow-3 §webkit-line-clamp) — поэтому своё поле и гейт.
    /// Какое сокращение записало `line_clamp` последним: `-webkit-line-clamp`
    /// ставит `continue: -webkit-legacy`, который действует только при
    /// `display: -webkit-box` с вертикальной ориентацией (css-overflow-4
    /// §5.1); оба сокращения — одни лонгхенды, побеждает последнее.
    pub clamp_legacy: Option<bool>,
    /// `line-clamp: auto` — обрезка по max-height контейнера
    /// (css-overflow-4 §line-clamp), без счёта строк.
    pub clamp_auto: Option<bool>,
    /// `fill` для SVG-фигур (CSS-презентация, SVG 2).
    pub svg_fill: Option<String>,
    /// `stroke` и `stroke-width` фигуры (SVG 2 §presentation attributes):
    /// правила из `<style>` с селекторами до растеризатора иначе не доедут —
    /// он видит только сериализованную разметку.
    pub svg_stroke: Option<String>,
    pub svg_stroke_width: Option<String>,
    /// CSS-геометрия фигуры (SVG 2 §Geometry properties): `x` и `y`.
    /// Ширина и высота уже живут в `width`/`height`.
    pub svg_x: Option<Len>,
    pub svg_y: Option<Len>,
    pub webkit_box: Option<bool>,
    pub webkit_box_vertical: Option<bool>,
    /// `text-fit` — подбор кегля под ширину коробки.
    pub text_fit: Option<TextFit>,
    /// `hyphenate-character` — чем показывать перенос слова. Пусто — ничем.
    pub hyphen_char: Option<String>,
    /// `hyphens: auto` — слогораздел ставит сам движок, а не разметка.
    pub hyphens_auto: Option<bool>,
    /// Язык узла (атрибут `lang`): по нему выбираются образцы слогораздела.
    pub lang: Option<String>,
    /// `vertical-align` внутри строки и ячейки таблицы.
    pub vertical_align: Option<Align>,
    pub pointer_events_none: Option<bool>,
    /// `table-layout: fixed` — колонки равной ширины, без замера содержимого.
    pub table_fixed: Option<bool>,
    /// `column-count` — на сколько колонок резать содержимое.
    pub column_count: Option<u16>,
    /// `column-width` — минимальная ширина колонки.
    pub column_width: Option<Len>,
    /// `column-height` (css-multicol-2 §ch) — заданная высота колонки;
    /// `auto` хранится отсутствием значения.
    pub column_height: Option<Len>,
    /// `column-wrap` (css-multicol-2 §cwr): `Some(true)` — `wrap`, лишние
    /// колонки уходят в новый ряд; `Some(false)` — `nowrap`, вбок; `None` —
    /// `auto`: как `wrap` при заданном `column-height`, иначе `nowrap`.
    pub column_wrap: Option<bool>,
    /// `column-gap` — зазор между колонками многоколоночного потока.
    /// Умолчание CSS — `normal`, то есть один кегль.
    pub column_gap: Option<Len>,
    /// `translate` — визуальный сдвиг, не меняющий раскладку.
    pub translate: Option<(Len, Len)>,
    /// `text-shadow`: смещение, размытие и цвет — ПЕРВАЯ (верхняя) тень списка.
    pub text_shadow: Option<Shadow>,
    /// Остальные тени `text-shadow` в порядке записи: свойство — СПИСОК, и
    /// «the first shadow is on top» (css-text-decor-3 Overview.bs:881-882).
    /// Первая живёт в `text_shadow`: на неё смотрят зум и наследование.
    /// Прежде хвост отбрасывался — `-1em 0em orange, 1em 0em blue` у эталона
    /// `box-shadow-multiple-001-ref` рисовал одну оранжевую.
    pub text_shadow_rest: Vec<Shadow>,
    /// `text-shadow` с длинами в единицах шрифта ждёт своего кегля, как
    /// `shadow_raw` у `box-shadow` (вычисленное значение — «three absolute
    /// lengths», css-text-decor-3 Overview.bs:868-869), и разбирается в
    /// `resolve_em`.
    pub text_shadow_raw: Option<String>,
    /// `text-shadow: none`, записанное самим элементом: гасит унаследованный
    /// список (без флага пустой разбор читался как «не задано»).
    pub text_shadow_none: bool,
    /// `animation` — ссылка на набор кадров.
    pub animation: Option<AnimSpec>,
    /// `transition` — длительность перехода в секундах.
    pub transition: Option<f32>,
    /// `direction: rtl` — письмо справа налево.
    pub rtl: Option<bool>,
    /// `resize` — по каким осям элемент тянется мышью.
    pub resize: Option<(bool, bool)>,
    /// `transform`/`rotate`/`scale`: поворот в радианах и масштаб по осям.
    pub transform: Option<Transform>,
    /// `rotate` вокруг оси z (радианы) и `scale` по осям — отдельными полями:
    /// разложение в `transform` порядок теряет, а css-transforms-2 §ctm ставит
    /// их СЛЕВА от списка `transform` (п.4-5 перед п.7). `None` — `none`.
    pub rotate_prop: Option<f32>,
    pub scale_prop: Option<(f32, f32)>,
    /// `offset-path` как записано (motion-1 §offset-path): `path('…')`,
    /// `ray(…)`, `<basic-shape>` или `url(#id)`. Разбирается не здесь:
    /// сэмплеру нужны все остальные `offset-*`, а каскад сводит их вразнобой.
    pub offset_path: Option<String>,
    /// `offset-distance`: длина или доля ДЛИНЫ ПУТИ (а не коробки).
    pub offset_distance: Option<Len>,
    /// `offset-rotate` как записано: `auto | reverse | <angle> | auto <angle>`.
    pub offset_rotate: Option<String>,
    /// `offset-anchor` как записано; `auto` — это точка `transform-origin`.
    pub offset_anchor: Option<String>,
    /// `offset-position` как записано: `normal | auto | <position>`.
    pub offset_position: Option<String>,
    /// `backface-visibility: hidden`.
    pub backface_hidden: Option<bool>,
    /// `transform-origin` в долях размера элемента.
    pub transform_origin: Option<(f32, f32)>,
    /// `transform`/`transform-origin` с длинами в единицах шрифта: запись
    /// ждёт своего кегля и разбирается в `resolve_em` (css-transforms-1
    /// §computed value: относительные длины становятся абсолютными).
    pub transform_raw: Option<String>,
    /// `box-shadow` с длинами в единицах шрифта — так же ждёт своего кегля и
    /// разбирается в `resolve_em` (`box-shadow-calc`: `calc(1em + 10px)`
    /// прежде ронял тень целиком — `parse_shadows` пропускает `em`).
    pub shadow_raw: Option<String>,
    /// Есть ли выше трансформированный предок: он — содержащий блок и для
    /// `position: fixed` (css-transforms-1 §transform-rendering: «…for all
    /// of its absolute-position descendants, fixed-position descendants»).
    pub transform_ancestor: bool,
    pub transform_origin_raw: Option<String>,
    /// Точка отсчёта преобразования В ТОЧКАХ по осям — когда записана длиной,
    /// а не долей. Долю из неё делает отрисовка: размер коробки известен там.
    pub transform_origin_px: (Option<f32>, Option<f32>),
    /// Третья координата `transform-origin` в точках (css-transforms-2);
    /// на плоскую матрицу не влияет, на 4×4 — `T(o)·M·T(−o)` по трём осям.
    pub transform_origin_z: Option<f32>,
    /// `perspective` (css-transforms-2 §perspective-property) — расстояние
    /// до глаза в css-точках для ОБЪЁМНЫХ ДЕТЕЙ, уже не меньше 1px («values
    /// less than 1px must be treated as 1px»); `none` = None.
    pub perspective: Option<f32>,
    /// `perspective-origin` долями коробки (умолчание 50% 50%) и в точках по
    /// осям, когда записан длиной — как `transform_origin`/`_px`.
    pub perspective_origin: Option<(f32, f32)>,
    pub perspective_origin_px: (Option<f32>, Option<f32>),
    /// `transform-box: fill-box` у SVG-фигуры (css-transforms-1
    /// §transform-box): длины `transform-origin` отсчитываются от рамки
    /// фигуры, а не от вьюпорта.
    pub transform_box_fill: Option<bool>,
    /// Опорная коробка `transform-box` целиком (css-transforms-1
    /// §transform-box) — для SVG-элементов без CSS-коробки: 0 — `view-box`,
    /// 1 — `fill-box` (и `content-box`: «the used value for content-box is
    /// fill-box»), 2 — `stroke-box` (и `border-box`). None — не задано.
    pub transform_box: Option<u8>,
    /// `vector-effect: non-scaling-stroke` (SVG 2 §vector-effect): толщина
    /// обводки задана в точках экрана.
    pub svg_non_scaling: Option<bool>,
    /// Ячейка матрицы перспективы (см. `PerspectiveFrame`); один и тот же
    /// `Rc` у `e.style` родителя, его `merged` и `inherited` детей.
    pub perspective_frame: Option<PerspectiveFrame>,
    /// `transform-style: preserve-3d` — элемент образует объёмный контекст
    /// (css-transforms-2 §transform-style-property). ИСПОЛЬЗУЕМОЕ значение
    /// гасят «групповые» свойства — это решает `render::flattens_3d`,
    /// потому что они могут быть записаны в блоке ПОСЛЕ `transform-style`.
    pub preserve_3d: Option<bool>,
    /// Ячейка накопленной 4×4 (см. `Frame3d`): заводится при разборе
    /// `transform-style`, наполняется `Transformed::paint` владельца,
    /// читается объёмным путём ПРЯМЫХ детей.
    pub frame_3d: Option<Frame3d>,
    /// `float`: -1 — влево, 1 — вправо, 0 — не обтекается.
    pub float: Option<i8>,
    /// Unemitted adjoining margin at the float's source position (CSS 2.1 §9.5.1).
    pub(crate) float_margin_offset: Option<f32>,
    /// Explicit `clear: inherit`; ordinary `clear` is non-inherited.
    pub(crate) clear_inherit: bool,
    /// `background-attachment: fixed` — плитка считается от области
    /// просмотра, а не от коробки.
    pub bg_fixed: Option<bool>,
    /// `clear` — сторона, с которой обтекание обрывается перед этим блоком:
    /// -1 слева, 1 справа, 0 с обеих. Стороны различаются, потому что
    /// `clear: left` мимо правого флоата проходит насквозь (CSS 2.1 §9.5.2).
    pub clear: Option<i8>,
    /// `writing-mode`: вертикальное письмо — блоки идут по горизонтали.
    pub vertical: Option<bool>,
    /// `writing-mode: vertical-rl` — блоки идут справа налево.
    pub vertical_rl: Option<bool>,
    /// `writing-mode: sideways-*`: глифы повёрнуты, а у `sideways-lr` строка
    /// идёт СНИЗУ вверх — содержимое прижимается к нижнему краю.
    pub sideways: Option<bool>,
    /// `text-combine-upright`: сколько знаков сжимается в один кегль
    /// (0 — `all`, 2..=4 — `digits N`). Наследуется.
    pub combine_upright: Option<u8>,
    /// Служебное: абзац собран для ПОВЁРНУТОЙ отрисовки вертикального
    /// письма — местам с `text-combine-upright` нужен контр-поворот.
    pub rotated_line: Option<bool>,
    /// Ограничение ОРТОГОНАЛЬНОГО потока: определённый размер ближайшего
    /// предка-контейнера прокрутки по оси потока (CSS Writing Modes §7.3).
    /// Наследуется вниз, потому что искать его надо ВВЕРХ по дереву, а на
    /// момент раскладки ребёнка предков уже не видно.
    pub ortho_limit: Option<f32>,
    /// Nearest ancestor scrollport, including an indefinite nearest scroller.
    pub(crate) orthogonal_scrollport: Option<[orthogonal::AxisSizes; 2]>,
    /// Used inline measurement contract of an ordinary orthogonal block.
    pub(crate) orthogonal_inline: Option<orthogonal::InlineConstraint>,
    /// Ячейка таблицы, ПАРАЛЛЕЛЬНОЙ своему письму: доступное инлайн-место у
    /// неё ОПРЕДЕЛЕНО — это мера её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), — и запасной предел §7.3
    /// (`ortho_limit`) применять нельзя: тот стоит на месте НЕОПРЕДЕЛЁННОГО
    /// инлайн-места (css-writing-modes-4 §7.3.1, «an additional constraint is
    /// used as a fallback in place of the available inline space»). Blink
    /// делит эти случаи ровно так же: `space_utils.h:36
    /// SetOrthogonalFallbackInlineSizeIfNeeded` выходит НЕ СДЕЛАВ НИЧЕГО при
    /// `IsParallelWritingMode(таблица, ячейка)`, а
    /// `table_layout_utils.cc:1363 SetupTableCellConstraintSpaceBuilder`
    /// кладёт ячейке `SetAvailableSize({cell_inline_size, …})`, где
    /// `cell_inline_size` собран из `column_locations` — то есть из дорожки.
    /// Флаг ставит `table()` своим ячейкам; ГЛУБЖЕ НЕ НАСЛЕДУЕТСЯ (как
    /// `hug_inline`): `inline::inherit` начинает с `own.clone()`, а у
    /// вложенного элемента поле пусто.
    pub ortho_col: bool,
    /// Physical paragraph of a `text-orientation: upright` vertical stack
    /// (its clone clears `vertical`). Not inherited, like `ortho_col`.
    pub(crate) upright_stack: bool,
    /// Повёрнутый абзац `vertical-lr`: строки-колонки идут слева направо —
    /// подача строк снизу вверх (см. `Paragraph::reversed_lines`).
    pub lines_reversed: Option<bool>,
    /// `overflow-wrap: anywhere` — в отличие от `break-word`, меняет размер
    /// по минимальному содержимому.
    pub wrap_anywhere: Option<bool>,
    /// `word-space-transform` — чем ПОКАЗЫВАТЬ точку переноса (`<wbr>`,
    /// нулевой пробел): обычным пробелом или идеографическим. Точкой переноса
    /// она при этом быть не перестаёт.
    pub word_space_char: Option<char>,
    /// Edge spacer of an inline box (`inline::SPACER`): (box id, physical
    /// left edge, parent direction is rtl). Bidi reordering moves the edges to
    /// the box's visually outermost fragments (CSS 2.1 §8.6).
    pub spacer_edge: Option<(u32, bool, bool)>,
    /// `text-autospace` — зазор в 1/8 кегля между иероглифом и соседней
    /// буквой или цифрой (css-text-4 §7). Наследуется, поэтому живёт здесь;
    /// сами зазоры расставляет `inline::autospace_pieces` по кускам абзаца.
    pub autospace_alpha: Option<bool>,
    pub autospace_numeric: Option<bool>,
    /// Сдвиг куска по вертикали в долях кегля: `vertical-align: super` и
    /// `sub`. Не наследуется — принадлежит самому куску.
    pub vertical_shift: Option<f32>,
    /// То же, но ПРОЦЕНТОМ: доля считается от `line-height` куска, а не от
    /// кегля (§10.8.1), и хранить её вместе с `em` нельзя.
    pub vertical_shift_pct: Option<f32>,
    /// Сдвиг от базовой линии, названный ДЛИНОЙ: хранится в точках, потому
    /// что доля кегля на момент разбора ещё неизвестна — у строчного своего
    /// кегля обычно нет, он приходит наследованием.
    pub vertical_shift_px: Option<f32>,
    /// Сдвиг, названный единицей ШРИФТА (`ex`, `ch`): хранится сырым —
    /// метрики гарнитуры и кегль известны только при наборе строки.
    pub vertical_shift_len: Option<Len>,
    /// `vertical-align: text-top` (`true`) и `text-bottom` (`false`): край
    /// куска равняется по краю ТЕКСТОВОЙ области родителя, а не строки, —
    /// величина зависит от кеглей обоих и считается при наборе.
    pub vertical_align_text: Option<bool>,
    /// Кегль РОДИТЕЛЯ строчного куска: `text-top`/`text-bottom` равняются по
    /// его текстовой области, а не по самой высокой в строке.
    pub vertical_align_base: Option<f32>,
    /// Накопленный относительный сдвиг строчных предков куска в точках
    /// (CSS 2.1 §9.4.3): двигает ТОЛЬКО отрисовку, места в потоке не меняет
    /// и строку не растит.
    pub rel_shift: Option<(f32, f32)>,
    /// `text-orientation: upright` — глифы стоят прямо, а не лежат боком.
    /// Меняет и меру `ch`: продвижение нуля идёт вдоль оси СТРОКИ, а она в
    /// вертикальном письме вертикальна, то есть равна кеглю.
    pub upright: Option<bool>,
    /// `text-orientation: sideways` — у вертикального текста ДОМИНАНТНАЯ
    /// базовая алфавитная, а не центральная (css-writing-modes-4 §4.2:
    /// «In vertical typographic mode, the central baseline is used as the
    /// dominant baseline when text-orientation is mixed or upright»).
    /// `upright` этого не различает: `Some(false)` — и `mixed`, и `sideways`.
    pub text_sideways: Option<bool>,
    /// Родитель — сетка (не лунки): 1 — горизонтальная, 2 — `vertical-lr`,
    /// 3 — `vertical-rl`. Ставится при наследовании; по нему элементу
    /// переставляются оси выравнивания вертикальной сетки и пишутся биты
    /// базовой по оси x (`apply.rs`).
    pub(crate) parent_grid: u8,
    /// Родитель — гибкий контейнер, сетка или лунки: элемент блокифицирован
    /// (css-display-3 §2.7), хотя `display` в стиле остаётся строчным.
    pub(crate) parent_flex_grid: bool,
    /// Родитель — лунки (`display: grid-lanes`).
    pub(crate) parent_lanes: bool,
    /// Родитель — подсетка (`grid-template-*: subgrid`).
    pub(crate) parent_subgrid: bool,
    /// Абзац вертикального письма набирается САМ, по оси строки решённой
    /// раскладкой (`lines::Paragraph::vertical`): `Some(rl)`. Ставится только
    /// на копию стиля внутри `render::paragraph`.
    pub(crate) para_vertical: Option<bool>,
    /// Логические стороны и размеры до перевода в физические.
    pub logical: Option<Box<Logical>>,
    /// Ширина пришла из ЛОГИЧЕСКОГО `inline-size` при вертикальном письме:
    /// физически это высота (оси не переставляются, см. `resolve_logical`) —
    /// потребители обязаны отличать её от настоящего `width`
    /// (table-cell-align-005 против table-progression-htb-001).
    pub width_from_inline: bool,
    /// `hyphens`: разрешён ли перенос по мягкому переносу.
    pub hyphenate: Option<bool>,
    /// `user-select: none` — текст не выделяется.
    pub no_select: Option<bool>,
    /// `frame-sizing: content-height` (css-sizing-4
    /// §frame-sizing): высота `<iframe>` — по содержимому вложенного
    /// документа, если он сам согласился (`<meta name=responsive-embedded-sizing>`).
    pub frame_sizing_height: bool,
    /// Стиль первой буквы абзаца (`::first-letter`).
    ///
    /// Живёт в стиле, а не в элементе, потому что абзац собирается из кусков
    /// уже без узла-родителя: до кусков доезжает только вычисленный стиль.
    pub first_letter: Option<Box<Computed>>,
    /// Renderer metadata: marker text is outside first-letter selection.
    pub first_letter_excluded: bool,
    /// Own declarations remain separate for fictitious inheritance in descendants.
    pub first_letter_own: Option<Box<Computed>>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Box<Computed>>,
    /// Только объявления `::first-line` самого узла — их получает первый
    /// блок-потомок, несущий первую строку (`render/first_line_descendants`).
    pub first_line_own: Option<Box<Computed>>,
    /// `initial-letter` (css-inline-3 §initial-letter): размер буквицы в
    /// строках и её осадка (sink) — на базовой какой строки она стоит.
    /// `None` — `normal`, обычная буква. Живёт в слое `::first-letter`.
    pub initial_letter: Option<(f32, u32)>,
    /// Фон строчного бокса: `<span style="background">` внутри абзаца.
    ///
    /// Обычный фон принадлежит коробке, а у строчного бокса коробки нет — он
    /// тянется по строкам вместе с текстом и рвётся на переносах. Поэтому
    /// цвет едет вниз вместе с текстовыми свойствами и попадает в прогон.
    pub inline_bg: Option<Color>,
    /// Рамка СТРОЧНОЙ коробки — цвет и толщина. Рисует её прогон текста
    /// вместе с фоном: перенос режет коробку на куски, и рамка каждого куска
    /// своя. Коробки в раскладке у такого `<span>` нет — иначе его текст
    /// перестаёт переноситься вместе с абзацем.
    pub inline_border: Option<(Color, [f32; 4])>,
    /// Поля вокруг фона строчного бокса: `padding` у `<span>`, по четырём
    /// сторонам в порядке `[верх, право, низ, лево]`.
    pub inline_pad: Option<[f32; 4]>,
    /// Скругление фона строчного бокса.
    pub inline_radius: Option<f32>,
    /// Сплошная заливка предка с `background-clip: text` (css-backgrounds-4
    /// §background-clip): глифы поддерева красятся цветом текста ПОВЕРХ неё.
    /// Ставит и снимает `inline::inherit`; `None` — такого предка нет.
    pub text_clip_fill: Option<Color>,
    /// Цвет текста ДО наложения на `text_clip_fill`: его получают внепоточные
    /// потомки — в геометрию текста они не входят.
    pub text_clip_raw: Option<Color>,
    /// `background-clip`: до какого края красится фон. `None` — до внешнего
    /// края рамки, как по умолчанию в CSS.
    pub bg_clip: Option<BgClip>,
    /// `border-image`: картинка вместо рамки.
    pub border_image: Option<BorderImage>,
    /// `background-origin`: от какого края коробки отсчитывается фоновая
    /// картинка. `None` — от внутреннего края рамки, как по умолчанию в CSS.
    pub bg_origin: Option<BgClip>,
    /// `overflow-clip-margin`: на сколько обрезка отступает НАРУЖУ от коробки.
    /// Узел, чей фон красит КАНВАС (CSS 2.2 §14.2): корневой html, а без
    /// его фона — body. Ставится сборкой документа, не каскадом.
    pub(crate) canvas_bg: bool,
    /// Письмо `<body>` НЕ стало письмом области просмотра: обособление есть
    /// либо на `<html>`, либо на самом `<body>`, и распространение свойств
    /// тела наружу выключено (css-contain-2 §containment-types: «when any
    /// containments are active on either the HTML html or body elements,
    /// propagation of properties from the body element to the initial
    /// containing block, the viewport, or the canvas background, is
    /// disabled»). Вычисленное письмо тела при этом остаётся при нём
    /// (css-writing-modes §3.1 — распространяется только used корневой
    /// коробки), поэтому отличить главный поток по одному лишь письму тела
    /// нельзя, и признак приходится нести пометкой. Ставится сборкой
    /// документа, не каскадом.
    pub(crate) wm_contained: bool,
    /// Коробка КОРНЯ документа: её содержащий блок — начальный, и высота его
    /// определена всегда (§10.5). Ставится вместе с пометкой канваса, чтобы
    /// снимаемая обёртка уносила признак с собой.
    pub(crate) root_box: bool,
    /// ОПРЕДЕЛЕНА ли высота содержащего блока: от неё зависит, считается ли
    /// доля высоты вообще (§10.5 — иначе значение равно `auto`).
    pub(crate) cb_height_def: bool,
    /// Режим quirks: высота, от которой ДЕТИ этой коробки решают долю
    /// высоты при неопределённом содержащем блоке (Quirks Mode §3.5 «The
    /// percentage height calculation quirk» — ближайший предок с не-`auto`
    /// высотой). `None` — квирка нет или опоры не нашлось.
    pub(crate) quirk_pct_base: Option<f32>,
    /// `calc-size(<basis>, <expr>)` у `width`, `height`, `min-width`,
    /// `min-height` (css-values-5 §calc-size): `(mul, add, max, min)` над
    /// размером основы-ключевого слова; само свойство при этом `auto`, а
    /// выражение применяет раскладка (`taffy::Style::calc_size`).
    pub(crate) calc_size: [Option<(f32, f32, f32, f32)>; 4],
    /// Коробка РАСТЯНУТА раскладкой: элемент гибкого контейнера или сетки без
    /// своей высоты получает её от полосы, и для потомков она определена.
    pub(crate) stretched: bool,
    /// Элемент КОЛОНКИ гибкого контейнера: определён ли главный размер
    /// контейнера (css-flexbox-1 §9.8 п.1). `None` — не элемент колонки.
    pub(crate) flex_main_def: Option<bool>,
    /// Элемент ГИБКОГО контейнера (родитель `display: flex | inline-flex`).
    /// Ставится сборкой детей ряда/колонки в `render::blocks`, не каскадом и
    /// не наследуется (`inline::inherit` клонирует СВОЙ стиль ребёнка). Нужен
    /// таблице: у неё гибким элементом становится обёртка с подписями
    /// (css-flexbox-1 §4).
    pub(crate) flex_item: bool,
    /// Floats of this ordinary block reach nothing after it (`dom::float_tail`):
    /// its float host may use in-flow auto height (CSS 2.1 §10.6.3).
    pub(crate) float_tail: bool,
    /// Довод `fit-content(<length-percentage>)` у `width`, `min-width`,
    /// `max-width` (по порядку); само значение остаётся `Len::FitContent`.
    pub(crate) fit_arg: [Option<Len>; 3],
    /// The size slot came from `stretch` (stored as `Len::Pct(1.0)`), in the
    /// order width, height, min-width, max-width, min-height, max-height.
    /// css-sizing-4 §4.1 fills the containing block with the margin box, so
    /// it is not a percentage for the content-box contract.
    pub(crate) stretch_size: [bool; 6],
    /// Intrinsic min/max constraints rewritten as a preferred keyword retain their sizing wrapper.
    pub(crate) intrinsic_wrapper_required: bool,
    pub clip_margin: Option<f32>,
    /// Коробка отсчёта края обрезки: 0 content, 1 padding, 2 border;
    /// None — умолчание (padding-box).
    pub clip_margin_box: Option<u8>,
    /// `clip-path: polygon(…)`: вершины в долях или точках коробки.
    pub clip_polygon: Option<Vec<(Len, Len)>>,
    /// Правило намотки полигона `clip-path: polygon(evenodd, …)`.
    pub clip_polygon_evenodd: bool,
    /// `mix-blend-mode`: как слой смешивается с тем, что под ним.
    pub blend: Option<u8>,
    /// `isolation: isolate`: поддерево смешивается внутри себя, а с кадром —
    /// уже готовой картинкой.
    pub isolate: Option<bool>,
    /// `will-change` (css-will-change-1): разряды `wc::*`; ноль — `auto`.
    /// Слоёв композитора у нас нет, поэтому от обещания остаётся ровно то,
    /// что дало бы само свойство: содержащий блок и контекст наложения.
    pub will_change: u8,
    /// `font-stretch` — ширина начертания в процентах от обычной.
    ///
    /// Это НЕ возможность OpenType: узкое начертание — отдельный шрифт
    /// семейства, и выбирается он при подборе.
    pub font_stretch: Option<f32>,
    /// `font-size-adjust` (css-fonts-5): метрика (0 ex-height, 1 cap-height,
    /// 2 ch-width, 3 ic-width, 4 ic-height; `u8::MAX` — `none`) и желаемая
    /// доля кегля (`NaN` — `from-font`). Наследуется.
    pub font_size_adjust: Option<(u8, f32)>,
    /// Вычисленный кегль и заданная высота строки ДО подгонки кегля. Детям
    /// уходит ИМЕННО вычисленный кегль («otherwise the effect would
    /// compound»), и от него же считаются `em` и числовой `line-height`.
    /// Пусто, пока используемый кегль равен вычисленному.
    pub font_adjust_base: Option<(f32, Option<Len>)>,
    /// `tab-size` — во сколько пробелов раскрывается табуляция.
    pub tab_size: Option<f32>,
    /// `tab-size` в ДЛИНЕ: шаг табуляции задан не числом знаков, а величиной.
    /// Наследуется абсолютным (`tab-size-inheritance-001`), поэтому к детям
    /// уходит уже в точках — перевод делает `inline::inherit`.
    pub tab_size_len: Option<Len>,
    /// `contain: paint` — содержимое обрезается по коробке.
    pub contain_paint: Option<bool>,
    /// `contain: size` — коробка меряется ПУСТОЙ (css-contain-1 §3):
    /// размер задают явные свойства и `contain-intrinsic-size`.
    pub contain_size: Option<bool>,
    /// `contain: inline-size` — обособлена только СТРОЧНАЯ ось
    /// (css-contain-2 §inline-size): содержимое не влияет на неё, но
    /// блочную ось по-прежнему задаёт.
    pub contain_inline_size: Option<bool>,
    /// `contain: layout|content` — независимый контекст форматирования.
    pub contain_layout: Option<bool>,
    /// `display: flow-root` — свой контекст форматирования (коробка Block).
    pub flow_root: Option<bool>,
    /// Статичная блочная коробка в потоке (не строчная, не таблица, не
    /// поле формы, не float): её чистый px-`transform` раскладка берёт на
    /// себя (`folded_shift`). Ставит `dom` после `finish_inline_display`.
    pub plain_block_box: bool,
    /// `display: inline` дословно (не inline-block): §9.7/§10.2 дорешиваются
    /// после каскада — см. `dom::finish_inline_display`.
    pub inline_display: Option<bool>,
    /// Абсолют/фиксированный, чей `display` до блокификации (§9.7) был
    /// строчного уровня (`inline-block`, `inline-flex`, …): статическая
    /// позиция считается для гипотетической коробки «если бы position был
    /// static» (CSS 2.1 §10.3.7, §10.6.4), то есть В СТРОКЕ. Ставит `dom`.
    pub abs_inline_level: bool,
    /// `display: run-in` — вбегание решает `dom::fold_run_ins`.
    pub run_in: Option<bool>,
    /// Есть ли выше по дереву коробка, устанавливающая содержащий блок для
    /// внепоточных потомков (§10.1 п.4). Ставится при наследовании: сам
    /// каскад предков не видит.
    pub(crate) cb_ancestor: bool,
    /// An ancestor is a multi-column container: the box may be fragmented
    /// across columns (css-break-3 §box-splitting). Set by inheritance.
    pub(crate) in_multicol: bool,
    /// Есть ли выше по дереву корень подложки (filter-effects-2
    /// §BackdropRoot): прозрачность, фильтр, маска, clip-path, смешивание,
    /// `backdrop-filter`, `will-change` с ними. Ставится при наследовании.
    pub(crate) backdrop_root_above: bool,
    /// `will-change` называет свойство, создающее корень подложки.
    pub(crate) will_change_root: bool,
    /// `backdrop-filter` задан (не `none`): корень подложки при ЛЮБОМ списке,
    /// даже тождественном `invert(0)`, у которого матрицы нет
    /// (filter-effects-2 Overview.bs:119; Blink
    /// paint_property_tree_builder.cc:1846 `!BackdropFilter().IsEmpty()`).
    pub(crate) backdrop_filter_set: bool,
    /// `view-transition-name` не `none` — тоже корень подложки
    /// (css-view-transitions-1 Overview.bs:582).
    pub(crate) vt_name: bool,
    /// `backdrop-filter: url(#id)` — ссылка на SVG `<filter>`; сводится к
    /// матрице 4×5 на отрисовке (`render::svg_filter_matrix`).
    pub(crate) backdrop_ref: Option<String>,
    /// `display: table-caption` — метка для таблицы.
    pub is_caption: Option<bool>,
    /// Род группы рядов: 0 — шапка, 1 — тело, 2 — подвал. `Display` у всех
    /// трёх ОДИН (`TableRowGroup`), раскладка у них одинаковая, — а §17.5.3
    /// требует переставить шапку вверх, подвал вниз. Различить их по
    /// `Display` нечем, поэтому род хранится отдельно.
    pub row_group_kind: Option<u8>,
    /// Колоночная роль: 0 — `display: table-column`, 1 —
    /// `table-column-group`. `Display` при этом ОСТАЁТСЯ `None`: коробки
    /// колонка не порождает (§17.2.1), и весь поток обходит её ровно как
    /// раньше. Метка — единственное, что сохраняет узел в дереве: из него
    /// берутся ширина дорожки (§17.5.2.1), слой краски (§17.5.1) и рамка
    /// для разбора сросшихся кромок (§17.6.2.1).
    pub col_role: Option<u8>,
    /// `contain: style` — счётчики и кавычки не выходят из поддерева.
    pub contain_style: Option<bool>,
    /// `contain-intrinsic-size`: подменная своя величина (css-sizing-5 §5).
    pub contain_intrinsic: (Option<f32>, Option<f32>),
    /// `content-visibility: hidden` — детей не собирать вовсе.
    pub skip_content: Option<bool>,
    /// `clip-path`/`mask`: обрезка по кругу или скруглённому прямоугольнику.
    /// Хранится долей радиуса от меньшей стороны либо радиусом в точках.
    pub clip_round: Option<f32>,
    /// The uniform `round` radius of `inset()`/`rect()`/`xywh()` as written
    /// (points or a percentage of the reference box).
    pub clip_round_len: Option<Len>,
    /// `clip-path: circle(...)|ellipse(...)` с параметрами: сырые аргументы
    /// формы (`shape:circle(...)`). Радиусы и центр зависят от размера
    /// коробки — он известен только отрисовке, поэтому форма растрируется
    /// маской буфера группы (см. `background::source`).
    pub clip_shape: Option<String>,
    /// `mask-size`: размер плитки маски; None — auto (интринзик картинки).
    pub mask_size: Option<(Len, Len)>,
    /// `mask-repeat`: пооосный запрет мощения (no-x, no-y) — ПЕРВОГО слоя.
    pub mask_no_repeat: Option<(bool, bool)>,
    /// То же ПО СЛОЯМ (css-masking-1 §7.6, `<repeat-style>#`): запись
    /// `no-repeat, repeat` задаёт свою укладку каждому слою. Список короче
    /// набора слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_repeat_list: Option<Vec<(bool, bool)>>,
    /// Per-layer `space`/`round` axes (css-masking-1 §7.6 →
    /// css-backgrounds-3 §3.4): 2 space, 3 round, anything else as
    /// `mask_repeat_list` says.
    pub mask_repeat_modes: Option<Vec<(u8, u8)>>,
    /// `mask-size: contain|cover` (1|2): вписывание по интринзику.
    pub mask_fit: Option<u8>,
    /// `mask-mode: luminance` — маскирует светимость, а не альфа.
    pub mask_luminance: Option<bool>,
    /// `mask-mode: alpha` — альфа и для ссылки на `<mask>` (css-masking-1
    /// §7.2: `match-source` берёт `mask-type` определения).
    pub mask_alpha_mode: Option<bool>,
    /// `mask-type: alpha` у элемента `<mask>` (css-masking-1 §7.16).
    pub mask_type_alpha: Option<bool>,
    /// `mask-origin`: коробка укладки плитки (0 border, 2 padding, 3 content).
    pub mask_origin: Option<u8>,
    /// Готовые коробки маски в CSS-точках (укладка t/r/b/l от коробки
    /// слоя внутрь; окраска — то же либо None = без обрезки) — для SVG-детей,
    /// у которых fill-/stroke-/view-box считает `svg::masked_layers`, а не
    /// рамка и отбивка (`render::grouped`).
    pub mask_box_override: Option<([f32; 4], Option<[f32; 4]>)>,
    /// Блок, вынесенный расщеплением строчного хозяина (block-in-inline,
    /// `render::blocks`): в дереве отрисовки он брат хозяина, а по DOM — его
    /// ребёнок. Объёмный контекст и перспектива деда на него не действуют:
    /// плоский строчный хозяин — лист контекста, поддерево сплющивается в его
    /// плоскость (css-transforms-2 §3d-rendering-context; §perspective — только
    /// прямые дети). Ставится при выносе, читает `render::transformed`.
    pub hoisted_block: bool,
    /// Пользовательская единица SVG-ребёнка в CSS-точках (масштаб `viewBox`
    /// или `zoom`); 0 — не задано (= 1). Интринзик плитки маски у такого
    /// ребёнка считается в его единицах (`interact::Grouped::mask_scale`).
    pub mask_user_scale: f32,
    /// `mask-clip`: коробка окраски маски; вне её элемент скрыт. 255 — no-clip.
    pub mask_clip: Option<u8>,
    /// `mask-composite` по слоям: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub mask_composite: Option<Vec<u8>>,
    /// `clip: rect(t r b l)` (CSS 2.1 §11.1.2, только absolute): координаты
    /// видимой области от углов border-box; None в позиции — auto (край).
    pub clip_rect: Option<[Option<f32>; 4]>,
    /// Тот же прямоугольник, но КАК НАПИСАН: единицы шрифта на разборе ещё не
    /// меряются, а `clip: rect(1em, …)` без них читался как `auto` и не
    /// обрезал вовсе (`visufx/clip-079/080/091/092`). Сводится к точкам в
    /// `resolve_em`, где кегль и метрики семейства уже известны.
    pub clip_len: Option<[Option<Len>; 4]>,
    /// `clip-path: inset(t r b l ...)`: срезы краёв видимой области.
    pub clip_inset: Option<[Len; 4]>,
    /// `clip-path: rect(t r b l)` — координаты КРАЁВ от верхнего-левого
    /// угла; None = auto (край коробки).
    pub clip_edges: Option<[Option<Len>; 4]>,
    /// `clip-path: xywh(x y w h)` — прямоугольник от угла.
    pub clip_xywh: Option<[Len; 4]>,
    /// `column-fill: auto` — колонки заполняются по очереди, без баланса.
    pub column_fill_auto: Option<bool>,
    /// `scroll-marker-group` (css-overflow-5): `Some(true)` — группа маркеров
    /// ПЕРЕД скроллером (`before`), `Some(false)` — после (`after`), `None` —
    /// `none`. Не наследуется.
    pub scroll_marker_group: Option<bool>,
    /// `column-span: all` — блок растянут на все колонки.
    pub column_span: Option<bool>,
    /// `page: <custom-ident>` — именованная страница (css-page-3 §"Using named
    /// pages"). `auto` хранится отсутствием значения: используемое значение
    /// берётся у ближайшего предка с именем (там же, шаг 1 алгоритма).
    pub page: Option<String>,
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v153, `scout-boxdeco-2026-09.md`):
    /// `box-decoration-break: clone` — украшение на каждом фрагменте
    /// (21 хунк: разбор, `Kid::clone_dec`, ветка в `fill_at`, `frags_of`,
    /// `clone_fragment`). Срез `L-brk` 2874: +2/−2 при ожидании +6…+19 —
    /// `clone-004`, `-012` взяты, но `clone-005.tentative` 0.00 → 99.00 и
    /// `clone-007` 0.00 → 2.08. Ветка `clone` в `fill_at` ломает уже
    /// работавший `slice` у вложенных случаев; нужен отдельный проход
    /// планирования фрагментов, а не правка общей укладки.
    /// `break-inside: avoid*` — коробку нельзя разрывать между колонками и
    /// страницами (css-break-3 §4.1). Свойство не разбиралось вовсе, и
    /// отличить монолит от обычной коробки было нечем.
    pub break_inside_avoid: bool,
    /// `box-decoration-break: clone` (css-break-4 §break-decoration): «Each
    /// box fragment is independently wrapped with the border, padding, and
    /// margin … The background is drawn independently in each fragment».
    /// Не наследуется; начальное `slice` = `false`.
    pub bdb_clone: bool,
    /// `break-before`/`break-after` (css-break-4 §3.1): принудительный разрыв
    /// колонки/страницы перед или после коробки.
    pub break_before_force: bool,
    pub break_after_force: bool,
    /// Те же свойства со ЗАПРЕЩАЮЩИМИ значениями — css-break-4 §3.1 «avoid
    /// break values»: `avoid`, `avoid-page`, `avoid-column`, `avoid-region`.
    /// Правило 1 §4.3: «A fragmented flow may break at a class A break point
    /// only if all the break-after and break-before values applicable to this
    /// break point allow it». Разбирались ТОЛЬКО принудительные значения, и
    /// сообщить движку запрет разрыва МЕЖДУ соседями было нечем.
    pub break_before_avoid: bool,
    pub break_after_avoid: bool,
    /// `margin-trim` (css-box-4 §margin-trim): биты обрезаемых ЛОГИЧЕСКИХ
    /// краёв: 1 — `block-start`, 2 — `block-end`, 4 — `inline-start`,
    /// 8 — `inline-end`. Начальное `none` (0). Блочный контейнер исполняет
    /// только блочные биты (`render::collapse_margins`), гибкий и сетка —
    /// все четыре (раскладка, `apply` переводит их в физические).
    pub margin_trim: u8,
    /// `zoom` (css-viewport-1 §zoom-property): СВОЙ множитель элемента, как
    /// написан; `None` — не задан. `0`/`0%` по спеке читаются единицей.
    /// Читает его ТОЛЬКО проход `zoom::resolve` после каскада.
    pub zoom: Option<f32>,
    /// Действующий зум («effective zoom», §599): произведение по цепочке
    /// предков вместе со своим. `None` ≡ 1 — выведенный `Default`
    /// тождество, и страница без `zoom` не несёт ни множителя, ни ветки.
    /// Ставится проходом `zoom::resolve` на каждый элемент под зумом; в
    /// слитый стиль попадает через `own.clone()` в `inline::inherit` —
    /// своей строки там не имеет. Читатели шага 2: природный размер
    /// картинки, `resolve_viewport`.
    pub zoom_eff: Option<f32>,
    /// `column-rule-*`: линейка между колонками.
    pub column_rule_width: Option<Len>,
    pub column_rule_visible: Option<bool>,
    pub column_rule_color: Option<Color>,
    /// `row-rule-*` (css-gaps-1 §color-style-width): линейка в ПОПЕРЕЧНОМ
    /// промежутке сетки/гибкого контейнера. Начальные значения те же, что у
    /// `column-rule-*`: `currentcolor`, `none`, `medium` — то есть без
    /// заданного стиля линейки нет.
    pub row_rule_width: Option<Len>,
    pub row_rule_visible: Option<bool>,
    pub row_rule_color: Option<Color>,
    /// `column-rule-break`/`row-rule-break` (css-gaps-1 §break): 0 — `none`,
    /// 1 — `normal` (начальное), 2 — `intersection`. Шаг 1 разбирает
    /// значение, но рисует всегда непрерывно: в сетке БЕЗ спанов все стыки
    /// крестовые, и `normal` по спеке проходит сквозь них.
    pub column_rule_break: Option<u8>,
    pub row_rule_break: Option<u8>,
    /// Списки значений линеек по промежуткам (css-gaps-1 §lists); `None` —
    /// значение одно и лежит в скалярных полях выше. Цвет `None` в списке —
    /// `currentcolor`.
    pub column_rule_widths: Option<GapList<Len>>,
    pub column_rule_styles: Option<GapList<bool>>,
    /// Single `double` rule style (css-gaps-1 §color-style-width: line styles
    /// as for borders) — painted as two lines; other styles stay solid.
    pub column_rule_double: bool,
    pub row_rule_double: bool,
    pub column_rule_colors: Option<GapList<Option<Color>>>,
    pub row_rule_widths: Option<GapList<Len>>,
    pub row_rule_styles: Option<GapList<bool>>,
    pub row_rule_colors: Option<GapList<Option<Color>>>,
    /// §inset: [cap-start, cap-end, junction-start, junction-end]; начальное 0.
    pub column_rule_inset: Option<[GapInset; 4]>,
    pub row_rule_inset: Option<[GapInset; 4]>,
    /// §visibility-items: 0 `normal`, 1 `all`, 2 `around`, 3 `between`.
    pub column_rule_visibility: Option<u8>,
    pub row_rule_visibility: Option<u8>,
    /// `rule-overlap: column-over-row` — колонки поверх рядов.
    pub rule_column_over_row: Option<bool>,
    /// `shape-outside`: сырая запись формы обтекания плавающего блока.
    pub shape_outside: Option<String>,
    /// `shape-margin`: поле вокруг формы обтекания; доля — от ширины
    /// содержащего блока.
    pub shape_margin: Option<Len>,
    /// `shape-image-threshold`: порог альфы для формы из картинки.
    pub shape_threshold: Option<f32>,
    /// Вырезы обтекания для абзацев ПОД этим элементом: формы слева и
    /// справа от верха первого абзаца (заполняет сборка shape-flow).
    pub flow_shapes:
        Option<std::sync::Arc<(Vec<crate::flow::FloatShape>, Vec<crate::flow::FloatShape>)>>,
    /// `mask-position`: смещение плитки; доля — от свободного места
    /// (коробка минус плитка), как у `background-position`.
    pub mask_pos: Option<(Len, Len)>,
    /// Смещение отсчитано от ПРАВОГО/НИЖНЕГО края (`right 30px bottom 25px`).
    pub mask_pos_far: (bool, bool),
    /// `mask-position` ПО СЛОЯМ (css-masking-1 §7.7, `<position>#`):
    /// `(x, y, от правого края, от нижнего края)`. Список короче набора
    /// слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_pos_list: Option<Vec<(Len, Len, bool, bool)>>,
    /// Эллиптические радиусы углов (`border-radius: H / V`), tl/tr/br/bl:
    /// растеризатор круглит только окружностью — такой угол уходит
    /// альфа-маской буфера группы (`shape:rrect(...)`).
    pub radius_ell: Option<[Option<(Len, Len)>; 4]>,
    /// Форма углов `corner-shape` (css-borders-4 §corner-shaping): параметр
    /// суперэллипса K по углам tl/tr/br/bl — `round`=1, `squircle`=2,
    /// `square`=+∞, `bevel`=0, `scoop`=−1, `notch`=−∞, `superellipse(K)`.
    /// `None` — все углы круглые (начальное значение). Угол с K≠1 при
    /// ненулевом радиусе рисуется растровой маской (`Computed::corner_shaped`).
    pub corner_shape: Option<[f32; 4]>,
    /// `border-shape` (css-borders-4 §border-shape); `None` — начальное `none`.
    /// Контур режет буфер группы (`render::grouped`), рамку красит слой
    /// (`render::decorations`), `border-radius` при этом игнорируется.
    pub border_shape: Option<BorderShape>,
    /// `filter`: цветовые преобразования, применённые к собственным цветам.
    pub filter: Option<Filter>,
    /// `filter: url(#id)` — ссылка на SVG-`<filter>` документа; рисуется
    /// растровым слоем поверх коробки (`interact::FilterLayer`).
    pub filter_ref: Option<String>,
    /// `filter: drop-shadow(...)` — тень фильтра; у коробки со сплошным
    /// фоном становится внешней тенью (`inline::inherit`).
    pub drop_shadow: Option<Shadow>,

    /// `background-image: url(...)` — ссылка на картинку-заливку.
    pub bg_image: Option<String>,
    pub bg_size: BgSize,
    pub bg_pos: BgPos,
    /// `object-position` замещаемого содержимого (css-images-3 §5.2).
    pub object_position: Option<BgPos>,
    /// `object-view-box` (css-images-4 §object-view-box): видимая область
    /// природного объекта как вырез `inset(top right bottom left)` в точках
    /// или долях природного размера — `rect()` и `xywh()` сводятся к нему
    /// при отрисовке (`render::view_box_rect`). Флаг — вид записи:
    /// 0 `inset`, 1 `rect`, 2 `xywh`.
    pub(crate) object_view_box: Option<(u8, [Len; 4])>,
    /// Сырые СПИСКИ фоновых свойств со слоями через запятую: (свойство,
    /// запись). Поля `bg_*` несут верхний слой; все слои строит `bg_layers`.
    pub(crate) bg_lists: Vec<(String, String)>,
    pub bg_repeat: Option<BgRepeat>,
    /// `content` псевдоэлемента — СПИСОК составляющих (css-content-3 §2):
    /// строки, `counter()`, `counters()`, `attr()` в любом порядке.
    pub content: Option<Vec<ContentItem>>,
    /// `quotes` (css-content-3 §4.1): пары кавычек по уровням вложенности.
    /// `None` inherits; `Some(None)` suppresses marks but keeps nesting depth.
    /// `Some(Some(empty))` is explicit auto; nonempty pairs are a custom system.
    /// Tree traversal resolves inheritance before generating pseudo content.
    pub quotes: Option<Option<Vec<(String, String)>>>,
    /// `counter-reset` — обнулить счётчик с этого узла.
    pub counter_reset: Option<String>,
    /// `counter-increment` — увеличить счётчик на этом узле.
    pub counter_increment: Option<String>,
    /// `counter-set` — присвоить счётчику значение (css-lists-3 §5).
    pub counter_set: Option<String>,
    /// Возможности шрифта (`font-feature-settings`, `font-variant`).
    pub font_features: Vec<(String, u32)>,
    /// Значение СВОЙСТВА `font-feature-settings` целиком (`normal` — пустой
    /// список). Отдельно от `font_features`: по css-fonts-4 §7.2 оно старше
    /// `font-variant-*` при ЛЮБОМ порядке объявлений и наследуется своим
    /// значением, а не пропадает, стоит ребёнку задать `font-variant`
    /// (`font-variant-04`).
    pub font_settings: Option<Vec<(String, u32)>>,
    /// Font-specific names stay unresolved until the used family is known
    /// (CSS Fonts 4 §font-variant-alternates-prop).
    pub font_alternates: Option<crate::fonts::alternates::Alternates>,
    /// `font-synthesis-weight|style|small-caps: none` — подмена начертания
    /// запрещена (css-fonts-4 §6.5). Ложь = `none`, пусто = `auto`.
    pub font_synth: (Option<bool>, Option<bool>, Option<bool>),
    /// `font-kerning`: 0 `none`, 1 `normal`, 2 `auto`; inherits independently
    /// of font-variant and resolves before font-feature-settings.
    pub font_kerning: Option<u8>,
    /// Знак акцента (`text-emphasis-style`, css-text-decor-3 §5): рисуется
    /// над каждым знаком базы, как надстрочная аннотация руби.
    pub text_emphasis: Option<String>,
    /// Акцент СНИЗУ (`text-emphasis-position: under`).
    pub emphasis_under: bool,
    /// `text-emphasis-color` (css-text-decor-3 §5.2); пусто — `currentColor`.
    pub emphasis_color: Option<Color>,
    /// `text-decoration-line` самой коробки (биты `DECOR_*`); пусто — не
    /// задано (`none`). Не наследуется: потомкам линии достаются через
    /// `decors` (css-text-decor-3 §2 «propagated»).
    pub td_lines: Option<u8>,
    /// `text-decoration-style` (не наследуется).
    pub td_style: Option<DecorStyle>,
    /// `text-decoration-color`; пусто — `currentColor` (не наследуется).
    pub td_color: Option<Color>,
    /// `text-decoration-thickness` (не наследуется).
    pub td_thickness: Option<DecorLen>,
    /// `text-decoration-inset` (css-text-decor-4 §4.1, не наследуется):
    /// `None` — 0, `Some(None)` — `auto`.
    pub td_inset: Option<Option<[DecorLen; 2]>>,
    /// `text-underline-offset` (наследуется).
    pub underline_offset: Option<DecorLen>,
    /// `text-underline-position` (наследуется), биты `UPOS_*`.
    pub underline_pos: Option<u8>,
    /// Блочный тег (`<p>`, `<div>`…): при пустом `display` коробка блочная
    /// (`dom.rs`, не наследуется) — украшениям нужна своя строчная коробка.
    pub block_tag: bool,
    /// `text-decoration-skip-ink` (наследуется): 0 `none`, 1 `auto`, 2 `all`.
    pub skip_ink: Option<u8>,
    /// `text-decoration-skip-spaces` (наследуется): 1 `start`, 2 `end`,
    /// 4 `all`, 0 `none`; пусто — начальное `start end`.
    pub skip_spaces: Option<u8>,
    /// Украшения, наложенные на текст коробки её предками и ею самой
    /// (css-text-decor-3 §2.1), от внешнего к внутреннему.
    pub decors: Vec<Decor>,
    /// `ruby-position` (css-ruby-1 §4.1): `Some(true)` — аннотация ПОД базой
    /// (`under`), `Some(false)` — над (`over`/`alternate`/`inter-character`),
    /// `None` — не задано. Наследуется (`inline::inherit`). Прежде делил флаг
    /// с акцентом, и `text-emphasis-position: under` переворачивал руби.
    pub ruby_under: Option<bool>,
    /// `ruby-align` (css-ruby-1 §4.3); `None` — начальное `space-around`.
    pub ruby_align: Option<RubyAlign>,
    /// `ruby-overhang` (css-ruby-1 §4.4); `None` — начальное `auto`. Наследуется.
    pub ruby_overhang: Option<RubyOverhang>,
    /// CSS Ruby §ruby-merge: 0 separate, 1 merge, 2 auto.
    pub ruby_merge: Option<u8>,
    /// Роль руби-коробки из `display: ruby*` (css-ruby-1 §2.1). Не
    /// наследуется. `display` при этом остаётся строчным (`InlineBlock` +
    /// `inline_display`), у `block ruby` — `Block`: все `match` по `Display`
    /// остаются как есть, роль читается отдельно.
    pub ruby_role: Option<RubyRole>,
    /// `caret-color` поля ввода.
    pub caret_color: Option<Color>,
    /// `accent-color` флажков и переключателей.
    pub accent_color: Option<Color>,
}

impl Default for BgSize {
    fn default() -> Self {
        BgSize::Auto
    }
}

impl Computed {
    /// Сдвиг из `transform`, который раскладка берёт на себя как
    /// относительное смещение — тем же путём, что и свойство `translate`
    /// (`apply::apply_box`). Чистый сдвиг — это смена начала координат
    /// (css-transforms-1 §transform-rendering), и разложенная на сдвинутом
    /// месте коробка обязана рисоваться байт в байт как сдвинутая: иначе
    /// при дробном масштабе экрана округление раскладки (до сдвига) и
    /// дробный сдвиг матрицей (после) расходились на пиксель — края коробки и
    /// глифы (Blink так же проносит дробное смещение сквозь 2D-сдвиг:
    /// `PaintPropertyTreeBuilder`, subpixel accumulation). Только статичная
    /// блочная коробка — её путь отрисовки один (`render.rs`, блочная ветка
    /// `transformed(animated(e))`), и края у неё не заданы.
    pub fn folded_shift(&self) -> Option<(f32, f32)> {
        use crate::computed::inh;
        if !matches!(self.position, None | Some(Position::Static))
            || self.hoisted_block
            || self.rotate_prop.is_some()
            || self.scale_prop.is_some()
            || self.animation.is_some()
            || self.inherit_bits & inh::TRANSFORM != 0
            || !self.plain_block_box
            // A fragmented box is shifted per fragment, but the column
            // layout places only its first fragment by the relative offset
            // (css-break-3 §box-splitting; `css-break/transform-000`): keep
            // the shift in the transform there.
            || self.in_multicol
        {
            return None;
        }
        if let Some((x, y)) = self.translate {
            if !matches!((x, y), (Len::Px(_), Len::Px(_))) {
                return None;
            }
        }
        self.transform
            .as_ref()?
            .pure_px_shift()
            .filter(|&(x, y)| x != 0.0 || y != 0.0)
    }

    /// Место под логические значения — заводится по первому обращению: у
    /// подавляющего большинства узлов их нет вовсе.
    fn logical(&mut self) -> &mut Logical {
        self.logical.get_or_insert_with(Default::default)
    }

    /// Разложить логические стороны и размеры по физическим.
    ///
    /// Зовётся ПОСЛЕ того, как письмо унаследовано: до этого неизвестно, какая
    /// ось строчная. Физическое значение, если оно задано, не трогается —
    /// логическое лишь заполняет пустое место.
    /// Стиль без СВОЕЙ краски: `visibility: hidden` прячет коробку, но не
    /// поддерево — потомок с `visibility: visible` обязан рисоваться (§11.2).
    /// Гасить целиком нельзя: раскладка обязана остаться прежней, поэтому
    /// снимается только краска, а размеры и рамки по толщине не трогаются.
    pub fn paint_off(&self) -> Computed {
        let mut c = self.clone();
        c.hidden = None;
        c.background = None;
        c.bg_image = None;
        c.gradient = None;
        c.gradient_raw = None;
        c.border_color = Some(crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        c.border_colors = [c.border_color; 4];
        c.outline = None;
        c.shadows.clear();
        c.text_shadow = None;
        c.text_shadow_rest.clear();
        c.underline = None;
        c.line_through = None;
        c
    }

    pub fn resolve_logical(&mut self, parent_vertical: Option<bool>, is_cell: bool) {
        let Some(logical) = self.logical.take() else {
            return;
        };
        // ОСИ НЕ ПЕРЕСТАВЛЯЮТСЯ, и это не упрощение, а следствие устройства
        // вертикального письма: оно рисуется ПОВОРОТОМ блока, то есть внутри
        // повёрнутого поддерева физические ширина и высота уже поменялись
        // местами. Перевод логических сторон «как в спецификации» переставил
        // бы их второй раз — замерено: writing-modes 191 → 189. Переставлять
        // здесь можно будет только вместе с отказом от поворота.
        // ПОВТОРНО ЗАМЕРЕНО И ОТКАЧЕНО (2026-08-09), уже на новом
        // вертикальном письме: перестановка ЦЕЛИКОМ — writing-modes 200 → 201,
        // flexbox 352 → 349; перестановка ТОЛЬКО размеров — те же числа.
        // Теряются `css-flexbox/gap-001-lr`, `gap-007-lr`, `gap-007-rl`
        // (получает `gap-002-rl`): размер там задан логическим свойством, и
        // после перестановки он ложится поперёк уже повёрнутой раскладки.
        // ОТКАТ ПРОТУХ (проверено 30.08 по свежему своду): из трёх названных
        // выше потерь `gap-007-lr` (0.99) и `gap-007-rl` (0.56) красные и без
        // перестановки, а «приобретение» `gap-002-rl` зелено само.
        // ЗАМЕРЕНО И ОТКАЧЕНО: исключать отсюда ячейку таблицы, чтобы вернуть
        // `table-cell-align-005` и `table-cell-valign-003`. Полный свод CSS3:
        // те же 4 потери — на этом шаге у ячейки ещё нет `display` (табличность
        // движок держит по ТЕГУ, а `Display::TableCell` приходит только из
        // авторского CSS).
        //
        // Переставляем только у УНАСЛЕДОВАВШЕГО письмо: у ортогонального узла
        // (письмо объявлено на нём самом, родитель горизонтален) перестановку
        // ниже по течению делает табличный и блочный код, и вторая здесь
        // складывалась с ней в поворот на месте.
        // РАЗМЕРЫ отображаются по СВОЕМУ письму, а не по письму родителя.
        // Таблица Abstract-Physical Mapping (css-writing-modes-4,
        // Overview.bs:1790-1827) даёт `block-size` -> width и
        // `inline-size` -> height ВСЕМ вертикальным письмам, а колонку
        // выбирает used-значение `writing-mode` САМОГО элемента: письма
        // родителя в таблице нет. Blink делает ровно это одним предикатом —
        // `computed_style.h:1270` `LogicalWidth() = IsHorizontalWritingMode()
        // ? Width() : Height()` (и так же Logical{Min,Max}{Width,Height},
        // строки 1275-1287).
        // Нашей поворотной модели это не мешает: коробки физические на всех
        // уровнях, вертикальный контейнер кладёт детей `flex_row`
        // (`render.rs:14461`), поэтому у ортогонального узла (письмо
        // объявлено на нём, родитель горизонтален) физическая ширина — его
        // БЛОЧНАЯ ось, а высота — СТРОЧНАЯ, ровно как у унаследовавшего
        // письмо. Стороны так считаются давно — `side_vertical` ниже.
        // Ячейка таблицы — единственное исключение: у неё логический
        // inline-size перекладывает в высоту сам табличный код
        // (`render.rs:16790-16800` по флагу `width_from_inline`), и второй
        // перевод здесь сложился бы с ним в поворот на месте
        // (`table-cell-align-005`, `table-cell-valign-003` — замеренный
        // откат в шапке функции).
        let vertical =
            self.vertical == Some(true) && (parent_vertical == Some(true) || !is_cell);
        let rtl = self.rtl == Some(true);
        // Стороны (поля/отступы/края) переставляются ПО-НАСТОЯЩЕМУ: блочный
        // поток вертикального письма собирается транспонированным рядом
        // (`flex_row(_reverse)`), а не поворотом — `margin-block` абзаца в
        // vertical-rl обязан лечь горизонтально (wm-propagation-body-*).
        // Размеры остаются без перестановки (замерено дважды, см. выше).
        let side_vertical = self.vertical == Some(true);
        let side_rl = self.vertical_rl == Some(true);
        let side_sw_lr = self.sideways == Some(true) && !side_rl;
        // Логическое значение ПЕРЕКРЫВАЕТ физическое, а не только заполняет
        // пустое. Так вело себя прежнее разложение при разборе (оно писало
        // прямо в физическое поле), и порядок объявлений в правиле от этого
        // сохранялся. Заполнение только пустого меняло исход там, где заданы
        // оба (замерено на `grid-self-alignment-baseline-with-grid-002`).
        let set = |slot: &mut Option<Len>, val: Option<Len>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // Размеры: строчная ось горизонтальна при обычном письме и
        // вертикальна при вертикальном.
        let set_ci = |slot: &mut Option<f32>, val: Option<f32>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // `contain-intrinsic-*-size` переставляется по СВОЕМУ письму, а не по
        // общему гейту `vertical` (тот требует ещё и вертикального родителя).
        // Причина: читается пара через `contains_width()`/`contains_height()`,
        // а те смотрят ТОЛЬКО на `self.vertical`. У ортогонального узла
        // (письмо объявлено на нём, родитель горизонтален) условия расходились,
        // и `contain-intrinsic-inline-size` приезжал поперёк — так падали
        // `contain-intrinsic-size-logical-002` и
        // `grid-lanes-contain-intrinsic-size-logical-001`. Размеры и стороны
        // ниже остаются на прежнем гейте: их перестановку у ортогонального
        // узла делает код ниже по течению (замер описан выше по функции).
        if side_vertical {
            set_ci(&mut self.contain_intrinsic.1, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.0, logical.ci_block);
        } else {
            set_ci(&mut self.contain_intrinsic.0, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.1, logical.ci_block);
        }
        if vertical {
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
            if self.vertical == Some(true) && logical.inline_size.is_some() {
                self.width_from_inline = true;
            }
            set(&mut self.width, logical.inline_size);
            set(&mut self.height, logical.block_size);
            set(&mut self.min_width, logical.min_inline);
            set(&mut self.min_height, logical.min_block);
            set(&mut self.max_width, logical.max_inline);
            set(&mut self.max_height, logical.max_block);
        }
        // Стороны. Начало строчной оси: слева (обычное письмо), справа (оно же
        // справа налево) или сверху (вертикальное). Начало оси блока: сверху,
        // а в вертикальном — справа при `vertical-rl` и слева при `-lr`.
        // Логическая сторона ложится на физическую, только если объявлена
        // ПОЗЖЕ её (порядок каскада): UA `padding-inline-start: 40px` у `ul`
        // против авторского `padding-top: 0` в `vertical-rl`
        // (`line-box-direction-vrl-019`, `block-flow-direction-vrl-021`).
        let phys_seq = self.side_seq;
        let sides = [
            (&logical.padding, 0u8, phys_seq.padding),
            (&logical.margin, 1, phys_seq.margin),
            (&logical.inset, 2, phys_seq.inset),
        ];
        for (from, which, pseq) in sides {
            let to = match which {
                0 => &mut self.padding,
                1 => &mut self.margin,
                _ => &mut self.inset,
            };
            let (i_start, i_end, b_start, b_end) = if side_vertical {
                // Начало оси блока: `vertical-rl` — ПРАВЫЙ край (поток блоков
                // идёт справа налево), `vertical-lr` — левый.
                let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
                // `sideways-lr`: строка идёт СНИЗУ ВВЕРХ (css-writing-modes-4
                // §3) — начало строчной оси у нижнего края, а не верхнего.
                let flip = side_sw_lr != rtl;
                let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
                (is, ie, bs, be)
            } else if rtl {
                (1u8, 3u8, 0u8, 2u8)
            } else {
                (3u8, 1u8, 0u8, 2u8)
            };
            for (side, val, lseq) in [
                (i_start, from.inline_start, from.seq[0]),
                (i_end, from.inline_end, from.seq[1]),
                (b_start, from.block_start, from.seq[2]),
                (b_end, from.block_end, from.seq[3]),
            ] {
                let slot = match side {
                    0 => &mut to.top,
                    1 => &mut to.right,
                    2 => &mut to.bottom,
                    _ => &mut to.left,
                };
                if lseq >= pseq[side as usize] {
                    set(slot, val);
                }
            }
        }
        // Логические кромки: раскладываются той же картой сторон — сырое
        // значение прогоняется через обычный разбор кромки на настоящую
        // физическую сторону.
        let (i_start, i_end, b_start, b_end) = if side_vertical {
            let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
            let flip = side_sw_lr != rtl;
            let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
            (is, ie, bs, be)
        } else if rtl {
            (1u8, 3u8, 0u8, 2u8)
        } else {
            (3u8, 1u8, 0u8, 2u8)
        };
        let borders = logical.border.clone();
        for (raw, side) in [
            (&borders[0], b_start),
            (&borders[1], i_end),
            (&borders[2], b_end),
            (&borders[3], i_start),
        ] {
            if let Some(v) = raw {
                self.apply_border_shorthand(v, Some(side as usize));
            }
        }
    }

    /// Перевести `em` в точки по размеру шрифта.
    ///
    /// Размер шрифта известен только после каскада, поэтому длины в `em`
    /// доживают до этого места неразрешёнными. Для самого `font-size` база —
    /// РОДИТЕЛЬСКИЙ размер, для остального — свой собственный.
    /// Перевести единицы окна в точки.
    ///
    /// Размер окна известен только сборщику дерева, поэтому `vh`/`vw` доживают
    /// до него неразрешёнными — как и `em` до размера шрифта.
    pub fn resolve_viewport(&mut self, viewport: (f32, f32)) {
        let fix = |l: &mut Option<Len>| match *l {
            Some(Len::Vw(k)) => *l = Some(Len::Px(k * viewport.0)),
            Some(Len::Vh(k)) => *l = Some(Len::Px(k * viewport.1)),
            Some(Len::Calc(i)) => {
                let mut s = crate::value::calc_get(i);
                // Без слагаемых окна складывать нечего: индекс остаётся
                // (арена append-only, `resolve_viewport` идёт на каждом
                // слитом стиле), а `collapse` стёр бы процентную смесь
                // `calc(50% - 3px)` в `None` уже после разбора.
                if s.vw != 0.0 || s.vh != 0.0 {
                    s.px += s.vw * viewport.0 + s.vh * viewport.1;
                    s.vw = 0.0;
                    s.vh = 0.0;
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                match *one {
                    Some(Len::Vw(k)) => *one = Some(Len::Px(k * viewport.0)),
                    Some(Len::Vh(k)) => *one = Some(Len::Px(k * viewport.1)),
                    Some(Len::Calc(i)) => {
                        let mut s = crate::value::calc_get(i);
                        // То же, что у размеров: смесь с долей доживает.
                        if s.vw != 0.0 || s.vh != 0.0 {
                            s.px += s.vw * viewport.0 + s.vh * viewport.1;
                            s.vw = 0.0;
                            s.vh = 0.0;
                            *one = s.collapse_mixed();
                        }
                    }
                    _ => {}
                }
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.font_size,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }

    /// Во сколько раз ИСПОЛЬЗУЕМЫЙ кегль отличается от вычисленного.
    ///
    /// css-fonts-5 §font-size-adjust: `u = (m / m′) s` — желаемая доля `m`,
    /// делённая на ту же метрику шрифта `m′`. `none` и `from-font` (метрика
    /// своего же первого доступного шрифта, отношение ровно 1) кегль не
    /// трогают. Метрику, которую не удалось снять, спека велит не подгонять.
    pub fn used_font_factor(&self, family: &str) -> f32 {
        // Дескриптор `size-adjust` масштабирует ВСЕ метрики лица; поверх него
        // `font-size-adjust` сводит метрику к заданной доле, и множитель
        // дескриптора сокращается: `m / (m′·k) · k = m / m′`
        // (`size-adjust-02/03`). `from-font` — метрика того же лица, то есть
        // остаётся один дескриптор.
        // Множитель — свойство лица, лицо выбирает наклон запроса.
        let slope = if self.oblique == Some(true) {
            2
        } else if self.italic == Some(true) {
            1
        } else {
            0
        };
        let size_adjust = crate::fonts::size_adjust(family, slope);
        match self.font_size_adjust {
            Some((metric, want)) if want.is_finite() => {
                match crate::metrics::adjust_aspect(family, metric) {
                    Some(have) if have > 0.0 => want / have,
                    _ => size_adjust,
                }
            }
            _ => size_adjust,
        }
    }

    pub fn resolve_em(&mut self, parent_font_px: f32) {
        // Сначала свой размер шрифта: от него считается всё остальное. Для
        // него самого единицы шрифта считаются от РОДИТЕЛЬСКОГО кегля.
        // Родовое `monospace` имени семейства не даёт, а меряться должно по
        // тому шрифту, которым текст в самом деле наберётся.
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        // Дорожки сетки в единицах шрифта: считаются от СВОЕГО кегля, он к
        // этому моменту уже разрешён вызывающим (см. ниже по функции).
        let own_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => parent_font_px,
        };
        // Длины в единицах шрифта внутри transform/transform-origin: свой
        // кегль известен только теперь.
        if self.transform_raw.is_some()
            || self.transform_origin_raw.is_some()
            || self.shadow_raw.is_some()
            || self.text_shadow_raw.is_some()
        {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::metrics::ch_ex_px(&family, own_font);
            if let Some(raw) = self.transform_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform", &px);
            }
            if let Some(raw) = self.transform_origin_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform-origin", &px);
            }
            if let Some(raw) = self.shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("box-shadow", &px);
            }
            // `text-shadow` наследуется ВЫЧИСЛЕННЫМ значением: `em` решается
            // кеглем того элемента, где тень объявлена, а потомки получают уже
            // точки (сырая запись есть только у своего стиля).
            if let Some(raw) = self.text_shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("text-shadow", &px);
            }
        }
        if let Some(raw) = self.gradient_em.take() {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::metrics::ch_ex_px(&family, own_font);
            let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
            if let Some(g) = parse_gradient(&px) {
                self.gradient = Some(g);
                if self.gradient_raw.is_some() {
                    self.gradient_raw = Some(px);
                }
            }
        }
        for list in [self.grid_tracks.as_mut(), self.grid_rows.as_mut()]
            .into_iter()
            .flatten()
        {
            for t in list.iter_mut() {
                t.resolve_font(&family, own_px);
            }
        }
        match self.font_size {
            Some(Len::Em(k)) => self.font_size = Some(Len::Px(k * parent_font_px)),
            Some(Len::Ch(k)) => {
                let (ch, _) = crate::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ch));
            }
            Some(Len::Ex(k)) => {
                let (_, ex) = crate::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ex));
            }
            Some(Len::Ic(k)) => {
                self.font_size = Some(Len::Px(k * crate::metrics::ic_px(&family, parent_font_px)));
            }
            _ => {}
        }
        let base = match self.font_size {
            Some(Len::Px(px)) => px,
            _ => parent_font_px,
        };
        // `ch` и `ex` меряются по ГЛИФАМ семейства, а не по кеглю: у Ahem
        // нуль занимает целый кегль, у текстового шрифта — около половины.
        // Семейство здесь уже унаследовано, поэтому замер возможен только на
        // этом шаге, вместе с `em`.
        // Повторный вызов на уже подогнанном стиле: база — сохранённый
        // вычисленный кегль, а не подогнанный.
        let base = self.font_adjust_base.map_or(base, |b| b.0);
        // Используемый кегль (css-fonts-5 §font-size-adjust): «affects the
        // size of relative units that are based on font metrics such as ex
        // and ch but does not affect the size of em units». `em` и числовой
        // `line-height` остаются от `base`, метрики шрифта — от `used`, и сам
        // текст набирается `used` (`font_size`), а детям уходит `base`.
        let used = base * self.used_font_factor(&family);
        if used != base && self.font_adjust_base.is_none() {
            self.font_adjust_base = Some((base, self.line_height));
            if let Some(Len::Pct(m)) = self.line_height {
                self.line_height = Some(Len::Px(m * base));
            }
            self.font_size = Some(Len::Px(used.max(0.01)));
        }
        let (mut ch, ex) = crate::metrics::ch_ex_px(&family, used);
        // `ch` — продвижение нуля вдоль оси строки. При стоящих глифах в
        // вертикальном письме строка идёт сверху вниз, и продвижение равно
        // кеглю, а не ширине глифа (CSS Writing Modes §7.4).
        if self.vertical == Some(true) && self.upright == Some(true) {
            ch = used;
        }
        // `ic` меряется по тому же семейству и тем же шагом, что `ch` и `ex`.
        let ic = crate::metrics::ic_px(&family, used);
        // `cap` — высота прописной того же лица (css-values-4 §6.1.4). Щуп
        // вертикальных метрик её уже отдаёт третьим числом (по нему
        // `text-box-trim` считает срез `cap`), своего замера не нужно.
        let cap = crate::metrics::vmetrics_px(&family, used).2;
        let to_px = move |l: &mut Option<Len>| match *l {
            Some(Len::Em(k)) => *l = Some(Len::Px(k * base)),
            Some(Len::EmPx(k, add)) => *l = Some(Len::Px(k * base + add)),
            Some(Len::Ch(k)) => *l = Some(Len::Px(k * ch)),
            Some(Len::Ex(k)) => *l = Some(Len::Px(k * ex)),
            Some(Len::Ic(k)) => *l = Some(Len::Px(k * ic)),
            // Смешанный calc: шрифтовые слагаемые складываются здесь — база
            // и метрики известны; остаток сворачивается заново.
            Some(Len::Calc(i)) => {
                let mut s = crate::value::calc_get(i);
                // Без шрифтовых слагаемых складывать нечего — индекс остаётся
                // прежним: арена append-only, а `resolve_em` идёт на каждом
                // наследовании, и повторное хранение раздувало бы её впустую.
                if s.em != 0.0 || s.ch != 0.0 || s.ex != 0.0 || s.ic != 0.0 || s.cap != 0.0 {
                    s.px += s.em * base + s.ch * ch + s.ex * ex + s.ic * ic + s.cap * cap;
                    s.em = 0.0;
                    s.ch = 0.0;
                    s.ex = 0.0;
                    s.ic = 0.0;
                    s.cap = 0.0;
                    // Процентная смесь обязана ДОЖИТЬ: `collapse` вернул бы
                    // `None` и стёр `text-indent: calc(1em + 50%)`. Для всего,
                    // что пришло из `Len::parse`, `pct == 0`, и обе свёртки
                    // совпадают.
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let fix = to_px;
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                to_px(one);
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.letter_spacing,
            &mut self.word_spacing,
            &mut self.text_indent,
            &mut self.line_height,
            &mut self.column_width,
            &mut self.column_height,
            &mut self.column_gap,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
        if let Some(o) = self.outline.as_mut() {
            fix(&mut o.width);
            fix(&mut o.offset);
        }
    }

    /// Тот же стиль без коробки — только то, что относится к тексту.
    ///
    /// Нужен там, где абзац разбит на куски: фон, отступы, рамка и размеры
    /// принадлежат абзацу целиком, и повторять их на каждом слове нельзя —
    /// иначе у каждого слова появляется своя подложка и своё поле.
    /// Итоговые возможности OpenType куска — в порядке старшинства
    /// css-fonts-4 §7.2: `font-variant-*` и прочие свойства, затем свойство
    /// `font-feature-settings`. Повтор тега схлопывается, побеждает
    /// последний: прежде в gpui уходило `liga 0, …, liga 1`, а
    /// `apply_font_features` дописывал после них ещё `liga 0`
    /// (`font-features-across-space-3`).
    pub fn used_features(&self) -> Vec<(String, u32)> {
        // Шаг 2 §7.2 — дескриптор правила `@font-face`, МЛАДШЕ свойств.
        let mut all: Vec<(String, u32)> =
            crate::fonts::face_features(self.font_family.as_deref().unwrap_or(""));
        all.extend(self.font_features.iter().cloned());
        all.extend(crate::fonts::alternates::resolve(
            self.font_family.as_deref().unwrap_or(""),
            self.font_alternates.as_ref(),
        ));
        self.add_kerning_feature(&mut all);
        // Шаг 4 §7.2: «setting a non-default value for the letter-spacing
        // property disables optional ligatures» (css-text-3 §8.2). Старше
        // `font-variant-ligatures`, младше `font-feature-settings`
        // (`font-feature-resolution-001/002`: `fvl-1 ls-1` — без лигатуры,
        // `ls-1 ffs-1` — с ней).
        if matches!(self.letter_spacing, Some(Len::Px(v) | Len::Em(v)) if v != 0.0) {
            for tag in ["liga", "clig", "dlig", "hlig"] {
                all.push((tag.to_string(), 0));
            }
        }
        if let Some(settings) = &self.font_settings {
            all.extend(settings.iter().cloned());
        }
        let mut out: Vec<(String, u32)> = Vec::with_capacity(all.len());
        for (tag, value) in all {
            out.retain(|(t, _)| *t != tag);
            out.push((tag, value));
        }
        out
    }

    pub fn text_only(&self) -> Computed {
        Computed {
            color: self.color,
            // Видимость — свойство ТЕКСТА тоже: скрытый кусок держит место, но
            // не красится, а запасная ветка «строка из слов» флаг теряла.
            hidden: self.hidden,
            font_size: self.font_size,
            // Гарнитура — свойство ТЕКСТА: без неё кусок в строчном ряду
            // набирался подменным системным шрифтом, и `@font-face` (в том
            // числе Ahem у стенда) не доезжал никуда, где рядом стоит
            // картинка или иной атом.
            font_family: self.font_family.clone(),
            font_families: self.font_families.clone(),
            font_weight: self.font_weight,
            font_weight_step: self.font_weight_step,
            italic: self.italic,
            oblique: self.oblique,
            underline: self.underline,
            line_through: self.line_through,
            line_height: self.line_height,
            text_align: self.text_align,
            text_align_last: self.text_align_last,
            break_word: self.break_word,
            balance_lines: self.balance_lines,
            bidi_override: self.bidi_override,
            bidi_isolate: self.bidi_isolate,
            bidi_embed: self.bidi_embed,
            hanging: self.hanging,
            nowrap: self.nowrap,
            monospace: self.monospace,
            letter_spacing: self.letter_spacing,
            font_features: self.font_features.clone(),
            font_kerning: self.font_kerning,
            font_settings: self.font_settings.clone(),
            font_alternates: self.font_alternates.clone(),
            ruby_merge: self.ruby_merge,
            text_transform: self.text_transform,
            ellipsis: self.ellipsis,
            overflow_marker: self.overflow_marker.clone(),
            clamp_mark: self.clamp_mark.clone(),
            line_clamp: self.line_clamp,
            clamp_legacy: self.clamp_legacy,
            clamp_auto: self.clamp_auto,
            svg_fill: self.svg_fill.clone(),
            // `stroke` и `stroke-width` в SVG НАСЛЕДУЮТСЯ (SVG 2 §Painting),
            // как и `fill`. Геометрия (`x`, `y`) — нет, её здесь нет намеренно.
            svg_stroke: self.svg_stroke.clone(),
            svg_stroke_width: self.svg_stroke_width.clone(),
            webkit_box: self.webkit_box,
            webkit_box_vertical: self.webkit_box_vertical,
            // Сдвиг от базовой линии — свойство ТЕКСТА: без него строчный
            // кусок в общем прогоне остаётся на базовой линии.
            vertical_shift: self.vertical_shift,
            vertical_shift_pct: self.vertical_shift_pct,
            vertical_shift_px: self.vertical_shift_px,
            vertical_shift_len: self.vertical_shift_len,
            vertical_align_text: self.vertical_align_text,
            vertical_align_base: self.vertical_align_base,
            rel_shift: self.rel_shift,
            text_fit: self.text_fit,
            hyphen_char: self.hyphen_char.clone(),
            // Кусок, собранный из `text_only`, бывает родителем: без базы он
            // отдал бы детям ПОДОГНАННЫЙ кегль, и подгонка накопилась бы.
            font_size_adjust: self.font_size_adjust,
            font_adjust_base: self.font_adjust_base,
            ..Computed::default()
        }
    }

    /// Смесь этого стиля с наведённым по доле перехода.
    ///
    /// Смешиваются свойства, которые в наведении и меняют: цвета, заливка,
    /// прозрачность, толщина рамки. Остальное берётся у наведённого стиля,
    /// как только доля переваливает половину — ступенькой, потому что
    /// промежуточного значения у них нет.
    pub fn blend(&self, hover: &Computed, k: f32) -> Computed {
        let k = k.clamp(0.0, 1.0);
        if k <= 0.0 {
            return self.clone();
        }
        let mut out = if k >= 0.5 {
            hover.clone()
        } else {
            self.clone()
        };
        let mix = |a: Option<Color>, b: Option<Color>| -> Option<Color> {
            match (a, b) {
                (Some(a), Some(b)) => Some(Color {
                    r: a.r + (b.r - a.r) * k,
                    g: a.g + (b.g - a.g) * k,
                    b: a.b + (b.b - a.b) * k,
                    a: a.a + (b.a - a.a) * k,
                }),
                (a, b) => b.or(a),
            }
        };
        out.background = mix(self.background, hover.background);
        out.color = mix(self.color, hover.color);
        out.border_color = mix(self.border_color, hover.border_color);
        out.opacity = match (self.opacity, hover.opacity) {
            (Some(a), Some(b)) => Some(a + (b - a) * k),
            (a, b) => b.or(a),
        };
        out
    }

    /// Собрать стиль узла: правила таблицы (по специфичности), затем `style=""`.
    pub fn resolve(matched: &mut Vec<&Rule>, inline: &Decls) -> Computed {
        Computed::resolve_with_vars(matched, inline, &Decls::new())
    }

    /// То же с переменными темы.
    pub fn resolve_with_vars(matched: &mut Vec<&Rule>, inline: &Decls, vars: &Decls) -> Computed {
        // Слой старше специфичности (css-cascade-5 §6.4): у обычных
        // объявлений поздний слой сильнее, у важных — ранний.
        matched.sort_by(|a, b| {
            (a.origin, &a.layer, a.sel.specificity(), a.order)
                .cmp(&(b.origin, &b.layer, b.sel.specificity(), b.order))
        });
        // `revert-layer` (css-cascade-5 §revert-layer) решается ДО прохода:
        // объявления откатываемого слоя (а у важного — и всё между его
        // обычным и важным уровнями) снимаются с копий правил.
        let reverted = revert_layers(matched);
        let owned_refs: Vec<&crate::css::Rule> = reverted.iter().flatten().collect();
        let matched: &mut Vec<&crate::css::Rule> = &mut if reverted.is_some() {
            owned_refs
        } else {
            matched.clone()
        };
        let mut c = Computed::default();
        // Два прохода по ВСЕМУ каскаду, а не внутри каждого правила: важность
        // — самый старший ключ сравнения (CSS Cascade §6.1), поэтому важное
        // объявление раннего правила обязано пережить обычное объявление
        // позднего. Пока проходы шли внутри правила, `!important` действовал
        // только против соседей по тому же блоку.
        for rule in matched.iter() {
            c.apply_pass(&rule.decls, vars, false);
        }
        c.apply_pass(inline, vars, false);
        // Важные идут в ОБРАТНОМ порядке происхождений: важное правило агента
        // старше важного авторского (§6.4.4), поэтому применяется последним.
        let mut important: Vec<&&Rule> = matched.iter().collect();
        important.sort_by(|a, b| {
            (std::cmp::Reverse(a.origin), std::cmp::Reverse(&a.layer), a.sel.specificity(), a.order)
                .cmp(&(std::cmp::Reverse(b.origin), std::cmp::Reverse(&b.layer), b.sel.specificity(), b.order))
        });
        for rule in important {
            c.apply_pass(&rule.decls, vars, true);
        }
        c.apply_pass(inline, vars, true);
        // `currentColor` в рамке и фоне значит «цвет текста этого элемента» —
        // подставляем уже после того, как цвет стал известен.
        if c.border_color_is_current {
            c.border_color = c.color;
        }

        // Окраска фильтром здесь НЕ делается: результат оседал в
        // долгоживущем стиле узла, и покадровая окраска в `inline::inherit`
        // применяла фильтр ВТОРОЙ раз (grayscale темнил вдвое). Единственная
        // точка окраски — слияние при отрисовке.
        c
    }

    // ПРОБОВАЛИ И ОТКАТИЛИ: блокификация под `float` и абсолютным
    // позиционированием (CSS 2.1 §9.7) — сворачивать `inline`, `inline-block`
    // и внутренние табличные виды в блок после каскада.
    //
    // Замер со всеми элементами: CSS2 4614 -> 4615 (+29/-28), oldfront
    // 2336 -> 2324. Без замещаемых (их размеры считает свой путь): CSS2
    // 4614 -> 4618, oldfront 2336 -> 2325. Потери обоих заходов — семья
    // `left-applies-to-*` и плавающие куски строки: у нас плавающий кусок
    // остаётся в общей строке текста нарочно, и блочным он рвёт соединение
    // букв (тот же корень, что у отката в `render.rs::wrap_floats`).
    //
    // Возвращаться, когда у флоатов появится своя коробка блока
    // (`bands.rs` + `BfcFlow`), а не ряд флекса.

    pub fn apply_decls(&mut self, d: &Decls) {
        for (k, v) in d {
            if k.starts_with(crate::css::CUSTOM_IMPORTANT) {
                continue;
            }
            for part in v.split(crate::css::DECL_SEP) {
                self.apply_one(k, part);
            }
        }
    }

    /// То же, но со словарём переменных: `var(--x)` подставляется значением.
    ///
    /// Без этого современные темы не работают вовсе — они целиком построены на
    /// переменных, и каждое такое объявление молча терялось.
    pub fn apply_decls_with_vars(&mut self, d: &Decls, vars: &Decls) {
        // Два прохода: сначала обычные объявления, затем помеченные
        // `!important`. Так важное перекрывает любое обычное независимо от
        // порядка правил — раньше пометка просто срезалась, и объявление
        // конкурировало на общих основаниях.
        // Порядок объявлений внутри правила: сперва СОКРАЩЁННЫЕ, потом
        // отдельные. `border: solid gray; border-width: 1px 2px 3px 4px`
        // обязано дать 1/2/3/4, а не 3px от сокращения; словарь объявлений
        // исходного порядка не помнит, и без сортировки исход зависел от
        // случайного обхода хеш-таблицы — от запуска к запуску РАЗНОГО
        // (`abs-pos-border-offset-001`). Общность меряется числом дефисов:
        // `border` < `border-width` < `border-top-width`.
        for important in [false, true] {
            self.apply_pass(d, vars, important);
        }
    }

    /// Один проход: только обычные объявления либо только важные.
    fn apply_pass<'a>(&mut self, d: &'a Decls, vars: &Decls, important: bool) {
        // Порядок ЗАПИСИ решает только между СОКРАЩЕНИЕМ и его длинным
        // свойством (`background` и `background-color`, `border` и
        // `border-width`): там он и виден — `background-color: red;
        // background: green` обязано дать зелёный, а обратная запись красный,
        // и словарь без порядка давал одно и то же.
        //
        // Между СОСЕДЯМИ (`border-width` и `border-style`, `white-space` и
        // `overflow-wrap`) порядок записи НЕ применяется: замерено, что от
        // него CSS2 теряет `border-width-012`, а CSS3 — девять пар
        // `textarea-pre-wrap-*`; наши свойства кое-где читают состояние друг
        // друга на применении, и полный порядок вскрывает эту зависимость.
        // Возвращать полный порядок вместе с независимым применением
        // объявлений.
        let order: Vec<&str> = d
            .get(crate::css::ORDER_KEY)
            .map(|s| s.split(crate::css::DECL_SEP).collect())
            .unwrap_or_default();
        let mut keys: Vec<&String> = d.keys().collect();
        // Внутри СЕМЬИ (сокращение и его длинные свойства) порядок — по
        // записи; сами семьи идут прежним порядком «сперва общее».
        // Семья — только НАСТОЯЩЕЕ сокращение со своими длинными свойствами.
        // По одному лишь общему началу судить нельзя: `overflow-wrap` не
        // часть `overflow`, и перестановка этой пары ЗАМЕРЕНА в минус —
        // девять пар `textarea-pre-wrap-*` уходят 0.00 → 0.76. Список
        // расширять по одному, каждое имя — со своим замером.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: добавить сюда `border` и `grid` (со своими
        // исключениями: `border-spacing`/`border-collapse`/`border-radius`/
        // `border-image` не части `border`, `grid-gap` не часть `grid`).
        // Статический просмотр нашёл 6 красных пар, где длинное свойство
        // `border-*` стоит ПЕРЕД сокращением, и 13 таких же с `grid`, но срез
        // из 1817 пар (border/grid/gap/margin-collapse/ch-units/line-names)
        // дал 1290 → 1290: ни одной пары в любую сторону. Значит порядок в
        // этих парах не решает — держат их другие корни.
        const SHORTHANDS: &[&str] = &["background"];
        let семья = |k: &'a str| -> &'a str {
            if font_members::contains(k) && d.contains_key("font") {
                return "font";
            }
            // A side shorthand (`border-right: 12px solid`) resets that
            // side's color to `currentColor`; a later `border-color: pink`
            // must win over it (css-cascade-4 §6.4: order of appearance).
            // Sorted by name the pair always ran `border-color` first, and
            // the side stayed black (css-gaps `grid-gap-decorations-*-ref`
            // `.col-rule`). Only side shorthands and the three all-side
            // shorthands share the order — `border-width`/`border-style`
            // among themselves keep the old order (see the note above).
            const EDGE: &[&str] = &[
                "border-top",
                "border-right",
                "border-bottom",
                "border-left",
                "border-color",
                "border-style",
                "border-width",
            ];
            if EDGE.contains(&k)
                && EDGE[..4].iter().any(|s| d.contains_key(*s))
                && EDGE[4..].iter().any(|s| d.contains_key(*s))
            {
                return "border-edge";
            }
            for root in SHORTHANDS {
                if k.len() > root.len()
                    && k.starts_with(root)
                    && k.as_bytes().get(root.len()) == Some(&b'-')
                    && d.contains_key(*root)
                {
                    return root;
                }
            }
            k
        };
        let место = |k: &str| order.iter().position(|n| *n == k).unwrap_or(usize::MAX);
        let mut keys: Vec<&'a String> = d.keys().collect();
        keys.sort_by_cached_key(|k| {
            let root = семья(k.as_str());
            (
                root.matches('-').count(),
                root,
                if order.is_empty() { 0 } else { место(k) },
                k.as_str(),
            )
        });
        let ordered = keys;
        // `all: revert` / `all: revert-layer` (css-cascade-5 §3.2 «all»,
        // §7.2): откат КАЖДОГО свойства, кроме `direction` и `unicode-bidi`,
        // тем же правилом, что откат одного свойства ниже, — всё, что этот
        // блок сказал о свойстве ДО `all`, снимается. Ключ `all` не
        // разбирался вовсе, и `background-color: red; border-color: red;
        // all: revert` оставлял красные поля ввода (`appearance-revert-001`).
        // Порядок — по первому появлению ключа (`ORDER_KEY`): повтор
        // свойства ПОСЛЕ `all` в том же блоке этим не различается (редкость).
        let all_at = d
            .get("all")
            .and_then(|v| {
                v.split(crate::css::DECL_SEP)
                    .filter(|part| is_important(part) == important)
                    .last()
            })
            .filter(|part| {
                matches!(
                    strip_important(part).trim(),
                    "revert" | "revert-layer" | "initial" | "unset"
                )
            })
            .map(|part| (место("all"), strip_important(part).trim() == "initial"));
        // `all: initial` / `all: unset` (css-cascade-5 §3.2) сбрасывает ВСЁ,
        // что каскад сказал до него, — не только в этом блоке, но и в ранних
        // правилах и в таблице агента: `div` становится строчным, цвет и
        // шрифт — начальными (`initial`) или наследуемыми (`unset`). Кроме
        // `direction` и `unicode-bidi`. Прежде ключ `all` понимал только
        // откат, и `.test { all: initial }` оставлял красную рамку, фон и
        // флоат раннего правила (`all-prop-001/002`). Сброс — СВОЙ шаг
        // прохода: важные объявления ранних правил применяются позже и
        // переживают его, как велит §6.1.
        let all_reset = all_at.filter(|_| {
            d.get("all")
                .and_then(|v| {
                    v.split(crate::css::DECL_SEP)
                        .filter(|part| is_important(part) == important)
                        .last()
                })
                .is_some_and(|part| matches!(strip_important(part).trim(), "initial" | "unset"))
        });
        let all_at = all_at.map(|(at, _)| at);
        if let Some((_, initial)) = all_reset {
            let keep = (
                self.rtl,
                self.bidi_override,
                self.bidi_isolate,
                self.bidi_plaintext,
                self.bidi_embed,
                self.bidi_inherit,
                self.decl_seq,
            );
            *self = Computed::default();
            (
                self.rtl,
                self.bidi_override,
                self.bidi_isolate,
                self.bidi_plaintext,
                self.bidi_embed,
                self.bidi_inherit,
                self.decl_seq,
            ) = keep;
            self.apply_one("display", "inline");
            if initial {
                // Наследуемые свойства: пустое поле у нас значит «от
                // родителя», поэтому начальное значение ставится явно.
                for key in [
                    "color",
                    "font-family",
                    "font-size",
                    "font-style",
                    "font-variant",
                    "font-weight",
                    "letter-spacing",
                    "line-height",
                    "list-style-position",
                    "list-style-type",
                    "quotes",
                    "text-align",
                    "text-indent",
                    "text-transform",
                    "visibility",
                    "white-space",
                    "word-spacing",
                ] {
                    if let Some(start) = initial_value(key) {
                        self.apply_one(key, start);
                    }
                }
            }
        }
        for k in &ordered {
            let Some(v) = d.get(*k) else { continue };
            if k.starts_with("--") || k.as_str() == crate::css::ORDER_KEY
                || k.starts_with(crate::css::CUSTOM_IMPORTANT) {
                continue;
            }
            if let Some(at) = all_at
                && место(k.as_str()) < at
                && !matches!(k.as_str(), "direction" | "unicode-bidi")
            {
                continue;
            }
            // `revert`/`revert-layer` — не ЗНАЧЕНИЕ, а откат каскада
            // (css-cascade-5 §7.2, §7.3): объявление отменяет всё, что этот же
            // блок сказал о свойстве в ту же важность, — блок целиком лежит в
            // одном слое и одном происхождении, откатывать внутри него некуда.
            // Пока слово уезжало в разбор значения, оно там не читалось,
            // объявление выходило негодным (§4.1.7) — и прежнее `red` из того
            // же блока переживало откат (`revert-layer-001`: сплошной красный
            // квадрат вместо зелёного).
            let parts: Vec<&str> = v
                .split(crate::css::DECL_SEP)
                .filter(|part| is_important(part) == important)
                .collect();
            // САМО слово откатa по-прежнему уходит в `apply_one`: для 33
            // свойств из `initial_value()` он значит сброс к начальному, и
            // трогать это поведение здесь незачем.
            let from = parts
                .iter()
                .rposition(|part| {
                    matches!(strip_important(part).trim(), "revert" | "revert-layer")
                })
                .unwrap_or(0);
            for part in parts[from..].iter().copied() {
                // Типизированный `attr()` подставляется тем же шагом, что и
                // `var()` (css-values-5 §7.7): после него значение разбирается
                // как обычное.
                let resolved = resolve_sibling(resolve_attrs(k.as_str(), &resolve_vars(strip_important(part), vars)));
                // CSS Variables §3: invalid after substitution means unset;
                // a preceding specified color cannot survive the computed value.
                if k.as_str() == "color"
                    && crate::css::variable_values::has_var(strip_important(part))
                    && Color::parse(&resolved).is_none()
                    && !matches!(resolved.trim().to_ascii_lowercase().as_str(),
                        "inherit" | "initial" | "unset" | "revert" | "revert-layer")
                {
                    self.apply_one(k, "unset");
                } else {
                    self.apply_one(k, &resolved);
                }
            }
        }
    }

    // `pub(crate)`: `motion` синтезирует строку `transform` и кормит её тем же
    // разборщиком — отдельного конвейера под offset-трансформ нет.
    pub(crate) fn apply_one(&mut self, key: &str, val: &str) {
        self.decl_seq += 1;
        let v = val.trim();
        // Общие для всех свойств слова `initial`/`unset`/`revert`. Для
        // НАСЛЕДУЕМОГО свойства это не «оставить как есть»: незаданное поле у
        // нас берётся от родителя, поэтому такое объявление молча наследовало
        // вместо сброса — на `static-position` отступ первой строки уходил в
        // абсолютный блок, и красное проступало из-под него.
        // `unset` у НАСЛЕДУЕМОГО свойства — это `inherit`, у ненаследуемого —
        // `initial` (css-cascade-4 §7.3.3). Прежде любое `unset` шло в
        // начальное значение, и `color: unset` давал чёрный вместо цвета
        // родителя (`unset-val-001`).
        if v == "unset" && inherited_property(key) {
            return self.apply_one(key, "inherit");
        }
        if matches!(v, "initial" | "unset" | "revert" | "revert-layer")
            && let Some(start) = initial_value(key)
        {
            return self.apply_one(key, start);
        }
        // Начальные значения, которые годятся ТОЛЬКО для `initial`/`unset`:
        // `revert` у автора откатывает к таблице агента, а там у `div`
        // `display: block`, не начальное `inline`.
        if matches!(v, "initial" | "unset")
            && let Some(start) = match key {
                "background-color" => Some("transparent"),
                "background-image" => Some("none"),
                "display" => Some("inline"),
                "float" => Some("none"),
                "position" => Some("static"),
                "opacity" => Some("1"),
                _ => None,
            }
        {
            return self.apply_one(key, start);
        }
        // Фон — СПИСОК слоёв (css-backgrounds-3 §2.1: «comma-separated list
        // of values … the first value represents the top layer»). Одиночные
        // поля стиля несут верхний слой, а список целиком хранится сырым —
        // по нему рисуются все слои (`Computed::bg_layers`). Запись без
        // запятой список своего свойства снимает; сокращение — все.
        if BG_LIST_KEYS.contains(&key) {
            if key == "background" {
                self.bg_lists.clear();
            } else {
                self.bg_lists.retain(|(k, _)| k != key);
            }
            if top_level_comma(v).is_some() {
                if key != "background" {
                    // Верхний слой — обычным разбором; список кладётся ПОСЛЕ:
                    // вложенный вызов того же свойства его бы снял.
                    let first = background_layers(v)[0].to_string();
                    self.apply_one(key, &first);
                    self.bg_lists.push((key.to_string(), v.to_string()));
                    return;
                }
                self.bg_lists.push((key.to_string(), v.to_string()));
            }
        }
        // Свойства разнесены по группам (`props/*`): у каждого ключа ровно одна группа,
        // ветви внутри группы идут в исходном порядке.
        let groups: [fn(&mut Self, &str, &str, &str, &mut bool); 13] = [
            Self::apply_display_flex,
            Self::apply_grid,
            Self::apply_box_model,
            Self::apply_border,
            Self::apply_position,
            Self::apply_background,
            Self::apply_mask_clip,
            Self::apply_font,
            Self::apply_text,
            Self::apply_text_decor,
            Self::apply_multicol,
            Self::apply_transform,
            Self::apply_effects,
        ];
        for group in groups {
            let mut hit = true;
            group(self, key, val, v, &mut hit);
            if hit {
                return;
            }
        }
    }

    /// Толщина рамки, как её видит модель коробки. Начальный `border-style`
    /// — `none`, а рамка без рисунка вычисляется в ноль (css-backgrounds-3
    /// §4.3), поэтому заданная, но не нарисованная толщина не занимает места.
    /// Разбор всех пяти свойств рамки-картинки в одну запись.
    ///
    /// Сокращение несёт до трёх частей через косую: `<источник> <срез> /
    /// <ширина> / <вылет>` и укладку в конце. Отдельные свойства дополняют ту
    /// же запись, поэтому она заводится по первому же из них.
    fn set_border_image(&mut self, name: &str, v: &str) {
        let mut image = self.border_image.clone().unwrap_or(BorderImage {
            src: String::new(),
            slice: [BorderImageSlice::Pct(1.0); 4],
            fill: false,
            width: [BorderImageWidth::Times(1.0); 4],
            outset: [0.0; 4],
            repeat: (Tiling::None, Tiling::None),
        });
        let mut set = |part: &str, value: &str| match part {
            "border-image-source" => {
                let value = value.trim();
                if value == "none" {
                    image.src.clear();
                } else if value.starts_with("linear-gradient(")
                    || value.starts_with("radial-gradient(")
                    || value.starts_with("conic-gradient(")
                {
                    // Источником может быть любой `<image>`, включая градиент
                    // (css-backgrounds-3 §6.1). Запись хранится как есть:
                    // растрирует её загрузчик картинок.
                    image.src = value.to_string();
                } else if let Some(url) = parse_url(value) {
                    image.src = url;
                }
            }
            "border-image-slice" => {
                image.fill = value.split_whitespace().any(|w| w == "fill");
                let nums: Vec<&str> = value.split_whitespace().filter(|w| *w != "fill").collect();
                let one = |t: &str| match t.strip_suffix('%') {
                    Some(n) => n
                        .parse()
                        .ok()
                        .map(|k: f32| BorderImageSlice::Pct(k / 100.0)),
                    None => t.parse().ok().map(BorderImageSlice::Px),
                };
                if let Some(sides) = four(&nums, one) {
                    image.slice = sides;
                }
            }
            "border-image-width" => {
                let one = |t: &str| {
                    if t == "auto" {
                        return Some(BorderImageWidth::Auto);
                    }
                    if let Some(n) = t.strip_suffix('%') {
                        return n
                            .parse()
                            .ok()
                            .map(|k: f32| BorderImageWidth::Pct(k / 100.0));
                    }
                    // Голое число — множитель толщины рамки, и проверяется ДО
                    // Len: та принимает числа без единиц как точки, и
                    // `border-image-width: 1` превращался в рамку 1px.
                    if let Ok(k) = t.parse::<f32>() {
                        return Some(BorderImageWidth::Times(k));
                    }
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(BorderImageWidth::Px(v)),
                        _ => None,
                    }
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.width = sides;
                }
            }
            "border-image-outset" => {
                let one = |t: &str| match Len::parse(t) {
                    Some(Len::Px(v)) => Some(v),
                    // Голое число — тоже множитель толщины рамки, но её здесь
                    // ещё нет; берём как точки, это ближе всего к правде.
                    _ => t.parse().ok(),
                };
                let words: Vec<&str> = value.split_whitespace().collect();
                if let Some(sides) = four(&words, one) {
                    image.outset = sides;
                }
            }
            "border-image-repeat" => {
                let one = |t: &str| match t {
                    "stretch" => Some(Tiling::None),
                    "repeat" => Some(Tiling::Repeat),
                    "round" => Some(Tiling::Round),
                    "space" => Some(Tiling::Space),
                    _ => None,
                };
                let mut it = value.split_whitespace();
                if let (Some(x), y) = (it.next().and_then(one), it.next().and_then(one)) {
                    image.repeat = (x, y.unwrap_or(x));
                }
            }
            _ => {}
        };
        if name == "border-image" {
            // Части сокращения разделены косой: источник со срезом, ширина,
            // вылет. Укладка и `fill` живут в своих частях и находятся по
            // ключевым словам.
            // Косая режет части ТОЛЬКО вне скобок: в адресе она разделяет
            // каталоги, и запись рвалась по первому же пути (`url(C:/tmp/…)`).
            let mut parts: Vec<&str> = vec![];
            let mut depth = 0i32;
            let mut from = 0usize;
            for (at, ch) in v.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    '/' if depth == 0 => {
                        parts.push(&v[from..at]);
                        from = at + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&v[from..]);
            let head = parts.first().copied().unwrap_or("");
            // Слова головы режутся ВНЕ скобок: у градиента-источника пробелы
            // внутри (`linear-gradient(green, green)`), и он должен остаться
            // одним словом.
            let words = split_outside_parens(head);
            let (urls, rest): (Vec<&String>, Vec<&String>) = words.iter().partition(|w| {
                w.starts_with("url(")
                    || w.starts_with("linear-gradient(")
                    || w.starts_with("radial-gradient(")
                    || w.starts_with("conic-gradient(")
                    || w.as_str() == "none"
            });
            if let Some(src) = urls.first() {
                set("border-image-source", src);
            }
            let (repeat, slice): (Vec<&&String>, Vec<&&String>) = rest
                .iter()
                .partition(|w| matches!(w.as_str(), "stretch" | "repeat" | "round" | "space"));
            if !slice.is_empty() {
                let joined: Vec<&str> = slice.iter().map(|w| w.as_str()).collect();
                set("border-image-slice", &joined.join(" "));
            }
            if !repeat.is_empty() {
                let joined: Vec<&str> = repeat.iter().map(|w| w.as_str()).collect();
                set("border-image-repeat", &joined.join(" "));
            }
            // Укладка (`stretch|repeat|round|space`) по грамматике `||` может
            // стоять и ПОСЛЕ ширины без своей косой: `/ 0px space round`.
            // Слова укладки вынимаются из хвостовых частей, остаток — ширина
            // и вылет.
            let mut tail_repeat: Vec<String> = vec![];
            let mut strip = |part: &str, reps: &mut Vec<String>| -> String {
                let (found, rest): (Vec<&str>, Vec<&str>) = part
                    .split_whitespace()
                    .partition(|w| matches!(*w, "stretch" | "repeat" | "round" | "space"));
                reps.extend(found.into_iter().map(str::to_string));
                rest.join(" ")
            };
            let width = parts.get(1).map(|p| strip(p, &mut tail_repeat));
            let outset = parts.get(2).map(|p| strip(p, &mut tail_repeat));
            drop(strip);
            if let Some(width) = width.filter(|w| !w.is_empty()) {
                set("border-image-width", &width);
            }
            if let Some(outset) = outset.filter(|o| !o.is_empty()) {
                set("border-image-outset", &outset);
            }
            if !tail_repeat.is_empty() {
                set("border-image-repeat", &tail_repeat.join(" "));
            }
        } else {
            set(name, v);
        }
        drop(set);
        // Запись хранится и БЕЗ источника: лонгхенды приходят в любом порядке,
        // и `border-image-slice` до `border-image-source` иначе выбрасывался —
        // источник, пришедший следом, получал срезы по умолчанию (вся
        // картинка), и рамка рисовалась четырьмя сжатыми копиями образа.
        // Не рисовать и не подавлять обычную рамку при пустом источнике —
        // забота потребителей.
        self.border_image = Some(image);
    }

    /// Активный кламп строк: стандартный `line-clamp` всегда, а
    /// `-webkit-line-clamp` — только в паре с `-webkit-box` по вертикали.
    pub fn clamp_lines(&self) -> Option<u32> {
        let legacy_ok = self.webkit_box == Some(true) && self.webkit_box_vertical == Some(true);
        self.line_clamp
            .filter(|_| self.clamp_legacy != Some(true) || legacy_ok)
    }

    /// Обводка `border-shape` по «relevant side» (css-borders-4
    /// §border-shape-relevant-side): первая сторона в порядке block-start,
    /// inline-start, block-end, inline-end со стилем не `none`, иначе
    /// block-start; берутся её толщина и цвет (Blink
    /// `RelevantSideForBorderShape`). Физические индексы t/r/b/l = 0..3;
    /// без цвета — цвет текста, без него чёрный (`currentColor`).
    pub fn border_shape_stroke(&self) -> (f32, Color) {
        let vertical = self.vertical == Some(true);
        let rl = self.vertical_rl == Some(true);
        let rtl = self.rtl == Some(true);
        let (block_start, block_end) = match (vertical, rl) {
            (false, _) => (0usize, 2usize),
            (true, true) => (1, 3),
            (true, false) => (3, 1),
        };
        let (inline_start, inline_end) = match (vertical, rtl) {
            (false, false) => (3usize, 1usize),
            (false, true) => (1, 3),
            (true, false) => (0, 2),
            (true, true) => (2, 0),
        };
        let order = [block_start, inline_start, block_end, inline_end];
        let side = order
            .iter()
            .copied()
            .find(|i| self.border_visible[*i] == Some(true))
            .unwrap_or(block_start);
        let w = self.borders();
        let width = match [w.top, w.right, w.bottom, w.left][side] {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let colour = self.border_colors[side]
            .or(self.border_color)
            .or(self.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        (width, colour)
    }

    /// Края опорной коробки `<geometry-box>` относительно border-box, t/r/b/l,
    /// наружу положительные: margin-box шире на поля, padding-box уже на
    /// рамку, content-box — на рамку и отбивку, half-border-box — на половину
    /// рамки (Blink `GeometryBoxUtils::ReferenceBoxBorderBoxOutsets`).
    pub fn geometry_outsets(&self, kind: u8) -> [f32; 4] {
        let px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = self.borders();
        let bw = [px(b.top), px(b.right), px(b.bottom), px(b.left)];
        let pd = [
            px(self.padding.top),
            px(self.padding.right),
            px(self.padding.bottom),
            px(self.padding.left),
        ];
        let mg = [
            px(self.margin.top),
            px(self.margin.right),
            px(self.margin.bottom),
            px(self.margin.left),
        ];
        let mut out = [0.0f32; 4];
        for i in 0..4 {
            out[i] = match kind {
                1 => mg[i],
                2 => -bw[i],
                3 => -(bw[i] + pd[i]),
                4 => -bw[i] / 2.0,
                _ => 0.0,
            };
        }
        out
    }

    /// Вынос слоёв `border-shape` за border-box (t/r/b/l): половина обводки
    /// наружу у одной фигуры, опорная коробка шире border-box (margin-box) и
    /// запас под митры прямолинейного контура — Blink держит предел митры 1e10
    /// у polygon, шип длиной w/sin(θ/2); 5w покрывает углы от ~23°
    /// (border-shape-polygon-miter-limit: шип ~80 px при w=20).
    pub fn border_shape_ext(&self) -> [f32; 4] {
        let Some(bs) = &self.border_shape else {
            return [0.0; 4];
        };
        let out = self.geometry_outsets(bs.outer_box);
        let stroke = if bs.inner.is_some() {
            0.0
        } else {
            self.border_shape_stroke().0
        };
        let spike = if stroke > 0.0 && crate::background::shape_is_linear(&bs.outer) {
            stroke * 5.0
        } else {
            0.0
        };
        let mut ext = out.map(|o| o.max(0.0) + stroke / 2.0 + spike);
        // Тени повторяют фигуру (css-borders-4 §border-shape-shadow-interaction)
        // и рисуются растром на той же области (`background::
        // border_shape_shadow_svg`): наружная уходит за border-box на разлёт,
        // смещение и хвост размытия (3σ = 1.5·blur); у внутренней хвост
        // размытия тоже нужен — область фильтра обрезает бросающий
        // прямоугольник, и без запаса край холста просвечивал бы.
        for sh in &self.shadows {
            let tail = sh.spread.max(0.0) + sh.blur.max(0.0) * 1.5 + 1.0;
            ext[0] = ext[0].max(tail - sh.y);
            ext[1] = ext[1].max(tail + sh.x);
            ext[2] = ext[2].max(tail + sh.y);
            ext[3] = ext[3].max(tail - sh.x);
        }
        for sh in &self.inset_shadows {
            let tail = sh.blur.max(0.0) * 1.5 + 1.0;
            for e in &mut ext {
                *e = e.max(tail);
            }
        }
        // Контур `outline` повторяет фигуру (слой над группой,
        // `background::border_shape_outline_svg`) — вынос на сдвиг и толщину.
        if let Some((w, off, _)) = self.shaped_outline() {
            let reach = (off + w).max(0.0) + 1.0;
            for e in &mut ext {
                *e = e.max(reach);
            }
        }
        ext
    }

    /// Контур `outline` коробки с `border-shape`, который рисуется по
    /// фигуре: (толщина, сдвиг, цвет). Только сплошной/`auto`/`double`
    /// (css-ui-4; Blink `BorderShapePainter::PaintOutline` остальные стили
    /// отдаёт обычному контуру) и видимый. Толщина без значения — `medium`
    /// (3 px), цвет без своего — `accent-color` при `auto`, иначе цвет текста,
    /// иначе чёрный (как у `render::decorations`); `outline-offset: inset` —
    /// минус толщина. Шрифтовые единицы — своим кеглем.
    pub fn shaped_outline(&self) -> Option<(f32, f32, Color)> {
        let o = self.outline.as_ref()?;
        self.border_shape.as_ref()?;
        if !matches!(o.style, Some(1) | Some(2) | Some(OUTLINE_DOUBLE)) {
            return None;
        }
        let em = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em,
            _ => 0.0,
        };
        let w = match o.width {
            None => 3.0,
            other => px_of(other),
        };
        if w <= 0.0 {
            return None;
        }
        let off = if o.inset { -w } else { px_of(o.offset) };
        let colour = o
            .color
            .or(if o.style == Some(2) { self.accent_color } else { None })
            .or(self.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        Some((w, off, colour))
    }

    /// Переполнение коробки с `border-shape` режется внутренним контуром
    /// фигуры (css-borders-4 §border-shape-overflow-interaction: «The inner
    /// border-shape clips the overflow content of the element»): маска
    /// группы берёт внутренний контур, а кольцо рамки ложится НАД буфером
    /// (`Grouped::over`). `scroll`/`auto` идут лентой прокрутки мимо группы.
    pub fn border_shape_clips(&self) -> bool {
        self.border_shape.is_some()
            && (matches!(self.overflow_x, Some(Overflow::Hidden) | Some(Overflow::Clip))
                || matches!(self.overflow_y, Some(Overflow::Hidden) | Some(Overflow::Clip)))
    }

    /// Тени `box-shadow` с решённым цветом: без своего цвета — цвет текста
    /// (css-backgrounds-3 §box-shadow, `currentColor`; метка — отрицательная
    /// альфа, как у `apply::shadow_colour`). `inset` — внутренние.
    pub fn resolved_shadows(&self, inset: bool) -> Vec<(Shadow, Color)> {
        let list = if inset { &self.inset_shadows } else { &self.shadows };
        list.iter()
            .map(|sh| {
                let colour = if sh.color.a < 0.0 {
                    self.color.unwrap_or(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    })
                } else {
                    sh.color
                };
                (sh.clone(), colour)
            })
            .collect()
    }

    /// Есть ли угол с формой, отличной от круглой, при ненулевом радиусе
    /// (css-borders-4 §corner-shaping: «if border-radius is 0, corner-shape
    /// won't have any effect»). Такой угол уходит растровой маской контура,
    /// а рамка красится кольцом по контуру (`render::decorations`).
    pub fn corner_shaped(&self) -> bool {
        let Some(k) = self.corner_shape else {
            return false;
        };
        let radii = [self.radius.tl, self.radius.tr, self.radius.br, self.radius.bl];
        k.iter().zip(radii).any(|(k, r)| {
            let shaped = (*k - 1.0).abs() > 1e-3;
            let has_radius = match r {
                Some(Len::Px(v)) => v > 0.0,
                Some(Len::Pct(p)) => p > 0.0,
                _ => false,
            };
            shaped && has_radius
        })
    }

    /// Обособление размера не действует на таблицу: её размер задают
    /// дорожки, а не «содержимое как таковое» (css-contain-2 §size
    /// containment; Blink `layout_table.h` — таблица не годится под него).
    fn size_containment_applies(&self) -> bool {
        !matches!(
            self.display,
            Some(Display::Table) | Some(Display::InlineTable)
        )
    }

    /// Повтор «сколько влезет» по оси РЯДОВ, если он задан и годен к
    /// передаче раскладке (есть размер дорожки).
    pub fn grid_rows_repeat(&self) -> Option<AutoRepeat> {
        let r = self.auto_repeat_rows?;
        (r.track.is_some() || r.track_pct.is_some()).then_some(r)
    }

    /// Обособлена ли СТРОЧНАЯ ось: `contain: size` держит обе, `inline-size`
    /// только её (css-contain-2 §containment-types).
    pub fn contains_inline_size(&self) -> bool {
        self.size_containment_applies()
            && (self.contain_size == Some(true) || self.contain_inline_size == Some(true))
    }

    /// Обособлена ли БЛОЧНАЯ ось.
    pub fn contains_block_size(&self) -> bool {
        self.size_containment_applies() && self.contain_size == Some(true)
    }

    /// То же по ФИЗИЧЕСКИМ осям: при вертикальном письме строчная ось идёт
    /// сверху вниз, и обособление меняется местами.
    pub fn contains_width(&self) -> bool {
        if self.vertical == Some(true) {
            self.contains_block_size()
        } else {
            self.contains_inline_size()
        }
    }

    /// Обособлена ли высота (физическая ось).
    pub fn contains_height(&self) -> bool {
        if self.vertical == Some(true) {
            self.contains_inline_size()
        } else {
            self.contains_block_size()
        }
    }

    pub fn borders(&self) -> Sides {
        let vis = self.border_visible;
        let keep = |w, i: usize| if vis[i] == Some(true) { w } else { None };
        Sides {
            top: keep(self.border_width.top, 0),
            right: keep(self.border_width.right, 1),
            bottom: keep(self.border_width.bottom, 2),
            left: keep(self.border_width.left, 3),
        }
    }

    /// `border-left-style: solid` — рисунок одной стороны. Без рисунка её
    /// толщина не считается, поэтому видимость помним по сторонам.
    fn set_border_style(&mut self, v: &str, side: Option<usize>) {
        let on = border_style(v);
        if let Some(r) = border_style_rank(v) {
            match side {
                None => self.border_side_styles = [Some(r); 4],
                Some(i) => self.border_side_styles[i] = Some(r),
            }
        }
        self.set_visible(side, on);
        if on {
            let w = match side {
                Some(0) => &mut self.border_width.top,
                Some(1) => &mut self.border_width.right,
                Some(2) => &mut self.border_width.bottom,
                _ => &mut self.border_width.left,
            };
            if w.is_none() {
                *w = Some(Len::Px(3.0));
            }
        }
    }

    fn set_visible(&mut self, side: Option<usize>, on: bool) {
        match side {
            None => self.border_visible = [Some(on); 4],
            Some(i) => self.border_visible[i] = Some(on),
        }
    }

    /// `border: 1px solid #333` — ширина и цвет; стиль линии GPUI различает
    /// только solid/dashed на весь элемент, поэтому его не разбираем.
    fn apply_border_shorthand(&mut self, v: &str, side: Option<usize>) {
        // Негодная часть роняет ВСЁ объявление (§4.2), а не пропускается:
        // `border: -1px solid red` не даёт ни рамки `medium`, ни красного
        // цвета. Проверка отдельным проходом — применение ниже правит поля по
        // ходу разбора, и откатить его на середине уже нельзя.
        let known = |token: &str| {
            token == "none"
                || token == "hidden"
                || border_style(token)
                || line_width(token).is_some()
                || Color::parse(token).is_some()
        };
        if !split_outside_parens(v)
            .iter()
            .all(|t| known(t.as_str().trim()))
        {
            return;
        }
        // Каждая часть встречается не больше ОДНОГО раза (§8.5.4: сокращение
        // это `<border-width> || <border-style> || <border-color>`).
        // `border: 1px solid red green` негодно целиком, а прежде вторая
        // краска просто побеждала первую.
        {
            let (mut w, mut st, mut c) = (0usize, 0usize, 0usize);
            for token in split_outside_parens(v) {
                let t = token.as_str().trim();
                if t == "none" || t == "hidden" || border_style(t) {
                    st += 1;
                } else if line_width(t).is_some() {
                    w += 1;
                } else if Color::parse(t).is_some() {
                    c += 1;
                }
            }
            if w > 1 || st > 1 || c > 1 {
                return;
            }
        }
        let mut width = None;
        let mut color = None;
        let mut visible_style = false;
        // Скобки не разрываем: `border: 1px solid rgba(0, 0, 0, .2)`.
        for token in split_outside_parens(v) {
            let token = token.as_str();
            if token == "none" || token == "hidden" {
                width = Some(Len::Px(0.0));
                self.set_visible(side, false);
                let r = border_style_rank(token);
                match side {
                    None => self.border_side_styles = [r; 4],
                    Some(i) => self.border_side_styles[i] = r,
                }
            } else if border_style(token) {
                visible_style = true;
                self.set_visible(side, true);
                if let Some(r) = border_style_rank(token) {
                    match side {
                        None => self.border_side_styles = [Some(r); 4],
                        Some(i) => self.border_side_styles[i] = Some(r),
                    }
                }
                self.border_dashed = Some(token == "dashed");
                self.border_dotted = Some(token == "dotted");
            } else if let Some(l) = line_width(token) {
                width = Some(l);
            } else if let Some(c) = Color::parse(token) {
                color = Some(c);
            }
        }
        // Толщина в сокращении необязательна: `border: orange solid` — это
        // рамка `medium`, то есть 3px (css-backgrounds-3 §4.2). Раньше такая
        // запись оставляла коробку БЕЗ рамки, и её содержимое считалось по
        // другой ширине (`word-break-break-all-062`).
        if width.is_none() && visible_style {
            width = Some(Len::Px(3.0));
        }
        // Цвет из БОКОВОГО сокращения принадлежит своей стороне: раньше он
        // писался в общий цвет, и `border-bottom: 2px solid red` красил все
        // четыре стороны.
        // Опущенная часть сокращения возвращается к НАЧАЛЬНОМУ значению
        // (§1.4.2, §8.5.4): у цвета это `currentColor`, то есть пусто — цвет
        // решает отрисовка. Прежде запись `border: solid 1em` оставляла цвет
        // от менее специфичного правила.
        match (color, side) {
            (Some(c), None) => {
                self.border_color = Some(c);
                self.border_colors = [Some(c); 4];
            }
            (Some(c), Some(i)) => self.border_colors[i] = Some(c),
            (None, None) => {
                self.border_color = None;
                self.border_colors = [None; 4];
            }
            // Боковое сокращение без цвета даёт стороне `currentColor`
            // (§8.5.4), и он обязан перебить общий цвет менее специфичного
            // правила: `div { border-color: red }` + `.test { border-top: solid
            // 1em }` — верх цвета ТЕКСТА (`border-shorthands-003`). Пустой слот
            // стороны значит «взять общий», поэтому сторона помечается.
            (None, Some(i)) => {
                self.border_colors[i] = None;
                self.border_side_current[i] = true;
            }
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

/// Рисунок рамки, при котором она ВИДНА. `none` и `hidden` сюда не входят:
/// они рамку убирают.
/// Ранг стиля кромки для разбора конфликтов (см. `border_side_styles`).
fn border_style_rank(v: &str) -> Option<u8> {
    Some(match v.to_ascii_lowercase().as_str() {
        "none" => 0,
        "hidden" => 1,
        "inset" => 3,
        "groove" => 4,
        "outset" => 5,
        "ridge" => 6,
        "dotted" => 7,
        "dashed" => 8,
        "solid" => 9,
        "double" => 10,
        _ => return None,
    })
}

fn border_style(v: &str) -> bool {
    // Значения CSS нечувствительны к регистру: `border: 1px SOLID red` — та же
    // рамка. К нижнему регистру приводится только ИМЯ свойства.
    matches!(
        v.to_ascii_lowercase().as_str(),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset"
    )
}

/// Толщина рамки словом: `thin`, `medium`, `thick` (css-backgrounds-3 §4.1).
/// Сторона по имени свойства: верх, право, низ, лево.
fn side_index(key: &str) -> usize {
    match key.split('-').nth(1) {
        Some("right") => 1,
        Some("bottom") => 2,
        Some("left") => 3,
        _ => 0,
    }
}

fn line_width(v: &str) -> Option<Len> {
    // Отрицательная толщина недействительна (§8.5.1) и делает объявление
    // НЕВАЛИДНЫМ целиком (§4.2): `border-width: -1px` доживало до отрисовки
    // вместо отката к прежнему значению.
    let non_negative = |l: Len| {
        (!matches!(
            l,
            Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v) if v < 0.0
        ))
        .then_some(l)
    };
    match v.to_ascii_lowercase().as_str() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v).and_then(non_negative),
    }
}

/// Пара значений «вдоль оси»: одно значение — обе грани, два — по порядку.
fn axis_pair(v: &str) -> (Option<Len>, Option<Len>) {
    let mut it = v.split_whitespace();
    let a = it.next().and_then(Len::parse);
    let b = it.next().and_then(Len::parse).or(a);
    (a, b)
}

/// Цвет стороны рамки; `currentColor` даёт цвет текста этого же узла.
fn side_color(v: &str, current: Option<Color>) -> Option<Color> {
    if v.eq_ignore_ascii_case("currentcolor") {
        return current;
    }
    Color::parse(v)
}

fn parse_align(v: &str) -> Option<Align> {
    align_keyword(v).unwrap_or(None)
}

/// Несёт ли значение выравнивания модификатор `safe` (css-align §5.3).
fn is_safe(v: &str) -> bool {
    v.split_whitespace().next() == Some("safe")
}

/// Значение выравнивания по css-align-3.
///
/// Исходов три, а не два. `Ok(Some)` — выравнивание задано; `Ok(None)` —
/// значение верное, но своего выравнивания не несёт (`normal`, `auto`), и поле
/// надо ОЧИСТИТЬ, чтобы решала раскладка; `Err(())` — объявление негодное,
/// и прежнее значение остаётся. Раньше эти три случая делились на два
/// по-разному у разных свойств: у `align-items` негодное значение сохраняло
/// прежнее, у `justify-items` — стирало его.
fn align_keyword(v: &str) -> Result<Option<Align>, ()> {
    let mut it = v.split_whitespace();
    let mut word = it.next().ok_or(())?;
    // `safe`/`unsafe` — что делать при переполнении области; сама позиция от
    // этого не меняется.
    if matches!(word, "safe" | "unsafe") {
        word = it.next().ok_or(())?;
    }
    Ok(match word {
        "center" => Some(Align::Center),
        // §anchor-center (только `align-self`/`justify-self`; у `*-items`
        // значение отброшено спекой — `apply` его там игнорирует).
        "anchor-center" => Some(Align::AnchorCenter),
        // `self-start`/`self-end` считаются по письму САМОГО элемента,
        // `start`/`end` — по письму контейнера (css-align-3 §6.2). Разница
        // видна, как только элемент несёт своё `direction`/`writing-mode`:
        // `flexbox-align-self-vert-002` ждёт `self-start` СПРАВА у элемента
        // с `direction: rtl`. Само значение остаётся физическим, а «мерить
        // по себе» помнится отдельным флагом `align_self_own_axis` — его
        // зеркалит `inline::inherit`, где известны письмо элемента И письмо
        // родителя.
        "start" | "flex-start" | "left" => Some(Align::Start),
        "end" | "flex-end" | "right" => Some(Align::End),
        "self-start" => Some(Align::Start),
        "self-end" => Some(Align::End),
        "stretch" => Some(Align::Stretch),
        // `first baseline` — обычное выравнивание по базовой линии. `last`
        // честно раскладке неизвестен, но для ОДНОСТРОЧНЫХ участников первая
        // и последняя базовые совпадают — суррогат первой ближе очистки
        // (flex-order-last-baseline; прежний None ронял участника в stretch).
        "baseline" | "first" | "last" => Some(Align::Baseline),
        // `normal` у растяжимого элемента даёт растяжение, а у замещаемого —
        // прижим к началу. Выбирает это сама раскладка, когда поле пусто.
        "normal" | "auto" => None,
        _ => return Err(()),
    })
}

fn parse_justify(v: &str) -> Option<Justify> {
    // `safe`/`unsafe` говорят, что делать при переполнении области; позиция
    // от этого не меняется, поэтому приставка снимается.
    let v = v
        .strip_prefix("safe ")
        .or_else(|| v.strip_prefix("unsafe "))
        .unwrap_or(v)
        .trim();
    match v {
        "center" => Some(Justify::Center),
        "flex-start" => Some(Justify::Start),
        "flex-end" => Some(Justify::End),
        // `left`/`right` физические; при письме слева направо они совпадают
        // с началом и концом строки.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: развести их в отдельные значения, чтобы при
        // rtl они НЕ переставлялись вместе с `start`/`end` (css-align-3 §4).
        // Правка верна по спеке, но полный свод обоих: 0 и 0 —
        // `flexbox_justifycontent-right-002` (8.53) держит не это.
        "start" => Some(Justify::WmStart),
        "end" => Some(Justify::WmEnd),
        "left" => Some(Justify::Left),
        "right" => Some(Justify::Right),
        "space-between" => Some(Justify::Between),
        "space-around" => Some(Justify::Around),
        "space-evenly" => Some(Justify::Evenly),
        "stretch" => Some(Justify::Stretch),
        _ => None,
    }
}

/// Одна грань размещения: `3`, `span 2`, `auto`.
/// Годно ли значение сокращения `background` целиком.
///
/// Негодное объявление ОТБРАСЫВАЕТСЯ, а не сбрасывает свои длинные свойства
/// (CSS 2.1 §4.1.7): `background: green` в одном правиле и `background: red\;`
/// (значение с экранированной точкой с запятой, то есть цвет `red;`) в
/// другом обязаны оставить фон зелёным. Разбор ниже намеренно снисходителен —
/// он берёт из записи всё, что узнал, — поэтому годность проверяется
/// отдельно, и только ею решается сброс (`escapes-002/014`, `keywords-000`).
fn background_shorthand_valid(v: &str) -> bool {
    if v.contains("gradient(") || v.contains("url(") {
        return true;
    }
    let mut any = false;
    for token in split_outside_parens(v) {
        // Запятая слоя (`none, none`) — не часть слова.
        let t = token.trim().trim_end_matches(',').trim();
        if t.is_empty() || t == "/" {
            continue;
        }
        any = true;
        let known = matches!(
            t,
            "none"
                | "transparent"
                | "initial"
                | "unset"
                | "revert"
                | "no-repeat"
                | "repeat"
                | "repeat-x"
                | "repeat-y"
                | "space"
                | "round"
                | "cover"
                | "contain"
                | "scroll"
                | "fixed"
                | "local"
                | "border-box"
                | "padding-box"
                | "content-box"
                | "text"
                | "left"
                | "right"
                | "top"
                | "bottom"
                | "center"
        ) || Len::parse(t).is_some()
            || Color::parse(t).is_some()
            || parse_image_color(t).is_some();
        if !known {
            return false;
        }
    }
    any
}

/// Токены записи шаблона для имён линий: `[имена]`, `функция(…)`, слова.
fn line_name_tokens(v: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut paren, mut bracket) = (0i32, 0i32);
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        cur.clear();
    };
    for ch in v.chars() {
        match ch {
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            '[' if paren == 0 => {
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
                bracket += 1;
                cur.push(ch);
            }
            ']' if paren == 0 => {
                bracket -= 1;
                cur.push(ch);
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
            }
            c if c.is_whitespace() && paren == 0 && bracket == 0 => flush(&mut cur, &mut out),
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// `[a b]` → `["a", "b"]`.
fn bracket_names(t: &str) -> Option<Vec<String>> {
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.split_whitespace().map(str::to_string).collect())
}

/// Имена линий тела `repeat(…)`: у шаблона — по линиям вокруг дорожек тела
/// (дорожек + 1), у `<line-name-list>` подсетки — по записи на линию.
fn repeat_body_names(rest: &str, subgrid: bool) -> Vec<Vec<String>> {
    let mut lines: Vec<Vec<String>> = if subgrid { Vec::new() } else { vec![Vec::new()] };
    for t in line_name_tokens(rest) {
        match bracket_names(&t) {
            Some(names) if subgrid => lines.push(names),
            Some(names) => {
                if let Some(last) = lines.last_mut() {
                    last.extend(names);
                }
            }
            None if !subgrid => lines.push(Vec::new()),
            None => {}
        }
    }
    lines
}

/// Имена линий записи `grid-template-*` (css-grid-2 §7.2.2 `<line-names>`,
/// §subgrid-listing `<line-name-list>`): по списку имён на линию между
/// КОМПОНЕНТАМИ шаблона — `repeat(N, …)` раскрыт на месте, как в
/// `parse_tracks`, а `repeat(auto-fill|auto-fit, …)` остаётся одним
/// компонентом со своими именами (`repeat`). `None` — имён нет.
fn parse_line_names(v: &str) -> Option<gpui::GridAxisLineNames> {
    let tokens = line_name_tokens(v);
    let subgrid = tokens.first().is_some_and(|t| t.eq_ignore_ascii_case("subgrid"));
    let mut out = gpui::GridAxisLineNames::default();
    let mut lines: Vec<Vec<String>> = if subgrid { Vec::new() } else { vec![Vec::new()] };
    let mut any = false;
    let mut after = false;
    for t in tokens.iter().skip(usize::from(subgrid)) {
        if let Some(names) = bracket_names(t) {
            any |= !names.is_empty();
            if subgrid {
                lines.push(names);
            } else if let Some(last) = lines.last_mut() {
                last.extend(names);
            }
            continue;
        }
        if let Some(inner) = t.strip_prefix("repeat(").and_then(|r| r.strip_suffix(')')) {
            let (count, rest) = inner.split_once(',')?;
            let body = repeat_body_names(rest, subgrid);
            any |= body.iter().any(|b| !b.is_empty());
            let count = count.trim();
            if count.eq_ignore_ascii_case("auto-fill") || count.eq_ignore_ascii_case("auto-fit") {
                if after {
                    return None;
                }
                out.before = std::mem::take(&mut lines);
                out.repeat = Some(body);
                lines = if subgrid { Vec::new() } else { vec![Vec::new()] };
                after = true;
            } else {
                let n: usize = count.parse().ok()?;
                for _ in 0..n.min(64) {
                    if subgrid {
                        lines.extend(body.iter().cloned());
                    } else {
                        if let (Some(last), Some(first)) = (lines.last_mut(), body.first()) {
                            last.extend(first.iter().cloned());
                        }
                        lines.extend(body.iter().skip(1).cloned());
                    }
                }
            }
            continue;
        }
        if !subgrid {
            lines.push(Vec::new());
        }
    }
    if !any {
        return None;
    }
    if after {
        out.after = lines;
    } else {
        out.before = lines;
    }
    Some(out)
}

/// Грань размещения по имени линии (css-grid-2 §8.3): `a`, `a 2`, `-1 a`,
/// `span a`, `span 2 a`. Без имени — `None` (числовую грань разбирает
/// `parse_placement`).
fn parse_named_placement(v: &str) -> Option<gpui::GridNamedLine> {
    let (mut span, mut num, mut name) = (false, None::<i16>, None::<String>);
    for t in v.split_whitespace() {
        if t.eq_ignore_ascii_case("span") {
            span = true;
        } else if let Ok(n) = t.parse::<i16>() {
            num = Some(n);
        } else if t.eq_ignore_ascii_case("auto") {
            return None;
        } else {
            name = Some(t.to_string());
        }
    }
    let name = name?;
    Some(if span {
        gpui::GridNamedLine::Span(name, num.unwrap_or(1).max(1) as u16)
    } else {
        gpui::GridNamedLine::Line(name, num.unwrap_or(0))
    })
}

/// Обе грани `grid-column`/`grid-row`: при одном значении-имени конец — то
/// же имя («if the first value is a <custom-ident>, the grid-row-end/
/// grid-column-end longhand is also set to that <custom-ident>», §8.4).
fn parse_named_pair(v: &str) -> [Option<gpui::GridNamedLine>; 2] {
    match v.split_once('/') {
        Some((a, b)) => [parse_named_placement(a), parse_named_placement(b)],
        None => {
            let start = parse_named_placement(v);
            let end = match &start {
                Some(gpui::GridNamedLine::Line(name, 0)) => Some(gpui::GridNamedLine::Line(name.clone(), 0)),
                _ => None,
            };
            [start, end]
        }
    }
}

fn parse_placement(v: &str) -> Placement {
    let v = v.trim();
    if let Some(n) = v.strip_prefix("span") {
        return n
            .trim()
            .parse()
            .map(Placement::Span)
            .unwrap_or(Placement::Auto);
    }
    v.parse().map(Placement::Line).unwrap_or(Placement::Auto)
}

/// `grid-column: 1 / 3` либо `span 2`.
fn parse_span(v: &str) -> Option<(Placement, Placement)> {
    match v.split_once('/') {
        Some((a, b)) => Some((parse_placement(a), parse_placement(b))),
        None => Some((parse_placement(v), Placement::Auto)),
    }
}

/// Голова сокращения `font` и семейство: семейство — хвост после размера.
/// Убрать пробелы вокруг косой черты: `50px / 1` → `50px/1`.
fn join_slash(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for part in v.split('/') {
        if !out.is_empty() {
            out.push('/');
            out.push_str(part.trim_start());
        } else {
            out.push_str(part.trim_end());
        }
    }
    out
}

fn split_font(v: &str) -> (&str, &str) {
    let mut end = 0;
    for token in split_outside_parens(v) {
        let at = v[end..].find(token.as_str()).map(|i| end + i).unwrap_or(end);
        end = at + token.len();
        if font_size_token(&token) {
            return (&v[..end], v[end..].trim());
        }
    }
    (v, "")
}

/// Кусок головы сокращения `font`, который задаёт КЕГЛЬ (быть может, с
/// `/высотой строки`). Голое число — ВЕС, а не кегль (§15.6: 100…900):
/// `Len::parse("900")` даёт точки, и `font: 900 2em Ahem` отдавал кегль
/// весу, а `2em Ahem` — семейству, после чего гибло всё
/// (`font-family-011`). Ключевые кегли и математические функции — тоже
/// кегль (§15.8; `font: calc(10 * 10px) sans-serif`, `font-148`).
fn font_size_token(token: &str) -> bool {
    let size = font_slash(token).map_or(token, |(s, _)| s);
    let lower = size.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large" | "xx-large"
            | "xxx-large" | "larger" | "smaller"
    ) || ["calc(", "min(", "max(", "clamp("].iter().any(|f| lower.starts_with(f))
    {
        return true;
    }
    size.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && (token.contains('/')
            || size == "0"
            || (Len::parse(size).is_some() && !size.chars().all(|c| c.is_ascii_digit())))
}

/// Косая черта ВНЕ скобок: `20px/1.5` делится, `calc(100px/2)` — нет.
fn font_slash(t: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, ch) in t.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return Some((&t[..i], &t[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Разбить значение по пробелам ВНЕ скобок: `rgba(0, 128, 0, .5)` —
/// один токен, а `split_whitespace` рассыпал его, и цвет пропадал
/// (`outline` с функциональным цветом).
fn split_ws_top(v: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ => {}
        }
        if ch.is_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&v[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(&v[st..]);
    }
    out
}

/// Разрывы сегмента в строке-маркере: ряд принудительных разрывов — один
/// пробел (css-text-3 §4.1.2, «Segment Break Transformation Rules»).
/// Строка знака обрыва (`text-overflow: <string>`, `block-ellipsis`): ряд
/// ПРИНУДИТЕЛЬНЫХ разрывов — один пробел. Принудительный разрыв по
/// css-text-3 §forced-line-break — сохранённый перевод строки и любой знак
/// классов UAX#14 BK/NL: VT, FF, NEL, LS, PS (Blink `line_truncator.cc`
/// `IsForcedLineBreak`/`SuppressLineBreaks`; `text-overflow-string-018…022`).
fn collapse_forced_breaks(text: &str) -> String {
    let forced = |c: char| {
        matches!(
            c,
            '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    };
    if !text.contains(forced) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if forced(ch) {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

fn collapse_segment_breaks(text: &str) -> String {
    if !text.contains(['\n', '\r']) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if matches!(ch, '\n' | '\r') {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

fn unescape_content(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    // Экранированный перевод строки внутри строки CSS — ПРОДОЛЖЕНИЕ, а не
    // знак: CSS 2.1 §4.1.3 «the newline itself has to be escaped with a
    // backslash (\). The newline is subsequently removed from the string»
    // (css-syntax-3 §4.3.5: newline после `\` потребляется). `css::unescape`
    // общий с именами и оставлял `\n`; в `white-space: pre` он становился
    // жёстким разрывом, и кошка `content-173` рвалась лишними строками.
    // Остальные экранирования уходят в `unescape` как есть; `\` в конце
    // строки (EOF) пропадает.
    let mut joined = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            joined.push(ch);
            continue;
        }
        match it.next() {
            Some('\n') | Some('\u{c}') => {}
            Some('\r') => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
            }
            Some(c) => {
                joined.push('\\');
                joined.push(c);
            }
            None => {}
        }
    }
    crate::css::unescape(&joined)
}

/// `url(...)` из значения фона; кавычки внутри необязательны.
///
/// Имя записи регистронезависимо (§3.3), поэтому `URL(` и `Url(` — та же
/// запись; экранирование в нём разбор объявления уже снял. Конец ищется
/// с учётом кавычек и экранирования: закрывающая скобка внутри строки записи
/// не закрывает (§4.3.6). И искать `url(` внутри строки нельзя вовсе —
/// `content: "url(x)"` записью не является.
pub(crate) fn parse_url(v: &str) -> Option<String> {
    let mut at = 0usize;
    while at < v.len() {
        let ch = v[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += v[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += crate::css::skip_string(&v[at..], ch);
                continue;
            }
            _ if crate::css::at_url(&v[at..]) => {
                let end = at + crate::css::skip_url(&v[at..]);
                // Обрыв на конце файла закрывает запись сам (§4.2): скобки
                // может не быть, и тогда резать последний знак нельзя, а
                // кавычка остаётся только открывающая (`uri-017`).
                let inner_end = if v[..end].ends_with(')') {
                    end - 1
                } else {
                    end
                };
                let inner = v[at + 4..inner_end.max(at + 4)].trim();
                let inner = match inner.chars().next() {
                    Some(q @ ('"' | '\'')) if inner.len() > 1 => {
                        let body = &inner[1..];
                        body.strip_suffix(q).unwrap_or(body)
                    }
                    _ => inner,
                };
                return (!inner.is_empty()).then(|| inner.to_string());
            }
            _ => at += ch.len_utf8(),
        }
    }
    None
}

/// Помечено ли объявление как важное.
/// Снять с копий правил объявления, откатанные `revert-layer`
/// (css-cascade-5 §revert-layer): «as if no rules were specified in the
/// current cascade layer — or between its normal and important levels».
/// Обычный откат в слое L снимает обычные объявления свойства в L; важный —
/// важные в L и в слоях после него (у важных они слабее) и обычные в L и
/// после него. `all: revert-layer` откатывает каждое свойство, чьё
/// объявление в том же слое стоит до него. `None` — откатывать нечего.
fn revert_layers(matched: &[&crate::css::Rule]) -> Option<Vec<crate::css::Rule>> {
    use crate::css::DECL_SEP;
    let is_rl = |part: &str| strip_important(part).trim().eq_ignore_ascii_case("revert-layer");
    if !matched
        .iter()
        .any(|r| r.decls.values().any(|v| v.split(DECL_SEP).any(is_rl)))
    {
        return None;
    }
    let mut rules: Vec<crate::css::Rule> = matched.iter().map(|r| (*r).clone()).collect();
    // Порядки каскада: обычный — по возрастанию, важный — слой по убыванию.
    let normal_key = |r: &crate::css::Rule| (r.origin, r.layer.clone(), r.sel.specificity(), r.order);
    let mut keys: Vec<String> = rules
        .iter()
        .flat_map(|r| r.decls.keys().cloned())
        .filter(|k| !k.starts_with("--") && k != crate::css::ORDER_KEY)
        .collect();
    keys.sort();
    keys.dedup();
    // Снять части свойства `key` важности `imp` у правил, прошедших фильтр.
    let strip = |rules: &mut Vec<crate::css::Rule>, key: &str, imp: bool, keep: &dyn Fn(&crate::css::Rule) -> bool| {
        for r in rules.iter_mut().filter(|r| !keep(r)) {
            if let Some(v) = r.decls.get(key) {
                let rest: Vec<&str> = v.split(DECL_SEP).filter(|p| is_important(p) != imp).collect();
                if rest.is_empty() {
                    r.decls.remove(key);
                } else {
                    let joined = rest.join(&DECL_SEP.to_string());
                    r.decls.insert(key.to_string(), joined);
                }
            }
        }
    };
    // Победитель свойства: (индекс правила, слой, значение) по порядку каскада.
    let winner = |rules: &Vec<crate::css::Rule>, key: &str, imp: bool| -> Option<(Vec<u32>, String)> {
        let mut best: Option<(&crate::css::Rule, String)> = None;
        for r in rules {
            let Some(v) = r.decls.get(key) else { continue };
            let Some(part) = v.split(DECL_SEP).filter(|p| is_important(p) == imp).last() else {
                continue;
            };
            let better = match &best {
                None => true,
                Some((b, _)) if imp => {
                    (std::cmp::Reverse(r.origin), std::cmp::Reverse(&r.layer), r.sel.specificity(), r.order)
                        >= (std::cmp::Reverse(b.origin), std::cmp::Reverse(&b.layer), b.sel.specificity(), b.order)
                }
                Some((b, _)) => normal_key(r) >= normal_key(b),
            };
            if better {
                best = Some((r, part.to_string()));
            }
        }
        best.map(|(r, v)| (r.layer.clone(), v))
    };
    // `all: revert-layer` (обычный): каждое свойство слоя, объявленное в
    // правиле НЕ позже правила с `all`, снимается в этом слое.
    let alls: Vec<(Vec<u32>, (u8, Vec<u32>, (u32, u32, u32), usize))> = rules
        .iter()
        .filter(|r| {
            r.decls
                .get("all")
                .is_some_and(|v| v.split(DECL_SEP).filter(|p| !is_important(p)).last().is_some_and(is_rl))
        })
        .map(|r| (r.layer.clone(), normal_key(r)))
        .collect();
    for (layer, at) in alls {
        for r in rules.iter_mut().filter(|r| r.layer == layer && normal_key(r) <= at) {
            let props: Vec<String> = r
                .decls
                .keys()
                .filter(|k| !k.starts_with("--") && *k != crate::css::ORDER_KEY && *k != "direction" && *k != "unicode-bidi")
                .cloned()
                .collect();
            for k in props {
                if let Some(v) = r.decls.get(&k) {
                    let rest: Vec<&str> = v.split(DECL_SEP).filter(|p| is_important(p)).collect();
                    if rest.is_empty() {
                        r.decls.remove(&k);
                    } else {
                        let joined = rest.join(&DECL_SEP.to_string());
                        r.decls.insert(k, joined);
                    }
                }
            }
        }
    }
    for key in keys {
        if key == "direction" || key == "unicode-bidi" || key == "all" {
            continue;
        }
        for _ in 0..16 {
            if let Some((layer, v)) = winner(&rules, &key, true) {
                if is_rl(&v) {
                    let l = layer.clone();
                    strip(&mut rules, &key, true, &|r| r.layer < l);
                    let l = layer.clone();
                    strip(&mut rules, &key, false, &|r| r.layer < l);
                    continue;
                }
                break;
            }
            if let Some((layer, v)) = winner(&rules, &key, false) {
                if is_rl(&v) {
                    let l = layer.clone();
                    strip(&mut rules, &key, false, &|r| r.layer != l);
                    continue;
                }
            }
            break;
        }
    }
    Some(rules)
}

fn is_important(v: &str) -> bool {
    v.to_ascii_lowercase()
        .replace(' ', "")
        .ends_with("!important")
}

/// Значение без пометки важности; пробел перед `!` тоже допустим.
fn strip_important(v: &str) -> &str {
    match v.to_ascii_lowercase().rfind('!') {
        Some(at) if v[at..].to_ascii_lowercase().replace(' ', "") == "!important" => v[..at].trim(),
        _ => v,
    }
}

/// Разбить значение по пробелам, НЕ заходя внутрь скобок.
///
/// `rgba(0, 0, 0, .5)` — это один токен, а не четыре: обычное деление по
/// пробелам разрывало функции с пробелами после запятых, и значение молча
/// пропадало.

pub(crate) fn split_outside_parens(v: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0usize;
    let mut cur = String::new();
    for ch in v.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(ch);
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

/// Процентная смесь `calc(A% ± Bpx)` парой (доля, точки) — для свойств,
/// которые доли решают САМИ при отрисовке, зная размер коробки
/// (css-values-4 §10.9). Любая другая природа в сумме (`ch`, `vw`, `em`…) —
/// `None`: её здесь сложить не с чем, и запись, как прежде, не применяется.
fn pct_px_pair(t: &str) -> Option<(f32, f32)> {
    match Len::parse_mixed(t)? {
        Len::Calc(i) => crate::value::calc_get(i).pct_px(),
        _ => None,
    }
}

/// Кавычка вокруг имени шрифта: `font-family: "Segoe UI", sans-serif`.
fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

/// Семейство, которое подставляется за родовое имя.
///
/// Родовое имя — не шрифт, а разряд: в системе шрифтов его нет, и поиск по
/// нему кончается ничем (а в отладочной сборке — паникой). Подставляются те
/// же семейства, что берёт Chrome на Windows, иначе разметка набирается
/// умолчанием движка и шире эталона.
///
/// `monospace` в таблицу не входит: за него отвечает признак `monospace`, и
/// семейство под него выбирается позже — из тех, что на машине есть
/// (см. `metrics::mono_family`). Здесь он только не считается именем шрифта.
pub fn generic_family(lower: &str) -> Option<&'static str> {
    match lower {
        "system-ui" | "sans-serif" | "ui-sans-serif" | "ui-rounded" => Some(GENERIC_SANS),
        "serif" | "ui-serif" => Some("Times New Roman"),
        "cursive" => Some("Comic Sans MS"),
        "fantasy" => Some("Impact"),
        _ => None,
    }
}

/// Родовое имя семейства — разряд шрифта, а не шрифт.
pub(crate) fn is_generic(lower: &str) -> bool {
    generic_family(lower).is_some() || matches!(lower, "monospace" | "ui-monospace")
}

/// Семейство за родовое `sans-serif`; оно же — умолчание документа.
pub const GENERIC_SANS: &str = "Segoe UI";

fn parse_overflow(v: &str) -> Option<Overflow> {
    match v {
        "hidden" => Some(Overflow::Hidden),
        "clip" => Some(Overflow::Clip),
        // `overlay` — устаревший синоним `auto` (css-overflow-3 §overflow:
        // «legacy value alias of auto»; `overflow-overlay`).
        "scroll" | "auto" | "overlay" => Some(Overflow::Scroll),
        "visible" => Some(Overflow::Visible),
        _ => None,
    }
}

/// Опорная коробка `<geometry-box>` (css-masking-1 §1.3.1.1 плюс
/// `half-border-box` css-borders-4): 0 border, 1 margin, 2 padding,
/// 3 content, 4 half-border. У элемента с CSS-коробкой `fill-box` =
/// content-box, `stroke-box`/`view-box` = border-box.
fn geometry_box_kind(word: &str) -> Option<u8> {
    Some(match word.trim().to_ascii_lowercase().as_str() {
        "border-box" | "stroke-box" | "view-box" => 0,
        "margin-box" => 1,
        "padding-box" => 2,
        "content-box" | "fill-box" => 3,
        "half-border-box" => 4,
        _ => return None,
    })
}

/// `border-shape: [ <basic-shape> <geometry-box>? ]{1,2}`. Фигура — функция
/// со скобками, режется по ПАРНОЙ закрывающей (внутри `polygon(...)`
/// запятые, внутри `path('...')` — что угодно); слово коробки — следом за
/// ней. Хвост, не разобранный в две фигуры, делает значение недействительным.
fn parse_border_shape(v: &str) -> Option<BorderShape> {
    let mut items: Vec<(String, Option<u8>)> = Vec::new();
    let mut rest = v.trim();
    while !rest.is_empty() && items.len() < 2 {
        let open = rest.find('(')?;
        let mut depth = 0usize;
        let mut close = None;
        for (i, ch) in rest[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close?;
        let shape = rest[..=close].trim().to_string();
        // Прямоугольные фигуры пишутся через пробел (css-shapes-1 §basic-shape:
        // `rect( [ <length-percentage> | auto ]{4} … )`, так же `inset()` и
        // `xywh()`); запятая делает всё объявление недействительным, и
        // `border-shape` остаётся `none` — Blink `ConsumeBasicShapeRect`
        // (`css_parsing_utils.cc:651-668`) берёт четыре длины подряд без
        // запятой. Прежде `rect(0, 0, 100%, 100%)` разбирался в пустой
        // прямоугольник, и маска фигуры прятала коробку целиком
        // (border-shape-inset-shadow-blur, -negative-spread: пустая страница).
        let head = shape[..open].trim_start().to_ascii_lowercase();
        if matches!(head.as_str(), "rect" | "inset" | "xywh") && shape.contains(',') {
            return None;
        }
        rest = rest[close + 1..].trim_start();
        let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let bx = geometry_box_kind(&rest[..word_end]);
        if bx.is_some() {
            rest = rest[word_end..].trim_start();
        }
        items.push((shape, bx));
    }
    if !rest.is_empty() {
        return None;
    }
    let mut it = items.into_iter();
    let (outer, outer_box) = it.next()?;
    Some(match it.next() {
        Some((inner, inner_box)) => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(0),
            inner: Some((inner, inner_box.unwrap_or(2))),
        },
        None => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(4),
            inner: None,
        },
    })
}

/// Параметр суперэллипса из одного значения `<corner-shape-value>`
/// (css-borders-4 §corner-shaping): ключевые слова — их числовые
/// эквиваленты по спеке, `superellipse(<number> | infinity | -infinity)` —
/// само число. `None` — не форма угла (запись отбрасывается).
fn corner_shape_param(tok: &str) -> Option<f32> {
    let t = tok.trim().to_ascii_lowercase();
    Some(match t.as_str() {
        "round" => 1.0,
        "squircle" => 2.0,
        "square" => f32::INFINITY,
        "bevel" => 0.0,
        "scoop" => -1.0,
        "notch" => f32::NEG_INFINITY,
        _ => {
            let inner = t.strip_prefix("superellipse(")?.strip_suffix(')')?.trim();
            match inner {
                "infinity" => f32::INFINITY,
                "-infinity" => f32::NEG_INFINITY,
                n => n.parse::<f32>().ok()?,
            }
        }
    })
}

/// `corner-shape: a [b [c [d]]]` → K по углам tl/tr/br/bl — раскладка та же,
/// что у `border-radius` (§corner-shaping-shorthand). Функции со скобками
/// внутри пробелов не содержат, поэтому режем по пробелам.
fn corner_shape_shorthand(raw: &str) -> Option<[f32; 4]> {
    let v: Vec<f32> = raw
        .split_whitespace()
        .map(corner_shape_param)
        .collect::<Option<Vec<_>>>()?;
    Some(match v.len() {
        1 => [v[0]; 4],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        4 => [v[0], v[1], v[2], v[3]],
        _ => return None,
    })
}

/// Подстановка `var(--x)` и `var(--x, запасное)`.
/// Сколько раз раскрывать переменные внутри переменных.
///
/// Тема обычно ссылается на тему: `--btn: var(--accent)`. Один проход такую
/// цепочку не раскрывал, и объявление уходило в разбор строкой `var(--accent)`.
/// Потолок нужен от кольцевых ссылок.
const VAR_DEPTH: usize = 8;

/// Смещение скобки, парной той, что открыла запись.
fn balanced_close(after_open: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in after_open.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Ширина линейки промежутка: ключевые слова css-gaps-1 §width те же, что у
/// рамок; отрицательная недействительна.
fn gap_width(t: &str) -> Option<Len> {
    match t.trim() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        t => Len::parse(t).filter(|l| {
            matches!(l, Len::Px(w) if *w >= 0.0) || matches!(l, Len::Em(k) if *k >= 0.0)
        }),
    }
}

/// Стиль линейки: `none`/`hidden` — не рисовать, прочие — рисовать (все
/// стили пока красятся сплошной полосой).
fn gap_style(t: &str) -> Option<bool> {
    match t.trim() {
        "none" | "hidden" => Some(false),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        _ => None,
    }
}

/// Цвет линейки; `currentcolor` — `None` (цвет текста контейнера).
fn gap_color(t: &str) -> Option<Option<Color>> {
    let t = t.trim();
    if t.eq_ignore_ascii_case("currentcolor") {
        return Some(None);
    }
    Color::parse(t).map(Some)
}

/// Втяжка конца (css-gaps-1 §inset): длина/доля или `overlap-join`.
fn gap_inset(t: &str) -> Option<GapInset> {
    let t = t.trim();
    if t == "overlap-join" {
        return Some(GapInset::OverlapJoin);
    }
    if t == "0" {
        return Some(GapInset::Len(Len::Px(0.0)));
    }
    Len::parse(t)
        .filter(|l| matches!(l, Len::Px(_) | Len::Pct(_) | Len::Em(_)))
        .map(GapInset::Len)
}

/// `<gap-rule> = <line-width> || <line-style> || <color>`: любой порядок,
/// каждая часть не более одного раза; лишний токен — недействительно.
fn gap_rule(entry: &str) -> Option<(Option<Len>, Option<bool>, Option<Option<Color>>)> {
    let (mut w, mut s, mut c) = (None, None, None);
    for token in split_outside_parens(entry) {
        if s.is_none() && let Some(v) = gap_style(&token) {
            s = Some(v);
        } else if w.is_none() && let Some(v) = gap_width(&token) {
            w = Some(v);
        } else if c.is_none() && let Some(v) = gap_color(&token) {
            c = Some(v);
        } else {
            return None;
        }
    }
    Some((w, s, c))
}

/// Список css-gaps-1 §lists: значения через запятую вне скобок; `repeat(N, …)`
/// раскрывается на месте, `repeat(auto, …)` допустим один раз и делит список
/// на ведущие и хвостовые. Любой неразобранный элемент — весь список
/// недействителен.
fn gap_list<T: Copy>(v: &str, one: impl Fn(&str) -> Option<T>) -> Option<GapList<T>> {
    let mut out = GapList { lead: vec![], auto: vec![], tail: vec![] };
    let mut seen_auto = false;
    for entry in crate::css::split_args(v) {
        let entry = entry.trim();
        let Some(inner) = entry
            .strip_prefix("repeat(")
            .and_then(|r| r.strip_suffix(')'))
        else {
            let val = one(entry)?;
            if seen_auto {
                out.tail.push(val);
            } else {
                out.lead.push(val);
            }
            continue;
        };
        let args = crate::css::split_args(inner);
        let (count, vals) = args.split_first()?;
        let vals: Vec<T> = vals.iter().map(|s| one(s.trim())).collect::<Option<Vec<T>>>()?;
        if vals.is_empty() {
            return None;
        }
        if count.trim() == "auto" {
            if seen_auto {
                return None;
            }
            seen_auto = true;
            out.auto = vals;
        } else {
            let n: usize = count.trim().parse().ok().filter(|n| *n >= 1)?;
            let dst = if seen_auto { &mut out.tail } else { &mut out.lead };
            for _ in 0..n {
                dst.extend_from_slice(&vals);
            }
        }
    }
    (out.lead.len() + out.auto.len() + out.tail.len() > 0).then_some(out)
}

impl Computed {
    /// Ширины линеек одной оси: первое значение — в скаляр (многоколонник),
    /// список — только когда значений больше одного или есть авто-повтор.
    fn set_gap_widths(&mut self, column: bool, l: &GapList<Len>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_width, &mut self.column_rule_widths)
        } else {
            (&mut self.row_rule_width, &mut self.row_rule_widths)
        };
        *scalar = l.first().or(*scalar);
        *list = l.is_plural().then(|| l.clone());
    }

    fn set_gap_styles(&mut self, column: bool, l: &GapList<bool>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_visible, &mut self.column_rule_styles)
        } else {
            (&mut self.row_rule_visible, &mut self.row_rule_styles)
        };
        *scalar = l.first().or(*scalar);
        *list = l.is_plural().then(|| l.clone());
    }

    fn set_gap_colors(&mut self, column: bool, l: &GapList<Option<Color>>) {
        let (scalar, list) = if column {
            (&mut self.column_rule_color, &mut self.column_rule_colors)
        } else {
            (&mut self.row_rule_color, &mut self.row_rule_colors)
        };
        *scalar = l.first().flatten();
        *list = l.is_plural().then(|| l.clone());
    }

    /// `column-rule`/`row-rule`/`rule` (css-gaps-1 §rule-shorthands): каждая
    /// часть сокращения ставит СВОЙ список; неназванные части сбрасываются в
    /// начальные (`medium`, `none`, `currentcolor`), как у любого сокращения.
    pub(crate) fn gap_rule_shorthand(&mut self, key: &str, v: &str) {
        let Some(list) = gap_list(v, gap_rule) else { return };
        let widths = list.map(|r| r.0.unwrap_or(Len::Px(3.0)));
        let styles = list.map(|r| r.1.unwrap_or(false));
        let colors = list.map(|r| r.2.flatten());
        let double = !v.contains(',')
            && split_outside_parens(v).iter().any(|t| t.trim().eq_ignore_ascii_case("double"));
        for column in [true, false] {
            if (column && key == "row-rule") || (!column && key == "column-rule") {
                continue;
            }
            if column {
                self.column_rule_double = double;
            } else {
                self.row_rule_double = double;
            }
            self.set_gap_widths(column, &widths);
            self.set_gap_styles(column, &styles);
            self.set_gap_colors(column, &colors);
        }
    }
}

/// Смещение первой запятой ВНЕ вложенных скобок.
/// Раскрытие записи в четыре стороны: 1 значение — все, 2 — верт/гориз,
/// 3 — верх/гориз/низ, 4 — по часовой. `None`, если разобрать не удалось.
fn four<T: Copy>(words: &[&str], one: impl Fn(&str) -> Option<T>) -> Option<[T; 4]> {
    let v: Vec<T> = words.iter().filter_map(|w| one(w)).collect();
    match v.len() {
        1 => Some([v[0]; 4]),
        2 => Some([v[0], v[1], v[0], v[1]]),
        3 => Some([v[0], v[1], v[2], v[1]]),
        4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    }
}

/// Фоновые свойства со списком слоёв (css-backgrounds-3 §2.1).
const BG_LIST_KEYS: [&str; 8] = [
    "background",
    "background-image",
    "background-size",
    "background-position",
    "background-repeat",
    "background-origin",
    "background-clip",
    "background-attachment",
];

impl Computed {
    /// Слои фона СВЕРХУ ВНИЗ, когда их больше одного: каждый — копия стиля с
    /// одним слоем (картинка, размер, положение, повтор, область) и без
    /// цвета фона — цвет лежит под всеми слоями и красится коробкой
    /// (css-backgrounds-3 §3.1, §2.1: значения списков, которых меньше
    /// слоёв, повторяются по кругу). Градиент слоя уходит в растровую плитку
    /// (`bg_image` с сырой записью), чтобы все слои шли одним путём и в
    /// своём порядке. `None` — слой один.
    /// `background-clip` of the background COLOR: css-backgrounds-3 §3.2,
    /// «the background color is clipped according to the background-clip
    /// value associated with the bottom-most background image layer». The
    /// number of layers comes from `background-image` (§2.1); a shorter
    /// `background-clip` list repeats, a longer one is truncated
    /// (`background-color-clip`: two `none` layers, clip list
    /// `border-box, content-box, border-box` → `content-box`).
    pub(crate) fn color_clip(&self) -> Option<BgClip> {
        let Some((_, clips)) = self.bg_lists.iter().find(|(k, _)| k == "background-clip") else {
            return self.bg_clip;
        };
        let Some((_, images)) = self.bg_lists.iter().find(|(k, _)| k == "background-image") else {
            return self.bg_clip;
        };
        let n = background_layers(images).len();
        let clips = background_layers(clips);
        if n < 2 || clips.is_empty() {
            return self.bg_clip;
        }
        let mut probe = Computed::default();
        probe.apply_one("background-clip", clips[(n - 1) % clips.len()]);
        probe.bg_clip
    }

    pub(crate) fn bg_layers(&self) -> Option<Vec<Computed>> {
        let short = self.bg_lists.iter().find(|(k, _)| k == "background").map(|(_, v)| v.clone());
        let image = self.bg_lists.iter().find(|(k, _)| k == "background-image").map(|(_, v)| v.clone());
        let images: Vec<String> = match (&image, &short) {
            (Some(v), _) | (None, Some(v)) => background_layers(v).into_iter().map(str::to_string).collect(),
            _ => return None,
        };
        if images.len() < 2 {
            return None;
        }
        // Длины слоёв в единицах шрифта (`1ch 0 / 4ch 1ch`): слой разбирается
        // заново из сырой записи уже ПОСЛЕ `resolve_em`, и `ch` в положении и
        // размере оставался нерешённым — слой выходил нулевым
        // (`hanging-whitespace-001..004`). Решаем по своему кеглю.
        let font_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let (ch, ex) = crate::metrics::ch_ex_px(&family, font_px);
        let px_of = |v: &str| -> String {
            if has_font_units(v) {
                font_lengths_to_px(v, font_px, 16.0, ex, ch)
            } else {
                v.to_string()
            }
        };
        let mut out = vec![];
        for (i, _) in images.iter().enumerate() {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            if let Some(v) = &short {
                c.bg_image = None;
                c.gradient = None;
                c.gradient_raw = None;
                c.bg_size = BgSize::Auto;
                c.bg_pos = BgPos::default();
                c.bg_repeat = None;
                c.bg_origin = None;
                let layers = background_layers(v);
                c.apply_one("background", &px_of(layers[i % layers.len()]));
                c.background = None;
            }
            for (k, v) in self.bg_lists.iter().filter(|(k, _)| k != "background") {
                let layers = background_layers(v);
                c.apply_one(k, &px_of(layers[i % layers.len()]));
            }
            c.bg_lists.clear();
            // Градиент слоя — плиткой: источником идёт сама функция
            // градиента из записи слоя (в сокращении рядом с ней размер,
            // положение и повтор).
            if c.bg_image.is_none()
                && c.gradient.is_some()
                && let Some(r) = images.get(i)
                && let Some(at) = r.find("gradient(")
            {
                let start = r[..at].rfind(|ch: char| ch.is_whitespace() || ch == ',').map_or(0, |p| p + 1);
                let mut depth = 0i32;
                let mut end = r.len();
                for (j, ch) in r[at..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = at + j + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                c.bg_image = Some(r[start..end].to_string());
            }
            c.gradient = None;
            c.gradient_raw = None;
            out.push(c);
        }
        Some(out)
    }
}

/// Слои фона: значение режется по запятым ВНЕ скобок.
///
/// Запятая внутри `rgba(…)` или `linear-gradient(…)` слой не кончает, поэтому
/// делить строку простым `split(',')` нельзя.
fn background_layers(v: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = v;
    while let Some(at) = top_level_comma(rest) {
        out.push(rest[..at].trim());
        rest = &rest[at + 1..];
    }
    out.push(rest.trim());
    out
}

fn top_level_comma(inner: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

thread_local! {
    /// Атрибуты элемента, чей стиль сейчас собирается, — источник для
    /// типизированного `attr()` (css-values-5 §7.7). Ставит `dom::walk` на
    /// время `resolve_with_vars` и сразу снимает: у стилей вне этого окна
    /// (наведение, кадры анимации, псевдоэлементы) хозяина нет, и там берётся
    /// запасное значение.
    static CURRENT_ATTRS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// Номер элемента среди братьев и их число — для `sibling-index()` и
    /// `sibling-count()` (css-values-5 §tree-counting). Ставит `dom::walk`
    /// на время каскада элемента, как и атрибуты.
    static CURRENT_SIBLING: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

pub fn set_current_sibling(at: Option<(usize, usize)>) {
    CURRENT_SIBLING.with(|c| c.set(at));
}

/// `sibling-index()` / `sibling-count()` — целым числом (css-values-5
/// §tree-counting: «sibling-index() … returns an <integer> … the index of the
/// element among its inclusive siblings, starting at 1»). Без хозяина
/// (вне каскада элемента) запись остаётся как есть и роняет объявление.
fn resolve_sibling(value: String) -> String {
    if !value.contains("sibling-") {
        return value;
    }
    match CURRENT_SIBLING.with(|c| c.get()) {
        Some((i, n)) => value
            .replace("sibling-index()", &i.to_string())
            .replace("sibling-count()", &n.to_string()),
        None => value,
    }
}

pub fn set_current_attrs(attrs: &[(String, String)]) {
    CURRENT_ATTRS.with(|a| *a.borrow_mut() = attrs.to_vec());
}

pub fn clear_current_attrs() {
    CURRENT_ATTRS.with(|a| a.borrow_mut().clear());
}

/// Значение атрибута по имени из `attr()`.
///
/// Префикс пространства имён (`foo|bar`): у атрибутов HTML пространства нет,
/// а реестра `@namespace` здесь не видно — атрибут считается отсутствующим, и
/// берётся запасное значение (`attr-namespace-non-existing`). `|bar` — по
/// локальному имени; `*|bar` сюда не доходит (`resolve_attrs`). Сравнение
/// ASCII-регистронезависимое: HTML-парсер опускает в нижний регистр только
/// ASCII, и запрос опускается так же, а не-ASCII знаки сравниваются как есть
/// (`html-attr-case-insensitivity`).
fn attr_value(name: &str) -> Option<String> {
    let local = match name.split_once('|') {
        Some(("", local)) => local,
        Some(_) => return None,
        None => name,
    };
    CURRENT_ATTRS.with(|a| {
        a.borrow()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(local))
            .map(|(_, v)| v.clone())
    })
}

/// Значение атрибута под типом `attr()`; `None` — не разбирается этим типом.
fn attr_cast(value: &str, ty: &str) -> Option<String> {
    let v = value.trim();
    let ty: String = ty
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // `<length>`: голое число длиной не бывает, кроме нуля (css-values-4
    // §6.1); `Len::parse` принимает его точками — отсекаем здесь.
    let length = |v: &str| match Len::parse(v) {
        Some(Len::Px(n)) if v.parse::<f32>().is_ok() => (n == 0.0).then(|| v.to_string()),
        Some(
            Len::Pct(_)
            | Len::Auto
            | Len::MinContent
            | Len::MaxContent
            | Len::FitContent
            | Len::Anchor(_),
        )
        | None => None,
        Some(_) => Some(v.to_string()),
    };
    let percentage = |v: &str| {
        v.strip_suffix('%')
            .and_then(|n| n.trim().parse::<f32>().ok())
            .map(|_| v.to_string())
    };
    match ty.as_str() {
        // Любые токены; `url()` из атрибута запрещён (attr-tainted).
        "type(*)" => (!v.is_empty() && !v.to_ascii_lowercase().contains("url("))
            .then(|| v.to_string()),
        "type(<length>)" => length(v),
        "type(<percentage>)" => percentage(v),
        "type(<length-percentage>)" => length(v).or_else(|| percentage(v)),
        "type(<number>)" | "number" => v.parse::<f32>().ok().map(|_| v.to_string()),
        "type(<integer>)" => v.parse::<i64>().ok().map(|_| v.to_string()),
        "type(<color>)" => Color::parse(v).map(|_| v.to_string()),
        // `attr(x px)` — число из атрибута с единицей из записи.
        unit if !unit.is_empty()
            && (unit == "%" || unit.chars().all(|c| c.is_ascii_alphabetic())) =>
        {
            v.parse::<f32>().ok().map(|_| format!("{v}{unit}"))
        }
        _ => None,
    }
}

/// Типизированный `attr()` (css-values-5 §7.7): `attr(<имя> <тип>, <запас>?)`
/// подставляется ДО разбора значения, как `var()`. Нетипизированная запись
/// (`attr(x)` — строка) остаётся как есть: её разбирают `content` и счётчики.
/// Негодный атрибут без запаса делает объявление недействительным на
/// вычислении — значение становится `unset`.
fn resolve_attrs(key: &str, value: &str) -> String {
    if key == "content"
        || !value
            .as_bytes()
            .windows(5)
            .any(|w| w.eq_ignore_ascii_case(b"attr("))
    {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    loop {
        // ASCII-опускание не сдвигает байтов: индекс годен и для `rest`.
        let Some(at) = rest.to_ascii_lowercase().find("attr(") else {
            break;
        };
        let after = &rest[at + 5..];
        let Some(close) = balanced_close(after) else {
            break;
        };
        let inner = &after[..close];
        let (head, fallback) = match top_level_comma(inner) {
            Some(i) => (inner[..i].trim(), Some(inner[i + 1..].trim())),
            None => (inner.trim(), None),
        };
        let (name, ty) = match head.split_once(char::is_whitespace) {
            Some((n, t)) => (n.trim(), t.trim()),
            None => (head, ""),
        };
        // `<attr-name>` — как `<wq-name>`, «but without the possibility of a
        // wildcard prefix» (css-values-5 §attr-notation, Overview.bs:2057-2059):
        // `attr(*|bar …)` негоден при разборе, запас не спасает
        // (`attr-namespace-wildcard`). Выброс объявления здесь выражается
        // `unset`, как у негодного атрибута без запаса.
        if name.starts_with("*|") {
            return "unset".to_string();
        }
        out.push_str(&rest[..at]);
        if ty.is_empty() {
            out.push_str(&rest[at..at + 5 + close + 1]);
        } else {
            match (attr_value(name).and_then(|v| attr_cast(&v, ty)), fallback) {
                (Some(v), _) => out.push_str(&v),
                (None, Some(f)) => out.push_str(f),
                (None, None) => return "unset".to_string(),
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn resolve_vars(value: &str, vars: &Decls) -> String {
    let mut out = value.to_string();
    for _ in 0..VAR_DEPTH {
        let Some(next) = crate::css::variable_values::substitute(
            &out, &mut |name| vars.get(name).cloned(),
        ) else {
            return "unset".into();
        };
        if next == out {
            break;
        }
        out = next;
        if !crate::css::variable_values::has_var(&out) {
            break;
        }
    }
    if out.trim().is_empty() && crate::css::variable_values::has_var(value) {
        "unset".into()
    } else {
        out
    }
}

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
    fn resolve_font_one(&mut self, family: &str, size_px: f32) {
        if let Track::Font(l) = *self {
            *self = Track::Px(crate::metrics::spacing_px(Some(l), family, size_px));
        }
    }
}

impl TrackSize {
    /// То же для обеих граней записи.
    fn resolve_font(&mut self, family: &str, size_px: f32) {
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
fn split_slash(v: &str) -> (&str, &str) {
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
fn strip_auto_flow(v: &str) -> &str {
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

/// Тело авто-повтора СПИСКОМ дорожек: `repeat(auto-fill, max-content
/// min-content)` → `[Single(MaxContent), Single(MinContent)]`.
///
/// Нужно, чтобы раскладка считала гипотетический размер КАЖДОЙ записи тела по
/// её собственной функции (css-grid-3 §7.2.1). `parse_tracks` уже знает и
/// `minmax()`, и `fit-content(N)` (последний как `MinMax(Auto, hi)`), поэтому
/// своего разбора здесь нет.
fn auto_fill_body_tracks(v: &str) -> Option<Vec<TrackSize>> {
    parse_tracks(v).as_deref().and_then(|l| {
        l.iter().find_map(|t| match t {
            TrackSize::AutoRepeat { tracks, .. } => Some(tracks.clone()),
            _ => None,
        })
    })
}

/// Длина тела авто-повтора в дорожках (1, если тело не разобралось).
fn auto_fill_body(v: &str) -> usize {
    parse_tracks(v)
        .as_deref()
        .and_then(|l| {
            l.iter().find_map(|t| match t {
                TrackSize::AutoRepeat { tracks, .. } => Some(tracks.len()),
                _ => None,
            })
        })
        .unwrap_or(1)
        .max(1)
}

/// Максимум `minmax(lo, hi)` в авто-повторе: `auto` или доля `fr`.
fn auto_fill_max(v: &str) -> (bool, Option<f32>) {
    let Some(rest) = v.split("minmax(").nth(1) else {
        return (false, None);
    };
    let Some(inner) = rest.find(')').map(|i| &rest[..i]) else {
        return (false, None);
    };
    let hi = inner.splitn(2, ',').nth(1).unwrap_or("").trim();
    if hi == "auto" {
        return (true, None);
    }
    let fr = hi
        .strip_suffix("fr")
        .and_then(|k| k.trim().parse::<f32>().ok());
    (false, fr)
}

/// Размер повторяемой дорожки в `repeat(auto-fill | auto-fit, …)`.
///
/// Берётся либо нижняя граница `minmax(N, …)`, либо сама дорожка, если она
/// задана точкой: `repeat(auto-fill, 100px)` — три колонки в трёхстах точках.
/// Все дорожки тела повтора `repeat(auto-fill | auto-fit, …)` в точках.
///
/// Тело бывает из нескольких дорожек — `repeat(auto-fill, 50px 50px)`
/// повторяет ПАРУ. Пока бралась одна, `Len::parse("50px 50px")` не разбирался
/// вовсе, и сетка не получала дорожек: `grid-auto-repeat-multiple-values-*`
/// рисовались одной плитой во всю ширину. Имена линий (`[all x v]`) к размеру
/// не относятся и выбрасываются.
///
/// Пусто, если тело из одной дорожки или хоть один кусок не разобрался: такой
/// случай ведёт прежняя ветка по `auto_fill_min`.
fn auto_fill_tracks(v: &str) -> Vec<f32> {
    if v.contains("minmax(") || v.contains("fit-content(") {
        return Vec::new();
    }
    let Some(rest) = v.split("repeat(").nth(1) else {
        return Vec::new();
    };
    let Some(end) = rest.rfind(')') else {
        return Vec::new();
    };
    let Some(body) = rest[..end].split_once(',').map(|(_, b)| b) else {
        return Vec::new();
    };
    // Имена линий в квадратных скобках размера не несут.
    let mut clean = String::with_capacity(body.len());
    let mut depth = 0usize;
    for ch in body.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => clean.push(ch),
            _ => {}
        }
    }
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(parts.len());
    for t in parts {
        match Len::parse(t) {
            Some(Len::Px(px)) => out.push(px),
            _ => return Vec::new(),
        }
    }
    out
}

fn auto_fill_min(v: &str) -> Option<f32> {
    let px_of = |t: &str| match Len::parse(t.trim()) {
        Some(Len::Px(px)) => Some(px),
        _ => None,
    };
    if let Some(rest) = v.split("minmax(").nth(1)
        && let Some(inner) = rest.find(')').map(|i| &rest[..i])
    {
        let mut args = inner.splitn(2, ',');
        let lo = args.next().unwrap_or("");
        let hi = args.next().unwrap_or("");
        // Число повторов считается по МАКСИМАЛЬНОЙ функции дорожки, если
        // она определённая, иначе по минимальной (css-grid-1 §7.2.3.2):
        // `minmax(min-content, 100px)` повторяется сотнями точек, а
        // `minmax(100px, 1fr)` — сотней из минимума.
        // Максимум ниже минимума поднимается до него (css-grid-2 §7.2.3.1:
        // «the max will be floored by the min» — `grid-auto-repeat-minmax`).
        return match (px_of(hi), px_of(lo)) {
            (Some(h), Some(l)) => Some(h.max(l)),
            (h, l) => h.or(l),
        };
    }
    let rest = v.split("repeat(").nth(1)?;
    let inner = &rest[..rest.rfind(')')?];
    px_of(inner.split(',').nth(1)?)
}

/// Потолок `fit-content(N)` в авто-повторе: дорожка не шире N.
pub(crate) fn auto_fill_fit_px(v: &str) -> Option<f32> {
    let rest = v.split("fit-content(").nth(1)?;
    let inner = &rest[..rest.find(')')?];
    match Len::parse(inner.trim()) {
        Some(Len::Px(px)) => Some(px),
        _ => None,
    }
}

/// Голая интрин-дорожка СНАРУЖИ `repeat(auto-fill | auto-fit, …)`.
///
/// css-grid-1 `<auto-track-list>` разрешает вокруг авто-повтора только
/// `<fixed-size>`; css-grid-3 §7.2.1 ослабила запись лишь ВНУТРИ `repeat()`.
/// `minmax()` снаружи законен в обе стороны (`minmax(<fixed-breadth>,
/// <track-breadth>)` и `minmax(<inflexible-breadth>, <fixed-breadth>)`) и сюда
/// НЕ попадает: иначе под нож ушли бы валидные
/// `css-grid/grid-definition/grid-auto-fill-columns-001` и родня, а также
/// `grid-lanes/invalidation/grid-lanes-change-intrinsic-size-with-auto-repeat-tracks-001`
/// (`repeat(auto-fill, 20px) minmax(min-content, 40px)`).
/// Имена линий в скобках размера не несут и негодности не создают.
fn auto_repeat_outside_intrinsic(v: &str) -> bool {
    let toks = tokenize_tracks(v);
    if !toks
        .iter()
        .any(|t| t.starts_with("repeat(") && (t.contains("auto-fill") || t.contains("auto-fit")))
    {
        return false;
    }
    toks.iter().any(|t| {
        let t = t.trim();
        if t.starts_with("repeat(") || (t.starts_with('[') && t.ends_with(']')) {
            return false;
        }
        t == "auto" || t == "min-content" || t == "max-content" || t.starts_with("fit-content(")
    })
}

/// Дорожка повтора задана ПО СОДЕРЖИМОМУ: `repeat(auto-fill, max-content)`
/// и родня. Точечного размера у неё нет — повторы считают сами элементы.
fn auto_fill_intrinsic(v: &str) -> bool {
    let Some(rest) = v.split("repeat(").nth(1) else {
        return false;
    };
    let inner = match rest.rfind(')') {
        Some(i) => &rest[..i],
        None => rest,
    };
    // `auto`-дорожку меряет прежний счёт по самому большому элементу
    // (column-auto-repeat-auto-001) — сюда только content-ключи.
    ["max-content", "min-content", "fit-content"]
        .iter()
        .any(|k| inner.contains(k))
        && auto_fill_min(v).is_none()
        && auto_fill_pct(v).is_none()
}

/// Доля контейнера в `repeat(auto-fill | auto-fit, N%)`.
///
/// Считается там же, где и точечный размер, но остаётся долей: в точки её
/// переводит раскладка, когда ширина контейнера уже решена.
fn auto_fill_pct(v: &str) -> Option<f32> {
    let pct_of = |t: &str| match Len::parse(t.trim()) {
        Some(Len::Pct(k)) => Some(k),
        _ => None,
    };
    if let Some(rest) = v.split("minmax(").nth(1)
        && let Some(inner) = rest.find(')').map(|i| &rest[..i])
    {
        let mut args = inner.splitn(2, ',');
        let lo = args.next().unwrap_or("");
        let hi = args.next().unwrap_or("");
        // Как и в точках: максимум сильнее минимума (css-grid-1 §7.2.3.2).
        return pct_of(hi).or_else(|| pct_of(lo));
    }
    let rest = v.split("repeat(").nth(1)?;
    let inner = &rest[..rest.rfind(')')?];
    pct_of(inner.split(',').nth(1)?)
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v197, `scout-gridoof-2026-09.md` жила И1,
/// 8 хунков): `fit-content(N)` теряет функцию дорожки в переводе CSS→gpui и
/// доезжает как `minmax(auto, N)` — ЗАКРЕПЛЕНИЕ вместо ПОТОЛКА; патч заводил
/// грани `Track::FitPx/FitPct`, вариант `GridTrack::FitContent` в
/// `vendor/gpui/src/geometry.rs` и ветки в `vendor/gpui/src/taffy.rs`.
/// Обещание +0…+2. Срез 1481 пара: **+27/−337**. Падение не логическое —
/// целые семейства ушли в «красное видно»: `block-aspect-ratio-*`,
/// `flex-aspect-ratio-*`, `grid-aspect-ratio-*`, `intrinsic-size-*`,
/// `multicol-rule-*`, таблицы, `hanging-punctuation-*`, `boundary-shaping-*`.
/// Новый вариант перечисления в вендорном `GridTrack` меняет раскладку далеко
/// за пределами сетки: под него идут ВСЕ дорожечные размеры gpui. Возвращать
/// только вместе с полным перебором потребителей `GridTrack` и своим сводом.
fn parse_tracks(v: &str) -> Option<Vec<TrackSize>> {
    fn single(t: &str) -> Option<Track> {
        let t = t.trim();
        // Именованная линия перед дорожкой: `[side] 240px`. Имя не несёт
        // размера, поэтому просто отбрасывается — но НЕ вместе с дорожкой.
        let t = t.trim_start_matches(|c| c == '[');
        let t = match t.find(']') {
            Some(at) => t[at + 1..].trim(),
            None => t,
        };
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
        match Len::parse(t) {
            Some(Len::Px(px)) => Some(Track::Px(px)),
            Some(Len::Pct(p)) => Some(Track::Pct(p)),
            // Единицы шрифта откладываются: раньше они роняли разбор, а с
            // ним и ВЕСЬ список дорожек — сетка выходила из равных долей.
            Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_) | Len::Ic(_))) => Some(Track::Font(l)),
            _ => None,
        }
    }

    /// Одна запись списка: `minmax(a, b)` либо одиночная дорожка.
    fn one(t: &str) -> Option<TrackSize> {
        let t = t.trim();
        if let Some(inner) = t.strip_prefix("minmax(").and_then(|s| s.strip_suffix(')')) {
            let (lo, hi) = inner.split_once(',')?;
            return Some(TrackSize::MinMax(single(lo)?, single(hi)?));
        }
        // `fit-content(N)` — дорожка по содержимому, но НЕ ШИРЕ N: раньше
        // сводилась к `Px(N)`, и потолок работал полом
        // (column-intrinsic-maximums).
        // Аргумент — `<length-percentage>` (css-grid-1 §7.2.3), а не только
        // точки: `fit-content(30%)` не разбирался и ронял `parse_tracks` на
        // `None` для ВСЕГО списка (`out.push(one(&token)?)` ниже), после чего
        // сетка сводилась к равным колонкам по `count_tracks`. Верхняя грань
        // теперь идёт через `single`, который знает px, проценты и единицы
        // шрифта; нераспознанный аргумент — это `auto`, а не потеря шаблона.
        if let Some(inner) = t
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let hi = match single(inner).unwrap_or(Track::Auto) {
                Track::Px(v) => Track::FitPx(v),
                Track::Pct(v) => Track::FitPct(v),
                other => other,
            };
            return Some(TrackSize::MinMax(Track::Auto, hi));
        }
        single(t).map(TrackSize::Single)
    }

    // Список дорожек с раскрытием `repeat(N, …)` НА МЕСТЕ. Раньше `repeat`
    // понимался только когда занимал весь список: запись `200px repeat(2, 1fr)`
    // теряла повтор целиком, и вместо трёх колонок оставалась одна — молча,
    // потому что нераспознанное просто отсеивалось.
    let mut out: Vec<TrackSize> = vec![];
    for token in tokenize_tracks(v) {
        if let Some(inner) = token
            .strip_prefix("repeat(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let (count, rest) = inner.split_once(',')?;
            let count = count.trim();
            let unit: Vec<TrackSize> = tokenize_tracks(rest)
                .iter()
                .filter_map(|t| one(t))
                .collect();
            if unit.is_empty() {
                return None;
            }
            // Число повторов бывает не числом: `auto-fill` и `auto-fit`
            // считает раскладка — ей известна ширина контейнера.
            if count.eq_ignore_ascii_case("auto-fill") || count.eq_ignore_ascii_case("auto-fit") {
                out.push(TrackSize::AutoRepeat {
                    fit: count.eq_ignore_ascii_case("auto-fit"),
                    tracks: unit,
                });
                continue;
            }
            let count: usize = count.parse().ok()?;
            for _ in 0..count.min(64) {
                out.extend(unit.iter().cloned());
            }
            continue;
        }
        // Отдельный токен имени линии — не дорожка: `[a] 50px [b] 40px [c]`
        // задаёт ДВЕ дорожки и три имени. Раньше такой токен ронял весь список
        // (`one` возвращал `None`), и сетка схлопывалась в одну колонку —
        // молча, включая ЭТАЛОНЫ (`subgrid/line-names-002-ref` рисовался
        // пустым).
        let bare = token.trim();
        if bare.starts_with('[') && bare.ends_with(']') {
            continue;
        }
        // Нераспознанная дорожка — повод отказаться от всего списка: тихо
        // укоротить его значит переставить всех детей.
        out.push(one(&token)?);
    }
    (!out.is_empty()).then_some(out)
}

/// CSS Grid 2 §7.2.2: a bracketed line-name group is one component, even
/// with multiple names or without whitespace before the adjacent track.
/// Use the same boundaries for sizes and names so their positions agree.
fn tokenize_tracks(v: &str) -> Vec<String> {
    line_name_tokens(v)
}

/// Число колонок в `grid-template-columns`: и `repeat(3, 1fr)`, и `1fr 1fr`.
fn count_tracks(v: &str) -> Option<u16> {
    if let Some(inner) = v.strip_prefix("repeat(").and_then(|s| s.strip_suffix(')')) {
        return inner.split(',').next()?.trim().parse().ok();
    }
    let n = v.split_whitespace().count();
    (n > 0).then_some(n as u16)
}

/// Выбор кандидата `image-set()` (css-images-4 §2.5).
///
/// * `None` — запись НЕГОДНА (отрицательное разрешение числом, вложенный
///   `image-set()`, чужое слово): объявление отбрасывается, прежнее живёт;
/// * `Some(None)` — запись годна, но пригодных кандидатов нет: «invalid image»;
/// * `Some(Some(src))` — СЫРАЯ запись выбранного `<image>` (`url(...)`,
///   градиент); строка-адрес оборачивается в `url(...)` (§2.5: «Each
///   `<string>` inside image-set() represents a `<url>`»).
fn image_set_pick(inner: &str) -> Option<Option<String>> {
    let mut options: Vec<(String, f32)> = vec![];
    for cand in crate::css::split_args(inner) {
        let mut image: Option<String> = None;
        let mut res: Option<f32> = None;
        let mut type_ok = true;
        for token in split_outside_parens(cand.trim()) {
            let low = token.to_ascii_lowercase();
            if let Some(body) = low.strip_prefix("type(") {
                // Неподдержанный тип снимает КАНДИДАТА, а не запись (§2.5).
                // Список — форматы, которые читает `background::decode`.
                let mime = body.trim_end_matches(')').trim().trim_matches(is_quote);
                type_ok &= matches!(
                    mime,
                    "image/png"
                        | "image/jpeg"
                        | "image/gif"
                        | "image/webp"
                        | "image/bmp"
                        | "image/svg+xml"
                );
                continue;
            }
            if let Some(r) = image_resolution(&low) {
                // Отрицательное ЧИСЛО вне диапазона по определению — ошибка
                // разбора. Из `calc()` оно приходит вычисленным: запись годна,
                // непригоден только кандидат (`negative-resolution-3`).
                if r < 0.0 && !low.starts_with("calc(") {
                    return None;
                }
                res = Some(r);
                continue;
            }
            if image.is_none() {
                if low.starts_with("image-set(") || low.starts_with("-webkit-image-set(") {
                    return None;
                }
                if token.len() >= 2 && token.starts_with(is_quote) {
                    image = Some(format!("url({token})"));
                    continue;
                }
                if low.contains('(') {
                    image = Some(token.clone());
                    continue;
                }
            }
            return None;
        }
        let Some(image) = image else { continue };
        let res = res.unwrap_or(1.0);
        // Шаг 1 (тип), нулевая/отрицательная плотность (картинке не из чего
        // взять природный размер) и шаг 2 (дубль разрешения среди оставшихся).
        if !type_ok || res <= 0.0 || options.iter().any(|(_, r)| *r == res) {
            continue;
        }
        options.push((image, res));
    }
    // Шаг 4 отдан UA: наименьшее разрешение, которого хватает на плотность
    // 1x, иначе наибольшее (Blink `CSSImageSetValue::GetBestOption`).
    let best = options
        .iter()
        .filter(|(_, r)| *r >= 1.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .or_else(|| options.iter().max_by(|a, b| a.1.total_cmp(&b.1)));
    Some(best.map(|(src, _)| src.clone()))
}

/// `<resolution>` в `dppx` (css-values-4 §6.3), в том числе внутри `calc()`.
/// Единицы разрешения переписываются точками (`1x` → `1px`, `96dpi` → `1px`),
/// арифметику считает готовый разборщик длин. `None` — в записи нет ни одного
/// разрешения либо она не сводится к числу.
fn image_resolution(token: &str) -> Option<f32> {
    if !token.is_ascii() {
        return None;
    }
    let b = token.as_bytes();
    let mut expr = String::with_capacity(token.len() + 8);
    let mut found = false;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        // Имя (`calc`, `url`, слово пути) переписывается целиком: цифра внутри
        // имени числом не начинается.
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_') {
                i += 1;
            }
            expr.push_str(&token[s..i]);
            continue;
        }
        let number = c.is_ascii_digit()
            || (c == b'.' && b.get(i + 1).is_some_and(|n| n.is_ascii_digit()));
        if !number {
            expr.push(c as char);
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        let n: f32 = token[s..i].parse().ok()?;
        let u = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let k = match &token[u..i] {
            "" => {
                expr.push_str(&token[s..i]);
                continue;
            }
            "x" | "dppx" => 1.0,
            "dpi" => 1.0 / 96.0,
            "dpcm" => 2.54 / 96.0,
            _ => return None,
        };
        found = true;
        expr.push_str(&format!("{}px", n * k));
    }
    if !found {
        return None;
    }
    match Len::parse(&expr)? {
        Len::Px(v) => Some(v),
        _ => None,
    }
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v168, `scout-bgimg-2026-09.md` §4
/// IMG-IMAGE-SET, 2 хунка): выбор кандидата `image-set()` по спеке — отсев по
/// MIME, дублям разрешения и «кандидатов не осталось ⇒ негодная картинка».
/// Обещание +13. Полный свод против v37: **+9/−17**. Плюсы — семья, где
/// кандидат ДОЛЖЕН быть отвергнут (`image-set-type-unsupported-*`,
/// `-zero-resolution-*`, `-negative-resolution-*`, градиенты). Минусы — 17
/// пар с единственным годным кандидатом (`image-set-rendering`, `-dpi-*`,
/// `-dppx-*`, `-calc-x-*`, `-no-res-*`, `-type-*`), все ровно 2.08: картинка
/// встала не на место. Отбор верен, теряется адрес выбранного кандидата —
/// возвращать вместе с разбором `<string>` как адреса (стенд `wptrun.rs:704`
/// перебазирует только `url(...)`).
/// Записи `<gradient>`, которых GPU-путь не выражает: коническая (обход по
/// углу) и все повторяющиеся (узор стопов мостится вдоль линии —
/// css-images-3 §3.6). Такие рисуются растровой плиткой — тем же путём,
/// которым уже ходит `conic-gradient()`.
pub(crate) fn gradient_as_raster(v: &str) -> bool {
    // `cross-fade()` (css-images-4 §2.6) — смесь картинок растром той же
    // плиткой (`background::rasterize_cross_fade`).
    v.starts_with("cross-fade(")
        || v.starts_with("conic-gradient(")
        || v.starts_with("repeating-linear-gradient(")
        || v.starts_with("repeating-radial-gradient(")
        || v.starts_with("repeating-conic-gradient(")
}

/// Пересчитать фильтром цвета стопов в ЗАПИСИ растрового градиента: такие
/// градиенты живут строкой в `bg_image`, а не полем `gradient`. Слова, не
/// являющиеся цветом (направление, позиции, `from`/`at`), остаются как есть.
pub(crate) fn filter_gradient_text(raw: &str, f: &Filter) -> String {
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return raw.to_string();
    };
    if close <= open {
        return raw.to_string();
    }
    let parts: Vec<String> = crate::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            split_outside_parens(part)
                .into_iter()
                .map(|w| match Color::parse(&w) {
                    Some(c) => {
                        let c = f.apply(c);
                        format!(
                            "rgba({},{},{},{})",
                            (c.r * 255.0).round(),
                            (c.g * 255.0).round(),
                            (c.b * 255.0).round(),
                            c.a
                        )
                    }
                    None => w,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    format!("{}{})", &raw[..=open], parts.join(", "))
}


/// `linear-gradient(90deg, #000, #fff)`. Направления словами приводим к углу.
/// `linear-gradient(...)` и `radial-gradient(...)`.
///
/// Позиции стопов сохраняются: без них полосы не расставить, а именно они
/// задают, где цвет меняется.
/// Угол направления градиента по единице (css-values-4 §7.1): `Some(Some(deg))`
/// — законный угол, `Some(None)` — число с НЕЗНАКОМОЙ единицей (`90degree`,
/// `0.25turns`): вся запись негодна; `None` — не размерность вовсе (цвет,
/// `to right`), решают прочие ветки.
fn gradient_angle(a: &str) -> Option<Option<f32>> {
    let a = a.trim();
    let cut = a.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = a.split_at(cut);
    let n: f32 = num.parse().ok()?;
    Some(match unit.to_ascii_lowercase().as_str() {
        "deg" => Some(n),
        "grad" => Some(n * 0.9),
        "rad" => Some(n.to_degrees()),
        "turn" => Some(n * 360.0),
        _ => None,
    })
}

pub(crate) fn parse_gradient(v: &str) -> Option<Gradient> {
    // Повторяющаяся запись отличается от обычной ТОЛЬКО тем, что узор стопов
    // мостится вдоль линии (css-images-3 §3.6): разбор у них общий, а
    // повторение делает растеризатор, заворачивая долю точки.
    let v = v.strip_prefix("repeating-").unwrap_or(v);
    // Повторяющаяся запись отличается от обычной ТОЛЬКО тем, что узор стопов
    // мостится вдоль линии (css-images-3 §3.6): разбор у них общий, а
    // повторение делает растеризатор, заворачивая долю точки.
    let v = v.strip_prefix("repeating-").unwrap_or(v);
    let radial = v.starts_with("radial-gradient(");
    let inner = v
        .strip_prefix(if radial {
            "radial-gradient("
        } else {
            "linear-gradient("
        })?
        .strip_suffix(')')?;
    let parts: Vec<&str> = crate::css::split_args(inner);
    if parts.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    let circle = radial && parts[0].contains("circle");
    // Способ ИНТЕРПОЛЯЦИИ (css-images-4 §3.4.1.1): `to right in hsl longer
    // hue` — суффикс отделяется от направления, иначе матч направления
    // промахивался и первый аргумент уходил в стопы.
    let (head, interp) = match parts[0].find(" in ") {
        Some(at) => (parts[0][..at].trim(), Some(parts[0][at + 4..].trim())),
        None if parts[0].trim_start().starts_with("in ") => {
            ("", Some(parts[0].trim_start()[3..].trim()))
        }
        None => (parts[0].trim(), None),
    };
    // Дуга тона нужна ЛЮБОМУ полярному пространству, не только `hsl`
    // (css-color-4 §12.4); умолчание — shorter.
    let hue_arc: u8 = interp.map_or(0, |i| {
        if i.contains("longer") {
            1
        } else if i.contains("increasing") {
            2
        } else if i.contains("decreasing") {
            3
        } else {
            0
        }
    });
    // Пространство смешения: явное из записи, иначе решается ниже по составу
    // стопов (css-color-4 §12.2).
    let named_space: Option<GradSpace> =
        interp
            .and_then(|i| i.split_whitespace().next())
            .and_then(|s| match s {
                "srgb" => Some(GradSpace::Srgb),
                "srgb-linear" | "xyz" | "xyz-d50" | "xyz-d65" | "display-p3-linear"
                | "rec2020-linear" | "a98-rgb-linear" | "prophoto-rgb-linear" => {
                    Some(GradSpace::Linear)
                }
                "oklab" => Some(GradSpace::Oklab),
                "oklch" => Some(GradSpace::Oklch),
                "lab" => Some(GradSpace::Lab),
                "lch" => Some(GradSpace::Lch),
                "hsl" => Some(GradSpace::Hsl),
                "hwb" => Some(GradSpace::Hwb),
                // Пространства с собственным охватом (`display-p3`, `a98-rgb`,
                // `rec2020`, `prophoto-rgb`) гамма-кодированы, и для цветов
                // ВНУТРИ охвата sRGB смешение в них от sRGB не отличается:
                // кривая одна и та же, а матрица первичных с интерполяцией
                // коммутирует. Заводить их отдельно нечем.
                _ => None,
            });
    if interp.is_some() && head.is_empty() {
        idx = 1;
    }
    let angle = match head {
        a if a.ends_with("deg") => {
            idx = 1;
            a.trim_end_matches("deg").trim().parse().unwrap_or(180.0)
        }
        // Размерность с другой единицей: `grad`/`rad`/`turn` — законный угол,
        // прочее (`90degree`, `100gradian`, `1.57radian`, `0.25turns`) делает
        // запись негодной целиком (`angle-units-001`). Прежде такой довод
        // падал в `_ => 180.0`, не читался цветом и молча пропускался —
        // градиент из оставшихся стопов КРАСИЛ.
        a if !radial && gradient_angle(a).is_some() => {
            idx = 1;
            gradient_angle(a).flatten()?
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
        // Остальные два угла (css-images-3 §3.1): без них направление
        // уходило в разбор стопов и выбрасывалось, а градиент шёл сверху вниз.
        "to bottom left" | "to left bottom" => {
            idx = 1;
            225.0
        }
        "to top left" | "to left top" => {
            idx = 1;
            315.0
        }
        "to top right" | "to right top" => {
            idx = 1;
            45.0
        }
        // У радиального первым идёт описание формы (`circle at center`) —
        // цветом оно не разбирается, поэтому просто пропускается.
        first
            if radial && Color::parse(first.split_whitespace().next().unwrap_or("")).is_none() =>
        {
            idx = 1;
            180.0
        }
        _ => 180.0,
    };

    // Стоп несёт до ДВУХ позиций (css-images-4 §3.4.1): `yellow 0% 25%` — это
    // два стопа одного цвета, так записывают жёсткие полосы. Цвет с запятыми
    // внутри (`rgba(…)`) остаётся одним словом только при резке вне скобок.
    let mut raw: Vec<(Color, Option<f32>)> = vec![];
    let mut raw_px: Vec<(Color, Option<f32>)> = vec![];
    let mut any_pct = false;
    // Умолчание пространства держится на ЗАПИСИ цветов, а не на их значениях
    // (css-color-4 §12.2), поэтому решается прямо здесь, пока текст стопа
    // ещё под рукой.
    let mut all_legacy = true;
    for p in &parts[idx..] {
        let words = split_outside_parens(p);
        let Some(colour) = words.first().and_then(|w| crate::color_space::interpolation_color(w)) else {
            continue;
        };
        all_legacy &= legacy_srgb_color(words[0].as_str());
        if words.len() == 1 {
            raw.push((colour, None));
            raw_px.push((colour, None));
            continue;
        }
        for t in &words[1..] {
            if let Some(n) = t
                .strip_suffix('%')
                .and_then(|n| n.trim().parse::<f32>().ok())
            {
                any_pct = true;
                raw.push((colour, Some(n / 100.0)));
                raw_px.push((colour, None));
            } else if let Some(Len::Px(v)) = Len::parse(t) {
                // Позиция в точках: долей не выразить, длина оси известна
                // только при отрисовке. Хранится своим списком.
                raw.push((colour, None));
                raw_px.push((colour, Some(v)));
            } else if let Some((pct, px)) = pct_px_pair(t) {
                // `calc(100% - 10px)` (css-images-4 §3.4.1: `<color-stop-length>`
                // = `<length-percentage>{1,2}`): доля и точки едут ПАРОЙ в
                // `stops_raw`, растр сложит их по длине оси. Прежде такой стоп
                // отбрасывался целиком, а градиент из одних `calc`-стопов
                // (`#five` в calc-background-linear-gradient-1) гас вовсе.
                any_pct = true;
                raw.push((colour, Some(pct)));
                raw_px.push((colour, Some(px)));
            } else if !t.trim().is_empty()
                && Len::parse(t).is_none()
                && t.trim().parse::<f32>().is_err()
            {
                continue;
            } else {
                raw.push((colour, None));
                raw_px.push((colour, None));
            }
        }
    }
    // css-images-4 §3.4.1 «Color Stop Lists»: список из ОДНОГО и более
    // стопов законен, и градиент из одного стопа красит этим цветом всю
    // картинку. Разворачиваем в пару одинаковых стопов на краях линии:
    // ниже `last = raw.len() - 1` при одном стопе давал 0/0 = NaN, а
    // `return None` гасил фон вовсе (`gradient-single-stop-001/-002/-004`).
    if raw.is_empty() {
        return None;
    }
    if raw.len() == 1 {
        let (colour, _) = raw[0];
        raw = vec![(colour, Some(0.0)), (colour, Some(1.0))];
        // Точечный список остаётся ПУСТЫМ: у сплошного цвета полос в точках
        // нет, а `stops_px` собирается только когда позиция есть у всех.
        raw_px = vec![(colour, None), (colour, None)];
    }
    // Фиксация по css-images-3 §3.5.3: позиция не меньше предыдущей, стопы
    // без позиции — поровну между соседями С позициями (а не по номеру в
    // списке). Тот же расклад, что у растра.
    let last = raw.len() - 1;
    let stops: Vec<(Color, f32)> = crate::background::place_stops(raw.clone());
    // TODO(gradient): `in hsl longer hue` — дуга тона синтетическими стопами
    // (css-images-4 §3.4.1.1) была за отладочным флагом HSL_ARC, замерена в
    // минус (−13/+2) и удалена; вернуться с точной математикой полос.
    // Точечные стопы пригодны к отрисовке, только когда позиции есть у ВСЕХ:
    // смешение точек с долями требует длины оси уже при разборе.
    let stops_px: Vec<(Color, f32)> = if !any_pct && raw_px.iter().all(|(_, p)| p.is_some()) {
        raw_px.iter().map(|(c, p)| (*c, p.unwrap_or(0.0))).collect()
    } else {
        vec![]
    };
    let stops_raw = raw
        .iter()
        .zip(raw_px.iter())
        .map(|((c, f), (_, p))| (*c, *f, *p))
        .collect();
    Some(Gradient {
        angle_deg: angle,
        radial,
        circle,
        from: stops[0].0,
        to: stops[last].0,
        stops,
        stops_px,
        stops_raw,
        // §12.2: без записи — sRGB, пока ВСЕ цвета устаревших форм, иначе
        // OKLab. Именно эта оговорка и держит совместимость: почти весь набор
        // пишет градиенты именами, `#hex` и `rgb()`, и остаётся в sRGB.
        space: named_space.unwrap_or(if all_legacy {
            GradSpace::Srgb
        } else {
            GradSpace::Oklab
        }),
        hue: hue_arc,
    })
}

/// Записан ли цвет УСТАРЕВШЕЙ формой sRGB: имя, `#hex`, `rgb()`, `rgba()`,
/// `hsl()`, `hsla()`, `hwb()` и их формы с прозрачностью (css-color-4 §12.2).
/// От ответа зависит пространство интерполяции по умолчанию.
fn legacy_srgb_color(token: &str) -> bool {
    let t = token.trim().to_ascii_lowercase();
    if t.starts_with('#') {
        return true;
    }
    match t.split_once('(') {
        // `rgb()` с `none` устаревшей записью не выражается: Blink вычисляет
        // его в `color(srgb …)`, и умолчание смешения становится OKLab
        // (`gradient-analogous-missing-components-004`, `gradient-eval-004`).
        // У `hsl()`/`hwb()` пространство и с `none` остаётся устаревшим.
        Some((name, body)) => match name.trim() {
            "rgb" | "rgba" => !body.contains("none"),
            "hsl" | "hsla" | "hwb" => true,
            _ => false,
        },
        // Имя цвета, `transparent` и `currentcolor` — тоже устаревшие формы.
        None => true,
    }
}
/// Ширина обводки: ключевые слова и любые шрифтовые/абсолютные длины.
fn outline_width_of(v: &str) -> Option<Len> {
    match v {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v).filter(|l| !matches!(l, Len::Pct(_))),
    }
}

/// Список `font-feature-settings` (css-fonts-4 §7.1): `normal` — пустой,
/// иначе `<opentype-tag> [ <integer [0,∞]> | on | off ]?` через запятую.
/// Тег — СТРОКА ровно из четырёх печатных ASCII-знаков в ЛЮБЫХ кавычках:
/// `'liga' off` прежде терялся целиком (`trim_matches('"')` оставлял шесть
/// знаков — `font-feature-resolution-001/002`). Тот же разбор нужен
/// дескриптору в `@font-face`.
pub(crate) fn feature_list(v: &str) -> Option<Vec<(String, u32)>> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("normal") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for token in v.split(',') {
        let token = token.trim();
        let q = token.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let rest = &token[1..];
        let end = rest.find(q)?;
        let tag = &rest[..end];
        if tag.len() != 4 || !tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return None;
        }
        let on = match rest[end + 1..].trim() {
            "" | "on" => 1,
            "off" => 0,
            n => n.parse::<u32>().ok()?,
        };
        out.push((tag.to_string(), on));
    }
    Some(out)
}

/// Годное имя семейства: строка в кавычках либо ряд идентификаторов.
///
/// Идентификатор по §4.1.3 начинается с буквы, подчёркивания, не-ASCII знака
/// или экранирования; за ними идут буквы, цифры, дефисы, подчёркивания и
/// экранирования. Цифра первой запрещена, дефис с цифрой следом — тоже.
pub(crate) fn family_name_ok(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    if (part.starts_with('"') && part.ends_with('"') && part.len() >= 2)
        || (part.starts_with('\'') && part.ends_with('\'') && part.len() >= 2)
    {
        return true;
    }
    part.split_whitespace().all(|word| {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let head_ok = first.is_alphabetic()
            || first == '_'
            || first == '\\'
            || first as u32 >= 0xa0
            || (first == '-'
                && word
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_alphabetic() || c == '_' || c as u32 >= 0xa0));
        head_ok
            && word.chars().all(|c| {
                c.is_alphanumeric() || c == '-' || c == '_' || c == '\\' || c as u32 >= 0xa0
            })
    })
}

/// Довод `fit-content(<length-percentage>)` (css-sizing-3 §4.1): только
/// точки или доля, неотрицательные. Прочее (`em`, `calc`) — `None`, и
/// значение ведёт себя, как прежде, голым `fit-content`.
fn fit_content_arg(v: &str) -> Option<Len> {
    let lower = v.trim().to_ascii_lowercase();
    let arg = lower.strip_prefix("fit-content(")?.strip_suffix(')')?;
    match Len::parse(arg) {
        Some(l @ (Len::Px(n) | Len::Pct(n))) if n >= 0.0 => Some(l),
        _ => None,
    }
}

/// Разобранный `calc-size()`.
enum CalcSize {
    /// Основа — длина: значение известно сразу.
    Fixed(f32),
    /// Основа — ключевое слово размера: `(mul, add, max, min)` над ним.
    Over((f32, f32, f32, f32)),
}

/// `calc-size(<basis>, <calc-sum>)` (css-values-5 §calc-size,
/// `csswg-drafts/css-values-5/Overview.bs`). Понимаются линейные выражения
/// над `size` (`size`, `size ± L`, `size * k`, `k * size`, `size / k`, их
/// суммы) и `min(size, L)` / `max(size, L)`; длины — в точках. Основа —
/// `auto`, `fit-content`, `min-content`, `max-content`, `content` или длина;
/// вложенный `calc-size()` и проценты не понимаются — объявление роняется.
fn calc_size_arg(v: &str) -> Option<CalcSize> {
    let inner = v.trim().strip_prefix("calc-size(")?.strip_suffix(')')?;
    let (basis, expr) = inner.split_once(',')?;
    let basis = basis.trim();
    let mut expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(e) = expr.strip_prefix("calc(").and_then(|e| e.strip_suffix(')')) {
        expr = e.to_string();
    }
    let px = |t: &str| t.strip_suffix("px").and_then(|n| n.parse::<f32>().ok());
    let f = if let Some(a) = expr.strip_prefix("min(size,").and_then(|e| e.strip_suffix(')')) {
        (1.0, 0.0, px(a)?, f32::MIN)
    } else if let Some(a) = expr.strip_prefix("max(size,").and_then(|e| e.strip_suffix(')')) {
        (1.0, 0.0, f32::MAX, px(a)?)
    } else {
        // Сумма членов: `size`, `size*k`, `k*size`, `size/k`, `L`.
        let (mut mul, mut add) = (0.0f32, 0.0f32);
        let mut rest = expr.as_str();
        let mut sign = 1.0f32;
        if let Some(r) = rest.strip_prefix('-') {
            sign = -1.0;
            rest = r;
        }
        loop {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let term = &rest[..end];
            if term == "size" {
                mul += sign;
            } else if let Some(k) = term.strip_prefix("size*").or_else(|| term.strip_suffix("*size")) {
                mul += sign * k.parse::<f32>().ok()?;
            } else if let Some(k) = term.strip_prefix("size/") {
                mul += sign / k.parse::<f32>().ok()?;
            } else {
                add += sign * px(term)?;
            }
            if end == rest.len() {
                break;
            }
            sign = if rest.as_bytes()[end] == b'-' { -1.0 } else { 1.0 };
            rest = &rest[end + 1..];
        }
        (mul, add, f32::MAX, f32::MIN)
    };
    if let Some(b) = px(basis) {
        let (mul, add, max, min) = f;
        return Some(CalcSize::Fixed((b * mul + add).min(max).max(min).max(0.0)));
    }
    matches!(basis, "auto" | "fit-content" | "min-content" | "max-content" | "content")
        .then_some(CalcSize::Over(f))
}

/// `object-view-box: none | <basic-shape-rect>` — `inset()`, `rect()`,
/// `round <'border-radius'>` of `inset()`/`rect()`/`xywh()` (css-shapes-1
/// §basic-shape-rect): one radius for all corners and both axes — every
/// listed value, before and after `/`, equal. `20px / 20px` is that radius;
/// unequal corners are not representable here and stay unrounded.
fn uniform_round(r: &str) -> Option<Len> {
    let mut it = r.split(|c: char| c == '/' || c.is_whitespace()).filter(|t| !t.is_empty());
    let first = Len::parse(it.next()?)?;
    it.all(|t| Len::parse(t) == Some(first)).then_some(first)
}

/// `xywh()` (css-images-4 §object-view-box; css-shapes-1 §basic-shape-rect).
/// Длины — точки или доли; `inset` с 1-3 значениями раскрывается как поля.
fn parse_view_box(v: &str) -> Option<(u8, [Len; 4])> {
    let v = v.trim();
    let (kind, inner) = if let Some(r) = v.strip_prefix("inset(") {
        (0u8, r)
    } else if let Some(r) = v.strip_prefix("rect(") {
        (1u8, r)
    } else if let Some(r) = v.strip_prefix("xywh(") {
        (2u8, r)
    } else {
        return None;
    };
    let inner = inner.strip_suffix(')')?;
    let parts: Vec<Len> = inner
        .split_whitespace()
        .map(|t| match Len::parse(t) {
            Some(l @ (Len::Px(_) | Len::Pct(_))) => Some(l),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let four = match (kind, parts.as_slice()) {
        (_, [a, b, c, d]) => [*a, *b, *c, *d],
        (0, [a]) => [*a, *a, *a, *a],
        (0, [a, b]) => [*a, *b, *a, *b],
        (0, [a, b, c]) => [*a, *b, *c, *b],
        _ => return None,
    };
    Some((kind, four))
}

fn assign_size(slot: &mut Option<Len>, v: &str) {
    // Смесь «доля ± точки» доживает индексом (`parse_mixed`): раскладка
    // складывает её сама (`DefiniteLength::Calc`, css-values-4 §10.9).
    // Прежде `calc(50% - 3px)` роняло объявление (`calc-width-block-1`).
    let parsed = size_range::parse(v);
    // `none` снимает предел (§10.4) — слот гаснет по праву. Прочая
    // неразборная запись объявление роняет: слот сохраняет прежнее значение,
    // а не гаснет (§4.2).
    if v.trim().eq_ignore_ascii_case("none") {
        *slot = None;
        return;
    }
    if let Some(l) = parsed {
        *slot = Some(l);
    }
}

/// Годна ли запись `box-shadow` ЦЕЛИКОМ (css-backgrounds-3 §7.1:
/// `none | <shadow>#`; тень — 2-4 длины, не больше одного цвета и одного
/// `inset`). `none` внутри списка делает декларацию негодной.
fn box_shadow_valid(v: &str) -> bool {
    if v.trim().eq_ignore_ascii_case("none") {
        return true;
    }
    crate::css::split_args(v).iter().all(|s| {
        let (mut lens, mut colours, mut insets) = (0, 0, 0);
        for token in tokenize_shadow(s) {
            match Len::parse(&token) {
                Some(Len::Pct(_)) => return false,
                Some(_) => lens += 1,
                None if token.eq_ignore_ascii_case("inset") => insets += 1,
                None if token.eq_ignore_ascii_case("currentcolor")
                    || Color::parse(&token).is_some() =>
                {
                    colours += 1
                }
                None => return false,
            }
        }
        (2..=4).contains(&lens) && colours <= 1 && insets <= 1
    })
}

fn parse_shadows(v: &str) -> Vec<Shadow> {
    let mut out = vec![];
    for s in crate::css::split_args(v) {
        // Внутренние тени не рисуются — но синтаксис их проверяется: одна
        // невалидная тень роняет ВСЮ декларацию (css-backgrounds-3 §7.2).
        let inner = s.contains("inset");
        let mut lens = vec![];
        let mut color = None;
        for token in tokenize_shadow(&s) {
            match Len::parse(&token) {
                Some(Len::Px(px)) => lens.push(Some(px)),
                // calc() из абсолютных единиц уже свёрнут в px; примесь
                // процентов невалидна для тени.
                Some(Len::Calc(id)) => {
                    let sum = crate::value::calc_get(id);
                    if sum.pct != 0.0 {
                        return vec![];
                    }
                    // Шрифтовые/оконные слагаемые здесь не резолвятся —
                    // тень пропускается, но декларация остаётся валидной.
                    let bare = crate::value::Sum {
                        px: 0.0,
                        pct: 0.0,
                        ..sum
                    };
                    lens.push((bare == crate::value::Sum::default()).then_some(sum.px));
                }
                Some(Len::Pct(_)) => return vec![],
                // em/vh и прочее — валидно, но контекста тут нет.
                Some(_) => lens.push(None),
                None => {
                    if let Some(c) = Color::parse(&token) {
                        color = Some(c);
                    } else if token == "currentcolor" {
                        // Явный `currentColor` = как отсутствие цвета:
                        // метка a = -1 дорешается при слиянии стилей.
                    } else if token != "inset" {
                        return vec![];
                    }
                }
            }
        }
        // Длин бывает от двух до четырёх (§7.2).
        if lens.len() < 2 || lens.len() > 4 {
            return vec![];
        }
        if inner || lens.iter().any(Option::is_none) {
            continue;
        }
        let lens: Vec<f32> = lens.into_iter().flatten().collect();
        out.push(Shadow {
            x: lens[0],
            y: lens[1],
            blur: lens.get(2).copied().unwrap_or(0.0),
            spread: lens.get(3).copied().unwrap_or(0.0),
            // Тень без цвета берёт currentColor (css-backgrounds-3
            // §7): цвет текста известен только после слияния стилей,
            // отрицательная альфа — метка «дорешать там».
            color: color.unwrap_or(Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: -1.0,
            }),
        });
    }
    out
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
    #[test]
    fn var_fallback_with_nested_parens_survives_a_defined_variable() {
        // Конец записи — парная скобка, а запятая ищется на верхнем уровне:
        // иначе при ЗАДАННОЙ переменной оставалась лишняя скобка и значение
        // умирало, а при незаданной выходило случайно верно — из-за чего
        // дефект и не был виден.
        let mut vars = super::super::css::Decls::new();
        vars.insert("--c".into(), "red".into());
        let mut c = super::Computed::default();
        c.apply_decls_with_vars(
            &super::super::css::parse_decls("color: var(--c, rgba(0,0,0,.5))"),
            &vars,
        );
        assert_eq!(c.color, crate::value::Color::parse("red"));
        // Незаданная переменная берёт запасное значение ЦЕЛИКОМ.
        let mut c = super::Computed::default();
        c.apply_decls_with_vars(
            &super::super::css::parse_decls("color: var(--none, rgba(0,0,0,1))"),
            &super::super::css::Decls::new(),
        );
        assert_eq!(c.color, crate::value::Color::parse("rgba(0,0,0,1)"));
    }

    #[test]
    fn important_survives_a_later_ordinary_rule() {
        // Важность — самый старший ключ сравнения (CSS Cascade §6.1): важное
        // объявление раннего правила переживает обычное объявление позднего,
        // даже если то и специфичнее. Пока проходы шли внутри правила,
        // `!important` действовал только против соседей по своему блоку.
        let early = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p").expect("селектор тега"),
            decls: super::super::css::parse_decls("color: red !important"),
            order: 0,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let late = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p.x").expect("селектор класса"),
            decls: super::super::css::parse_decls("color: green"),
            order: 1,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let mut matched = vec![&early, &late];
        let c = super::Computed::resolve(&mut matched, &super::super::css::Decls::new());
        assert_eq!(c.color, crate::value::Color::parse("red"));
    }

    #[test]
    fn author_sheet_beats_user_agent_regardless_of_specificity() {
        // Происхождение старше специфичности (CSS Cascade §6.4.4). Пока обе
        // таблицы сравнивались только специфичностью, `* { margin: 0 }` со
        // специфичностью (0,0,0) проигрывал умолчанию `p { margin: 6px 0 }`
        // — то есть не работал ни один reset.
        let ua = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p").expect("селектор тега"),
            decls: super::super::css::parse_decls("margin-top: 6px"),
            order: 0,
            origin: 0,
            layer: vec![u32::MAX],
        };
        let author = super::super::css::Rule {
            sel: super::super::css::Selector::parse("*").expect("универсальный селектор"),
            decls: super::super::css::parse_decls("margin-top: 0"),
            order: 1,
            origin: 1,
            layer: vec![u32::MAX],
        };
        let mut matched = vec![&ua, &author];
        let c = super::Computed::resolve(&mut matched, &super::super::css::Decls::new());
        assert_eq!(c.margin.top, Some(super::Len::Px(0.0)));
    }

    #[test]
    fn font_shorthand_takes_size_with_line_height_and_family() {
        // Пробелы вокруг косой черты допустимы, и семейство начинается ПОСЛЕ
        // высоты строки: раньше «/ 1 Ahem» уезжало в семейство целиком, и
        // страница набиралась чужим шрифтом.
        let mut c = super::Computed::default();
        c.apply_one("font", "50px / 1 Ahem");
        assert_eq!(c.font_size, Some(super::Len::Px(50.0)));
        assert_eq!(c.line_height, Some(super::Len::Pct(1.0)));
        assert_eq!(c.font_family.as_deref(), Some("Ahem"));
    }

    use super::*;
    use crate::css::parse_decls;

    fn computed(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn flex_shorthand_zeroes_the_omitted_basis() {
        // Опущенная основа — 0%, а не ширина элемента.
        let one = computed("flex: 1");
        assert_eq!(one.flex_grow, Some(1.0));
        assert_eq!(one.flex_shrink, Some(1.0));
        assert_eq!(one.flex_basis, Some(Len::Pct(0.0)));
        let two = computed("flex: 0 1");
        assert_eq!(two.flex_grow, Some(0.0));
        assert_eq!(two.flex_shrink, Some(1.0));
        assert_eq!(two.flex_basis, Some(Len::Pct(0.0)));
        // Длина в сокращении — это основа, а рост и сжатие становятся 1.
        let len = computed("flex: 30px");
        assert_eq!(len.flex_grow, Some(1.0));
        assert_eq!(len.flex_shrink, Some(1.0));
        assert_eq!(len.flex_basis, Some(Len::Px(30.0)));
        let pair = computed("flex: 2 30px");
        assert_eq!(pair.flex_grow, Some(2.0));
        assert_eq!(pair.flex_shrink, Some(1.0));
        assert_eq!(pair.flex_basis, Some(Len::Px(30.0)));
        // Ключевые слова оставляют основу `auto`.
        assert_eq!(computed("flex: none").flex_basis, Some(Len::Auto));
        assert_eq!(computed("flex: auto").flex_basis, Some(Len::Auto));
        assert_eq!(computed("flex: initial").flex_basis, Some(Len::Auto));
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
        assert_eq!(
            t,
            vec![
                TrackSize::Single(Track::Px(120.0)),
                TrackSize::Single(Track::Auto),
                TrackSize::Single(Track::Fr(1.0)),
            ]
        );
    }

    #[test]
    fn repeat_expands_into_equal_tracks() {
        let t = computed("grid-template-columns: repeat(3, 1fr)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![TrackSize::Single(Track::Fr(1.0)); 3]);
    }

    #[test]
    fn minmax_keeps_both_bounds() {
        // Обе грани доходят до раскладки: сведение к одной меняло ширину
        // колонки и расходилось с браузером.
        let t = computed("grid-template-columns: minmax(120px, 1fr)")
            .grid_tracks
            .unwrap();
        assert_eq!(t, vec![TrackSize::MinMax(Track::Px(120.0), Track::Fr(1.0))]);
    }

    #[test]
    fn content_sized_tracks_are_recognised() {
        let t = computed("grid-template-columns: min-content max-content")
            .grid_tracks
            .unwrap();
        assert_eq!(
            t,
            vec![
                TrackSize::Single(Track::MinContent),
                TrackSize::Single(Track::MaxContent),
            ]
        );
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

#[cfg(test)]
mod gradient_tests {
    use super::*;
    use crate::css::parse_decls;

    fn c(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn radial_gradient_is_recognised_with_its_shape() {
        let g = c("background: radial-gradient(circle at center, #fff, #000)")
            .gradient
            .unwrap();
        assert!(g.radial && g.circle, "форма окружности обязана дойти");
        let e = c("background: radial-gradient(#fff, #000)")
            .gradient
            .unwrap();
        assert!(e.radial && !e.circle, "без ключевого слова — эллипс");
    }

    #[test]
    fn every_stop_survives_with_its_position() {
        let g = c("background: linear-gradient(180deg, #e03131 0%, #fcc419 50%, #2f9e44 100%)")
            .gradient
            .unwrap();
        assert_eq!(
            g.stops.len(),
            3,
            "средний стоп терялся — градиент был двух-цветным"
        );
        assert_eq!(g.stops[1].1, 0.5);
    }

    #[test]
    fn stops_without_positions_spread_evenly() {
        let g = c("background: linear-gradient(90deg, #000, #888, #fff)")
            .gradient
            .unwrap();
        assert_eq!(g.stops[1].1, 0.5, "равномерная раскладка, как в CSS");
    }
}

/// Начальное значение свойства словами — для `initial`/`unset`/`revert`.
///
/// Значения взяты из спецификаций; свойства, начальное значение которых у нас
/// и так «поле не задано», сюда не входят — им сброс не нужен.
/// Наследуется ли свойство по умолчанию (столбец «Inherited» таблиц
/// свойств CSS). Нужен `unset`: у наследуемого он значит `inherit`.
fn inherited_property(key: &str) -> bool {
    matches!(
        key,
        "color"
            | "font"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "font-stretch"
            | "letter-spacing"
            | "word-spacing"
            | "line-height"
            | "text-align"
            | "text-align-last"
            | "text-indent"
            | "text-transform"
            | "visibility"
            | "white-space"
            | "list-style"
            | "list-style-type"
            | "list-style-position"
            | "list-style-image"
            | "direction"
            | "writing-mode"
            | "quotes"
            | "cursor"
            | "tab-size"
            | "word-break"
            | "overflow-wrap"
            | "word-wrap"
            | "hyphens"
            | "text-orientation"
            | "border-collapse"
            | "border-spacing"
            | "caption-side"
            | "empty-cells"
    )
}

fn initial_value(key: &str) -> Option<&'static str> {
    Some(match key {
        "border" => "0 none",
        "border-radius" => "0",
        "corner-shape" => "round",
        "border-shape" => "none",
        "color" => "black",
        "margin" => "0",
        "padding" => "0",
        "direction" => "ltr",
        "font-family" => "serif",
        "font-size" => "medium",
        "font-style" => "normal",
        "font-variant" => "normal",
        "font-weight" => "normal",
        "letter-spacing" => "normal",
        "line-break" => "auto",
        "line-height" => "normal",
        "list-style-position" => "outside",
        "list-style-type" => "disc",
        "quotes" => "auto",
        "overflow-wrap" | "word-wrap" => "normal",
        "tab-size" => "8",
        "text-align" => "start",
        "text-justify" => "auto",
        "text-combine-upright" => "none",
        "text-indent" => "0",
        "text-orientation" => "mixed",
        "text-transform" => "none",
        "vertical-align" => "baseline",
        "visibility" => "visible",
        "white-space" => "normal",
        "word-break" => "normal",
        "word-spacing" => "normal",
        "writing-mode" => "horizontal-tb",
        _ => return None,
    })
}

#[cfg(test)]
mod border_image_tests {
    use super::*;

    /// Сокращение несёт источник, срез и укладку разом.
    #[test]
    fn shorthand_carries_source_slice_and_repeat() {
        let mut c = Computed::default();
        c.apply_one("border-image", "url(C:/tmp/border.png) 27 round");
        let bi = c.border_image.expect("рамка-картинка разобрана");
        assert_eq!(bi.src, "C:/tmp/border.png");
        assert_eq!(bi.slice[0], BorderImageSlice::Px(27.0));
        assert_eq!(bi.repeat, (Tiling::Round, Tiling::Round));
    }

    /// Отдельные свойства дополняют ту же запись.
    #[test]
    fn longhands_add_up() {
        let mut c = Computed::default();
        c.apply_one("border-image-source", "url(C:/tmp/b.png)");
        c.apply_one("border-image-slice", "30% fill");
        c.apply_one("border-image-repeat", "round space");
        let bi = c.border_image.expect("рамка-картинка разобрана");
        assert!(bi.fill);
        assert_eq!(bi.slice[1], BorderImageSlice::Pct(0.3));
        assert_eq!(bi.repeat, (Tiling::Round, Tiling::Space));
    }
}

/// Множитель роста/сжатия гибкого элемента: `<number>` или `calc()` из чисел
/// (css-values-4 §10.1), неотрицательный. `calc(infinity)` (§10.7.1) —
/// наибольшее представимое: здесь — конечное большое, чтобы сумма
/// множителей и доли свободного места не уходили в бесконечность и `NaN`
/// (`flex-grow-009`: `flex: calc(infinity) 0 0px` забирает всё место).
fn flex_factor(v: &str) -> Option<f32> {
    let g = crate::value::number(v)?;
    if g.is_nan() || g < 0.0 {
        return None;
    }
    Some(g.min(1.0e18))
}

/// `stretch` and its prefixed spellings (css-sizing-4 §4.1).
fn stretch_keyword(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("stretch")
        || v.eq_ignore_ascii_case("-webkit-fill-available")
        || v.eq_ignore_ascii_case("-moz-available")
}
