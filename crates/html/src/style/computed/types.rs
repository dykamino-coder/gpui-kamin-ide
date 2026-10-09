//! Значения свойств: перечисления и записи, из которых собран Computed.

use crate::style::computed::*;

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

pub(crate) fn parse_decor_style(t: &str) -> Option<DecorStyle> {
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
pub(crate) fn parse_decor_length(t: &str) -> Option<DecorLen> {
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
pub(crate) fn parse_decor_thickness(t: &str) -> Option<DecorLen> {
    Some(match t {
        "auto" => DecorLen::Auto,
        "from-font" => DecorLen::FromFont,
        "thin" => DecorLen::Px(1.0),
        "medium" => DecorLen::Px(3.0),
        "thick" => DecorLen::Px(5.0),
        _ => parse_decor_length(t)?,
    })
}

impl Default for BgSize {
    fn default() -> Self {
        BgSize::Auto
    }
}
