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

/// Разряды `inherit_bits`: ненаследуемые свойства, у которых слово `inherit`
/// обязано скопировать вычисленное значение родителя (§6.2.1).
pub(crate) mod inh {
    pub(crate) const BG_REPEAT: u16 = 1 << 0;
    pub(crate) const Z_INDEX: u16 = 1 << 1;
    pub(crate) const OUTLINE_W: u16 = 1 << 2;
    pub(crate) const DISPLAY: u16 = 1 << 3;
    pub(crate) const BG_IMAGE: u16 = 1 << 4;
    pub(crate) const BG_POS: u16 = 1 << 5;
    pub(crate) const CLIP: u16 = 1 << 6;
    pub(crate) const BG_ORIGIN: u16 = 1 << 7;
    pub(crate) const BG_CLIP: u16 = 1 << 8;
    pub(crate) const BG_SIZE: u16 = 1 << 9;
    pub(crate) const TRANSFORM: u16 = 1 << 10;
    pub(crate) const TRANSFORM_ORIGIN: u16 = 1 << 11;
    pub(crate) const OUTLINE_C: u16 = 1 << 12;
    pub(crate) const OUTLINE_S: u16 = 1 << 13;
    pub(crate) const OUTLINE_O: u16 = 1 << 14;
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
fn font_lengths_to_px(v: &str, em: f32, rem: f32, ex: f32, ch: f32) -> String {
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
    /// none/hidden гасят, dotted/dashed рисуются как solid (приближение).
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
    /// По форме текста. Своей отрисовки для него нет: фон под текстом мы
    /// показать не умеем, поэтому такой фон не рисуется вовсе — это ближе к
    /// правде, чем закрасить всю коробку.
    Text,
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
    let word = |t: &str| -> Option<Len> {
        match t {
            "left" | "top" => Some(Len::Pct(0.0)),
            "center" => Some(Len::Pct(0.5)),
            "right" | "bottom" => Some(Len::Pct(1.0)),
            other => Len::parse(other),
        }
    };
    let mut x: Option<Len> = None;
    let mut y: Option<Len> = None;
    let mut free: Vec<Option<Len>> = vec![];
    for t in v.split_whitespace() {
        match t {
            "left" | "right" => x = word(t),
            "top" | "bottom" => y = word(t),
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
        }
    }
}

impl Transform {
    /// Дописать функцию справа: `M := M · [l | v]`.
    fn push(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
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

    /// Применить к цвету.
    pub fn apply(&self, c: Color) -> Color {
        let (mut r, mut g, mut b) = (c.r, c.g, c.b);
        // Порядок как в CSS: функции применяются слева направо, а записаны
        // они у нас в фиксированном порядке — для набора без повторов это то
        // же самое.
        if self.sepia > 0.0 {
            let k = self.sepia;
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
            let k = self.grayscale;
            r += (lum - r) * k;
            g += (lum - g) * k;
            b += (lum - b) * k;
            let sat = self.saturate;
            r = lum + (r - lum) * sat;
            g = lum + (g - lum) * sat;
            b = lum + (b - lum) * sat;
        }
        if self.invert > 0.0 {
            let k = self.invert;
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
            a: (c.a * self.opacity).clamp(0.0, 1.0),
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
    /// `attr(имя)`.
    Attr(String),
}

/// Порядковые номера объявлений физических сторон (top, right, bottom, left)
/// для полей, отступов и краёв — см. `LogicalSides::seq`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SideSeq {
    pub padding: [u32; 4],
    pub margin: [u32; 4],
    pub inset: [u32; 4],
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
    /// Имя области у ребёнка: `grid-area: header`.
    pub grid_area_name: Option<String>,
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
    pub auto_repeat_cols: Option<AutoRepeat>,
    pub auto_repeat_rows: Option<AutoRepeat>,

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
    pub(crate) inherit_bits: u16,
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
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub line_through: Option<bool>,
    pub line_height: Option<Len>,
    pub text_align: Option<TextAlign>,
    /// `text-align-last` — выключка ПОСЛЕДНЕЙ строки абзаца. Отдельное
    /// свойство, потому что по умолчанию последняя строка не растягивается:
    /// иначе абзац из одного слова разъехался бы во всю ширину.
    pub text_align_last: Option<TextAlign>,
    /// `text-justify: none` — выключка запрещена, строка идёт как `start`.
    pub no_justify: Option<bool>,
    /// `hanging-punctuation` — какая пунктуация выходит за край строки.
    pub hanging: Option<Hanging>,
    pub nowrap: Option<bool>,
    /// Переводы строк значимы (`white-space: pre*`).
    pub preserve_newlines: Option<bool>,
    /// Пробелы значимы. У `pre-line` переводы строк значимы, а пробелы нет —
    /// без отдельного поля он схлопывал и то и другое.
    pub keep_spaces: Option<bool>,
    pub monospace: Option<bool>,
    /// Первое конкретное имя из `font-family`.
    pub font_family: Option<String>,
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
    pub object_fit: Option<String>,

    /// `aspect-ratio` — отношение ширины к высоте.
    pub aspect_ratio: Option<f32>,
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
    /// `order`: визуальный порядок в гибкой строке. Раскладка под нами его не
    /// знает, поэтому детей переставляет сам сборщик дерева.
    pub order: Option<i32>,
    pub align_content: Option<Justify>,
    /// `justify-items`/`justify-self` — поперечная ось В СЕТКЕ.
    pub justify_items: Option<Align>,
    /// Модификатор `safe` у выравниваний (css-align §5.3): при переполнении
    /// области выравнивание падает в `start`, чтобы содержимое не обрезалось.
    /// Без него позиция сохраняется и элемент вылезает (unsafe/дефолт).
    pub justify_self_safe: bool,
    pub justify_items_safe: bool,
    pub align_self_safe: bool,
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
    pub justify_self: Option<Align>,

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
    /// `border-spacing` таблицы: горизонтальный и вертикальный зазор.
    pub border_spacing: Option<(Option<Len>, Option<Len>)>,
    pub outline: Option<Outline>,
    /// `backdrop-filter: blur(N)` — размытие того, что под элементом.
    pub backdrop_blur: Option<f32>,

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
    /// `unicode-bidi: isolate` — кусок не влияет на порядок соседей.
    pub bidi_isolate: Option<bool>,
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
    /// `column-gap` — зазор между колонками многоколоночного потока.
    /// Умолчание CSS — `normal`, то есть один кегль.
    pub column_gap: Option<Len>,
    /// `translate` — визуальный сдвиг, не меняющий раскладку.
    pub translate: Option<(Len, Len)>,
    /// `text-shadow`: смещение, размытие и цвет.
    pub text_shadow: Option<Shadow>,
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
    /// `transform-origin` в долях размера элемента.
    pub transform_origin: Option<(f32, f32)>,
    /// `transform`/`transform-origin` с длинами в единицах шрифта: запись
    /// ждёт своего кегля и разбирается в `resolve_em` (css-transforms-1
    /// §computed value: относительные длины становятся абсолютными).
    pub transform_raw: Option<String>,
    /// Есть ли выше трансформированный предок: он — содержащий блок и для
    /// `position: fixed` (css-transforms-1 §transform-rendering: «…for all
    /// of its absolute-position descendants, fixed-position descendants»).
    pub transform_ancestor: bool,
    pub transform_origin_raw: Option<String>,
    /// Точка отсчёта преобразования В ТОЧКАХ по осям — когда записана длиной,
    /// а не долей. Долю из неё делает отрисовка: размер коробки известен там.
    pub transform_origin_px: (Option<f32>, Option<f32>),
    /// `float`: -1 — влево, 1 — вправо, 0 — не обтекается.
    pub float: Option<i8>,
    /// `clear: inherit` — сторону берёт родитель. Своего наследования у
    /// `clear` нет (свойство ненаследуемое), поэтому ключевое слово помнится
    /// отдельно и разрешается там, где родительский стиль под рукой.
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
    /// Стиль первой буквы абзаца (`::first-letter`).
    ///
    /// Живёт в стиле, а не в элементе, потому что абзац собирается из кусков
    /// уже без узла-родителя: до кусков доезжает только вычисленный стиль.
    pub first_letter: Option<Box<Computed>>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Box<Computed>>,
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
    /// Коробка КОРНЯ документа: её содержащий блок — начальный, и высота его
    /// определена всегда (§10.5). Ставится вместе с пометкой канваса, чтобы
    /// снимаемая обёртка уносила признак с собой.
    pub(crate) root_box: bool,
    /// ОПРЕДЕЛЕНА ли высота содержащего блока: от неё зависит, считается ли
    /// доля высоты вообще (§10.5 — иначе значение равно `auto`).
    pub(crate) cb_height_def: bool,
    /// Коробка РАСТЯНУТА раскладкой: элемент гибкого контейнера или сетки без
    /// своей высоты получает её от полосы, и для потомков она определена.
    pub(crate) stretched: bool,
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
    /// `font-stretch` — ширина начертания в процентах от обычной.
    ///
    /// Это НЕ возможность OpenType: узкое начертание — отдельный шрифт
    /// семейства, и выбирается он при подборе.
    pub font_stretch: Option<f32>,
    /// `tab-size` — во сколько пробелов раскрывается табуляция.
    pub tab_size: Option<u8>,
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
    /// `display: inline` дословно (не inline-block): §9.7/§10.2 дорешиваются
    /// после каскада — см. `dom::finish_inline_display`.
    pub inline_display: Option<bool>,
    /// `display: run-in` — вбегание решает `dom::fold_run_ins`.
    pub run_in: Option<bool>,
    /// Есть ли выше по дереву коробка, устанавливающая содержащий блок для
    /// внепоточных потомков (§10.1 п.4). Ставится при наследовании: сам
    /// каскад предков не видит.
    pub(crate) cb_ancestor: bool,
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
    /// `clip-path: circle(...)|ellipse(...)` с параметрами: сырые аргументы
    /// формы (`shape:circle(...)`). Радиусы и центр зависят от размера
    /// коробки — он известен только отрисовке, поэтому форма растрируется
    /// маской буфера группы (см. `background::source`).
    pub clip_shape: Option<String>,
    /// `mask-size`: размер плитки маски; None — auto (интринзик картинки).
    pub mask_size: Option<(Len, Len)>,
    /// `mask-repeat`: пооосный запрет мощения (no-x, no-y).
    pub mask_no_repeat: Option<(bool, bool)>,
    /// `mask-size: contain|cover` (1|2): вписывание по интринзику.
    pub mask_fit: Option<u8>,
    /// `mask-mode: luminance` — маскирует светимость, а не альфа.
    pub mask_luminance: Option<bool>,
    /// `mask-origin`: коробка укладки плитки (0 border, 2 padding, 3 content).
    pub mask_origin: Option<u8>,
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
    /// `column-span: all` — блок растянут на все колонки.
    pub column_span: Option<bool>,
    /// `break-inside: avoid*` — коробку нельзя разрывать между колонками и
    /// страницами (css-break-3 §4.1). Свойство не разбиралось вовсе, и
    /// отличить монолит от обычной коробки было нечем.
    pub break_inside_avoid: bool,
    /// `break-before`/`break-after` (css-break-4 §3.1): принудительный разрыв
    /// колонки/страницы перед или после коробки.
    pub break_before_force: bool,
    pub break_after_force: bool,
    /// `column-rule-*`: линейка между колонками.
    pub column_rule_width: Option<Len>,
    pub column_rule_visible: Option<bool>,
    pub column_rule_color: Option<Color>,
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
    /// Эллиптические радиусы углов (`border-radius: H / V`), tl/tr/br/bl:
    /// растеризатор круглит только окружностью — такой угол уходит
    /// альфа-маской буфера группы (`shape:rrect(...)`).
    pub radius_ell: Option<[Option<(f32, f32)>; 4]>,
    /// `filter`: цветовые преобразования, применённые к собственным цветам.
    pub filter: Option<Filter>,
    /// `filter: url(#id)` — ссылка на SVG-`<filter>` документа; рисуется
    /// растровым слоем поверх коробки (`interact::FilterLayer`).
    pub filter_ref: Option<String>,

    /// `background-image: url(...)` — ссылка на картинку-заливку.
    pub bg_image: Option<String>,
    pub bg_size: BgSize,
    pub bg_pos: BgPos,
    /// `object-position` замещаемого содержимого (css-images-3 §5.2).
    pub object_position: Option<BgPos>,
    pub bg_repeat: Option<BgRepeat>,
    /// `content` псевдоэлемента — СПИСОК составляющих (css-content-3 §2):
    /// строки, `counter()`, `counters()`, `attr()` в любом порядке.
    pub content: Option<Vec<ContentItem>>,
    /// `counter-reset` — обнулить счётчик с этого узла.
    pub counter_reset: Option<String>,
    /// `counter-increment` — увеличить счётчик на этом узле.
    pub counter_increment: Option<String>,
    /// `counter-set` — присвоить счётчику значение (css-lists-3 §5).
    pub counter_set: Option<String>,
    /// Возможности шрифта (`font-feature-settings`, `font-variant`).
    pub font_features: Vec<(String, u32)>,
    /// `font-synthesis-weight|style|small-caps: none` — подмена начертания
    /// запрещена (css-fonts-4 §6.5). Ложь = `none`, пусто = `auto`.
    pub font_synth: (Option<bool>, Option<bool>, Option<bool>),
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
    /// Градиент, которому нужна МЕХАНИКА ПЛИТКИ (размер, повтор, позиция,
    /// свой край): сплошная заливка её не умеет, рисует слой-картинка.
    pub(crate) fn gradient_as_tile(&self) -> bool {
        self.gradient_raw.is_some()
            && (self.bg_size != crate::computed::BgSize::Auto
                || self.bg_repeat.is_some()
                || self.bg_pos.x.is_some()
                || self.bg_pos.y.is_some()
                || self.bg_origin.is_some()
                // Цвет фона лежит ПОД всеми слоями (css-backgrounds-3 §3.1):
                // у заливки коробки место одно, поэтому цвет — ей, градиент —
                // слоем сверху (`bg-color-with-gradient`).
                || self.background.is_some_and(|c| c.a > 0.0))
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
        c.underline = None;
        c.line_through = None;
        c
    }

    pub fn resolve_logical(&mut self, parent_vertical: Option<bool>) {
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
        let vertical = self.vertical == Some(true) && parent_vertical == Some(true);
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
        if vertical {
            set_ci(&mut self.contain_intrinsic.1, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.0, logical.ci_block);
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
            set_ci(&mut self.contain_intrinsic.0, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.1, logical.ci_block);
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
                s.px += s.vw * viewport.0 + s.vh * viewport.1;
                s.vw = 0.0;
                s.vh = 0.0;
                *l = s.collapse();
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
                        s.px += s.vw * viewport.0 + s.vh * viewport.1;
                        s.vw = 0.0;
                        s.vh = 0.0;
                        *one = s.collapse();
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
        sides(&mut self.inset);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }

    pub fn resolve_em(&mut self, parent_font_px: f32) {
        // Сначала свой размер шрифта: от него считается всё остальное. Для
        // него самого единицы шрифта считаются от РОДИТЕЛЬСКОГО кегля.
        // Родовое `monospace` имени семейства не даёт, а меряться должно по
        // тому шрифту, которым текст в самом деле наберётся.
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::metrics::mono_family().to_string()
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
        if self.transform_raw.is_some() || self.transform_origin_raw.is_some() {
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
        let (mut ch, ex) = crate::metrics::ch_ex_px(&family, base);
        // `ch` — продвижение нуля вдоль оси строки. При стоящих глифах в
        // вертикальном письме строка идёт сверху вниз, и продвижение равно
        // кеглю, а не ширине глифа (CSS Writing Modes §7.4).
        if self.vertical == Some(true) && self.upright == Some(true) {
            ch = base;
        }
        // `ic` меряется по тому же семейству и тем же шагом, что `ch` и `ex`.
        let ic = crate::metrics::ic_px(&family, base);
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
                s.px += s.em * base + s.ch * ch + s.ex * ex + s.ic * ic;
                s.em = 0.0;
                s.ch = 0.0;
                s.ex = 0.0;
                s.ic = 0.0;
                *l = s.collapse();
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
            &mut self.column_gap,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        sides(&mut self.border_width);
        sides(&mut self.inset);
        for corner in [
            &mut self.radius.tl,
            &mut self.radius.tr,
            &mut self.radius.br,
            &mut self.radius.bl,
        ] {
            fix(corner);
        }
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
            font_weight: self.font_weight,
            italic: self.italic,
            underline: self.underline,
            line_through: self.line_through,
            line_height: self.line_height,
            text_align: self.text_align,
            text_align_last: self.text_align_last,
            break_word: self.break_word,
            balance_lines: self.balance_lines,
            bidi_override: self.bidi_override,
            bidi_isolate: self.bidi_isolate,
            hanging: self.hanging,
            nowrap: self.nowrap,
            monospace: self.monospace,
            letter_spacing: self.letter_spacing,
            font_features: self.font_features.clone(),
            text_transform: self.text_transform,
            ellipsis: self.ellipsis,
            overflow_marker: self.overflow_marker.clone(),
            line_clamp: self.line_clamp,
            clamp_legacy: self.clamp_legacy,
            clamp_auto: self.clamp_auto,
            svg_fill: self.svg_fill.clone(),
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
        matched.sort_by_key(|r| (r.origin, r.sel.specificity(), r.order));
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
        important.sort_by_key(|r| (std::cmp::Reverse(r.origin), r.sel.specificity(), r.order));
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
        for k in &ordered {
            let Some(v) = d.get(*k) else { continue };
            if k.starts_with("--") || k.as_str() == crate::css::ORDER_KEY {
                continue;
            }
            for part in v.split(crate::css::DECL_SEP) {
                if is_important(part) != important {
                    continue;
                }
                let resolved = resolve_vars(strip_important(part), vars);
                self.apply_one(k, &resolved);
            }
        }
    }

    fn apply_one(&mut self, key: &str, val: &str) {
        self.decl_seq += 1;
        let v = val.trim();
        // Общие для всех свойств слова `initial`/`unset`/`revert`. Для
        // НАСЛЕДУЕМОГО свойства это не «оставить как есть»: незаданное поле у
        // нас берётся от родителя, поэтому такое объявление молча наследовало
        // вместо сброса — на `static-position` отступ первой строки уходил в
        // абсолютный блок, и красное проступало из-под него.
        if matches!(v, "initial" | "unset" | "revert" | "revert-layer")
            && let Some(start) = initial_value(key)
        {
            return self.apply_one(key, start);
        }
        match key {
            "box-sizing" => self.border_box = Some(v == "border-box"),
            "display"
                if v.trim().eq_ignore_ascii_case("-webkit-box")
                    || v.trim().eq_ignore_ascii_case("-webkit-inline-box") =>
            {
                self.webkit_box = Some(true);
            }
            "display" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::DISPLAY;
                    return;
                }
                self.inline_display = None;
                // Каскад мог поставить группу выше по важности, а ниже —
                // обычный блок: метка рода не переживает своё значение.
                self.row_group_kind = None;
                self.col_role = None;
                // Запись из ДВУХ слов (CSS Display 3): `inline grid-lanes`,
                // `block flow` и родня — внешний вид и внутренний.
                //
                // ★ ЗАМЕРЕНО: разбирать её ЦЕЛИКОМ (внешний `inline` → свои
                // строчные виды) — минус: css-grid 393 → 381. В наборе 233
                // файла пишут `display: inline grid`, и наша строчная сетка
                // им хуже блочной. Поэтому из двух слов берётся только то,
                // чего иначе не выразить вовсе, — раскладка лунками.
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разбор записи из ДВУХ слов
                // (`display: inline grid-lanes`, CSS Display 3). Полный разбор
                // (внешний `inline` → свои строчные виды) — css-grid 393 → 381:
                // в наборе 233 файла пишут `display: inline grid`, и наша
                // строчная сетка им хуже блочной. Разбор только ради лунок —
                // 393 → 386, обтяжка содержимого у них же — 382. То есть наша
                // раскладка лунками этим 188 файлам пока ХУЖЕ обычного блока;
                // возвращать разбор — вместе с настоящей строчной коробкой.
                self.display = match v {
                    "flex" => Some(Display::Flex),
                    "inline-flex" => Some(Display::InlineFlex),
                    "grid" => Some(Display::Grid),
                    // CSS Grid 3: раскладка ЛУНКАМИ. Элементы идут в самую
                    // короткую лунку, а не в решётку — поэтому это отдельный
                    // вид, а не разновидность сетки.
                    // Строчный вариант ведёт себя в потоке иначе, но лунки
                    // внутри те же: без него контейнер падал в умолчание, и
                    // вся укладка шла столбиком (`*-subgrid-grid-gap-*`).
                    "grid-lanes" | "masonry" => Some(Display::GridLanes),
                    "inline-grid-lanes" | "inline-masonry" => {
                        self.lanes_inline = true;
                        Some(Display::GridLanes)
                    }
                    // Двухсловная строчная сетка: прежний откат (393→381) был
                    // БЕЗ обтяжки InlineGrid — теперь строчная сетка обнимает
                    // Px-дорожки, и разбор снимается с полки (эталоны
                    // subgrid-alignment-* пишут `display: inline grid`).
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разбирать `display: inline
                    // grid-lanes` и `inline masonry` (сейчас запись не
                    // разбирается вовсе — `_ => self.display`, и 150 пар свода
                    // получают обычный блок вместо лунок). Отдавали блочные
                    // лунки, БЕЗ обтяжки `lanes_inline` (её прошлый замер:
                    // grid-семья 573 → 558). Срез из этих 150 пар: 28 зелёных
                    // → 18. Приобретено НОЛЬ, потеряно десять — все
                    // подсеточные и по содержимому (`grid-lanes-subgrid-001b/
                    // c/d` 0.09 → 11.10, `grid-lanes-subgrid-intrinsic-sizing`
                    // 0.28 → 10.27, `column-subgrid-extra-margin-002/004`).
                    // То есть этим 122 красным мешает не отсутствие лунок, а
                    // подсетка и вклад содержимого: настоящий контейнер лунок
                    // им пока ХУЖЕ блока. Возвращать вместе с подсеткой лунок.
                    "inline grid" => Some(Display::InlineGrid),
                    "inline flex" => Some(Display::InlineFlex),
                    "none" => Some(Display::None),
                    "block" => Some(Display::Block),
                    "inline-block" => Some(Display::InlineBlock),
                    "inline" => {
                        // Метка «настоящий строчный»: блокификация под
                        // float/abspos (§9.7) и запрет width/height на
                        // незамещаемом (§10.2) решаются после каскада.
                        self.inline_display = Some(true);
                        Some(Display::InlineBlock)
                    }
                    "inline-grid" => Some(Display::InlineGrid),
                    // Элемент исчезает, дети встают на его место.
                    "contents" => Some(Display::Contents),
                    "list-item" => Some(Display::ListItem),
                    // Табличные роли: своей табличной раскладки у нас нет,
                    // но строка — это ряд, ячейка — блок, а сама таблица
                    // ведёт себя как блок. Это ближе к правде, чем ничего.
                    "table" => Some(Display::Table),
                    // Таблица, стоящая В СТРОКЕ, как inline-block.
                    "inline-table" => Some(Display::InlineTable),
                    "table-row-group" | "table-header-group" | "table-footer-group" => {
                        self.row_group_kind = Some(match v {
                            "table-header-group" => 0,
                            "table-footer-group" => 2,
                            _ => 1,
                        });
                        Some(Display::TableRowGroup)
                    }
                    // run-in решается ПОСЛЕ построения дерева: вбегает
                    // первым строчным в следующий блок или остаётся блоком
                    // (dom::fold_run_ins).
                    "run-in" => {
                        self.run_in = Some(true);
                        Some(Display::Block)
                    }
                    "flow-root" => {
                        // Коробка блочная, но признак не теряется: это
                        // свой контекст форматирования (css-display-3).
                        self.flow_root = Some(true);
                        Some(Display::Block)
                    }
                    "table-row" => Some(Display::TableRow),
                    "table-cell" => Some(Display::TableCell),
                    // Заголовок таблицы — обычный блок. Колонки коробок не
                    // порождают вовсе: они только задают ширину столбцам.
                    "table-caption" => {
                        // Заголовок — блочная коробка с МЕТКОЙ: таблица ищет
                        // его по ней, а не только по тегу caption.
                        self.is_caption = Some(true);
                        Some(Display::Block)
                    }
                    // Колонка коробки НЕ порождает (§17.2.1): `Display`
                    // остаётся `None`. Метка живёт отдельно — по ней узел
                    // переживает разбор дерева, и только по ней его находит
                    // таблица.
                    "table-column" | "table-column-group" => {
                        self.col_role = Some(u8::from(v == "table-column-group"));
                        Some(Display::None)
                    }
                    // Запись из ДВУХ слов: из неё берётся только внутренний
                    // вид «лунки» — его иначе не выразить вовсе. Полный разбор
                    // двух слов ЗАМЕРЕН и откачен (см. комментарий выше).
                    two if two
                        .split_whitespace()
                        .any(|w| w == "grid-lanes" || w == "masonry") =>
                    {
                        if two.split_whitespace().any(|w| w == "inline") {
                            self.lanes_inline = true;
                        }
                        Some(Display::GridLanes)
                    }
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
            "flex-wrap" => {
                self.flex_wrap = Some(v == "wrap" || v == "wrap-reverse");
                // Обратный перенос кладёт строки с другого края: одна строка
                // в контейнере уезжает вниз, а не остаётся вверху.
                self.flex_wrap_reverse = Some(v == "wrap-reverse");
            }
            // Отрицательные значения невалидны (css-flexbox-1 §7.2: «Negative
            // values are not allowed») — объявление отбрасывается целиком
            // (`flex-shrink-002`, `flex-basis-004`).
            "flex-grow" => {
                if let Ok(g) = v.trim().parse::<f32>()
                    && g >= 0.0
                {
                    self.flex_grow = Some(g);
                }
            }
            "flex-shrink" => {
                if let Ok(g) = v.trim().parse::<f32>()
                    && g >= 0.0
                {
                    self.flex_shrink = Some(g);
                }
            }
            // `flex: 1` — сокращение для grow/shrink/basis; берём первое число.
            // `flex: <рост> <сжатие> <основа>` со всеми сокращёнными формами.
            // Раньше бралось только первое число, и `flex: 0 0 200px` терял
            // фиксированную основу — блок начинал растягиваться.
            "flex" => match v {
                "auto" => {
                    self.flex_grow = Some(1.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "none" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(0.0);
                    self.flex_basis = Some(Len::Auto);
                }
                "initial" => {
                    self.flex_grow = Some(0.0);
                    self.flex_shrink = Some(1.0);
                    self.flex_basis = Some(Len::Auto);
                }
                _ => {
                    // Опущенные части сокращения берут НЕ начальные значения
                    // свойств: рост и сжатие становятся 1, а основа — 0%, а не
                    // `auto`. Отсюда весь смысл записи `flex: 1`: элемент
                    // делит место поровну, забыв свою ширину. Раньше основа при
                    // двух числах оставалась `auto`, и `flex: 0 1` держал
                    // ширину элемента вместо нуля.
                    let parts: Vec<&str> = v.split_whitespace().collect();
                    let number = |t: &str| t.parse::<f32>().ok();
                    match parts.as_slice() {
                        [one] => match number(one) {
                            Some(g) => {
                                self.flex_grow = Some(g);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Some(Len::Pct(0.0));
                            }
                            None => {
                                self.flex_grow = Some(1.0);
                                self.flex_shrink = Some(1.0);
                                self.flex_basis = Len::parse(one);
                            }
                        },
                        [a, b] => {
                            self.flex_grow = number(a);
                            match number(b) {
                                Some(shrink) => {
                                    self.flex_shrink = Some(shrink);
                                    self.flex_basis = Some(Len::Pct(0.0));
                                }
                                None => {
                                    self.flex_shrink = Some(1.0);
                                    self.flex_basis = Len::parse(b);
                                }
                            }
                        }
                        [a, b, c] => {
                            // Безразмерная основа кроме нуля делает ВСЁ
                            // объявление невалидным (`flex: 0 0 4` не
                            // применяется вовсе, flexbox_flex-*-unitless-basis).
                            if number(c).is_some_and(|n| n != 0.0) {
                                return;
                            }
                            self.flex_grow = number(a);
                            self.flex_shrink = number(b);
                            self.flex_basis = Len::parse(c);
                        }
                        _ => {}
                    }
                }
            },
            "flex-basis" if v == "content" => {
                self.flex_basis = Some(Len::Auto);
                self.basis_content = Some(true);
            }
            "flex-basis" => {
                if let Some(l) = Len::parse(v)
                    && !matches!(l, Len::Px(x) | Len::Pct(x) if x < 0.0)
                {
                    self.flex_basis = Some(l);
                }
            }
            "align-self" => {
                if let Ok(a) = align_keyword(v) {
                    self.align_self = a;
                    self.align_self_safe = is_safe(v);
                }
            }
            "align-items" => {
                if let Ok(a) = align_keyword(v) {
                    self.align_items = a;
                    self.align_items_safe = is_safe(v);
                }
            }
            // `space-evenly` и `space-around` различаются шириной крайних
            // промежутков — сведение их в одно значение расходилось с
            // браузером на 27 точек (поймано сравнением).
            "justify-content" => {
                self.justify_content = parse_justify(v);
                self.justify_content_safe = is_safe(v);
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
            // Одно свойство служит двум раскладкам: в сетке и гибкой строке
            // это зазор между ячейками, в многоколоночном потоке — между
            // колонками. Пишем в оба поля, читает нужное та раскладка, которая
            // включена.
            "column-gap" => {
                self.gap = Some((self.gap.and_then(|g| g.0), Len::parse(v)));
                self.column_gap = Len::parse(v);
            }
            // `repeat(auto-fill | auto-fit, minmax(N, 1fr))` — «сколько
            // влезет»: число колонок известно только раскладке. Раньше запись
            // не разбиралась вовсе, и вся сетка схлопывалась в одну колонку.
            //
            // СДЕЛАНО (прежний откат снят): списку СЛОЖНЕЕ одинокого повтора
            // (`10px repeat(auto-fill, 30px) 50px`) пишется и `grid_tracks`,
            // и `grid_cols`, а сам повтор внутри непустого списка
            // разворачивает раскладка лунок. Замерено по css-grid (1133 пары)
            // 645 -> 648 и по всему css3 2417 -> 2420: приобретено 3
            // (`grid-auto-repeat-multiple-values-002/003`,
            // `row-auto-repeat-013`), потеряно 0.
            //
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: писать список ВСЕГДА, в том числе для
            // одинокого повтора. По css-grid 645 -> 604: приобретено 6,
            // потеряно 47 (`column-auto-repeat-001/013/017/018/027..030`,
            // `-auto-001/011..014/025/026`, `-fit-content-004/005`,
            // `-max-content-001/002` и далее). Записанный список уводит
            // одинокий повтор с прежнего пути раскладки, а тот считает число
            // повторов точнее: по долям, по содержимому и по `fit-content`.
            // Отсюда условие `l.len() > 1` ниже — оно не заплатка, а граница
            // между двумя честными путями счёта повторов.
            "grid-template-columns" if v.contains("auto-fill") || v.contains("auto-fit") => {
                self.grid_auto_fill_min = auto_fill_min(v);
                self.grid_auto_fill_tracks = auto_fill_tracks(v);
                // Список пишется и при авто-повторе: дорожки ДО и ПОСЛЕ него
                // (`max-content repeat(auto-fill, max-content) max-content`)
                // иначе теряются целиком. Разворот самого повтора при
                // непустом списке делает раскладка лунок — это и есть
                // условие возврата из прежнего отката.
                if let Some(list) = parse_tracks(v).filter(|l| l.len() > 1) {
                    self.grid_cols = count_tracks(v);
                    self.grid_tracks = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_cols = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_min,
                    track_pct: auto_fill_pct(v),
                    intrinsic: auto_fill_intrinsic(v),
                    intrinsic_min: auto_fill_intrinsic(v) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v),
                    max_auto,
                    max_fr,
                });
            }
            // То же по РЯДАМ: у раскладки лунками дорожки задают ряды, когда
            // `grid-lanes-direction: row` (`row-auto-repeat-001`).
            "grid-template-rows" if v.contains("auto-fill") || v.contains("auto-fit") => {
                self.grid_auto_fill_row = auto_fill_min(v);
                // Только когда вокруг повтора ЕСТЬ свои дорожки: одинокий
                // повтор целиком ведёт прежний путь раскладки, он считает
                // число повторов точнее (доли, содержимое, `fit-content`).
                if let Some(list) = parse_tracks(v).filter(|l| l.len() > 1) {
                    self.grid_rows = Some(list);
                }
                let (max_auto, max_fr) = auto_fill_max(v);
                self.auto_repeat_rows = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_row,
                    track_pct: auto_fill_pct(v),
                    intrinsic: auto_fill_intrinsic(v),
                    intrinsic_min: auto_fill_intrinsic(v) && v.contains("min-content"),
                    fit_px: auto_fill_fit_px(v),
                    max_auto,
                    max_fr,
                });
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: ИМЕНА ЛИНИЙ целиком (план — в
            // `target/scout-linenames.md`, шаги A1-A3). Написано и работало:
            // разбор имён списка дорожек (`[a] 50px 50px [a] 50px 50px [a]` →
            // `[["a"],[],["a"],[],["a"]]`, с раскрытием счётного `repeat`),
            // разбор именованной грани (`span a`, `a -1`, голое имя с поиском
            // `имя-start`/`имя-end`), поле `NamedEdge` рядом с `Placement`
            // (чтобы тот остался `Copy`), разрешитель имён в номера линий с
            // неявными именами от `grid-template-areas`, вызванный и для
            // классической сетки, и для лунок. Печатью подтверждено, что
            // разрешитель ДОХОДИТ до контейнера с именами и разрешает грани.
            // Срез css-grid (1133 пары, 646 зелёных): 646 — ноль приобретено,
            // ноль потеряно; поимённо не сдвинулась НИ ОДНА из 33 пар с
            // именами линий, а `grid-lanes-grid-placement-named-lines-001/002`
            // ушли 16.30 → 17.92 и 14.11 → 14.54.
            // Значит эти пары держат не имена: `column-line-names-011` —
            // субсетка (шаг B), `-016` — имена внутри `repeat(auto-fill, …)`
            // (шаг C), а `-003` при верно разрешённых гранях (span a / a -1 →
            // линии 3..5) остаётся на 0.52. Возвращать вместе с шагами B и C.
            "grid-template-columns" => {
                self.subgrid |= v.contains("subgrid");
                self.grid_cols = count_tracks(v);
                self.grid_tracks = parse_tracks(v);
            }
            // `grid: <ряды> / <колонки>` и `grid-template: <ряды> / <колонки>`
            // — самая частая короткая запись сетки в тестах и в вёрстке.
            // Формы с `auto-flow` описывают неявные дорожки: там сторона со
            // словом задаёт направление автопотока, а вторая — шаблон.
            "grid" | "grid-template" => {
                self.subgrid |= v.contains("subgrid");
                let (rows, cols) = split_slash(v);
                match (rows.contains("auto-flow"), cols.contains("auto-flow")) {
                    (true, _) => {
                        self.grid_auto_flow = Some(if rows.contains("dense") {
                            AutoFlow::RowDense
                        } else {
                            AutoFlow::Row
                        });
                        self.apply_one("grid-auto-rows", strip_auto_flow(rows));
                        self.apply_one("grid-template-columns", cols);
                    }
                    (_, true) => {
                        self.grid_auto_flow = Some(if cols.contains("dense") {
                            AutoFlow::ColDense
                        } else {
                            AutoFlow::Col
                        });
                        self.apply_one("grid-template-rows", rows);
                        self.apply_one("grid-auto-columns", strip_auto_flow(cols));
                    }
                    _ => {
                        self.apply_one("grid-template-rows", rows);
                        if !cols.is_empty() {
                            self.apply_one("grid-template-columns", cols);
                        }
                    }
                }
            }

            // Отрицательная длина делает объявление размера невалидным
            // (CSS 2.1 §10): `max-height: -1px` доезжал до раскладки и
            // схлопывал коробку в ноль. У `min-*` отрицательное поднимает
            // сама раскладка, но объявление всё равно отбрасывается.
            "width" => {
                self.width_inherit = v == "inherit";
                assign_size(&mut self.width, v);
            }
            "height" => {
                self.height_inherit = v == "inherit";
                assign_size(&mut self.height, v);
            }
            // Пределы не наследуются, но `inherit` берёт значение родителя
            // явно (§6.2.1). Без этой ветки `assign_size` стирал слот в
            // `None`, и `max-height: inherit` снимал предел вовсе.
            "min-width" => {
                self.minmax_inherit[0] = v == "inherit";
                assign_size(&mut self.min_width, v);
            }
            "min-height" => {
                self.minmax_inherit[1] = v == "inherit";
                assign_size(&mut self.min_height, v);
            }
            "max-width" => {
                self.minmax_inherit[2] = v == "inherit";
                assign_size(&mut self.max_width, v);
            }
            "max-height" => {
                self.minmax_inherit[3] = v == "inherit";
                assign_size(&mut self.max_height, v);
            }

            "padding" => {
                if v == "inherit" {
                    self.padding_inherit = true;
                    return;
                }
                let parsed = Sides::shorthand(v);
                let neg = |l: &Option<Len>| {
                    matches!(l, Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if *n < 0.0)
                };
                if neg(&parsed.top)
                    || neg(&parsed.right)
                    || neg(&parsed.bottom)
                    || neg(&parsed.left)
                {
                    return;
                }
                self.padding = parsed;
                // Спор с логическими сторонами решает порядок объявлений.
                self.side_seq.padding = [self.decl_seq; 4];
            }
            "padding-top" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[0] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.top = Len::parse(v);
                self.side_seq.padding[0] = self.decl_seq;
            }
            "padding-right" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[1] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.right = Len::parse(v);
                self.side_seq.padding[1] = self.decl_seq;
            }
            "padding-bottom" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[2] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.bottom = Len::parse(v);
                self.side_seq.padding[2] = self.decl_seq;
            }
            "padding-left" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[3] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.left = Len::parse(v);
                self.side_seq.padding[3] = self.decl_seq;
            }
            // Физическая запись ГАСИТ логический слот той же стороны: разбор
            // идёт в порядке каскада, и авторский `margin: 0` обязан бить
            // более ранний `margin-block` таблицы агента — а разрешение
            // логических идёт после каскада и иначе перекрывало бы всё.
            // Соответствие сторон берётся горизонтальное: письмо на разборе
            // ещё неизвестно, а гасят почти всегда сбросом всех сторон.
            "margin" => {
                if v == "inherit" {
                    self.margin_inherit = [true; 4];
                    return;
                }
                self.margin = Sides::shorthand(v);
                self.side_seq.margin = [self.decl_seq; 4];
            }
            "margin-top" => {
                if v == "inherit" {
                    self.margin_inherit[0] = true;
                    return;
                }
                self.margin.top = Len::parse(v);
                self.side_seq.margin[0] = self.decl_seq;
            }
            "margin-right" => {
                if v == "inherit" {
                    self.margin_inherit[1] = true;
                    return;
                }
                self.margin.right = Len::parse(v);
                self.side_seq.margin[1] = self.decl_seq;
            }
            "margin-bottom" => {
                if v == "inherit" {
                    self.margin_inherit[2] = true;
                    return;
                }
                self.margin.bottom = Len::parse(v);
                self.side_seq.margin[2] = self.decl_seq;
            }
            "margin-left" => {
                if v == "inherit" {
                    self.margin_inherit[3] = true;
                    return;
                }
                self.margin.left = Len::parse(v);
                self.side_seq.margin[3] = self.decl_seq;
            }

            "border" => {
                // `border: inherit` — рамка родителя целиком: слово копирует
                // вычисленное значение, самим разбором его не выразить.
                if v == "inherit" {
                    self.border_inherit = true;
                    self.border_inherit_w = [true; 4];
                    self.border_inherit_s = [true; 4];
                    self.border_inherit_c = [true; 4];
                    return;
                }
                self.apply_border_shorthand(v, None)
            }
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                let i = match key {
                    "border-top" => 0,
                    "border-right" => 1,
                    "border-bottom" => 2,
                    _ => 3,
                };
                // `border-bottom: inherit` — все три части ОДНОЙ стороны.
                if v == "inherit" {
                    self.border_inherit_w[i] = true;
                    self.border_inherit_s[i] = true;
                    self.border_inherit_c[i] = true;
                    return;
                }
                self.apply_border_shorthand(v, Some(i))
            }
            "border-width" if v == "inherit" => self.border_inherit_w = [true; 4],
            "border-style" if v == "inherit" => self.border_inherit_s = [true; 4],
            "border-color" if v == "inherit" => self.border_inherit_c = [true; 4],
            "border-top-width"
            | "border-right-width"
            | "border-bottom-width"
            | "border-left-width"
                if v == "inherit" =>
            {
                self.border_inherit_w[side_index(key)] = true;
            }
            "border-top-style"
            | "border-right-style"
            | "border-bottom-style"
            | "border-left-style"
                if v == "inherit" =>
            {
                self.border_inherit_s[side_index(key)] = true;
            }
            "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
                if v == "inherit" =>
            {
                self.border_inherit_c[side_index(key)] = true;
            }
            "border-width" => {
                // Толщина словом (`thin`/`medium`/`thick`, §8.5.1) до сюда не
                // доезжала: общее сокращение по сторонам знает только длины, и
                // запись `border-width: thin medium medium medium` стирала
                // толщину на всех сторонах — рамка пропадала целиком.
                let list: Vec<Option<Len>> = v.split_whitespace().map(line_width).collect();
                // Недействительное значение делает НЕВАЛИДНЫМ всё объявление
                // (§4.2), а не одну сторону: иначе опечатка гасила рамку.
                if list.is_empty() || list.len() > 4 || list.iter().any(Option::is_none) {
                    return;
                }
                let at = |i: usize| -> Option<Len> {
                    let pick = match (list.len(), i) {
                        (1, _) => 0,
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i,
                    };
                    list[pick]
                };
                self.border_width = Sides {
                    top: at(0),
                    right: at(1),
                    bottom: at(2),
                    left: at(3),
                };
            }
            "border-collapse" => self.border_collapse = Some(v == "collapse"),
            "empty-cells" => self.empty_cells_hide = Some(v.trim() == "hide"),
            "border-color" => {
                // От одного до четырёх значений, как у всякого сокращения по
                // сторонам (§8.5.2). Прежде строка разбиралась целиком, и
                // запись `border-color: red orange red yellow` не давала
                // НИЧЕГО: цвет пропадал на всех сторонах разом.
                let list: Vec<&str> = v.split_whitespace().collect();
                if list.is_empty() || list.len() > 4 {
                    return;
                }
                if list.len() == 1 {
                    if v.eq_ignore_ascii_case("currentcolor") {
                        self.border_color_is_current = true;
                    } else {
                        self.border_color = Color::parse(v);
                    }
                    return;
                }
                let colors: Vec<Option<Color>> =
                    list.iter().map(|t| side_color(t, self.color)).collect();
                // Недействительное значение делает НЕВАЛИДНЫМ всё объявление
                // (§4.2), а не одну сторону.
                if colors.iter().any(Option::is_none) {
                    return;
                }
                let at = |i: usize| -> Option<Color> {
                    let pick = match (colors.len(), i) {
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i,
                    };
                    colors[pick]
                };
                for i in 0..4 {
                    self.border_colors[i] = at(i);
                }
                // Общий цвет остаётся у верхней стороны: его читают пути, не
                // знающие о сторонах.
                self.border_color = at(0);
            }
            "border-radius" => {
                // Эллиптические радиусы: `H / V` (css-backgrounds-3 §5.1) —
                // углы с rx≠ry не выразить круглым скруглением растеризатора,
                // форма уходит альфа-маской буфера группы.
                if let Some((hs, vs)) = v.split_once('/') {
                    let h = radius_shorthand(hs.trim());
                    let vv = radius_shorthand(vs.trim());
                    self.radius = h;
                    let p = |a: Option<Len>, b: Option<Len>| match (a, b) {
                        (Some(Len::Px(x)), Some(Len::Px(y))) if (x - y).abs() > 0.01 => {
                            Some((x, y))
                        }
                        _ => None,
                    };
                    let ell = [
                        p(h.tl, vv.tl),
                        p(h.tr, vv.tr),
                        p(h.br, vv.br),
                        p(h.bl, vv.bl),
                    ];
                    if ell.iter().any(|c| c.is_some()) {
                        self.radius_ell = Some(ell);
                    }
                } else {
                    self.radius = radius_shorthand(v);
                }
            }
            "border-top-left-radius"
            | "border-top-right-radius"
            | "border-bottom-right-radius"
            | "border-bottom-left-radius" => {
                // Двухзначный лонгхенд — эллиптический угол `rx ry`.
                let mut it = v.split_whitespace();
                let x = it.next().and_then(Len::parse);
                let y = it.next().and_then(Len::parse);
                let slot = match key {
                    "border-top-left-radius" => 0,
                    "border-top-right-radius" => 1,
                    "border-bottom-right-radius" => 2,
                    _ => 3,
                };
                match slot {
                    0 => self.radius.tl = x,
                    1 => self.radius.tr = x,
                    2 => self.radius.br = x,
                    _ => self.radius.bl = x,
                }
                if let (Some(Len::Px(rx)), Some(Len::Px(ry))) = (x, y)
                    && (rx - ry).abs() > 0.01
                {
                    let mut ell = self.radius_ell.unwrap_or([None; 4]);
                    ell[slot] = Some((rx, ry));
                    self.radius_ell = Some(ell);
                }
            }

            "position" => {
                self.position = match v {
                    "absolute" => Some(Position::Absolute),
                    "relative" => Some(Position::Relative),
                    "static" => Some(Position::Static),
                    // `fixed` отсчитывается от окна: своей системы отсчёта у
                    // него нет, и сборщик дерева ставит его отдельным слоем.
                    "fixed" => Some(Position::Fixed),
                    "sticky" | "-webkit-sticky" => Some(Position::Sticky),
                    _ => self.position,
                }
            }
            "top" => {
                self.inset_inherit[0] = v == "inherit";
                self.inset.top = Len::parse(v);
                self.side_seq.inset[0] = self.decl_seq;
            }
            "right" => {
                self.inset_inherit[1] = v == "inherit";
                self.inset.right = Len::parse(v);
                self.side_seq.inset[1] = self.decl_seq;
            }
            "bottom" => {
                self.inset_inherit[2] = v == "inherit";
                self.inset.bottom = Len::parse(v);
                self.side_seq.inset[2] = self.decl_seq;
            }
            "left" => {
                self.inset_inherit[3] = v == "inherit";
                self.inset.left = Len::parse(v);
                self.side_seq.inset[3] = self.decl_seq;
            }
            "inset" => {
                self.inset = Sides::shorthand(v);
                self.side_seq.inset = [self.decl_seq; 4];
            }
            "overflow" => {
                // Запись из двух слов — оси по отдельности
                // (css-overflow-3 §3): `overflow: clip visible`.
                let mut it = v.split_whitespace();
                let x = it.next().and_then(parse_overflow);
                let y = it.next().and_then(parse_overflow).or(x);
                self.overflow_x = x;
                self.overflow_y = y;
            }
            "overflow-x" => self.overflow_x = parse_overflow(v),
            "overflow-y" => self.overflow_y = parse_overflow(v),
            "opacity" => self.opacity = v.parse().ok(),
            // Поле обрезки: край, по которому режется вылезшее содержимое,
            // отодвигается наружу (css-overflow-3 §5). Запись допускает и
            // указание коробки отсчёта — её мы не различаем, край один.
            "overflow-clip-margin" => {
                // `<visual-box> || <length>`: коробка отсчёта и поле, в любом
                // порядке, любая часть может отсутствовать (умолчание —
                // padding-box, поле 0).
                let mut margin = None;
                let mut bx = None;
                for w in v.split_whitespace() {
                    match w {
                        "border-box" => bx = Some(2u8),
                        "padding-box" => bx = Some(1),
                        "content-box" => bx = Some(0),
                        t => {
                            if let Some(Len::Px(px)) = Len::parse(t)
                                && px >= 0.0
                            {
                                margin = Some(px);
                            }
                        }
                    }
                }
                if margin.is_some() || bx.is_some() {
                    self.clip_margin = Some(margin.unwrap_or(0.0));
                    self.clip_margin_box = bx;
                }
            }

            // Сокращение несёт всё сразу: `background: #fff url(a.png) no-repeat`
            // — и цвет, и картинку, и режим повтора. Раньше побеждало что-то
            // одно, и картинка терялась при заданном цвете.
            "background" | "background-color" => {
                if v == "inherit" {
                    self.background_inherit = true;
                    // Сокращение наследует ВЕСЬ фон, а не только цвет
                    // (css-backgrounds-3 §2.1): картинку, повтор, положение и
                    // размер. Флага два, потому что `background-color:
                    // inherit` чужую картинку тащить не должен.
                    self.background_all_inherit = key == "background";
                    return;
                }
                // Сокращение СБРАСЫВАЕТ все свои длинные свойства
                // (css-backgrounds-3 §2.1): `background-color: red;
                // background: bottom fixed` оставляет фон ПРОЗРАЧНЫМ, а
                // прежде красный переживал сокращение. Сбрасывает только
                // `background`; `background-color` трогает лишь цвет.
                // Сокращение СБРАСЫВАЕТ свои длинные свойства
                // (css-backgrounds-3 §2.1): `background-color: red;
                // background: bottom fixed` оставляет фон ПРОЗРАЧНЫМ.
                // Сбрасывается только ЦВЕТ: остальные части разбор ниже берёт
                // из записи не полностью, и полный сброс терял то, чего он не
                // умеет прочесть обратно (замерено: девять пар
                // `textarea-pre-wrap-*` уходили 0.00 → 0.76).
                // Негодное объявление не сбрасывает ничего (§4.1.7).
                if key == "background" && background_shorthand_valid(v) {
                    self.background = None;
                    self.background_rcs = None;
                }
                if key == "background-color" || background_shorthand_valid(v) {
                    self.bg_explicit = true;
                }
                // Цвет от `currentColor` решается не здесь: цвет элемента
                // известен только после каскада, а запись наследуется
                // нерешённой — и голое слово, и относительная функция, и
                // `color-mix` с ним (css-color-4 §7.1, css-color-5 §4.1).
                let low = v.to_ascii_lowercase();
                if !v.contains("gradient(")
                    && (low == "currentcolor"
                        || low.contains("(from ")
                        || (low.starts_with("color-mix(") && low.contains("currentcolor")))
                {
                    self.background_rcs = Some(v.to_string());
                    return;
                }
                // Фон — СПИСОК слоёв через запятую (css-backgrounds-3 §3.10):
                // первый рисуется ПОВЕРХ остальных, цвет разрешён только
                // последнему. Рисуем верхний слой и цвет нижнего: своего места
                // под остальные у нас пока нет, но терять их молча нельзя —
                // список целиком уходил в разбор ОДНОГО слоя, тот его не
                // понимал, и фон пропадал весь (`background-attachment-margin-
                // root-001`: страница выходила пустой).
                let layers = background_layers(v);
                let top = layers.first().copied().unwrap_or(v);
                let bottom = layers.last().copied().unwrap_or(v);
                if top.starts_with("linear-gradient(") || top.starts_with("radial-gradient(") {
                    self.gradient = parse_gradient(top);
                    // Цвет ищется в НИЖНЕМ слое (он один его допускает);
                    // при единственном слое нижний == верхний, и цвет стоит
                    // там же, рядом с градиентом: `background: linear-… green`.
                    for token in split_outside_parens(bottom) {
                        if token.contains('(') {
                            continue;
                        }
                        if let Some(c) = Color::parse(&token) {
                            self.background = Some(c);
                        }
                    }
                    return;
                }
                let v = top;
                if let Some(url) = parse_url(v) {
                    self.bg_image = Some(url);
                }
                // Значение режется по пробелам ВНЕ скобок: иначе
                // `rgba(0, 0, 0, .5)` распадался на куски, ни один из которых
                // не цвет, и фон терялся целиком — самая частая запись
                // полупрозрачной подложки.
                // Слова и длины ПОЛОЖЕНИЯ копятся отдельно и решаются одним
                // разбором: ключевое слово несёт СВОЮ ось (css-backgrounds-3
                // §3.6), поэтому `bottom repeat-x` и `repeat-x bottom` — одно
                // и то же. Разбор тот же, что у отдельного свойства, иначе
                // тест и эталон разойдутся механикой, а не раскладкой.
                let mut pos: Vec<String> = vec![];
                for token in split_outside_parens(v) {
                    match token.as_str() {
                        "no-repeat" => self.bg_repeat = Some(BgRepeat::NoRepeat),
                        "repeat-x" => self.bg_repeat = Some(BgRepeat::RepeatX),
                        "repeat-y" => self.bg_repeat = Some(BgRepeat::RepeatY),
                        "repeat" => self.bg_repeat = Some(BgRepeat::Repeat),
                        "cover" => self.bg_size = BgSize::Cover,
                        "contain" => self.bg_size = BgSize::Contain,
                        // Привязка и коробки положением НЕ являются: слово
                        // съедается здесь, иначе уедет в разбор положения.
                        "scroll" | "fixed" | "local" | "border-box" | "padding-box"
                        | "content-box" => {}
                        t if t.starts_with("url(") => {}
                        // Ключевые слова осей и длины — это положение.
                        "left" | "right" | "top" | "bottom" | "center" => pos.push(token.clone()),
                        t if Len::parse(t).is_some() => pos.push(token.clone()),
                        t => {
                            if let Some(c) = Color::parse(t) {
                                self.background = Some(c);
                            }
                        }
                    }
                }
                // Положение ставится ТОЛЬКО когда слово о нём в записи есть:
                // разбор пустой строки отдаёт «по центру», и каждый фон без
                // положения уехал бы в середину коробки.
                if !pos.is_empty() {
                    self.bg_pos = parse_pos_words(&pos.join(" "));
                }
                // Цвет живёт в НИЖНЕМ слое списка — верхний его не допускает.
                if layers.len() > 1 {
                    for token in split_outside_parens(bottom) {
                        if let Some(c) = Color::parse(&token) {
                            self.background = Some(c);
                        }
                    }
                }
            }
            "box-shadow" => {
                if v == "inherit" {
                    self.shadow_inherit = true;
                    return;
                }
                // `inset` в записи означает тень ВНУТРИ фигуры: раньше такая
                // запись просто не рисовалась.
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.shadows = parse_shadows(&outer.join(","));
                self.inset_shadows = parse_shadows(&inset.join(",").replace("inset", " "));
            }

            // Неразборный цвет делает объявление недействительным (§4.2):
            // прежнее значение живёт, а не сменяется умолчанием. Пустой слот
            // у нас и означает «взять у родителя», поэтому `inherit` его
            // очищает (`color-174`).
            "color" => {
                self.color = if v == "inherit" {
                    None
                } else {
                    Color::parse(v).or(self.color)
                }
            }
            "font-size" => {
                // Отрицательный кегль и неразборная запись делают объявление
                // НЕВАЛИДНЫМ (§4.2, §15.7): прежнее значение остаётся, а не
                // стирается в `None` (`c526-font-sz-003`: `-0.5in`).
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                // Слово (`medium`, `larger`) как и прежде даёт `None`: его
                // разбирают другие пути. Держится только отрицательная длина.
                self.font_size = match Len::parse(v) {
                    Some(l) if neg(&l) => self.font_size,
                    other => other,
                };
            }
            "font-weight" => {
                self.font_weight = match v {
                    "bold" | "bolder" => Some(700),
                    "normal" => Some(400),
                    n => n.parse().ok(),
                }
            }
            "font-style" => self.italic = Some(v == "italic" || v == "oblique"),
            "font-family" => {
                // Имя семейства — либо строка в кавычках, либо ряд
                // ИДЕНТИФИКАТОРОВ (§15.3). Неверное имя делает объявление
                // недействительным целиком (§4.2): прежде разбор просто
                // пропускал негодное имя и брал следующее из списка, из-за
                // чего `font-family: 1Ahem, Ahem` набиралось шрифтом Ahem.
                if !v.split(',').all(|part| family_name_ok(part.trim())) {
                    return;
                }
                let lower = v.to_ascii_lowercase();
                // Моноширинный запрос несёт смысл (код) и решает выбор
                // встроенного шрифта, если названного в системе нет.
                self.monospace = Some(lower.contains("mono") || lower.contains("courier"));
                // Родовое имя — не пустое место: браузер подставляет за него
                // конкретный системный шрифт, и без подстановки разметка
                // набиралась умолчанием движка, шире браузерного. Берётся то
                // же семейство, что подставляет Chrome на этой системе.
                let generic = v
                    .split(',')
                    .map(|f| f.trim().trim_matches(is_quote).to_ascii_lowercase())
                    .find_map(|f| generic_family(&f));
                // Первое НЕ родовое имя списка уходит в шрифт как есть:
                // подстановкой недостающего занимается сама система шрифтов.
                // Имя нормализуется до сравнения: неквотированное имя из
                // нескольких слов — это один пробел между ними (§15.3).
                let norm = |f: &str| {
                    let un = crate::css::unescape(f);
                    un.split_whitespace().collect::<Vec<_>>().join(" ")
                };
                let usable = |f: &str| {
                    let lower = f.to_ascii_lowercase();
                    !f.is_empty()
                        && !is_generic(&lower)
                        && !matches!(lower.as_str(), "inherit" | "initial")
                };
                // Первое УСТАНОВЛЕННОЕ имя списка: браузер идёт по списку, пока
                // не найдёт шрифт (§15.3). Прежде бралось первое подходящее по
                // виду, и `font-family: Courier New, Ahem` при отсутствующем
                // `Courier New` набиралось подменой вместо `Ahem`.
                let installed = v
                    .split(',')
                    .map(|f| norm(f.trim().trim_matches(is_quote)))
                    .find(|f| usable(f) && crate::metrics::font_installed(f));
                if let Some(found) = installed {
                    self.font_family = Some(found);
                    return;
                }
                self.font_family = v
                    .split(',')
                    .map(|f| f.trim().trim_matches(is_quote))
                    .find(|f| {
                        let lower = f.to_ascii_lowercase();
                        !f.is_empty()
                            && !is_generic(&lower)
                            && !matches!(lower.as_str(), "inherit" | "initial")
                    })
                    // Неквотированное имя из нескольких слов НОРМАЛИЗУЕТСЯ:
                    // последовательность пробельных знаков (включая переводы
                    // строк) — это один пробел (`Courier   New` == `Courier
                    // New`, CSS2 §15.3; font-family-013 и родня). Экранирование
                    // раскрывается как в любом идентификаторе.
                    .map(|f| {
                        let un = crate::css::unescape(f);
                        un.split_whitespace().collect::<Vec<_>>().join(" ")
                    })
                    .or_else(|| generic.map(str::to_string));
            }
            "text-decoration" | "text-decoration-line" => {
                // Недействительный токен делает объявление НЕВАЛИДНЫМ целиком
                // (§4.2): прежде свойство искалось подстрокой, и
                // `text-decoration: diagonal` проезжало как «нет подчёркивания»
                // вместо того, чтобы оставить прежнее значение
                // (`c71-fwd-parsing-003`).
                let line = |t: &str| {
                    matches!(
                        t,
                        "none"
                            | "underline"
                            | "overline"
                            | "line-through"
                            | "blink"
                            | "spelling-error"
                            | "grammar-error"
                    )
                };
                // У сокращения к линиям добавляются рисунок, толщина и цвет.
                let extra = |t: &str| {
                    key == "text-decoration"
                        && (matches!(
                            t,
                            "solid"
                                | "double"
                                | "dotted"
                                | "dashed"
                                | "wavy"
                                | "auto"
                                | "from-font"
                        ) || Len::parse(t).is_some()
                            || Color::parse(t).is_some())
                };
                let lower = v.to_ascii_lowercase();
                let mut words = lower.split_whitespace().peekable();
                if words.peek().is_none() {
                    return;
                }
                if !lower.split_whitespace().all(|t| line(t) || extra(t)) {
                    return;
                }
                self.underline = Some(lower.split_whitespace().any(|t| t == "underline"));
                self.line_through = Some(lower.split_whitespace().any(|t| t == "line-through"));
            }
            "line-height" => {
                // Голое число в line-height — множитель, а не пиксели, и
                // наследуется оно множителем: у потомка своя высота строки.
                //
                // Доля — наоборот: §10.8.1 «Computed value: for <length> and
                // <percentage> the absolute value», то есть `200%` считается
                // от СВОЕГО кегля и наследуется уже точками. У нас обе записи
                // давали `Len::Pct`, доля доживала до потомка и множилась на
                // его кегль (`c548-ln-ht-003` против зелёной `-004` — та же
                // разметка, разная запись). `Len::Em` сводится к точкам до
                // наследования, поэтому доля тегируется им.
                let parsed = match v.parse::<f32>() {
                    Ok(mult) if !v.ends_with("px") => Some(Len::Pct(mult)),
                    _ => match Len::parse(v) {
                        Some(Len::Pct(k)) if v.trim_end().ends_with('%') => Some(Len::Em(k)),
                        other => other,
                    },
                };
                // Отрицательная высота строки недействительна (§10.8.1):
                // объявление отбрасывается целиком, прежнее значение живёт.
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                self.line_height = match parsed {
                    Some(l) if neg(&l) => self.line_height,
                    other => other,
                };
            }
            // `text-justify: none` запрещает выключку целиком: строка с
            // `text-align: justify` прижимается к началу, как `start`
            // (css-text-3 §7.3). Прочие значения различают, ЧТО растягивать —
            // пробелы или знаки; у нас растягиваются пробелы, и это поведение
            // `auto`/`inter-word`.
            "text-justify" => {
                self.no_justify = match v {
                    "none" => Some(true),
                    "auto" | "inter-word" | "inter-character" | "distribute" => Some(false),
                    _ => self.no_justify,
                };
            }
            "text-align" => {
                self.text_align = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" | "justify-all" => Some(TextAlign::Justify),
                    _ => self.text_align,
                };
                // `justify-all` — это выключка ВМЕСТЕ с последней строкой:
                // сокращение от `text-align: justify` + `text-align-last:
                // justify`.
                if v == "justify-all" {
                    self.text_align_last = Some(TextAlign::Justify);
                }
            }
            "text-align-last" => {
                self.text_align_last = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" => Some(TextAlign::Justify),
                    _ => self.text_align_last,
                }
            }
            // `pre` сохраняет переводы строк — это не то же самое, что запрет
            // переноса: раньше `pre` помечался как `nowrap`, и текст склеивался
            // в одну строку.
            "cursor" => self.cursor = Some(v.to_string()),
            // Заливка SVG-геометрии: свойство презентации доезжает до
            // разметки при растеризации (SVG 2 §presentation attributes).
            "fill" => self.svg_fill = Some(v.to_string()),
            "caption-side" => self.caption_bottom = Some(v.eq_ignore_ascii_case("bottom")),
            "visibility" => {
                self.hidden = Some(v == "hidden" || v == "collapse");
                self.collapsed = Some(v == "collapse");
            }
            "letter-spacing" => self.letter_spacing = Len::parse_spacing(v),
            "text-overflow" => {
                // css-overflow-4 §5: clip | ellipsis | <строка>, до двух
                // сторон. Наша обрезка — конец строки: берётся последнее
                // не-clip значение.
                let mut on = false;
                let mut marker = None;
                // Резка по пробелам ВНЕ кавычек: маркер-строка может
                // нести пробел; метка '\u{0}' отличает строку от ключевого слова.
                let mut tokens: Vec<String> = vec![];
                let mut cur = String::new();
                let mut quote: Option<char> = None;
                for ch in v.chars() {
                    match quote {
                        Some(q) if ch == q => quote = None,
                        Some(_) => cur.push(ch),
                        None if ch == '"' || ch == '\'' => {
                            quote = Some(ch);
                            if cur.is_empty() {
                                cur.push('\u{0}');
                            }
                        }
                        None if ch.is_whitespace() => {
                            if !cur.is_empty() {
                                tokens.push(std::mem::take(&mut cur));
                            }
                        }
                        None => cur.push(ch),
                    }
                }
                if !cur.is_empty() {
                    tokens.push(cur);
                }
                for t in tokens {
                    if let Some(text) = t.strip_prefix('\u{0}') {
                        on = true;
                        // Строка объявления несёт экранирование (css-syntax
                        // §4.3.7: `\0A` — перевод строки, `\2026` — многоточие),
                        // а разрывы сегмента в ней преобразуются, как в тексте
                        // (css-text-3 §4.1.2; Blink `line_truncator.cc`
                        // `SuppressLineBreaks`): ряд принудительных разрывов —
                        // один пробел (`text-overflow-string-009…016`).
                        marker = Some(collapse_segment_breaks(&unescape_content(text)));
                    } else if t == "ellipsis" {
                        on = true;
                        marker = None;
                    }
                }
                self.ellipsis = Some(on);
                self.overflow_marker = marker;
            }
            "list-style-position" => {
                self.list_style_inside = Some(v.trim() == "inside");
            }
            "list-style" | "list-style-type" => {
                // Сокращение задаёт ВСЕ составляющие: не названное в нём
                // размещение возвращается к начальному `outside`
                // (css-lists-3 §4). Долгая форма чужого значения не трогает.
                if key == "list-style" {
                    self.list_style_inside = Some(false);
                    for token in v.split_whitespace() {
                        match token {
                            "inside" => self.list_style_inside = Some(true),
                            "outside" => self.list_style_inside = Some(false),
                            _ => {}
                        }
                    }
                }
                self.no_marker = Some(v.contains("none"));
                // Строковый маркер: значение в кавычках берётся дословно,
                // счётчик не участвует (list-style-type-string-*).
                let t = v.trim();
                if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
                    || (t.starts_with(char::from(39))
                        && t.ends_with(char::from(39))
                        && t.len() >= 2)
                {
                    self.marker_text = Some(t[1..t.len() - 1].to_string());
                    self.no_marker = Some(false);
                    return;
                }
                // Вид маркера — ИМЯ стиля счётчика (css-lists-3 §3): любое,
                // а не восемь избранных. Ключевые слова размещения и `url()`
                // именем не являются.
                for token in v.split_whitespace() {
                    // Размещение, картинка и глобальные ключевые слова именем
                    // стиля не являются: последние решает каскад, а до него
                    // они означали бы «стиль по имени initial».
                    if matches!(
                        token,
                        "inside"
                            | "outside"
                            | "none"
                            | "inherit"
                            | "initial"
                            | "unset"
                            | "revert"
                            | "revert-layer"
                    ) || token.starts_with("url(")
                    {
                        continue;
                    }
                    self.list_style_type = Some(token.to_string());
                }
            }
            "object-fit" => self.object_fit = Some(v.to_string()),
            "white-space" => {
                // `pre` не переносит строки — так же, как `nowrap`; переносят
                // только `pre-wrap` и `pre-line`.
                self.nowrap = Some(matches!(v, "nowrap" | "pre"));
                // `pre-line` — единственный, кто хранит переводы строк, но
                // схлопывает пробелы; остальные `pre*` хранят и пробелы.
                self.keep_spaces = Some(matches!(v, "pre" | "pre-wrap" | "break-spaces"));
                self.preserve_newlines = Some(matches!(
                    v,
                    "pre" | "pre-wrap" | "pre-line" | "break-spaces"
                ));
                self.break_after_spaces = Some(v == "break-spaces");
            }

            // --- Логические свойства ---------------------------------------
            // Письмо у нас только слева направо и сверху вниз, поэтому
            // логические оси совпадают с физическими один в один.
            "inline-size" => self.logical().inline_size = Len::parse(v),
            "block-size" => self.logical().block_size = Len::parse(v),
            "min-inline-size" => self.logical().min_inline = Len::parse(v),
            "min-block-size" => self.logical().min_block = Len::parse(v),
            "max-inline-size" => self.logical().max_inline = Len::parse(v),
            "max-block-size" => self.logical().max_block = Len::parse(v),
            "padding-inline" | "padding-block" | "margin-inline" | "margin-block"
            | "inset-inline" | "inset-block" => {
                let (a, b) = axis_pair(v);
                let block = key.ends_with("block");
                let which = key.split('-').next().unwrap_or("").to_string();
                let seq = self.decl_seq;
                let logical = self.logical();
                let target = match which.as_str() {
                    "padding" => &mut logical.padding,
                    "margin" => &mut logical.margin,
                    _ => &mut logical.inset,
                };
                if block {
                    target.block_start = a;
                    target.block_end = b;
                    target.seq[2] = seq;
                    target.seq[3] = seq;
                } else {
                    target.inline_start = a;
                    target.inline_end = b;
                    target.seq[0] = seq;
                    target.seq[1] = seq;
                }
            }
            "padding-inline-start" | "padding-inline-end" | "padding-block-start"
            | "padding-block-end" | "margin-inline-start" | "margin-inline-end"
            | "margin-block-start" | "margin-block-end" | "inset-inline-start"
            | "inset-inline-end" | "inset-block-start" | "inset-block-end" => {
                let seq = self.decl_seq;
                let parsed = Len::parse(v);
                let logical = self.logical();
                let target = if key.starts_with("padding") {
                    &mut logical.padding
                } else if key.starts_with("margin") {
                    &mut logical.margin
                } else {
                    &mut logical.inset
                };
                let (slot, i) = if key.ends_with("inline-start") {
                    (&mut target.inline_start, 0)
                } else if key.ends_with("inline-end") {
                    (&mut target.inline_end, 1)
                } else if key.ends_with("block-start") {
                    (&mut target.block_start, 2)
                } else {
                    (&mut target.block_end, 3)
                };
                *slot = parsed;
                target.seq[i] = seq;
            }

            // --- Раскладка --------------------------------------------------
            "aspect-ratio" => {
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): разбирать `auto <ratio>`
                // (css-sizing-4 §5.1), отбрасывая слово и оставляя отношение
                // ВСЕМ коробкам — срез css-sizing 264 -> 259 (+3/−8):
                // у замещаемого с природным соотношением `auto` велит
                // предпочесть природное (`replaced-element-020/029/030`), а
                // для этого нужен признак «auto рядом», который отрисовка
                // замещаемого учтёт в `image_with::ratio_of`. Возвращать
                // парой «отношение + флаг», не голым отношением.
                self.aspect_ratio = match v.split_once('/') {
                    Some((a, b)) => match (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
                        (Ok(a), Ok(b)) if b != 0.0 => Some(a / b),
                        _ => None,
                    },
                    None => v.parse().ok(),
                }
            }
            "order" => self.order = v.parse().ok(),
            "flex-flow" => {
                for token in v.split_whitespace() {
                    let prop = if token.starts_with("wrap") || token == "nowrap" {
                        "flex-wrap"
                    } else {
                        "flex-direction"
                    };
                    self.apply_one(prop, token);
                }
            }
            "align-content" => {
                self.align_content = parse_justify(v);
                self.align_content_safe = is_safe(v);
            }
            "justify-items" => {
                self.justify_items = parse_align(v);
                self.justify_items_safe = is_safe(v);
            }
            // Значение бывает составным: `row fill-reverse`, `column
            // track-reverse`. Сверка со строкой ЦЕЛИКОМ путала ось на каждом
            // таком тесте.
            "grid-lanes-pack" => self.lanes_dense = v.contains("dense"),
            // Порог «равенства» лунок при авто-выборе (css-grid-3): лунки с
            // разницей заполнения меньше порога считаются равными и берутся в
            // ПОРЯДКЕ ДОКУМЕНТА. `normal` (дефолт!) = 1em, `infinite` —
            // строгий порядок укладки безотносительно высот.
            "flow-tolerance" | "item-tolerance" => {
                self.lanes_tolerance = match v.trim() {
                    "normal" => None,
                    "infinite" => Some(Len::Px(f32::INFINITY)),
                    t => Len::parse(t),
                };
            }
            // Первая часть — ось лунок, дальше — реверсы: `fill-reverse`
            // заполняет лунки с другого конца, `track-reverse` перечисляет
            // сами лунки в обратном порядке (css-grid-3).
            "grid-lanes-direction" => {
                self.lanes_row = Some(v.split_whitespace().next() == Some("row"));
                self.lanes_fill_reverse = v.split_whitespace().any(|w| w == "fill-reverse");
                self.lanes_track_reverse = v.split_whitespace().any(|w| w == "track-reverse");
            }
            "justify-self" => {
                self.justify_self = parse_align(v);
                self.justify_self_safe = is_safe(v);
            }
            // `place-*` — сокращения «поперёк / вдоль»; одно значение задаёт обе оси.
            "place-items" | "place-content" | "place-self" => {
                let (a, b) = match v.split_once(char::is_whitespace) {
                    Some((a, b)) => (a.trim(), b.trim()),
                    None => (v, v),
                };
                let (cross, main) = match key {
                    "place-items" => ("align-items", "justify-items"),
                    "place-content" => ("align-content", "justify-content"),
                    _ => ("align-self", "justify-self"),
                };
                self.apply_one(cross, a);
                self.apply_one(main, b);
            }
            "grid-gap" => self.apply_one("gap", v),
            "grid-row-gap" => self.apply_one("row-gap", v),
            "grid-column-gap" => self.apply_one("column-gap", v),
            "grid-template-rows" => {
                self.subgrid |= v.contains("subgrid");
                self.grid_rows = parse_tracks(v);
            }
            "grid-auto-columns" => {
                // `grid-auto-columns: A B C` задаёт НЕСКОЛЬКО неявных дорожек,
                // и раскладка их циклит. Пока бралась первая, вторая колонка
                // получала ширину первой (`grid-support-grid-auto-columns-
                // rows-002`, `grid-floats-no-intrude-002`).
                let all = parse_tracks(v).unwrap_or_default();
                self.grid_auto_cols_list = if all.len() > 1 { all.clone() } else { Vec::new() };
                self.grid_auto_cols = all.into_iter().next();
            }
            "grid-auto-rows" => {
                let all = parse_tracks(v).unwrap_or_default();
                self.grid_auto_rows_list = if all.len() > 1 { all.clone() } else { Vec::new() };
                self.grid_auto_rows = all.into_iter().next();
            }
            "grid-auto-flow" => {
                let dense = v.contains("dense");
                self.grid_auto_flow = Some(match (v.contains("column"), dense) {
                    (true, true) => AutoFlow::ColDense,
                    (true, false) => AutoFlow::Col,
                    (false, true) => AutoFlow::RowDense,
                    (false, false) => AutoFlow::Row,
                })
            }
            "grid-column" => self.grid_col = parse_span(v),
            "grid-row" => self.grid_row = parse_span(v),
            "grid-column-start" => {
                let end = self.grid_col.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_col = Some((parse_placement(v), end));
            }
            "grid-column-end" => {
                let start = self.grid_col.map(|c| c.0).unwrap_or(Placement::Auto);
                self.grid_col = Some((start, parse_placement(v)));
            }
            "grid-row-start" => {
                let end = self.grid_row.map(|c| c.1).unwrap_or(Placement::Auto);
                self.grid_row = Some((parse_placement(v), end));
            }
            "grid-row-end" => {
                let start = self.grid_row.map(|c| c.0).unwrap_or(Placement::Auto);
                self.grid_row = Some((start, parse_placement(v)));
            }
            "grid-template-areas" => {
                // Каждая строка записи — ряд сетки: `"head head" "side main"`.
                let rows: Vec<Vec<String>> = v
                    .split('"')
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .map(|r| r.split_whitespace().map(str::to_string).collect())
                    .filter(|r: &Vec<String>| !r.is_empty())
                    .collect();
                self.grid_areas = (!rows.is_empty()).then_some(rows);
            }
            // `grid-area: строка / колонка / конец строки / конец колонки`.
            "grid-area" => {
                let parts: Vec<&str> = v.split('/').map(str::trim).collect();
                let at = |i: usize| {
                    parts
                        .get(i)
                        .map(|p| parse_placement(p))
                        .unwrap_or(Placement::Auto)
                };
                if parts.len() >= 2 {
                    self.grid_row = Some((at(0), at(2)));
                    self.grid_col = Some((at(1), at(3)));
                } else if let Some(name) = parts.first().filter(|n| !n.is_empty()) {
                    // Одно значение — это ИМЯ области: номера линий для него
                    // знает только контейнер со своей раскладкой имён.
                    self.grid_area_name = Some((*name).to_string());
                }
            }
            "z-index" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::Z_INDEX;
                    return;
                }
                // Целое за пределами i32 КЛАМПИТСЯ, а не падает в auto
                // (z-index-001: -2147483649 обязан остаться меньше -100).
                self.z_index = v
                    .parse::<i64>()
                    .ok()
                    .map(|n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            }

            // --- Рамки и обводка --------------------------------------------
            // Толщина словом (`thin`/`medium`/`thick`) — законное значение;
            // неразобранное значение НЕ стирает уже заданную толщину.
            "border-top-width" => self.border_width.top = line_width(v).or(self.border_width.top),
            "border-right-width" => {
                self.border_width.right = line_width(v).or(self.border_width.right)
            }
            "border-bottom-width" => {
                self.border_width.bottom = line_width(v).or(self.border_width.bottom)
            }
            "border-left-width" => {
                self.border_width.left = line_width(v).or(self.border_width.left)
            }
            // Логические кромки уходят в логический слой: физическая сторона
            // известна только после наследования письма (css-logical-1 §4.2;
            // у `vertical-rl` inline-start — ВЕРХ, разбор же писал влево:
            // logical-props-001).
            "border-inline" => {
                let l = self.logical();
                l.border[1] = Some(v.to_string());
                l.border[3] = Some(v.to_string());
            }
            "border-block" => {
                let l = self.logical();
                l.border[0] = Some(v.to_string());
                l.border[2] = Some(v.to_string());
            }
            "border-block-start" => self.logical().border[0] = Some(v.to_string()),
            "border-block-end" => self.logical().border[2] = Some(v.to_string()),
            "border-inline-start" => self.logical().border[3] = Some(v.to_string()),
            "border-inline-end" => self.logical().border[1] = Some(v.to_string()),
            "border-block-start-color" => self.border_colors[0] = side_color(v, self.color),
            "border-block-end-color" => self.border_colors[2] = side_color(v, self.color),
            "border-inline-start-color" => self.border_colors[3] = side_color(v, self.color),
            "border-inline-end-color" => self.border_colors[1] = side_color(v, self.color),
            "border-block-start-width" => {
                self.border_width.top = line_width(v).or(self.border_width.top)
            }
            "border-block-end-width" => {
                self.border_width.bottom = line_width(v).or(self.border_width.bottom)
            }
            "border-inline-start-width" => {
                self.border_width.left = line_width(v).or(self.border_width.left)
            }
            "border-inline-end-width" => {
                self.border_width.right = line_width(v).or(self.border_width.right)
            }
            "border-block-start-style" => self.set_border_style(v, Some(0)),
            "border-block-end-style" => self.set_border_style(v, Some(2)),
            "border-inline-start-style" => self.set_border_style(v, Some(3)),
            "border-inline-end-style" => self.set_border_style(v, Some(1)),
            "border-top-color" => self.border_colors[0] = side_color(v, self.color),
            "border-right-color" => self.border_colors[1] = side_color(v, self.color),
            "border-bottom-color" => self.border_colors[2] = side_color(v, self.color),
            "border-left-color" => self.border_colors[3] = side_color(v, self.color),
            "border-top-style" => self.set_border_style(v, Some(0)),
            "border-right-style" => self.set_border_style(v, Some(1)),
            "border-bottom-style" => self.set_border_style(v, Some(2)),
            "border-left-style" => self.set_border_style(v, Some(3)),
            "border-style" => {
                // От одного до четырёх значений, как у любого сокращения по
                // сторонам: `border-style: solid none` — рамка сверху и снизу
                // (css-backgrounds-3 §4.2). Прежде строка сравнивалась целиком,
                // и любая многозначная запись гасила рамку на всех сторонах.
                let side = |list: &[&str], i: usize| -> String {
                    let pick = match (list.len(), i) {
                        (1, _) => 0,
                        (2, 0 | 2) => 0,
                        (2, _) => 1,
                        (3, 0) => 0,
                        (3, 2) => 2,
                        (3, _) => 1,
                        _ => i.min(list.len().saturating_sub(1)),
                    };
                    list[pick].to_ascii_lowercase()
                };
                let list: Vec<&str> = v.split_whitespace().collect();
                if list.is_empty() {
                    return;
                }
                // `dashed` и `dotted` — разные узоры; `double`, `groove` и
                // прочие рельефные сводятся к сплошной: рельефа в конвейере нет.
                self.border_dashed = Some(side(&list, 0) == "dashed");
                self.border_dotted = Some(side(&list, 0) == "dotted");
                let widths = [
                    &mut self.border_width.top,
                    &mut self.border_width.right,
                    &mut self.border_width.bottom,
                    &mut self.border_width.left,
                ];
                for (i, w) in widths.into_iter().enumerate() {
                    let one = side(&list, i);
                    let on = border_style(&one);
                    self.border_visible[i] = Some(on);
                    // Ранг рисунка — участник разбора сросшихся кромок
                    // (§17.6.2.1), и `hidden` там гасит соседей. Сокращение
                    // его не писало вовсе, поэтому `border-style: hidden` на
                    // ряде или группе до разбора не доезжал.
                    if let Some(rank) = border_style_rank(&one) {
                        self.border_side_styles[i] = Some(rank);
                    }
                    // Свой рисунок рамки делает её видимой: начальная толщина
                    // `medium` — это 3px, и задавать её отдельно не требуется.
                    if on && w.is_none() {
                        *w = Some(Len::Px(3.0));
                    }
                    if one == "none" || one == "hidden" {
                        *w = Some(Len::Px(0.0));
                    }
                }
            }
            "border-spacing" => {
                let (a, b) = axis_pair(v);
                self.border_spacing = Some((a, b));
            }
            "outline" => {
                // `outline: inherit` — вычисленное значение родителя целиком
                // (§6.2.1): само свойство не наследуется, поэтому копируются
                // все его части (`outline-002`).
                if v == "inherit" {
                    self.inherit_bits |= inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S;
                    return;
                }
                // Умолчание CSS — `medium`, три точки: без него запись
                // `outline: solid red` не рисовала ничего.
                let mut o = self.outline.unwrap_or(Outline {
                    width: Some(Len::Px(3.0)),
                    color: None,
                    offset: None,
                    style: None,
                });
                for token in split_ws_top(v) {
                    if let Some(st) = outline_style_of(token) {
                        o.style = Some(st);
                    } else if let Some(l) = outline_width_of(token) {
                        o.width = Some(l);
                    } else if let Some(c) = Color::parse(token) {
                        o.color = Some(c);
                    }
                }
                self.outline = Some(o);
            }
            "outline-width" | "outline-color" | "outline-offset" | "outline-style" => {
                // `outline-width: inherit` берёт у родителя ТОЛЬКО толщину:
                // остальные части обводки остаются своими. Разбор слова здесь
                // не выражается — `outline_width_of("inherit")` даёт `None`, и
                // толщина пропадала.
                if v == "inherit" {
                    self.inherit_bits |= match key {
                        "outline-width" => inh::OUTLINE_W,
                        "outline-color" => inh::OUTLINE_C,
                        "outline-style" => inh::OUTLINE_S,
                        _ => inh::OUTLINE_O,
                    };
                    return;
                }
                let mut o = self.outline.unwrap_or_default();
                match key {
                    "outline-width" => o.width = outline_width_of(v).or(o.width),
                    "outline-color" => o.color = Color::parse(v),
                    "outline-style" => o.style = outline_style_of(v),
                    _ => o.offset = Len::parse(v),
                }
                self.outline = Some(o);
            }
            "backdrop-filter" => {
                self.backdrop_blur = v
                    .strip_prefix("blur(")
                    .and_then(|r| r.strip_suffix(')'))
                    .and_then(Len::parse)
                    .and_then(|l| match l {
                        Len::Px(v) => Some(v),
                        _ => None,
                    })
            }
            "background-image" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_IMAGE;
                    return;
                }
                // `none` ГАСИТ картинку (§14.2.1): ветки под него не было
                // вовсе, и заданный ранее адрес переживал отмену.
                if v.trim().eq_ignore_ascii_case("none") {
                    self.bg_image = None;
                    self.gradient = None;
                    self.gradient_raw = None;
                } else if v.starts_with("linear-gradient(") || v.starts_with("radial-gradient(") {
                    self.gradient = parse_gradient(v);
                    // Сырая запись нужна фону РЯДА таблицы: он рисуется
                    // слоем картинки, и градиент туда идёт источником.
                    self.gradient_raw = Some(v.to_string());
                } else if let Some(rest) = v.strip_prefix("filter(") {
                    // `filter(<image>, <filter-list>)` (filter-effects-1 §12):
                    // фильтр применяется К КАРТИНКЕ, не к элементу — цвета
                    // градиента пересчитываются на месте.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let parts = crate::css::split_args(inner);
                    if let Some(img) = parts.first().map(|p| p.trim())
                        && (img.starts_with("linear-gradient(")
                            || img.starts_with("radial-gradient("))
                        && let Some(mut g) = parse_gradient(img)
                    {
                        let mut tmp = Self::default();
                        tmp.apply_one("filter", &parts[1..].join(" "));
                        if let Some(f) = tmp.filter {
                            g.from = f.apply(g.from);
                            g.to = f.apply(g.to);
                            for stop in g.stops.iter_mut() {
                                stop.0 = f.apply(stop.0);
                            }
                        }
                        self.gradient = Some(g);
                    } else if let Some(img) = parts.first().map(|p| p.trim())
                        && img.starts_with("conic-gradient(")
                    {
                        // Конический идёт растровой плиткой — фильтр к нему
                        // пока не доносим; сама картинка лучше, чем ничего.
                        self.bg_image = Some(img.to_string());
                    }
                } else if v.starts_with("conic-gradient(") {
                    // Конический GPU-путь не умеет — сразу растровой плиткой
                    // (css-images-4 §2.3; растеризатор уже есть).
                    self.bg_image = Some(v.to_string());
                } else if let Some(rest) = v.strip_prefix("image(") {
                    // `image(<url>? , <color>?)` (css-images-4 §2.4): цвет —
                    // запасной слой; сплошная заливка выражается градиентом
                    // из одного цвета.
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let mut url = None;
                    let mut color = None;
                    for part in crate::css::split_args(inner) {
                        let part = part.trim();
                        if let Some(u) = parse_url(part) {
                            url = Some(u);
                        } else if let Some(c) = Color::parse(part.trim_matches(is_quote)) {
                            color = Some(c);
                        }
                    }
                    match (url, color) {
                        (Some(u), _) => self.bg_image = Some(u),
                        (None, Some(c)) => self.gradient = Some(solid_gradient(c)),
                        _ => {}
                    }
                } else if let Some(rest) = v
                    .strip_prefix("image-set(")
                    .or_else(|| v.strip_prefix("-webkit-image-set("))
                {
                    // Первый кандидат с неотрицательным разрешением и без
                    // неподдержанного type() (css-images-4 §2.5).
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    for cand in crate::css::split_args(inner) {
                        let cand = cand.trim();
                        let bad_type = cand
                            .split("type(")
                            .nth(1)
                            .is_some_and(|t| !t.contains("image/"));
                        let neg_res = cand.split_whitespace().any(|t| {
                            t.starts_with('-') && (t.ends_with('x') || t.ends_with("dppx"))
                        });
                        if bad_type || neg_res {
                            continue;
                        }
                        let src = cand.split_whitespace().next().unwrap_or("");
                        if let Some(u) = parse_url(src) {
                            self.bg_image = Some(u);
                            break;
                        }
                        let trimmed = src.trim_matches(is_quote);
                        if !trimmed.is_empty() && trimmed != src {
                            self.bg_image = Some(trimmed.to_string());
                            break;
                        }
                    }
                } else if let Some(rest) = v.strip_prefix("cross-fade(") {
                    // `cross-fade(p% A, B)`: смесь ЦВЕТОВ выражается сплошной
                    // заливкой; с картинками берётся первая (приближение).
                    let inner = rest.rfind(')').map(|i| &rest[..i]).unwrap_or(rest);
                    let parts = crate::css::split_args(inner);
                    let mut p = 0.5f32;
                    let mut colors: Vec<Color> = vec![];
                    let mut url = None;
                    for part in &parts {
                        for tok in part.split_whitespace() {
                            if let Some(pc) = tok.strip_suffix('%') {
                                if let Ok(v) = pc.parse::<f32>() {
                                    p = (v / 100.0).clamp(0.0, 1.0);
                                }
                            } else if let Some(c) = Color::parse(tok) {
                                colors.push(c);
                            } else if let Some(u) = parse_url(tok) {
                                url.get_or_insert(u);
                            }
                        }
                    }
                    if colors.len() >= 2 {
                        let (a, b) = (colors[0], colors[1]);
                        let mix = Color {
                            r: a.r * p + b.r * (1.0 - p),
                            g: a.g * p + b.g * (1.0 - p),
                            b: a.b * p + b.b * (1.0 - p),
                            a: a.a * p + b.a * (1.0 - p),
                        };
                        self.gradient = Some(solid_gradient(mix));
                    } else if let Some(u) = url {
                        self.bg_image = Some(u);
                    }
                } else if v.trim_end().ends_with(')')
                    && let Some(url) = parse_url(v)
                {
                    // Хвост после `url(...)` делает объявление недействительным
                    // (§4.2): `background-image: url(x) repeat` не картинка.
                    self.bg_image = Some(url);
                }
            }

            // --- Текст -------------------------------------------------------
            // `font: [начертание] [вес] размер[/интерлиньяж] семейство`.
            "font" => {
                // Все части сокращения наследуемые: `inherit` для них — это
                // «своего значения нет», то есть ОЧИСТКА слота. Подстановка
                // родительского значения тут не работает: ниже по разбору
                // `own.font_size.or(parent.font_size)` вернул бы свой прежний
                // (`font: 0 Ahem; font: inherit` оставлял нулевой кегль).
                if v == "inherit" {
                    self.font_size = None;
                    self.font_family = None;
                    self.font_weight = None;
                    self.italic = None;
                    self.line_height = None;
                    return;
                }
                // `font: 50px / 1 Ahem` — вокруг косой черты разрешены пробелы,
                // а кегль с высотой строки обязаны разбираться одним куском:
                // иначе «/ 1 Ahem» уезжало в семейство шрифта целиком.
                let value = join_slash(v);
                let (head, family) = split_font(&value);
                // Неизвестное слово в голове сокращения тоже валит его целиком
                // (§4.2): `font: bold highlighted 100% serif` не задаёт ни
                // начертания, ни кегля (`c71-fwd-parsing-003`). Слова головы —
                // это начертание, наклон, вариант, растяжение и системные
                // ключевые слова; всё прочее начинается с цифры или точки.
                let head_word = |t: &str| {
                    matches!(
                        t,
                        "normal"
                            | "italic"
                            | "oblique"
                            | "small-caps"
                            | "bold"
                            | "bolder"
                            | "lighter"
                            | "ultra-condensed"
                            | "extra-condensed"
                            | "condensed"
                            | "semi-condensed"
                            | "semi-expanded"
                            | "expanded"
                            | "extra-expanded"
                            | "ultra-expanded"
                            | "xx-small"
                            | "x-small"
                            | "small"
                            | "medium"
                            | "large"
                            | "x-large"
                            | "xx-large"
                            | "larger"
                            | "smaller"
                            | "caption"
                            | "icon"
                            | "menu"
                            | "message-box"
                            | "small-caption"
                            | "status-bar"
                    ) || t.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                };
                if head
                    .split_whitespace()
                    .any(|t| !head_word(&t.to_ascii_lowercase()))
                {
                    return;
                }
                // Недействительная часть валит СОКРАЩЕНИЕ целиком (§4.2):
                // `font: 4em/-2em serif` не задаёт ни кегля, ни семейства
                // (`font-146`). Проверка идёт до записи любого куска.
                if head.split_whitespace().any(|t| {
                    t.split_once('/').is_some_and(|(_, lh)| {
                        let neg = |l: &Len| {
                            matches!(
                                l,
                                Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                                    if *v < 0.0
                            )
                        };
                        lh.parse::<f32>().is_ok_and(|m| m < 0.0)
                            || Len::parse(lh).as_ref().is_some_and(neg)
                    })
                }) {
                    return;
                }
                for token in head.split_whitespace() {
                    match token {
                        "italic" | "oblique" => self.italic = Some(true),
                        "bold" | "bolder" => self.font_weight = Some(700),
                        t if t.starts_with(|c: char| c.is_ascii_digit()) => {
                            if let Some((size, lh)) = t.split_once('/') {
                                self.apply_one("font-size", size);
                                self.apply_one("line-height", lh);
                            } else if t == "0"
                                || (Len::parse(t).is_some()
                                    && !t.chars().all(|c| c.is_ascii_digit()))
                            {
                                // `font: 0 Ahem` — ноль это ДЛИНА (кегль 0),
                                // а не вес: вес 0 зацикливал подбор шрифта
                                // (vars-font-shorthand-001 висел).
                                self.apply_one("font-size", t);
                            } else {
                                self.font_weight =
                                    t.parse().ok().filter(|w| (1..=1000).contains(w));
                            }
                        }
                        _ => {}
                    }
                }
                if !family.is_empty() {
                    self.apply_one("font-family", family);
                }
            }
            "word-spacing" => self.word_spacing = Len::parse_spacing(v),
            "text-transform" => {
                // Свойство наследуемое: `inherit` очищает свой слот, иначе
                // прежнее объявление того же правила его переживало.
                if v == "inherit" {
                    self.text_transform = None;
                    return;
                }
                // Значений бывает несколько сразу (`capitalize full-width`):
                // разбираются все, неизвестное пропускается, а не обнуляет
                // объявление целиком.
                for word in v.split_ascii_whitespace() {
                    self.text_transform = Some(match word {
                        "uppercase" => TextTransform::Upper,
                        "lowercase" => TextTransform::Lower,
                        "capitalize" => TextTransform::Capitalize,
                        "full-width" | "fullwidth" => TextTransform::FullWidth,
                        "none" => TextTransform::None,
                        _ => continue,
                    });
                }
            }
            // Отступ первой строки. Кроме длины значение несёт до двух
            // ключевых слов (css-text-3 §7.1): `each-line` повторяет отступ
            // после КАЖДОГО жёсткого разрыва, `hanging` переворачивает выбор —
            // отступ получают все строки, КРОМЕ той, что получила бы его.
            // Порядок слов свободный, поэтому значение разбирается по словам.
            "text-indent" => {
                let (mut each, mut hang) = (false, false);
                for word in v.split_ascii_whitespace() {
                    match word.to_ascii_lowercase().as_str() {
                        "each-line" => each = true,
                        "hanging" => hang = true,
                        len => self.text_indent = Len::parse(len).or(self.text_indent),
                    }
                }
                self.text_indent_each_line = each.then_some(true);
                self.text_indent_hanging = hang.then_some(true);
            }
            // Свисающая пунктуация: знак выходит ЗА край коробки, чтобы край
            // текста читался ровным. Значения складываются: `first last`.
            "hanging-punctuation" => {
                let mut h = Hanging::default();
                for word in v.split_ascii_whitespace() {
                    match word {
                        "first" => h.first = true,
                        "last" => h.last = true,
                        "force-end" => h.force_end = true,
                        "allow-end" => h.allow_end = true,
                        _ => {}
                    }
                }
                self.hanging = (h != Hanging::default()).then_some(h);
            }
            // Разрыв ВНУТРИ слова разрешают по-разному, и разница видна на
            // экране. `word-break: break-all` и `line-break: anywhere` рвут
            // слово всегда. А `overflow-wrap` — только когда слово иначе не
            // влезает: короткое сначала целиком уходит на следующую строку.
            "text-autospace" => {
                // `normal` = оба разряда, `no-autospace` = ни одного.
                // Разряды могут стоять и по отдельности, и вместе.
                let (alpha, numeric) = match v {
                    "no-autospace" => (false, false),
                    "normal" | "auto" => (true, true),
                    other => (
                        other.contains("ideograph-alpha"),
                        other.contains("ideograph-numeric"),
                    ),
                };
                self.autospace_alpha = Some(alpha);
                self.autospace_numeric = Some(numeric);
            }
            "word-space-transform" => {
                // Точка переноса показывается пробелом: обычным или
                // идеографическим (css-text-4). `none` и `auto-phrase`
                // не показывают ничего.
                // Значение из ДВУХ слов (`ideographic-space auto-phrase`,
                // css-text-4 §word-space-transform) сравнением целиком не
                // ловилось и падало в `none`. Ключевое слово ищем среди
                // разделённых пробелом кусков.
                self.word_space_char = v.split_whitespace().find_map(|w| match w {
                    "space" => Some(' '),
                    "ideographic-space" => Some('\u{3000}'),
                    _ => None,
                });
            }
            "overflow-wrap" | "word-wrap" => {
                self.break_word = Some(matches!(v, "break-word" | "anywhere"));
                // `anywhere` отличается от `break-word` ровно одним: он МЕНЯЕТ
                // размер по минимальному содержимому — слово рвётся и при его
                // подсчёте. `break-word` на этот размер не влияет
                // (css-text-3 §5.5), и на этой разнице построено целое
                // семейство тестов.
                self.wrap_anywhere = Some(v == "anywhere");
            }
            "word-break" => {
                // `break-word` — устаревший псевдоним, и по спецификации он
                // равен `overflow-wrap: anywhere`, а не `break-word`: разница
                // в том, что `anywhere` УЧИТЫВАЕТСЯ в размере по минимальному
                // содержимому. Пока стоял `break_word`, ячейка с
                // `max-width: 0` не сжималась до знака (`word-break-min-content-001`).
                if v == "break-word" {
                    self.break_word = Some(true);
                    self.wrap_anywhere = Some(true);
                    self.break_anywhere = Some(false);
                    self.keep_all = Some(false);
                } else {
                    self.break_anywhere = Some(v == "break-all");
                    self.keep_all = Some(v == "keep-all");
                }
            }
            "line-break" => {
                self.break_anywhere = Some(v == "anywhere");
                self.break_anywhere_strict = Some(v == "anywhere");
                // `auto` у Blink ведёт себя строго (`LineBreakStrictness::
                // kDefault`), поэтому послабления — только у явных
                // `normal`/`loose`; `line-break-normal-011`, `-loose-*`.
                self.line_break_loose = Some(match v {
                    "normal" => 1,
                    "loose" => 2,
                    _ => 0,
                });
            }
            "hyphenate-character" => {
                // Значение — строка в кавычках; `auto` значит «сам знак
                // переноса».
                self.hyphen_char = Some(if v == "auto" {
                    "\u{2010}".to_string()
                } else {
                    v.trim_matches(['"', '\'']).to_string()
                });
            }
            "text-fit" => {
                self.text_fit = (v != "none").then(|| {
                    let mut f = TextFit {
                        grow: false,
                        shrink: false,
                        per_line: false,
                        all: false,
                        target: None,
                    };
                    for word in v.split_whitespace() {
                        match word {
                            "grow" => f.grow = true,
                            "shrink" => f.shrink = true,
                            "consistent" => f.per_line = false,
                            "per-line" => f.per_line = true,
                            "per-line-all" => {
                                f.per_line = true;
                                f.all = true;
                            }
                            other => {
                                if let Some(pct) = other.strip_suffix('%') {
                                    if let Ok(n) = pct.parse::<f32>() {
                                        f.target = Some(n / 100.0);
                                    }
                                }
                            }
                        }
                    }
                    f
                });
            }
            "text-wrap" | "text-wrap-mode" | "text-wrap-style" => {
                if matches!(v, "nowrap" | "wrap") {
                    self.nowrap = Some(v == "nowrap");
                }
                // `balance` выравнивает длины строк абзаца: последняя строка
                // не должна оставаться коротким огрызком.
                self.balance_lines = Some(v == "balance");
            }
            "vertical-align" => {
                // Надстрочный и подстрочный кусок остаются В СТРОКЕ, только
                // сдвигаются от базовой линии, — это не выравнивание коробки,
                // поэтому у них своё поле. Доли кегля браузерные.
                // Сдвиг хранится ДОЛЕЙ кегля: у длины она считается при
                // разборе, у процента она и есть написанное (CSS 2.1 §10.8.1
                // считает процент от `line-height`, но у нас доля умножается
                // на кегль — при `line-height: normal` это то же самое с
                // точностью до полулидинга, а точная формула ждёт модели
                // строчной коробки).
                self.vertical_shift = match v {
                    "super" => Some(-1.0 / 3.0),
                    "sub" => Some(1.0 / 5.0),
                    other => match Len::parse(other) {
                        // Долей кегля пишутся процент и `em` — их и храним
                        // долей. Ось сдвига смотрит вниз, а положительное
                        // значение поднимает знак ВВЕРХ.
                        Some(Len::Pct(k)) => {
                            // Процент — доля `line-height`, и считается он
                            // позже: кладём в своё поле.
                            self.vertical_shift_pct = (k != 0.0).then_some(-k);
                            None
                        }
                        Some(Len::Em(k)) => (k != 0.0).then_some(-k),
                        _ => None,
                    },
                };
                // Точки и единицы ШРИФТА считаются сразу в точках: доля
                // кегля тут не годится — `ex` зависит от метрик гарнитуры, а
                // кегль строчного приходит наследованием уже после разбора.
                match Len::parse(v) {
                    // Точки не зависят ни от чего — сразу в поле.
                    Some(Len::Px(px)) => self.vertical_shift_px = (px != 0.0).then_some(-px),
                    // Единицы шрифта ждут набора: у строчного своих метрик
                    // обычно нет, они приходят наследованием уже после
                    // разбора, и здесь вышли бы от чужой гарнитуры.
                    Some(l @ (Len::Ex(_) | Len::Ch(_))) => self.vertical_shift_len = Some(l),
                    _ => {}
                }
                self.vertical_align_text = match v {
                    "text-top" => Some(true),
                    "text-bottom" => Some(false),
                    _ => None,
                };
                self.vertical_align = match v {
                    "middle" => Some(Align::Center),
                    "top" => Some(Align::Start),
                    "bottom" => Some(Align::End),
                    "baseline" => Some(Align::Baseline),
                    _ => None,
                }
            }
            // css-overflow-4 §5.1: `none | [<integer> || <'block-ellipsis'>]
            // -webkit-legacy?`; `auto` — срез по высоте контейнера. Строка
            // многоточия идёт маркером абзаца (`lines::marker_str`).
            "line-clamp" => {
                self.line_clamp = None;
                self.clamp_auto = None;
                self.clamp_legacy = Some(false);
                let mut rest = v.trim();
                while !rest.is_empty() {
                    let quote = rest.as_bytes()[0];
                    let (word, tail) = if quote == b'"' || quote == b'\'' {
                        match rest[1..].find(quote as char) {
                            Some(end) => (&rest[..end + 2], &rest[end + 2..]),
                            None => (rest, ""),
                        }
                    } else {
                        match rest.find(char::is_whitespace) {
                            Some(end) => (&rest[..end], &rest[end..]),
                            None => (rest, ""),
                        }
                    };
                    rest = tail.trim_start();
                    if word.len() >= 2 && (word.starts_with('"') || word.starts_with('\'')) {
                        self.overflow_marker = Some(word[1..word.len() - 1].to_string());
                    } else if word.eq_ignore_ascii_case("auto") {
                        self.clamp_auto = Some(true);
                    } else if let Ok(n) = word.parse::<u32>() {
                        self.line_clamp = Some(n);
                    }
                }
            }
            "-webkit-line-clamp" => {
                self.line_clamp = v.trim().parse().ok();
                self.clamp_auto = None;
                self.clamp_legacy = Some(true);
            }
            "-webkit-box-orient" => {
                self.webkit_box_vertical = Some(v.eq_ignore_ascii_case("vertical"))
            }

            // --- Прочее ------------------------------------------------------
            "pointer-events" => self.pointer_events_none = Some(v == "none"),
            "table-layout" => self.table_fixed = Some(v == "fixed"),

            // --- Фоновая картинка --------------------------------------------
            // Запись бывает и ПОосевой: `repeat space`, `round no-repeat`.
            // Один keyword задаёт обе оси, два — свою каждой.
            "background-repeat" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_REPEAT;
                    return;
                }
                let word = |w: &str| match w {
                    "no-repeat" => Some(Tiling::None),
                    "space" => Some(Tiling::Space),
                    "round" => Some(Tiling::Round),
                    "repeat" => Some(Tiling::Repeat),
                    _ => None,
                };
                let mut it = v.split_whitespace();
                self.bg_repeat = match (it.next(), it.next()) {
                    (Some("repeat-x"), None) => Some(BgRepeat::RepeatX),
                    (Some("repeat-y"), None) => Some(BgRepeat::RepeatY),
                    (Some(x), Some(y)) => match (word(x), word(y)) {
                        (Some(x), Some(y)) => Some(BgRepeat::Axes(x, y)),
                        _ => None,
                    },
                    (Some(x), None) => word(x).map(|t| BgRepeat::Axes(t, t)),
                    _ => None,
                }
            }
            // Картинка вместо рамки. Свойств пять, и каждое дополняет одну и
            // ту же запись — поэтому разбор общий.
            "border-image"
            | "border-image-source"
            | "border-image-slice"
            | "border-image-width"
            | "border-image-outset"
            | "border-image-repeat" => self.set_border_image(key, v),
            // Область покраски фона. `border-box` — умолчание, поэтому оно
            // же и сбрасывает признак: свойство наследуемым не является, но
            // перебить заданное ранее в том же наборе обязано.
            // Откуда отсчитывается картинка. Умолчание — внутренний край
            // рамки, и `padding-box` его же и означает.
            "background-origin" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_ORIGIN;
            }
            "background-clip" | "-webkit-background-clip" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_CLIP;
            }
            "background-size" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::BG_SIZE;
            }
            "background-origin" => {
                self.bg_origin = match v.trim() {
                    "border-box" => Some(BgClip::BorderBox),
                    "content-box" => Some(BgClip::ContentBox),
                    _ => None,
                }
            }
            "background-clip" | "-webkit-background-clip" => {
                self.bg_clip = match v.trim() {
                    "padding-box" => Some(BgClip::PaddingBox),
                    "content-box" => Some(BgClip::ContentBox),
                    "text" => Some(BgClip::Text),
                    _ => None,
                }
            }
            "background-size" => {
                self.bg_size = match v {
                    "cover" => BgSize::Cover,
                    "contain" => BgSize::Contain,
                    _ => {
                        // Отрицательная длина делает декларацию невалидной
                        // целиком (css-backgrounds-3 §3.9) — размер не трогать.
                        let neg = |l: &Option<Len>| matches!(l, Some(Len::Px(v) | Len::Pct(v)) if *v < 0.0);
                        let mut it = v.split_whitespace();
                        let w = it.next().and_then(Len::parse);
                        let h = it.next().and_then(Len::parse);
                        if neg(&w) || neg(&h) {
                            return;
                        }
                        if w.is_none() && h.is_none() {
                            BgSize::Auto
                        } else {
                            BgSize::Fixed(w, h)
                        }
                    }
                }
            }
            "background-position" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::BG_POS;
                    return;
                }
                self.bg_pos = parse_pos_words(v);
            }
            // `object-position` — та же грамматика, но для замещаемого
            // содержимого (css-images-3 §5.2).
            "object-position" => {
                self.object_position = Some(parse_pos_words(v));
            }
            // Пооосевые продольные свойства (css-backgrounds-4 §4.1):
            // одна ось, вторая не трогается.
            "background-position-x" | "background-position-y" => {
                let val = match v {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    other => Len::parse(other),
                };
                if val.is_some() {
                    if key.ends_with("-x") {
                        self.bg_pos.x = val;
                    } else {
                        self.bg_pos.y = val;
                    }
                }
            }
            "background-attachment" => {
                // `fixed` привязывает плитку к ОБЛАСТИ ПРОСМОТРА: считается
                // она от окна, а красится всё равно только внутри коробки
                // (css-backgrounds-3 §3.10).
                self.bg_fixed = Some(v.eq_ignore_ascii_case("fixed"));
            }

            // --- Псевдоэлементы и шрифт ---------------------------------------
            "counter-reset" => self.counter_reset = Some(v.to_string()),
            "counter-increment" => self.counter_increment = Some(v.to_string()),
            "counter-set" => self.counter_set = Some(v.to_string()),
            "content" => {
                match v {
                    // ПУСТАЯ строка — не то же самое, что `none`: коробка
                    // псевдоэлемента создаётся, просто в ней нет знаков. На
                    // этом стоит целый приём эталонов WPT — `::after` с
                    // `content: ""` и `inset: 0` накрывает красное зелёным
                    // (`overflow-wrap-anywhere-001` и родня).
                    "none" | "normal" => self.content = None,
                    // Негодная запись НЕ применяется вовсе, прежнее значение
                    // остаётся (CSS 2.1 §4.1.8): иначе мусор вроде
                    // `counter(a,b,c)` печатался литералом и `counters-002`
                    // показывал слово FAIL.
                    other => {
                        if let Some(list) = parse_content(other) {
                            self.content = Some(list);
                        }
                    }
                }
            }
            "font-synthesis" | "font-synthesis-weight" | "font-synthesis-style"
            | "font-synthesis-small-caps" => {
                // css-fonts-4 §6.5: `auto` разрешает подмену, `none`
                // запрещает; у сокращения перечислены разрешённые части.
                let allow = |what: &str| match key {
                    "font-synthesis" => v.contains(what),
                    _ => v.trim() != "none",
                };
                match key {
                    "font-synthesis-weight" => self.font_synth.0 = Some(allow("weight")),
                    "font-synthesis-style" => self.font_synth.1 = Some(allow("style")),
                    "font-synthesis-small-caps" => self.font_synth.2 = Some(allow("small-caps")),
                    _ => {
                        self.font_synth = (
                            Some(allow("weight")),
                            Some(allow("style")),
                            Some(allow("small-caps")),
                        )
                    }
                }
                // Подмена ВЕСА и НАКЛОНА выражается своими тегами возможностей:
                // подбор грани в gpui читает их и отвергает поддельную грань
                // (`nsyw`/`nsys` — свои, не OpenType). Малые прописные мы не
                // синтезируем вовсе, поэтому у них тега нет.
                for (tag, on) in [("nsyw", self.font_synth.0), ("nsys", self.font_synth.1)] {
                    self.font_features.retain(|(t, _)| t != tag);
                    if on == Some(false) {
                        self.font_features.push((tag.to_string(), 1));
                    }
                }
            }
            "font-kerning" => {
                // css-fonts-4 §6.4: `none` гасит кернинг, `normal` включает,
                // `auto` оставляет решение шрифту (у нас — включён).
                self.font_features.retain(|(t, _)| t != "kern");
                match v.trim() {
                    "none" => self.font_features.push(("kern".to_string(), 0)),
                    "normal" => self.font_features.push(("kern".to_string(), 1)),
                    _ => {}
                }
            }
            "font-feature-settings" => {
                // Низкоуровневые теги через запятую: `"tnum" 1, "liga" off`.
                for token in v.split(',') {
                    let mut parts = token.split_whitespace();
                    let tag = parts.next().unwrap_or("").trim_matches('"');
                    if tag.len() == 4 {
                        let on = match parts.next() {
                            Some("off") | Some("0") => 0,
                            Some("on") | None => 1,
                            Some(n) => n.parse().unwrap_or(1),
                        };
                        self.font_features.push((tag.to_string(), on));
                    }
                }
            }
            "font-variant"
            | "font-variant-caps"
            | "font-variant-numeric"
            | "font-variant-ligatures"
            | "font-variant-east-asian"
            | "font-variant-position"
            | "font-variant-alternates" => {
                // Значения разделяются ПРОБЕЛОМ (css-fonts-4 §6); каждое
                // свойство сперва чистит СВОЮ подгруппу тегов — повтор и
                // `normal` переопределяют, а не копятся при наследовании.
                const CAPS: &[&str] = &["smcp", "c2sc", "pcap", "c2pc", "unic", "titl"];
                const NUMERIC: &[&str] = &[
                    "lnum", "onum", "pnum", "tnum", "frac", "afrc", "ordn", "zero",
                ];
                const LIGA: &[&str] = &["liga", "clig", "dlig", "hlig", "calt"];
                const EAST: &[&str] = &[
                    "jp78", "jp83", "jp90", "jp04", "smpl", "trad", "fwid", "pwid", "ruby",
                ];
                const POS: &[&str] = &["subs", "sups"];
                const ALT: &[&str] = &["hist", "salt", "swsh", "ornm", "nalt"];
                let alt_tag = |t: &str| {
                    (t.len() == 4 && (t.starts_with("ss") || t.starts_with("cv")))
                        && t[2..].bytes().all(|b| b.is_ascii_digit())
                };
                let groups: &[&[&str]] = match key {
                    "font-variant-caps" => &[CAPS],
                    "font-variant-numeric" => &[NUMERIC],
                    "font-variant-ligatures" => &[LIGA],
                    "font-variant-east-asian" => &[EAST],
                    "font-variant-position" => &[POS],
                    "font-variant-alternates" => &[ALT],
                    _ => &[CAPS, NUMERIC, LIGA, EAST, POS, ALT],
                };
                self.font_features.retain(|(t, _)| {
                    !groups.iter().any(|g| g.contains(&t.as_str()))
                        && !(matches!(key, "font-variant" | "font-variant-alternates")
                            && alt_tag(t))
                });
                for token in v.split_whitespace() {
                    let push: &[(&str, u32)] = match token {
                        "small-caps" => &[("smcp", 1)],
                        "all-small-caps" => &[("smcp", 1), ("c2sc", 1)],
                        "petite-caps" => &[("pcap", 1)],
                        "all-petite-caps" => &[("pcap", 1), ("c2pc", 1)],
                        "unicase" => &[("unic", 1)],
                        "titling-caps" => &[("titl", 1)],
                        "lining-nums" => &[("lnum", 1)],
                        "oldstyle-nums" => &[("onum", 1)],
                        "proportional-nums" => &[("pnum", 1)],
                        "tabular-nums" => &[("tnum", 1)],
                        "diagonal-fractions" => &[("frac", 1)],
                        "stacked-fractions" => &[("afrc", 1)],
                        "ordinal" => &[("ordn", 1)],
                        "slashed-zero" => &[("zero", 1)],
                        "common-ligatures" => &[("liga", 1), ("clig", 1)],
                        "no-common-ligatures" => &[("liga", 0), ("clig", 0)],
                        "discretionary-ligatures" => &[("dlig", 1)],
                        "no-discretionary-ligatures" => &[("dlig", 0)],
                        "historical-ligatures" => &[("hlig", 1)],
                        "no-historical-ligatures" => &[("hlig", 0)],
                        "contextual" => &[("calt", 1)],
                        "no-contextual" => &[("calt", 0)],
                        "jis78" => &[("jp78", 1)],
                        "jis83" => &[("jp83", 1)],
                        "jis90" => &[("jp90", 1)],
                        "jis04" => &[("jp04", 1)],
                        "simplified" => &[("smpl", 1)],
                        "traditional" => &[("trad", 1)],
                        "full-width" => &[("fwid", 1)],
                        "proportional-width" => &[("pwid", 1)],
                        "ruby" => &[("ruby", 1)],
                        "sub" => &[("subs", 1)],
                        "super" => &[("sups", 1)],
                        "historical-forms" => &[("hist", 1)],
                        // `none` выключает лигатуры по умолчанию.
                        "none" => &[("liga", 0), ("clig", 0), ("calt", 0)],
                        "normal" => &[],
                        raw => {
                            // Функциональные альтернаты: номер -> ssNN/cvNN.
                            let func = |name: &str, pre: &str| {
                                raw.strip_prefix(name)
                                    .and_then(|r| r.strip_suffix(')'))
                                    .and_then(|n| n.trim().parse::<u32>().ok())
                                    .filter(|n| (1..=99).contains(n))
                                    .map(|n| format!("{pre}{n:02}"))
                            };
                            if let Some(t) =
                                func("styleset(", "ss").or_else(|| func("character-variant(", "cv"))
                            {
                                self.font_features.push((t, 1));
                            } else if raw.starts_with("swash(") {
                                self.font_features.push(("swsh".into(), 1));
                            } else if raw.starts_with("ornaments(") {
                                self.font_features.push(("ornm".into(), 1));
                            } else if raw.starts_with("annotation(") {
                                self.font_features.push(("nalt".into(), 1));
                            }
                            &[]
                        }
                    };
                    for (t, on) in push {
                        self.font_features.push(((*t).into(), *on));
                    }
                }
            }
            "font-stretch" => {
                // Ключевые слова CSS — это проценты от обычной ширины.
                let pct = match v {
                    "ultra-condensed" => Some(50),
                    "extra-condensed" => Some(62),
                    "condensed" => Some(75),
                    "semi-condensed" => Some(87),
                    "normal" => Some(100),
                    "semi-expanded" => Some(112),
                    "expanded" => Some(125),
                    "extra-expanded" => Some(150),
                    "ultra-expanded" => Some(200),
                    other => other.trim_end_matches('%').parse::<i32>().ok(),
                };
                self.font_stretch = pct.map(|p| p as f32);
            }
            "caret-color" => self.caret_color = Color::parse(v),
            "accent-color" => self.accent_color = Color::parse(v),

            // --- Многоколоночный поток ----------------------------------------
            // Невалидное значение НЕ затирает прежнее (каскад CSS
            // отбрасывает объявление целиком): `column-count: -1` после
            // `column-count: 2` оставляет двойку. Ноль и минус невалидны.
            "column-count" => {
                if v.trim() == "auto" {
                    self.column_count = None;
                } else if let Ok(n) = v.trim().parse::<u16>()
                    && n > 0
                {
                    self.column_count = Some(n);
                }
            }
            "column-width" => {
                if v.trim() == "auto" {
                    self.column_width = None;
                } else if let Some(l) = Len::parse(v.trim()) {
                    // Отрицательная и нулевая ширина колонки невалидны.
                    if !matches!(l, Len::Px(w) if w <= 0.0) {
                        self.column_width = Some(l);
                    }
                }
            }
            // `column-fill`: балансировать ли колонки (дефолт balance).
            "column-fill" => self.column_fill_auto = Some(v.trim() == "auto"),
            // `column-span: all` — растяжка на все колонки.
            "column-span" => self.column_span = Some(v.trim() == "all"),
            // `avoid`, `avoid-column`, `avoid-page`, `avoid-region` — все
            // запрещают разрыв ВНУТРИ коробки; `auto` разрешает.
            // `page-break-inside` — устаревшее написание того же (css-break-3
            // §6.4 требует считать их одним свойством).
            "break-inside" | "page-break-inside" => {
                self.break_inside_avoid = v.trim().starts_with("avoid")
            }
            "break-before" | "page-break-before" => {
                self.break_before_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
            }
            "break-after" | "page-break-after" => {
                self.break_after_force = matches!(
                    v.trim(),
                    "column" | "page" | "always" | "left" | "right" | "recto" | "verso" | "region"
                );
            }
            "column-rule-width" => {
                self.column_rule_width = match v.trim() {
                    "thin" => Some(Len::Px(1.0)),
                    "medium" => Some(Len::Px(3.0)),
                    "thick" => Some(Len::Px(5.0)),
                    t => match Len::parse(t) {
                        Some(l) if !matches!(l, Len::Px(w) if w < 0.0) => Some(l),
                        _ => self.column_rule_width,
                    },
                }
            }
            "column-rule-style" => {
                self.column_rule_visible = Some(!matches!(v.trim(), "none" | "hidden"));
            }
            "column-rule-color" => self.column_rule_color = Color::parse(v.trim()),
            "column-rule" => {
                // Сокращение: ширина, стиль, цвет в любом порядке. Незнакомый
                // токен делает недействительным ВСЁ объявление (CSS 2.1
                // §4.1.7: `column-rule: normal red 1em` игнорируется целиком,
                // прежнее значение остаётся).
                let mut vis = None;
                let mut w = None;
                let mut col = None;
                let mut ok = true;
                for token in v.split_whitespace() {
                    match token {
                        "none" | "hidden" => vis = Some(false),
                        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset"
                        | "outset" => vis = Some(true),
                        "thin" => w = Some(Len::Px(1.0)),
                        "medium" => w = Some(Len::Px(3.0)),
                        "thick" => w = Some(Len::Px(5.0)),
                        t => {
                            if let Some(l) = Len::parse(t) {
                                if matches!(l, Len::Px(v) if v < 0.0) {
                                    ok = false;
                                } else {
                                    w = Some(l);
                                }
                            } else if let Some(c) = Color::parse(t) {
                                col = Some(c);
                            } else {
                                ok = false;
                            }
                        }
                    }
                }
                if ok {
                    if vis.is_some() {
                        self.column_rule_visible = vis;
                    }
                    if w.is_some() {
                        self.column_rule_width = w;
                    }
                    if col.is_some() {
                        self.column_rule_color = col;
                    }
                }
            }
            "columns" => {
                // `columns: <ширина> <число>` в любом порядке; `auto` оставляет
                // сторону нерешённой (не затирать уже разобранную ширину).
                for token in v.split_whitespace() {
                    if token == "auto" {
                        continue;
                    }
                    match token.parse::<u16>() {
                        Ok(n) if n > 0 => self.column_count = Some(n),
                        Ok(_) => {}
                        Err(_) => {
                            if let Some(l) = Len::parse(token) {
                                self.column_width = Some(l);
                            }
                        }
                    }
                }
            }

            // --- Сдвиг и тень текста ------------------------------------------
            "translate" => {
                let mut it = v.split_whitespace();
                let x = it.next().and_then(Len::parse).unwrap_or(Len::Px(0.0));
                let y = it.next().and_then(Len::parse).unwrap_or(Len::Px(0.0));
                self.translate = Some((x, y));
            }
            "text-shadow" => self.text_shadow = parse_shadows(v).into_iter().next(),

            // --- Время --------------------------------------------------------
            "animation"
            | "animation-name"
            | "animation-duration"
            | "animation-iteration-count"
            | "animation-direction"
            | "animation-delay"
            | "animation-play-state" => {
                let mut a = self.animation.clone().unwrap_or(AnimSpec {
                    name: String::new(),
                    seconds: 0.0,
                    infinite: false,
                    alternate: false,
                    delay: 0.0,
                    paused: false,
                });
                // В сокращении второе время — задержка (css-animations §5).
                let mut times = 0usize;
                let mut set_time = |a: &mut AnimSpec, sec: f32, times: &mut usize| match key {
                    "animation-delay" => a.delay = sec,
                    "animation-duration" => a.seconds = sec,
                    _ => {
                        if *times == 0 {
                            a.seconds = sec;
                        } else {
                            a.delay = sec;
                        }
                        *times += 1;
                    }
                };
                for token in v.split_whitespace() {
                    if let Some(sec) = token.strip_suffix("ms").and_then(|n| n.parse::<f32>().ok())
                    {
                        set_time(&mut a, sec / 1000.0, &mut times);
                    } else if let Some(sec) =
                        token.strip_suffix('s').and_then(|n| n.parse::<f32>().ok())
                    {
                        set_time(&mut a, sec, &mut times);
                    } else if token == "paused" {
                        a.paused = true;
                    } else if token == "infinite" {
                        a.infinite = true;
                    } else if token == "alternate" {
                        a.alternate = true;
                    } else if token.parse::<f32>().is_err()
                        && !matches!(
                            token,
                            "linear"
                                | "ease"
                                | "ease-in"
                                | "ease-out"
                                | "ease-in-out"
                                | "normal"
                                | "reverse"
                                | "both"
                                | "forwards"
                                | "backwards"
                                | "running"
                                | "paused"
                                | "none"
                        )
                    {
                        a.name = token.to_string();
                    }
                }
                // Свойства без имени (`animation-play-state` до сокращения)
                // копят состояние: имя может прийти следующей декларацией.
                self.animation = Some(a);
            }
            "transition" | "transition-duration" => {
                // Из записи перехода нужна только длительность: какие свойства
                // меняются, видно по разнице стилей.
                self.transition = v.split_whitespace().find_map(|t| {
                    t.strip_suffix("ms")
                        .and_then(|n| n.parse::<f32>().ok())
                        .map(|ms| ms / 1000.0)
                        .or_else(|| t.strip_suffix('s').and_then(|n| n.parse::<f32>().ok()))
                });
            }

            // --- Письмо и цветовые фильтры -------------------------------------
            "direction" => self.rtl = Some(v == "rtl"),
            // `unicode-bidi` решает, разбирать ли встроенность или задавать её
            // силой. Отмена (`bidi-override`) ставит знаки в заданную сторону
            // как есть, изоляция (`isolate`) прячет кусок от соседей.
            "unicode-bidi" => {
                self.bidi_override = Some(matches!(v, "bidi-override" | "isolate-override"));
                // `plaintext` — НЕ разновидность `isolate`: он не обкладывает
                // текст знаками встраивания, а выбирает сторону письма для
                // каждого абзаца между жёсткими разрывами по первому сильному
                // знаку. Пока он считался изоляцией, содержимое обкладывалось
                // LRI/PDI, и в узкой коробке строка не рисовалась вовсе.
                self.bidi_isolate = Some(matches!(v, "isolate" | "isolate-override"));
                self.bidi_plaintext = Some(v == "plaintext");
            }
            "resize" => {
                self.resize = match v {
                    "both" => Some((true, true)),
                    "horizontal" => Some((true, false)),
                    "vertical" => Some((false, true)),
                    _ => None,
                }
            }
            "filter" => {
                let mut f = self.filter.unwrap_or_else(Filter::neutral);
                for call in v.split(')') {
                    let Some((name, arg)) = call.split_once('(') else {
                        continue;
                    };
                    let name = name.trim();
                    let arg = arg.trim();
                    // Доля пишется и процентом, и числом.
                    let amount = || -> f32 {
                        match arg.strip_suffix('%') {
                            Some(n) => n.trim().parse::<f32>().unwrap_or(100.0) / 100.0,
                            None => arg.parse::<f32>().unwrap_or(1.0),
                        }
                    };
                    match name {
                        "url" => {
                            let id = arg.trim_matches(|c| c == '"' || c == '\'').trim();
                            if let Some(id) = id.strip_prefix('#') {
                                self.filter_ref = Some(id.to_string());
                            }
                        }
                        "grayscale" => f.grayscale = amount(),
                        "brightness" => f.brightness = amount(),
                        "saturate" => f.saturate = amount(),
                        "invert" => f.invert = amount(),
                        "sepia" => f.sepia = amount(),
                        "opacity" => f.opacity = amount(),
                        "hue-rotate" => {
                            f.hue_rotate = arg.trim_end_matches("deg").parse().unwrap_or(0.0)
                        }
                        "blur" => f.blur = arg.trim_end_matches("px").trim().parse().unwrap_or(0.0),
                        "contrast" => f.contrast = amount(),
                        // `drop-shadow` и цветовые матрицы — не наш случай.
                        _ => {}
                    }
                }
                self.filter = Some(f);
            }

            // --- Преобразования -----------------------------------------------
            "transform" if v.trim() == "inherit" => self.inherit_bits |= inh::TRANSFORM,
            "transform-origin" if v.trim() == "inherit" => {
                self.inherit_bits |= inh::TRANSFORM_ORIGIN
            }
            "transform" if has_font_units(v) => self.transform_raw = Some(v.to_string()),
            "transform-origin" if has_font_units(v) => {
                self.transform_origin_raw = Some(v.to_string())
            }
            "transform" => {
                let mut t = self.transform.unwrap_or_default();
                // Невалидный аргумент отбрасывает ВСЁ объявление (CSS-каскад),
                // а не превращается в ноль (`scale(invalid)` схлопывал фигуру,
                // хотя обязан быть проигнорирован — svg-document-styles-005).
                let mut invalid = false;
                for call in v.split(')') {
                    let Some((name, arg)) = call.split_once('(') else {
                        continue;
                    };
                    let (name, arg) = (name.trim(), arg.trim());
                    let nums: Vec<f32> = arg
                        .split(',')
                        .filter_map(|n| {
                            n.trim()
                                .trim_end_matches("grad")
                                .trim_end_matches("turn")
                                .trim_end_matches("deg")
                                .trim_end_matches("rad")
                                .trim_end_matches("px")
                                .trim_end_matches('%')
                                .parse::<f32>()
                                .ok()
                        })
                        .collect();
                    // Угол i-го аргумента с ЕГО единицей: grad — 400 на
                    // оборот, turn — целый оборот; rad проверяется после
                    // grad («grad» кончается на «rad»).
                    let angle_at = |i: usize| -> f32 {
                        let t = arg.split(',').nth(i).unwrap_or("").trim();
                        let v = nums.get(i).copied().unwrap_or(0.0);
                        if t.ends_with("grad") {
                            v * std::f32::consts::PI / 200.0
                        } else if t.ends_with("turn") {
                            v * std::f32::consts::TAU
                        } else if t.ends_with("rad") {
                            v
                        } else {
                            v.to_radians()
                        }
                    };
                    // Доля i-го аргумента: процент — сотая (scale(50%) = 0.5).
                    let frac_at = |i: usize, def: f32| -> f32 {
                        let t = arg.split(',').nth(i).unwrap_or("").trim();
                        match nums.get(i) {
                            None => def,
                            Some(v) if t.ends_with('%') => v / 100.0,
                            Some(v) => *v,
                        }
                    };
                    if nums.is_empty() && !arg.is_empty() && name != "none" {
                        invalid = true;
                    }
                    let first = nums.first().copied().unwrap_or(0.0);
                    let angle = angle_at(0);
                    // Имена функций регистронезависимы (css-transforms-1
                    // §7, CSS Syntax §4): `scale3D`, `rotatex`, `translateY`
                    // — одно и то же.
                    let lower = name.to_ascii_lowercase();
                    let name = lower.as_str();
                    // `matrix()`/`matrix3d()` — только числа (css-transforms-1
                    // §matrix): единица делает объявление невалидным
                    // (`transform-matrix-008`).
                    if matches!(name, "matrix" | "matrix3d")
                        && arg.split(',').any(|a| a.trim().parse::<f32>().is_err())
                    {
                        invalid = true;
                    }
                    match name {
                        "rotate" | "rotatez" => {
                            t.rotate_rad += angle;
                            t.push(Transform::rot(angle), NO_SHIFT);
                        }
                        "scale" => {
                            let sx = frac_at(0, 1.0);
                            let sy = frac_at(1, sx);
                            t.scale.0 *= sx;
                            t.scale.1 *= sy;
                            t.push(Transform::diag(sx, sy), NO_SHIFT);
                        }
                        "skew" => {
                            let ay = angle_at(1);
                            t.skew_rad.0 += angle;
                            t.skew_rad.1 += ay;
                            t.push([[1.0, angle.tan()], [ay.tan(), 1.0]], NO_SHIFT);
                        }
                        // matrix(a b c d e f): разложение на компоненты
                        // (перенос, поворот, масштаб, скос) — QR-подобное,
                        // как в css-transforms §16 (декомпозиция).
                        "matrix" if nums.len() == 6 => {
                            let (a, b, c, d, e2, f2) =
                                (nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]);
                            t.translate.0 += e2;
                            t.translate.1 += f2;
                            t.push([[a, c], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
                            let sx = (a * a + b * b).sqrt();
                            if sx > 1e-6 {
                                t.rotate_rad += b.atan2(a);
                                let det = a * d - b * c;
                                let sy = det / sx;
                                t.scale.0 *= sx;
                                t.scale.1 *= sy;
                                let shear = (a * c + b * d) / det.max(1e-6);
                                t.skew_rad.0 += shear.atan();
                            }
                        }
                        "skewx" => {
                            t.skew_rad.0 += angle;
                            t.push([[1.0, angle.tan()], [0.0, 1.0]], NO_SHIFT);
                        }
                        "skewy" => {
                            t.skew_rad.1 += angle;
                            t.push([[1.0, 0.0], [angle.tan(), 1.0]], NO_SHIFT);
                        }
                        "scalex" => {
                            let k = frac_at(0, 1.0);
                            t.scale.0 *= k;
                            t.push(Transform::diag(k, 1.0), NO_SHIFT);
                        }
                        "scaley" => {
                            let k = frac_at(0, 1.0);
                            t.scale.1 *= k;
                            t.push(Transform::diag(1.0, k), NO_SHIFT);
                        }
                        // Поворот вокруг оси экрана БЕЗ перспективы — это
                        // прямая проекция на плоскость, то есть сжатие поперёк
                        // оси ровно на косинус угла (css-transforms-2 §11):
                        // `rotateX(60deg)` даёт половину высоты. Перспективы у
                        // нас нет, и приближением это не является — при
                        // `perspective: none` так считает и браузер.
                        "rotatex" => {
                            t.scale.1 *= angle.cos();
                            t.push(Transform::diag(1.0, angle.cos()), NO_SHIFT);
                        }
                        "rotatey" => {
                            t.scale.0 *= angle.cos();
                            t.push(Transform::diag(angle.cos(), 1.0), NO_SHIFT);
                        }
                        // Поворот вокруг произвольной оси: та же проекция, но
                        // ось задана вектором. Ось экрана даёт обычный поворот,
                        // остальные — сжатие поперёк себя.
                        "rotate3d" => {
                            let (x, y, z) = (
                                nums.first().copied().unwrap_or(0.0),
                                nums.get(1).copied().unwrap_or(0.0),
                                nums.get(2).copied().unwrap_or(0.0),
                            );
                            let len = (x * x + y * y + z * z).sqrt();
                            if len > 0.0 {
                                let last = nums.get(3).copied().unwrap_or(0.0);
                                let a = if arg.contains("rad") {
                                    last
                                } else {
                                    last.to_radians()
                                };
                                let (x, y, z) = (x / len, y / len, z / len);
                                let (kx, ky) = (
                                    1.0 - y.abs() * (1.0 - a.cos()),
                                    1.0 - x.abs() * (1.0 - a.cos()),
                                );
                                t.rotate_rad += a * z;
                                t.scale.1 *= ky;
                                t.scale.0 *= kx;
                                t.push(Transform::rot(a * z), NO_SHIFT);
                                t.push(Transform::diag(kx, ky), NO_SHIFT);
                            }
                        }
                        // Третья ось без перспективы ничего не меняет: смещение
                        // по ней не видно, а масштаб по ней не на что влиять.
                        "translatez" | "perspective" => {}
                        "scalez" => {}
                        "scale3d" => {
                            let sy = nums.get(1).copied().unwrap_or(1.0);
                            t.scale.0 *= first;
                            t.scale.1 *= sy;
                            t.push(Transform::diag(first, sy), NO_SHIFT);
                        }
                        "translate3d" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            let mut v = NO_SHIFT;
                            for (i, dest) in [&mut t.translate.0, &mut t.translate.1]
                                .into_iter()
                                .enumerate()
                            {
                                if let Some(raw) = parts.get(i) {
                                    let d = raw.trim_end_matches("px").parse::<f32>().unwrap_or(0.0);
                                    *dest += d;
                                    v[i][0] = d;
                                }
                            }
                            t.push(Transform::diag(1.0, 1.0), v);
                        }
                        // Проценты в сдвиге считаются от СВОЕГО размера —
                        // на этом стоит типовое центрирование
                        // `translate(-50%, -50%)`. Раньше процент срезался как
                        // единица, и элемент уезжал на 50 точек.
                        "translate" | "translatex" | "translatey" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            let mut v = NO_SHIFT;
                            let mut axis = |i: usize, x: bool| {
                                let Some(raw) = parts.get(i) else { return };
                                let value = raw
                                    .trim_end_matches("px")
                                    .trim_end_matches('%')
                                    .parse::<f32>()
                                    .unwrap_or(0.0);
                                let row = if x { 0 } else { 1 };
                                let dest = if raw.ends_with('%') {
                                    // Доля своей ширины по x, высоты по y.
                                    v[row][1 + row] = value / 100.0;
                                    if x {
                                        &mut t.translate_pct.0
                                    } else {
                                        &mut t.translate_pct.1
                                    }
                                } else {
                                    v[row][0] = value;
                                    if x {
                                        &mut t.translate.0
                                    } else {
                                        &mut t.translate.1
                                    }
                                };
                                *dest += value / if raw.ends_with('%') { 100.0 } else { 1.0 };
                            };
                            match name {
                                "translatex" => axis(0, true),
                                "translatey" => axis(0, false),
                                _ => {
                                    axis(0, true);
                                    axis(1, false);
                                }
                            }
                            t.push(Transform::diag(1.0, 1.0), v);
                        }
                        // Скос выразить нечем: матрица GPUI хранит поворот и
                        // масштаб, но не сдвиг осей.
                        _ => {}
                    }
                }
                if !invalid {
                    self.transform = Some(t);
                }
            }
            "rotate" => {
                let mut t = self.transform.unwrap_or_default();
                let raw = v.trim();
                let value = raw
                    .trim_end_matches("deg")
                    .trim_end_matches("rad")
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(0.0);
                t.rotate_rad = if raw.contains("rad") {
                    value
                } else {
                    value.to_radians()
                };
                self.transform = Some(t);
            }
            "scale" => {
                let mut t = self.transform.unwrap_or_default();
                // Процент — это доля: `scale: 150%` равно 1.5, а не 150.
                let nums: Vec<f32> = v
                    .split_whitespace()
                    .filter_map(|n| match n.strip_suffix('%') {
                        Some(p) => p.parse::<f32>().ok().map(|v| v / 100.0),
                        None => n.parse::<f32>().ok(),
                    })
                    .collect();
                let x = nums.first().copied().unwrap_or(1.0);
                t.scale = (x, nums.get(1).copied().unwrap_or(x));
                self.transform = Some(t);
            }
            "transform-origin" => {
                // Точка отсчёта хранится ДОЛЯМИ коробки. Точечная запись
                // (`transform-origin: 0 0`, `20px 40px`) до неё не доводилась
                // и молча превращалась в центр — скос и поворот шли вокруг
                // другой точки (`css-skew-001`). Точки в доли переводит
                // отрисовка (`transform_origin_px`) — размер известен там.
                let axis = |t: &str, default: f32| -> f32 {
                    match t {
                        "left" | "top" => 0.0,
                        "center" => 0.5,
                        "right" | "bottom" => 1.0,
                        other => match Len::parse(other) {
                            Some(Len::Pct(p)) => p,
                            _ => default,
                        },
                    }
                };
                let px_axis = |t: &str| -> Option<f32> {
                    match Len::parse(t) {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    }
                };
                // Ключевые слова несут СВОЮ ось (css-transforms-1 §5.2):
                // одиночное `top` значит `center top`, `top left` = `left top`.
                let mut xs: Option<&str> = None;
                let mut ys: Option<&str> = None;
                let mut free: Vec<&str> = vec![];
                for t in v.split_whitespace() {
                    match t {
                        "left" | "right" => xs = Some(t),
                        "top" | "bottom" => ys = Some(t),
                        other => free.push(other),
                    }
                }
                let mut free = free.into_iter();
                let first = xs.or_else(|| free.next()).unwrap_or("center");
                let second = ys.or_else(|| free.next()).unwrap_or("center");
                self.transform_origin_px = (px_axis(first), px_axis(second));
                self.transform_origin = Some((axis(first, 0.5), axis(second, 0.5)));
            }
            // Трёхмерной сцены нет: без объёмных преобразований перспектива
            // ничего не меняет, поэтому разбирается и не делает ничего.
            "perspective" | "transform-style" | "backface-visibility" => {}

            // --- Обтекание и направление письма --------------------------------
            "float" => {
                self.float = match v {
                    "left" => Some(-1),
                    "right" => Some(1),
                    _ => Some(0),
                }
            }
            "clear" => {
                self.clear_inherit = v == "inherit";
                self.clear = match v {
                    "left" => Some(-1),
                    "right" => Some(1),
                    "both" => Some(0),
                    _ => None,
                }
            }
            "text-orientation" => self.upright = Some(v == "upright"),
            "writing-mode" => {
                // `sideways-*` отличается от `vertical-*` только поворотом
                // глифов, а направление потока у них общее.
                let vertical = v.starts_with("vertical") || v.starts_with("sideways");
                self.vertical = Some(vertical);
                self.vertical_rl = Some(vertical && v.ends_with("rl"));
                self.sideways = Some(v.starts_with("sideways"));
            }
            "text-combine-upright" => {
                let mut it = v.split_whitespace();
                self.combine_upright = match it.next() {
                    Some("none") | None => None,
                    Some("all") => Some(0),
                    Some("digits") => Some(it.next().and_then(|n| n.parse().ok()).unwrap_or(2)),
                    _ => None,
                };
            }

            // --- Переносы и обрезка -------------------------------------------
            "hyphens" => {
                self.hyphenate = Some(v != "none");
                // `manual` только УВАЖАЕТ расставленные знаки мягкого
                // переноса, сам их не ставит. Разделять обязательно: под одним
                // флагом `auto` и `manual` вели себя одинаково.
                self.hyphens_auto = Some(v == "auto");
            }
            "tab-size" => match v.parse::<u8>() {
                // Число — множитель ширины знака, длина — готовый шаг. Одно
                // отменяет другое: последнее объявление и есть значение.
                Ok(n) => {
                    self.tab_size = Some(n);
                    self.tab_size_len = None;
                }
                Err(_) => {
                    if let Some(len) = Len::parse(v) {
                        self.tab_size_len = Some(len);
                        self.tab_size = None;
                    }
                }
            },
            "contain" => {
                // Разбор по словам: подстрочный поиск ловил «size» в
                // «inline-size» и не видел paint внутри `content`
                // (css-contain-1 §3.1: strict = size layout paint style,
                // content = layout paint style).
                let mut bits = (false, false, false, false);
                let mut inline_only = false;
                for w in v.split_whitespace() {
                    match w {
                        // `paint` и `strict` обрезают содержимое по коробке —
                        // это ровно то, что делает скрытое переполнение;
                        // `size` считает коробку ПУСТОЙ: её размер задают
                        // явные свойства и `contain-intrinsic-size`.
                        "size" => bits.0 = true,
                        "inline-size" => inline_only = true,
                        "layout" => bits.1 = true,
                        "paint" => bits.2 = true,
                        "style" => bits.3 = true,
                        "strict" => bits = (true, true, true, true),
                        "content" => {
                            bits.1 = true;
                            bits.2 = true;
                            bits.3 = true;
                        }
                        _ => {}
                    }
                }
                self.contain_size = Some(bits.0);
                self.contain_inline_size = Some(inline_only);
                self.contain_layout = Some(bits.1);
                self.contain_paint = Some(bits.2);
                self.contain_style = Some(bits.3);
            }
            "content-visibility" => {
                // `hidden` = size+layout+paint containment, содержимое
                // пропускается целиком (css-contain-2 §4). `auto` для
                // reftest без прокрутки всегда «релевантен» = visible.
                if v.trim() == "hidden" {
                    self.contain_size = Some(true);
                    self.contain_paint = Some(true);
                    self.contain_layout = Some(true);
                    self.skip_content = Some(true);
                }
            }
            "contain-intrinsic-size" => {
                // Одно или два значения; `auto <длина>` — длина как запас.
                let nums: Vec<f32> = v
                    .split_whitespace()
                    .filter(|w| *w != "auto")
                    .filter_map(|w| match Len::parse(w) {
                        Some(Len::Px(px)) => Some(px),
                        _ => None,
                    })
                    .collect();
                self.contain_intrinsic = match nums.as_slice() {
                    [one] => (Some(*one), Some(*one)),
                    [w, h, ..] => (Some(*w), Some(*h)),
                    _ => (None, None),
                };
            }
            "contain-intrinsic-width" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.0 = Some(w);
                }
            }
            "contain-intrinsic-height" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.contain_intrinsic.1 = Some(h);
                }
            }
            "contain-intrinsic-block-size" => {
                if let Some(Len::Px(h)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_block = Some(h);
                }
            }
            "contain-intrinsic-inline-size" => {
                if let Some(Len::Px(w)) = Len::parse(v.trim_start_matches("auto").trim()) {
                    self.logical().ci_inline = Some(w);
                }
            }
            "mix-blend-mode" => {
                // Номера совпадают с формулами в шейдере: смешивание считается
                // при сборке буфера группы, поэтому доступны все режимы CSS,
                // включая те, где цвет берётся целиком (тон, насыщенность).
                self.blend = match v {
                    "multiply" => Some(1),
                    "screen" => Some(2),
                    "darken" => Some(3),
                    "lighten" => Some(4),
                    "overlay" => Some(5),
                    "color-dodge" => Some(6),
                    "color-burn" => Some(7),
                    "hard-light" => Some(8),
                    "soft-light" => Some(9),
                    "difference" => Some(10),
                    "exclusion" => Some(11),
                    "hue" => Some(12),
                    "saturation" => Some(13),
                    "color" => Some(14),
                    "luminosity" => Some(15),
                    _ => Some(0),
                }
            }
            "isolation" => self.isolate = Some(v == "isolate"),
            "shape-outside" => {
                let t = v.trim();
                if t != "none" {
                    self.shape_outside = Some(t.to_string());
                }
            }
            "shape-margin" => self.shape_margin = Len::parse(v.trim()),
            "shape-image-threshold" => {
                self.shape_threshold = v.trim().parse::<f32>().ok().map(|t| t.clamp(0.0, 1.0));
            }
            // Устаревшее `clip` (CSS 2.1): rect с запятыми или пробелами;
            // `auto` в позиции — соответствующий край коробки.
            "clip" => {
                if v == "inherit" {
                    self.inherit_bits |= inh::CLIP;
                    return;
                }
                if let Some(rest) = v.trim().strip_prefix("rect(") {
                    let rest = rest.trim_end_matches(')');
                    // Разделители — ЛИБО три запятые, ЛИБО одни пробелы:
                    // смешанная запись невалидна, свойство игнорируется
                    // (clip-rect-comma-002..004).
                    let commas = rest.matches(',').count();
                    if commas != 0 && commas != 3 {
                        return;
                    }
                    let parts: Vec<&str> = rest
                        .split([',', ' '])
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .collect();
                    if parts.len() == 4 {
                        let side = |t: &str| match t {
                            "auto" => None,
                            // Нулевая длина есть ноль в любой единице, и
                            // `rect(-0em, …)` обязан обрезать, а не читаться
                            // как `auto` (`visufx/clip-076…102`). Кегель на
                            // этом шаге ещё не известен, поэтому ненулевые
                            // относительные единицы по-прежнему мимо.
                            _ => match Len::parse(t) {
                                Some(Len::Px(v)) => Some(v),
                                Some(
                                    Len::Em(v)
                                    | Len::Ex(v)
                                    | Len::Ch(v)
                                    | Len::Ic(v)
                                    | Len::Lh(v)
                                    | Len::Vh(v)
                                    | Len::Vw(v),
                                ) if v == 0.0 => Some(0.0),
                                _ => None,
                            },
                        };
                        self.clip_rect = Some([
                            side(parts[0]),
                            side(parts[1]),
                            side(parts[2]),
                            side(parts[3]),
                        ]);
                        let raw = |t: &str| match t {
                            "auto" => None,
                            _ => Len::parse(t),
                        };
                        self.clip_len = Some([
                            raw(parts[0]),
                            raw(parts[1]),
                            raw(parts[2]),
                            raw(parts[3]),
                        ]);
                    }
                }
            }
            // Плитка маски (css-masking §7.6–7.8). `cover`/`contain` пока не
            // разобраны — им нужен интринзик картинки при вычислении.
            "mask-size" | "-webkit-mask-size" => match v.trim() {
                // Вписывание с сохранением пропорции (css-masking §7.8 ->
                // css-backgrounds §3.9): считается от интринзика при отрисовке.
                "contain" => self.mask_fit = Some(1),
                "cover" => self.mask_fit = Some(2),
                // `auto` (и `auto auto`) — начальное значение: интринзик.
                "auto" | "auto auto" => self.mask_size = None,
                _ => {
                    let mut it = v.split_whitespace();
                    if let Some(x) = it.next().and_then(Len::parse) {
                        let y = it.next().and_then(Len::parse).unwrap_or(x);
                        self.mask_size = Some((x, y));
                    }
                }
            },
            "mask-mode" => self.mask_luminance = Some(v.trim() == "luminance"),
            "mask-composite" | "-webkit-mask-composite" => {
                self.mask_composite = Some(
                    v.split(',')
                        .map(|t| match t.trim() {
                            "subtract" => 1,
                            "intersect" => 2,
                            "exclude" => 3,
                            _ => 0,
                        })
                        .collect(),
                );
            }
            "mask-origin" | "-webkit-mask-origin" => {
                self.mask_origin = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    _ => Some(0),
                }
            }
            "mask-clip" | "-webkit-mask-clip" => {
                self.mask_clip = match v.trim() {
                    "padding-box" => Some(2),
                    "content-box" => Some(3),
                    "no-clip" => Some(255),
                    _ => Some(0),
                }
            }
            "mask-repeat" | "-webkit-mask-repeat" => {
                // Пооосно (css-backgrounds §3.4): `repeat-x` = repeat по x,
                // одна плитка по y; два слова — оси по порядку.
                let t: Vec<&str> = v.split_whitespace().collect();
                self.mask_no_repeat = Some(match t.as_slice() {
                    ["repeat-x"] => (false, true),
                    ["repeat-y"] => (true, false),
                    [a] => (*a == "no-repeat", *a == "no-repeat"),
                    [a, b] => (*a == "no-repeat", *b == "no-repeat"),
                    _ => (false, false),
                });
            }
            "mask-position" | "-webkit-mask-position" => {
                let word = |t: &str| match t {
                    "left" | "top" => Some(Len::Pct(0.0)),
                    "center" => Some(Len::Pct(0.5)),
                    "right" | "bottom" => Some(Len::Pct(1.0)),
                    _ => Len::parse(t),
                };
                let toks: Vec<&str> = v.split_whitespace().collect();
                // Четырёхзначная запись — пары «край смещение»: `left 40%
                // bottom 60%` (css-backgrounds-3 §3.6); от правого/нижнего
                // края доля зеркалится.
                if toks.len() == 4 {
                    let pair = |edge: &str, off: &str| -> Option<(Len, bool)> {
                        let l = Len::parse(off)?;
                        Some((l, matches!(edge, "right" | "bottom")))
                    };
                    let horiz = matches!(toks[0], "left" | "right");
                    let (xe, xo, ye, yo) = if horiz {
                        (toks[0], toks[1], toks[2], toks[3])
                    } else {
                        (toks[2], toks[3], toks[0], toks[1])
                    };
                    if let (Some((x, fx)), Some((y, fy))) = (pair(xe, xo), pair(ye, yo)) {
                        self.mask_pos = Some((x, y));
                        self.mask_pos_far = (fx, fy);
                    }
                } else if let Some(x) = toks.first().and_then(|t| word(t)) {
                    let y = toks.get(1).and_then(|t| word(t)).unwrap_or(Len::Pct(0.5));
                    self.mask_pos = Some((x, y));
                }
            }
            "user-select" | "-webkit-user-select" => self.no_select = Some(matches!(v, "none")),
            "clip-path" | "mask" | "mask-image" => {
                // Маска-ИЗОБРАЖЕНИЕ (url/градиент): источник хранится строкой,
                // растрируется при сборке группы, альфа умножается в композите
                // буфера (css-masking §7.1; mask-image-1a).
                if key != "clip-path" {
                    if v.contains("-gradient(") {
                        self.mask_image = Some(v.trim().to_string());
                    } else if v.contains("url(") {
                        // Слоёв может быть несколько (`url(a), url(b)`) —
                        // строка хранится ЦЕЛИКОМ, разбор при отрисовке.
                        self.mask_image = Some(v.trim().to_string());
                    }
                    // Сокращение `mask` несёт и укладку (css-masking §7.9).
                    if v.contains("no-repeat") {
                        self.mask_no_repeat = Some((true, true));
                    }
                }
                // Круг и эллипс — это скруглённый прямоугольник с радиусом в
                // половину стороны; `inset(… round R)` — он же с заданным
                // радиусом. Многоугольник прямоугольной маской не выразить —
                // его гасит по форме сборка буфера группы.
                let v = v.trim();
                // Опорная коробка формы (css-masking §1.3.1.1): слово до или
                // после функции; точки полигона отсчитываются от неё
                // (clip-path-polygon-008: margin-box).
                // У элемента с CSS-коробкой `fill-box` = content-box,
                // `stroke-box`/`view-box` = border-box (css-masking-1 §1.3.1.1).
                self.clip_ref = if v.contains("margin-box") {
                    Some(1)
                } else if v.contains("padding-box") {
                    Some(2)
                } else if v.contains("content-box") || v.contains("fill-box") {
                    Some(3)
                } else if v.contains("border-box")
                    || v.contains("stroke-box")
                    || v.contains("view-box")
                {
                    Some(0)
                } else {
                    self.clip_ref
                };
                if key == "clip-path" {
                    self.clip_bare_box = !v.is_empty()
                        && !v.contains('(')
                        && v.split_whitespace().all(|w| w.ends_with("-box"));
                }
                // `clip-path: shape(...)` (css-shapes-2): команды хранятся
                // с `;` вместо запятых (по ним режутся слои), доли резолвит
                // отрисовка по размеру коробки.
                if key == "clip-path"
                    && let Some(rest) = v.trim().strip_prefix("shape(")
                {
                    // После скобки может стоять опорная коробка
                    // (`shape(...) content-box`) — режем по ПОСЛЕДНЕЙ скобке.
                    let rest = match rest.rfind(')') {
                        Some(i) => &rest[..i],
                        None => rest,
                    }
                    .trim();
                    let (rule, body) = match rest.split_once(' ') {
                        Some((r @ ("nonzero" | "evenodd"), b)) => (r, b),
                        _ => ("nonzero", rest),
                    };
                    self.clip_shape = Some(format!("shapedef:{rule}:{}", body.replace(',', ";")));
                }
                // `clip-path: path(правило, 'd')` — контур SVG: форма
                // растрируется маской покрытия; запятые в d заменяются
                // пробелами (грамматика SVG им равнозначна), потому что по
                // запятым верхнего уровня режутся СЛОИ маски.
                if key == "clip-path"
                    && let Some(rest) = v.trim().strip_prefix("path(")
                {
                    let rest = match rest.rfind(')') {
                        Some(i) => &rest[..i],
                        None => rest,
                    }
                    .trim();
                    let (rule, d) = match rest.split_once(',') {
                        Some((r, d)) if matches!(r.trim(), "nonzero" | "evenodd") => {
                            (r.trim(), d.trim())
                        }
                        _ => ("nonzero", rest),
                    };
                    let d = d.trim_matches(|c| c == '"' || c == '\'').replace(',', " ");
                    self.clip_shape = Some(format!("pathdef:{rule}:{d}"));
                }
                // `clip-path: url(#id)` — ссылка на <clipPath>: форма
                // растрируется маской покрытия при отрисовке.
                if key == "clip-path"
                    && let Some(url) = parse_url(v)
                    && let Some(id) = url.strip_prefix('#')
                {
                    self.clip_shape = Some(format!("clipref:{id}"));
                }
                if let Some(rest) = v.strip_prefix("polygon(") {
                    let rest = match rest.rfind(')') {
                        Some(i) => &rest[..i],
                        None => rest,
                    };
                    // Первым может стоять правило намотки (css-shapes-1
                    // §3.1): `polygon(evenodd, …)`. Вершин любое число —
                    // больше восьми (предел шейдера) и `evenodd` уходят
                    // растровой маской-путём при отрисовке.
                    let (rule, rest) = match rest.trim_start().split_once(',') {
                        Some((r, tail)) if matches!(r.trim(), "nonzero" | "evenodd") => {
                            (r.trim(), tail)
                        }
                        _ => ("nonzero", rest),
                    };
                    let points: Vec<(Len, Len)> = rest
                        .split(',')
                        .filter_map(|pair| {
                            let mut it = pair.split_whitespace();
                            let x = Len::parse(it.next()?)?;
                            let y = Len::parse(it.next()?)?;
                            Some((x, y))
                        })
                        .collect();
                    if points.len() >= 3 {
                        self.clip_polygon = Some(points);
                        self.clip_polygon_evenodd = rule == "evenodd";
                    }
                } else if v.starts_with("circle(") || v.starts_with("ellipse(") {
                    // Форма с параметрами (радиусы, `at`, ключевые стороны)
                    // растрируется маской: радиус и центр считаются от
                    // размеров коробки при отрисовке. Скруглённая коробка
                    // остаётся запасным путём для формы без аргументов.
                    let args = v
                        .split_once('(')
                        .map(|(_, r)| r.trim_end_matches(')').trim())
                        .unwrap_or("");
                    if args.is_empty() {
                        self.clip_round = Some(0.5);
                    } else {
                        self.clip_shape = Some(format!("shape:{}", v.trim()));
                    }
                } else if let Some(rest) = v.strip_prefix("rect(") {
                    // Края видимой области (css-shapes-1 §basic-shape):
                    // top/right/bottom/left от верхнего-левого угла, auto —
                    // край опорной коробки; хвост `round R` — скругление.
                    let inner = rest.trim_end_matches(')');
                    let sides_part = inner.split("round").next().unwrap_or("").trim();
                    let vals: Vec<Option<Len>> = sides_part
                        .split_whitespace()
                        .map(|t| if t == "auto" { None } else { Len::parse(t) })
                        .collect();
                    if vals.len() == 4 {
                        self.clip_edges = Some([vals[0], vals[1], vals[2], vals[3]]);
                        self.clip_round = inner
                            .split("round")
                            .nth(1)
                            .and_then(|r| Len::parse(r.trim()))
                            .and_then(|l| match l {
                                Len::Px(v) => Some(v),
                                _ => None,
                            })
                            .or(self.clip_round);
                    }
                } else if let Some(rest) = v.strip_prefix("xywh(") {
                    let inner = rest.trim_end_matches(')');
                    let sides_part = inner.split("round").next().unwrap_or("").trim();
                    let vals: Vec<Len> = sides_part
                        .split_whitespace()
                        .filter_map(Len::parse)
                        .collect();
                    if vals.len() == 4 {
                        self.clip_xywh = Some([vals[0], vals[1], vals[2], vals[3]]);
                        self.clip_round = inner
                            .split("round")
                            .nth(1)
                            .and_then(|r| Len::parse(r.trim()))
                            .and_then(|l| match l {
                                Len::Px(v) => Some(v),
                                _ => None,
                            })
                            .or(self.clip_round);
                    }
                } else if let Some(rest) = v.strip_prefix("inset(") {
                    // Стороны вырезки (css-shapes-1 §3.1.1.1): 1-4 значения
                    // TRBL до слова round; доли резолвит отрисовка.
                    let sides_part = rest
                        .trim_end_matches(')')
                        .split("round")
                        .next()
                        .unwrap_or("")
                        .trim();
                    let vals: Vec<Len> = sides_part
                        .split_whitespace()
                        .filter_map(Len::parse)
                        .collect();
                    let pick = |i: usize| -> Len {
                        match vals.len() {
                            1 => vals[0],
                            2 => vals[i % 2],
                            3 => vals[i.min(2)].to_owned(),
                            4 => vals[i],
                            _ => Len::Px(0.0),
                        }
                    };
                    if !vals.is_empty() {
                        self.clip_inset = Some([pick(0), pick(1), pick(2), pick(3)]);
                    }
                    let inner = rest.trim_end_matches(')');
                    let radius = inner
                        .split("round")
                        .nth(1)
                        .and_then(|r| Len::parse(r.trim()))
                        .and_then(|l| match l {
                            Len::Px(v) => Some(v),
                            Len::Pct(p) => Some(p),
                            l @ (Len::Em(_)
                            | Len::EmPx(..)
                            | Len::Ch(_)
                            | Len::Ic(_)
                            | Len::Ex(_)
                            | Len::Lh(_)
                            | Len::LhPx(..)) => crate::metrics::fallback_len_px(l, "", 16.0),
                            Len::Vw(_) | Len::Vh(_) | Len::Calc(_) => None,
                            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
                        });
                    self.clip_round = Some(radius.unwrap_or(0.0));
                }
            }

            _ => {}
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

    /// Уходит ли скругление углов альфа-маской буфера группы.
    ///
    /// Растеризатор круглит только окружностью и жмёт каждый угол к половине
    /// меньшей стороны; эллиптические углы (`H / V`) и большой НЕОДНОРОДНЫЙ
    /// радиус (спека жмёт одним множителем от суммы смежных, §5.5) рисуются
    /// точной растровой маской, а обычное скругление при этом снимается.
    pub fn radius_masked(&self) -> bool {
        if self.radius_ell.is_some() {
            return true;
        }
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let round = [
            side(self.radius.tl),
            side(self.radius.tr),
            side(self.radius.br),
            side(self.radius.bl),
        ];
        let (w, h) = (side(self.width), side(self.height));
        let max_r = round.iter().cloned().fold(0.0f32, f32::max);
        let uniform = round.iter().all(|r| (r - round[0]).abs() < 0.01);
        w > 0.0 && h > 0.0 && !uniform && max_r > w.min(h) * 0.5 + 0.01
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
            (None, Some(i)) => self.border_colors[i] = None,
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
        // `self-start`/`self-end` считаются по письму САМОГО элемента,
        // `start`/`end` — по письму контейнера. Пока обе оси физические, это
        // одно и то же.
        "start" | "flex-start" | "self-start" | "left" => Some(Align::Start),
        "end" | "flex-end" | "self-end" | "right" => Some(Align::End),
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
            || Color::parse(t).is_some();
        if !known {
            return false;
        }
    }
    any
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
    for token in v.split_whitespace() {
        let at = v[end..].find(token).map(|i| end + i).unwrap_or(end);
        end = at + token.len();
        let numeric = token.starts_with(|c: char| c.is_ascii_digit());
        if numeric && (token.contains('/') || Len::parse(token).is_some()) {
            return (&v[..end], v[end..].trim());
        }
    }
    (v, "")
}

/// Разбор значения `content` в список составляющих (css-content-3 §2).
///
/// `None` — запись негодна целиком: неизвестная функция, лишний или
/// недостающий аргумент, незакрытая кавычка. Такое объявление применять
/// нельзя, иначе его остатки печатаются литеральным текстом.
pub(crate) fn parse_content(raw: &str) -> Option<Vec<ContentItem>> {
    let bytes = raw.as_bytes();
    let mut at = 0usize;
    let mut out = vec![];
    while at < bytes.len() {
        let ch = raw[at..].chars().next()?;
        if ch.is_whitespace() {
            at += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' {
            let body = at + ch.len_utf8();
            let len = crate::css::skip_string(&raw[body..], ch);
            // Незакрытая строка обрывается переводом строки — значение негодно.
            if !raw[body..body + len].ends_with(ch) {
                return None;
            }
            out.push(ContentItem::Str(unescape_content(
                &raw[body..body + len - ch.len_utf8()],
            )));
            at = body + len;
            continue;
        }
        // Дальше только функция: `counter(`, `counters(`, `attr(`.
        let rest = &raw[at..];
        let open = rest.find('(')?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let close = at + open + 1 + find_close(&rest[open + 1..])?;
        let args = crate::css::split_args(&raw[at + open + 1..close]);
        let arg = |i: usize| -> Option<String> {
            let a = args.get(i)?.trim();
            let unq = a
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix('"'))
                .or_else(|| a.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')));
            Some(unq.map_or_else(|| a.to_string(), unescape_content))
        };
        match name.as_str() {
            "counter" if args.len() == 1 || args.len() == 2 => out.push(ContentItem::Counter(
                arg(0)?,
                arg(1).unwrap_or_else(|| "decimal".to_string()),
            )),
            "counters" if args.len() == 2 || args.len() == 3 => out.push(ContentItem::Counters(
                arg(0)?,
                arg(1)?,
                arg(2).unwrap_or_else(|| "decimal".to_string()),
            )),
            "attr" if args.len() == 1 => out.push(ContentItem::Attr(arg(0)?)),
            // ПРОБОВАЛИ И ОТКАТИЛИ: принимать `url()` (§12.2 объявляет его
            // действительным) и класть в псевдоэлемент синтетический `<img>`.
            // Проба по 195 парам `generated-content`: флипов ноль, потеряна
            // `before-after-table-whitespace-001` (0.15 -> 0.58) и просела
            // `before-after-images-001` (0.00 -> 0.41). Обе требуют, чтобы
            // НЕНАЙДЕННАЯ картинка давала коробку НУЛЕВОГО размера — сперва
            // это, потом уже `url()`.
            _ => return None,
        }
        at = close + 1;
    }
    (!out.is_empty()).then_some(out)
}

/// Индекс парной закрывающей скобки от места ПОСЛЕ открывающей.
fn find_close(after_open: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = 0usize;
    while at < after_open.len() {
        let ch = after_open[at..].chars().next()?;
        match ch {
            '"' | '\'' => {
                at += ch.len_utf8();
                at += crate::css::skip_string(&after_open[at..], ch);
                continue;
            }
            '(' => depth += 1,
            ')' if depth == 0 => return Some(at),
            ')' => depth -= 1,
            _ => {}
        }
        at += ch.len_utf8();
    }
    None
}

/// Экранирование внутри строки содержимого: `\A` — перевод строки, прочие
/// коды — свои знаки, `\"` — сама кавычка.
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
    crate::css::unescape(text)
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
fn is_generic(lower: &str) -> bool {
    generic_family(lower).is_some() || matches!(lower, "monospace" | "ui-monospace")
}

/// Семейство за родовое `sans-serif`; оно же — умолчание документа.
pub const GENERIC_SANS: &str = "Segoe UI";

fn parse_overflow(v: &str) -> Option<Overflow> {
    match v {
        "hidden" => Some(Overflow::Hidden),
        "clip" => Some(Overflow::Clip),
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

fn resolve_vars(value: &str, vars: &Decls) -> String {
    let mut out = resolve_vars_once(value, vars);
    for _ in 1..VAR_DEPTH {
        if !out.contains("var(") {
            break;
        }
        let next = resolve_vars_once(&out, vars);
        if next == out {
            break;
        }
        out = next;
    }
    out
}

fn resolve_vars_once(value: &str, vars: &Decls) -> String {
    if !value.contains("var(") {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find("var(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 4..];
        // Конец записи — ПАРНАЯ скобка, а не первая попавшаяся: запасное
        // значение само бывает записью со скобками. Пока бралась первая,
        // `var(--c, rgba(0,0,0,.5))` при ЗАДАННОЙ переменной давал `red)` —
        // лишняя скобка убивала значение. При незаданной выходило случайно
        // верно, поэтому дефект и жил (CSS Variables §3).
        let Some(close) = balanced_close(after) else {
            out.push_str(&rest[at..]);
            return out;
        };
        let inner = &after[..close];
        // Запятая тоже ищется на верхнем уровне: внутри `rgba(0,0,0,.5)` их
        // три, и разрез по первой откусил бы запасное значение.
        let (name, fallback) = match top_level_comma(inner) {
            Some(i) => (inner[..i].trim(), inner[i + 1..].trim()),
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
    /// Доля ШИРИНЫ СЕТКИ (`25%`) — не путать с долей остатка (`fr`).
    Pct(f32),
    /// Длина в единицах ШРИФТА (`2ch`, `1em`, `8rem`): кегль и метрики на
    /// разборе ещё неизвестны, величина считается вместе с прочими `em`.
    Font(Len),
    Auto,
    MinContent,
    MaxContent,
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
        if let Some(inner) = t
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
            && let Some(Len::Px(px)) = Len::parse(inner.trim())
        {
            return Some(TrackSize::MinMax(Track::Auto, Track::Px(px)));
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

/// Разбить список дорожек по пробелам, не заходя внутрь скобок.
/// ЗАМЕРЕНО И ОТКАЧЕНО: считать квадратную скобку так же, как круглую, чтобы
/// многоимённая группа `[a b] 50px` не разрывалась по пробелу. Разрыв правда
/// роняет ВЕСЬ список дорожек (`one(&token)?` на куске `[a`), но полный свод
/// CSS3 дал приобретено 0, потеряно 1 —
/// `grid-auto-repeat-multiple-values-005` 0.00 -> 3.60. Проверено по частям:
/// счёт дорожек ни при чём, весь итог даёт сама группировка. Возвращаться
/// вместе с настоящими именами линий (план — `target/scout-linenames.md`).
fn tokenize_tracks(v: &str) -> Vec<String> {
    split_outside_parens(v)
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
/// `linear-gradient(...)` и `radial-gradient(...)`.
///
/// Позиции стопов сохраняются: без них полосы не расставить, а именно они
/// задают, где цвет меняется.
pub(crate) fn parse_gradient(v: &str) -> Option<Gradient> {
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
    // (в hsl, метод longer?) — интерполяция нужна ЛЮБОМУ `in hsl`:
    // shorter (дефолт) идёт короткой дугой тона, longer — длинной; эталоны
    // пишут ref через `in hsl` без метода (gradient-longer-hue-hsl-002-ref).
    // Метод дуги тона (css-color-4 §hue-interpolation): shorter (дефолт),
    // longer, increasing, decreasing.
    let hsl_interp: Option<u8> = interp
        .filter(|i| matches!(i.split_whitespace().next(), Some("hsl") | Some("hwb")))
        .map(|i| {
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
    if interp.is_some() && head.is_empty() {
        idx = 1;
    }
    let angle = match head {
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
    for p in &parts[idx..] {
        let words = split_outside_parens(p);
        let Some(colour) = words.first().and_then(|w| Color::parse(w)) else {
            continue;
        };
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
    if raw.len() < 2 {
        return None;
    }
    // Стопы без позиции распределяются равномерно — так же, как в CSS.
    let last = raw.len() - 1;
    let mut stops: Vec<(Color, f32)> = raw
        .iter()
        .enumerate()
        .map(|(i, (c, pos))| (*c, pos.unwrap_or(i as f32 / last as f32)))
        .collect();
    // `in hsl longer hue`: тон идёт ДЛИННОЙ дугой (css-images-4 §3.4.1.1).
    // Растр интерполирует линейно в sRGB, поэтому дуга выкладывается
    // СИНТЕТИЧЕСКИМИ промежуточными стопами (gradient-longer-hue-hsl-001).
    // ЗАМЕРЕНО В МИНУС без флага (−13/+2: дуга даёт 0.6–0.8% против
    // эталонов — точность растеризации полос; single-stop 18.25) —
    // остаётся за HSL_ARC до точной математики.
    if let (Some(method), true) = (
        hsl_interp.filter(|_| std::env::var("HSL_ARC").is_ok()),
        stops.len() >= 2,
    ) {
        let mut dense: Vec<(Color, f32)> = vec![];
        for w in stops.windows(2) {
            let ((c1, p1), (c2, p2)) = (w[0], w[1]);
            dense.push((c1, p1));
            let (h1, s1, l1) = crate::color_space::rgb_to_hsl(c1);
            let (h2, s2, l2) = crate::color_space::rgb_to_hsl(c2);
            // shorter: дуга в (-180,180]; longer — противоположная ей.
            let mut d = (h2 - h1).rem_euclid(360.0);
            match method {
                // shorter: дуга в (-180, 180].
                0 => {
                    if d > 180.0 {
                        d -= 360.0;
                    }
                }
                // longer: противоположная короткой.
                1 => {
                    if d > 180.0 {
                        d -= 360.0;
                    }
                    if d > 0.0 {
                        d -= 360.0;
                    } else {
                        d += 360.0;
                    }
                }
                // increasing: тон только растёт (0..360).
                2 => {}
                // decreasing: тон только убывает.
                _ => {
                    if d > 0.0 {
                        d -= 360.0;
                    }
                }
            }
            const K: usize = 48;
            for i in 1..K {
                let t = i as f32 / K as f32;
                let h = (h1 + d * t).rem_euclid(360.0);
                let (r, g, b) =
                    crate::color_space::hsl_to_rgb(h, s1 + (s2 - s1) * t, l1 + (l2 - l1) * t);
                let a = c1.a + (c2.a - c1.a) * t;
                dense.push((Color { r, g, b, a }, p1 + (p2 - p1) * t));
            }
        }
        dense.push(*stops.last().unwrap());
        stops = dense;
    }
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
    })
}
/// Стиль обводки: 0 = не рисуется (none/hidden), 1 = рисуется.
fn outline_style_of(v: &str) -> Option<u8> {
    match v {
        "none" | "hidden" => Some(0),
        "solid" | "dotted" | "dashed" | "double" | "groove" | "ridge" | "inset" | "outset"
        | "auto" => Some(1),
        _ => None,
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

/// Присвоить размер коробки: отрицательная длина делает объявление
/// НЕВАЛИДНЫМ, и слот не трогается вовсе (§10) — повторное свойство
/// `width: 0; width: -1px` обязано оставить нуль от первой записи, а
/// сброс в None делал ширину авто и красил красное (width-001 и родня).
/// Сплошная заливка как градиент из одного цвета (image()/cross-fade()).
fn solid_gradient(c: Color) -> Gradient {
    Gradient {
        angle_deg: 180.0,
        from: c,
        to: c,
        ..Default::default()
    }
}

/// Годное имя семейства: строка в кавычках либо ряд идентификаторов.
///
/// Идентификатор по §4.1.3 начинается с буквы, подчёркивания, не-ASCII знака
/// или экранирования; за ними идут буквы, цифры, дефисы, подчёркивания и
/// экранирования. Цифра первой запрещена, дефис с цифрой следом — тоже.
fn family_name_ok(part: &str) -> bool {
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

fn assign_size(slot: &mut Option<Len>, v: &str) {
    let parsed = Len::parse(v);
    // Отрицательный размер невалиден в ЛЮБОЙ единице (CSS 2.1 §10.4:
    // `min-width`/`min-height` — «Value: <length> | <percentage> | inherit»,
    // отрицательные значения не допускаются). Прежде отбраковывались только
    // px, проценты и em, а `min-height: -1ex` доживал до раскладки.
    let negative = match parsed {
        Some(
            Len::Px(n)
            | Len::Pct(n)
            | Len::Em(n)
            | Len::Vh(n)
            | Len::Vw(n)
            | Len::Ch(n)
            | Len::Ex(n)
            | Len::Ic(n)
            | Len::Lh(n),
        ) => n < 0.0,
        Some(Len::EmPx(a, b) | Len::LhPx(a, b)) => a < 0.0 || b < 0.0,
        _ => false,
    };
    if negative {
        return;
    }
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
        assert_eq!(c.color, super::super::value::Color::parse("red"));
        // Незаданная переменная берёт запасное значение ЦЕЛИКОМ.
        let mut c = super::Computed::default();
        c.apply_decls_with_vars(
            &super::super::css::parse_decls("color: var(--none, rgba(0,0,0,1))"),
            &super::super::css::Decls::new(),
        );
        assert_eq!(c.color, super::super::value::Color::parse("rgba(0,0,0,1)"));
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
        };
        let late = super::super::css::Rule {
            sel: super::super::css::Selector::parse("p.x").expect("селектор класса"),
            decls: super::super::css::parse_decls("color: green"),
            order: 1,
            origin: 1,
        };
        let mut matched = vec![&early, &late];
        let c = super::Computed::resolve(&mut matched, &super::super::css::Decls::new());
        assert_eq!(c.color, super::super::value::Color::parse("red"));
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
        };
        let author = super::super::css::Rule {
            sel: super::super::css::Selector::parse("*").expect("универсальный селектор"),
            decls: super::super::css::parse_decls("margin-top: 0"),
            order: 1,
            origin: 1,
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
fn initial_value(key: &str) -> Option<&'static str> {
    Some(match key {
        "border" => "0 none",
        "border-radius" => "0",
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
