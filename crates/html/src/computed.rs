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

/// Вид маркера списка (`list-style-type`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
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
    pub min_inline: Option<Len>,
    pub min_block: Option<Len>,
    pub max_inline: Option<Len>,
    pub max_block: Option<Len>,
    pub padding: LogicalSides,
    pub margin: LogicalSides,
    pub inset: LogicalSides,
}

/// Четыре логические стороны коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogicalSides {
    pub inline_start: Option<Len>,
    pub inline_end: Option<Len>,
    pub block_start: Option<Len>,
    pub block_end: Option<Len>,
}

/// `outline`: рамка ВНЕ коробки, не влияющая на раскладку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outline {
    pub width: Option<Len>,
    pub color: Option<Color>,
    pub offset: Option<Len>,
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
    /// Доля ширины коробки, которую надо заполнить (`text-fit: grow 75%`).
    pub target: f32,
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
#[derive(Clone, Debug, PartialEq)]
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
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            rotate_rad: 0.0,
            skew_rad: (0.0, 0.0),
            scale: (1.0, 1.0),
            translate: (0.0, 0.0),
            translate_pct: (0.0, 0.0),
        }
    }
}

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
}

#[derive(Clone, Debug, Default)]
pub struct Computed {
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
    pub overflow_y: Option<Overflow>,
    pub opacity: Option<f32>,

    pub background: Option<Color>,
    /// Относительный цвет фона (css-color-5): функция с `from currentColor`
    /// не решается при разборе — она наследуется КАК ФУНКЦИЯ и считается от
    /// цвета каждого элемента заново.
    pub background_rcs: Option<String>,
    /// `background-color: inherit`: фон не наследуемый, слово переносит
    /// вычисленное значение родителя (включая нерешённую функцию).
    pub(crate) background_inherit: bool,
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
    /// Форма курсора: у GPUI набор совпадает с CSS почти буква в букву.
    pub cursor: Option<String>,
    /// `visibility: hidden` — место занимает, но не рисуется.
    pub hidden: Option<bool>,
    /// `visibility: collapse` — не «невидимый», а ВЫБРОШЕННЫЙ из строки
    /// гибкого контейнера: перенос и размеры считаются без него.
    pub collapsed: Option<bool>,
    pub letter_spacing: Option<Len>,
    pub ellipsis: Option<bool>,
    /// `list-style: none` — навигация, свёрстанная на списках, иначе идёт с
    /// точками.
    pub no_marker: Option<bool>,
    /// Вид маркера, если документ его задал.
    pub marker: Option<Marker>,
    pub object_fit: Option<String>,

    /// `aspect-ratio` — отношение ширины к высоте.
    pub aspect_ratio: Option<f32>,
    /// `order`: визуальный порядок в гибкой строке. Раскладка под нами его не
    /// знает, поэтому детей переставляет сам сборщик дерева.
    pub order: Option<i32>,
    pub align_content: Option<Justify>,
    /// `justify-items`/`justify-self` — поперечная ось В СЕТКЕ.
    pub justify_items: Option<Align>,
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
    pub justify_self: Option<Align>,

    pub grid_rows: Option<Vec<TrackSize>>,
    pub grid_auto_cols: Option<TrackSize>,
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
    pub border_dashed: Option<bool>,
    /// `border-style: dotted` — точечный узор.
    pub border_dotted: Option<bool>,
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
    /// `word-break: keep-all` — иероглифическое письмо переносится ТОЛЬКО по
    /// пробелам, между знаками разрыв запрещён.
    pub keep_all: Option<bool>,
    /// `white-space: break-spaces` — сохранённый пробел НЕ свисает за край:
    /// он занимает место в строке, и после каждого такого пробела разрешён
    /// перенос. Отличие от `pre-wrap`, где хвостовые пробелы висят снаружи.
    pub break_after_spaces: Option<bool>,
    /// `-webkit-line-clamp`: сколько строк оставить.
    pub line_clamp: Option<u32>,
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
    /// Точка отсчёта преобразования В ТОЧКАХ по осям — когда записана длиной,
    /// а не долей. Долю из неё делает отрисовка: размер коробки известен там.
    pub transform_origin_px: (Option<f32>, Option<f32>),
    /// `float`: -1 — влево, 1 — вправо, 0 — не обтекается.
    pub float: Option<i8>,
    /// `clear` — прервать обтекание перед этим блоком.
    pub clear: Option<bool>,
    /// `writing-mode`: вертикальное письмо — блоки идут по горизонтали.
    pub vertical: Option<bool>,
    /// `writing-mode: vertical-rl` — блоки идут справа налево.
    pub vertical_rl: Option<bool>,
    /// Ограничение ОРТОГОНАЛЬНОГО потока: определённый размер ближайшего
    /// предка-контейнера прокрутки по оси потока (CSS Writing Modes §7.3).
    /// Наследуется вниз, потому что искать его надо ВВЕРХ по дереву, а на
    /// момент раскладки ребёнка предков уже не видно.
    pub ortho_limit: Option<f32>,
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
    /// `text-orientation: upright` — глифы стоят прямо, а не лежат боком.
    /// Меняет и меру `ch`: продвижение нуля идёт вдоль оси СТРОКИ, а она в
    /// вертикальном письме вертикальна, то есть равна кеглю.
    pub upright: Option<bool>,
    /// Логические стороны и размеры до перевода в физические.
    pub logical: Option<Box<Logical>>,
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
    pub inline_border: Option<(Color, f32)>,
    /// Поля вокруг фона строчного бокса: `padding` у `<span>`.
    pub inline_pad: Option<(f32, f32)>,
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
    pub clip_margin: Option<f32>,
    /// `clip-path: polygon(…)`: вершины в долях или точках коробки.
    pub clip_polygon: Option<Vec<(Len, Len)>>,
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
    /// `clip-path`/`mask`: обрезка по кругу или скруглённому прямоугольнику.
    /// Хранится долей радиуса от меньшей стороны либо радиусом в точках.
    pub clip_round: Option<f32>,
    /// `filter`: цветовые преобразования, применённые к собственным цветам.
    pub filter: Option<Filter>,

    /// `background-image: url(...)` — ссылка на картинку-заливку.
    pub bg_image: Option<String>,
    pub bg_size: BgSize,
    pub bg_pos: BgPos,
    pub bg_repeat: Option<BgRepeat>,
    /// `content` псевдоэлемента: строка, `attr(имя)` либо `counter(имя)`.
    pub content: Option<String>,
    /// `counter-reset` — обнулить счётчик с этого узла.
    pub counter_reset: Option<String>,
    /// `counter-increment` — увеличить счётчик на этом узле.
    pub counter_increment: Option<String>,
    /// Возможности шрифта (`font-feature-settings`, `font-variant`).
    pub font_features: Vec<(String, u32)>,
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
    pub fn resolve_logical(&mut self) {
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
        let vertical = false;
        let rl = false;
        let rtl = self.rtl == Some(true);
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
        if vertical {
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
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
        let sides = [
            (&logical.padding, 0u8),
            (&logical.margin, 1),
            (&logical.inset, 2),
        ];
        for (from, which) in sides {
            let to = match which {
                0 => &mut self.padding,
                1 => &mut self.margin,
                _ => &mut self.inset,
            };
            let (i_start, i_end, b_start, b_end) = if vertical {
                let (bs, be) = if rl { (3u8, 1u8) } else { (1, 3) };
                let (is, ie) = if rtl { (2u8, 0u8) } else { (0, 2) };
                (is, ie, bs, be)
            } else if rtl {
                (1u8, 3u8, 0u8, 2u8)
            } else {
                (3u8, 1u8, 0u8, 2u8)
            };
            for (side, val) in [
                (i_start, from.inline_start),
                (i_end, from.inline_end),
                (b_start, from.block_start),
                (b_end, from.block_end),
            ] {
                let slot = match side {
                    0 => &mut to.top,
                    1 => &mut to.right,
                    2 => &mut to.bottom,
                    _ => &mut to.left,
                };
                set(slot, val);
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
            _ => {}
        };
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                match *one {
                    Some(Len::Vw(k)) => *one = Some(Len::Px(k * viewport.0)),
                    Some(Len::Vh(k)) => *one = Some(Len::Px(k * viewport.1)),
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
            Some(Len::Ch(k)) => *l = Some(Len::Px(k * ch)),
            Some(Len::Ex(k)) => *l = Some(Len::Px(k * ex)),
            Some(Len::Ic(k)) => *l = Some(Len::Px(k * ic)),
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
            font_size: self.font_size,
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
            line_clamp: self.line_clamp,
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
        // Фильтр — последнее, что происходит с элементом: он преобразует уже
        // готовые цвета, а не участвует в каскаде.
        if let Some(f) = c.filter {
            c.background = c.background.map(|col| f.apply(col));
            c.color = c.color.map(|col| f.apply(col));
            c.border_color = c.border_color.map(|col| f.apply(col));
            for side in c.border_colors.iter_mut() {
                *side = side.map(|col| f.apply(col));
            }
            if let Some(g) = c.gradient.as_mut() {
                g.from = f.apply(g.from);
                g.to = f.apply(g.to);
                for stop in g.stops.iter_mut() {
                    stop.0 = f.apply(stop.0);
                }
            }
            for sh in c.shadows.iter_mut() {
                sh.color = f.apply(sh.color);
            }
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
    fn apply_pass(&mut self, d: &Decls, vars: &Decls, important: bool) {
        let mut keys: Vec<&String> = d.keys().collect();
        keys.sort_by_key(|k| (k.matches('-').count(), k.as_str()));
        for k in &keys {
            let Some(v) = d.get(*k) else { continue };
            if k.starts_with("--") || is_important(v) != important {
                continue;
            }
            let resolved = resolve_vars(strip_important(v), vars);
            self.apply_one(k, &resolved);
        }
    }

    fn apply_one(&mut self, key: &str, val: &str) {
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
            "display" => {
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
                    "grid-lanes" | "masonry" | "inline-grid-lanes" | "inline-masonry" => {
                        Some(Display::GridLanes)
                    }
                    "none" => Some(Display::None),
                    "block" => Some(Display::Block),
                    "inline-block" => Some(Display::InlineBlock),
                    "inline" => Some(Display::InlineBlock),
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
                        Some(Display::TableRowGroup)
                    }
                    "flow-root" => Some(Display::Block),
                    "table-row" => Some(Display::TableRow),
                    "table-cell" => Some(Display::TableCell),
                    // Заголовок таблицы — обычный блок. Колонки коробок не
                    // порождают вовсе: они только задают ширину столбцам.
                    "table-caption" => Some(Display::Block),
                    "table-column" | "table-column-group" => Some(Display::None),
                    // Запись из ДВУХ слов: из неё берётся только внутренний
                    // вид «лунки» — его иначе не выразить вовсе. Полный разбор
                    // двух слов ЗАМЕРЕН и откачен (см. комментарий выше).
                    two if two.split_whitespace().any(|w| w == "grid-lanes" || w == "masonry") => {
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
            "flex-grow" => self.flex_grow = v.parse().ok(),
            "flex-shrink" => self.flex_shrink = v.parse().ok(),
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
            "flex-basis" => self.flex_basis = Len::parse(v),
            "align-self" => {
                if let Ok(a) = align_keyword(v) {
                    self.align_self = a;
                }
            }
            "align-items" => {
                if let Ok(a) = align_keyword(v) {
                    self.align_items = a;
                }
            }
            // `space-evenly` и `space-around` различаются шириной крайних
            // промежутков — сведение их в одно значение расходилось с
            // браузером на 27 точек (поймано сравнением).
            "justify-content" => self.justify_content = parse_justify(v),
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
            "grid-template-columns" if v.contains("auto-fill") || v.contains("auto-fit") => {
                self.grid_auto_fill_min = auto_fill_min(v);
                self.auto_repeat_cols = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_min,
                    track_pct: auto_fill_pct(v),
                });
            }
            // То же по РЯДАМ: у раскладки лунками дорожки задают ряды, когда
            // `grid-lanes-direction: row` (`row-auto-repeat-001`).
            "grid-template-rows" if v.contains("auto-fill") || v.contains("auto-fit") => {
                self.grid_auto_fill_row = auto_fill_min(v);
                self.auto_repeat_rows = Some(AutoRepeat {
                    fit: v.contains("auto-fit"),
                    track: self.grid_auto_fill_row,
                    track_pct: auto_fill_pct(v),
                });
            }
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
            "border-collapse" => self.border_collapse = Some(v == "collapse"),
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
                    // `fixed` отсчитывается от окна: своей системы отсчёта у
                    // него нет, и сборщик дерева ставит его отдельным слоем.
                    "fixed" => Some(Position::Fixed),
                    "sticky" | "-webkit-sticky" => Some(Position::Sticky),
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
            // Поле обрезки: край, по которому режется вылезшее содержимое,
            // отодвигается наружу (css-overflow-3 §5). Запись допускает и
            // указание коробки отсчёта — её мы не различаем, край один.
            "overflow-clip-margin" => {
                self.clip_margin = v
                    .split_whitespace()
                    .find_map(Len::parse)
                    .and_then(|l| match l {
                        Len::Px(px) if px > 0.0 => Some(px),
                        _ => None,
                    })
            }

            // Сокращение несёт всё сразу: `background: #fff url(a.png) no-repeat`
            // — и цвет, и картинку, и режим повтора. Раньше побеждало что-то
            // одно, и картинка терялась при заданном цвете.
            "background" | "background-color" => {
                if v == "inherit" {
                    self.background_inherit = true;
                    return;
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
                    // Цвет ищется в НИЖНЕМ слое: он один его и допускает.
                    if layers.len() > 1 {
                        for token in split_outside_parens(bottom) {
                            if let Some(c) = Color::parse(&token) {
                                self.background = Some(c);
                            }
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
                for token in split_outside_parens(v) {
                    match token.as_str() {
                        "no-repeat" => self.bg_repeat = Some(BgRepeat::NoRepeat),
                        "repeat-x" => self.bg_repeat = Some(BgRepeat::RepeatX),
                        "repeat-y" => self.bg_repeat = Some(BgRepeat::RepeatY),
                        "repeat" => self.bg_repeat = Some(BgRepeat::Repeat),
                        "cover" => self.bg_size = BgSize::Cover,
                        "contain" => self.bg_size = BgSize::Contain,
                        t if t.starts_with("url(") => {}
                        t => {
                            if let Some(c) = Color::parse(t) {
                                self.background = Some(c);
                            }
                        }
                    }
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
                // `inset` в записи означает тень ВНУТРИ фигуры: раньше такая
                // запись просто не рисовалась.
                let (inset, outer): (Vec<&str>, Vec<&str>) = crate::css::split_args(v)
                    .into_iter()
                    .partition(|one| one.contains("inset"));
                self.shadows = parse_shadows(&outer.join(","));
                self.inset_shadows = parse_shadows(&inset.join(",").replace("inset", " "));
            }

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
                self.font_family = v
                    .split(',')
                    .map(|f| f.trim().trim_matches(is_quote))
                    .find(|f| {
                        let lower = f.to_ascii_lowercase();
                        !f.is_empty()
                            && !is_generic(&lower)
                            && !matches!(lower.as_str(), "inherit" | "initial")
                    })
                    .map(str::to_string)
                    .or_else(|| generic.map(str::to_string));
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
            "visibility" => {
                self.hidden = Some(v == "hidden" || v == "collapse");
                self.collapsed = Some(v == "collapse");
            }
            "letter-spacing" => self.letter_spacing = Len::parse_spacing(v),
            "text-overflow" => self.ellipsis = Some(v == "ellipsis"),
            "list-style" | "list-style-type" => {
                self.no_marker = Some(v.contains("none"));
                // Вид маркера задаёт документ, а не приложение: у списка
                // возможностей и у нумерованного перечня он разный.
                for token in v.split_whitespace() {
                    self.marker = match token {
                        "disc" => Some(Marker::Disc),
                        "circle" => Some(Marker::Circle),
                        "square" => Some(Marker::Square),
                        "decimal" => Some(Marker::Decimal),
                        "lower-alpha" | "lower-latin" => Some(Marker::LowerAlpha),
                        "upper-alpha" | "upper-latin" => Some(Marker::UpperAlpha),
                        "lower-roman" => Some(Marker::LowerRoman),
                        _ => self.marker,
                    };
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
                let logical = self.logical();
                let target = match which.as_str() {
                    "padding" => &mut logical.padding,
                    "margin" => &mut logical.margin,
                    _ => &mut logical.inset,
                };
                if block {
                    target.block_start = a;
                    target.block_end = b;
                } else {
                    target.inline_start = a;
                    target.inline_end = b;
                }
            }
            "padding-inline-start" => self.logical().padding.inline_start = Len::parse(v),
            "padding-inline-end" => self.logical().padding.inline_end = Len::parse(v),
            "padding-block-start" => self.logical().padding.block_start = Len::parse(v),
            "padding-block-end" => self.logical().padding.block_end = Len::parse(v),
            "margin-inline-start" => self.logical().margin.inline_start = Len::parse(v),
            "margin-inline-end" => self.logical().margin.inline_end = Len::parse(v),
            "margin-block-start" => self.logical().margin.block_start = Len::parse(v),
            "margin-block-end" => self.logical().margin.block_end = Len::parse(v),
            "inset-inline-start" => self.logical().inset.inline_start = Len::parse(v),
            "inset-inline-end" => self.logical().inset.inline_end = Len::parse(v),
            "inset-block-start" => self.logical().inset.block_start = Len::parse(v),
            "inset-block-end" => self.logical().inset.block_end = Len::parse(v),

            // --- Раскладка --------------------------------------------------
            "aspect-ratio" => {
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
            "align-content" => self.align_content = parse_justify(v),
            "justify-items" => self.justify_items = parse_align(v),
            // Значение бывает составным: `row fill-reverse`, `column
            // track-reverse`. Сверка со строкой ЦЕЛИКОМ путала ось на каждом
            // таком тесте.
            "grid-lanes-pack" => self.lanes_dense = v.contains("dense"),
            // Первая часть — ось лунок, дальше — реверсы: `fill-reverse`
            // заполняет лунки с другого конца, `track-reverse` перечисляет
            // сами лунки в обратном порядке (css-grid-3).
            "grid-lanes-direction" => {
                self.lanes_row = Some(v.split_whitespace().next() == Some("row"));
                self.lanes_fill_reverse = v.split_whitespace().any(|w| w == "fill-reverse");
                self.lanes_track_reverse = v.split_whitespace().any(|w| w == "track-reverse");
            }
            "justify-self" => self.justify_self = parse_align(v),
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
                self.grid_auto_cols = parse_tracks(v).and_then(|t| t.into_iter().next())
            }
            "grid-auto-rows" => {
                self.grid_auto_rows = parse_tracks(v).and_then(|t| t.into_iter().next())
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
            "z-index" => self.z_index = v.parse().ok(),

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
            "border-inline" => {
                self.apply_border_shorthand(v, Some(1));
                self.apply_border_shorthand(v, Some(3));
            }
            "border-block" => {
                self.apply_border_shorthand(v, Some(0));
                self.apply_border_shorthand(v, Some(2));
            }
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
                // Умолчание CSS — `medium`, три точки: без него запись
                // `outline: solid red` не рисовала ничего.
                let mut o = self.outline.unwrap_or(Outline {
                    width: Some(Len::Px(3.0)),
                    color: None,
                    offset: None,
                });
                for token in v.split_whitespace() {
                    if token == "none" {
                        o.width = Some(Len::Px(0.0));
                    } else if let Some(l) = Len::parse(token) {
                        o.width = Some(l);
                    } else if let Some(c) = Color::parse(token) {
                        o.color = Some(c);
                    }
                }
                self.outline = Some(o);
            }
            "outline-width" | "outline-color" | "outline-offset" => {
                let mut o = self.outline.unwrap_or_default();
                match key {
                    "outline-width" => o.width = Len::parse(v),
                    "outline-color" => o.color = Color::parse(v),
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
                if v.starts_with("linear-gradient(") || v.starts_with("radial-gradient(") {
                    self.gradient = parse_gradient(v);
                } else if let Some(url) = parse_url(v) {
                    self.bg_image = Some(url);
                }
            }

            // --- Текст -------------------------------------------------------
            // `font: [начертание] [вес] размер[/интерлиньяж] семейство`.
            "font" => {
                // `font: 50px / 1 Ahem` — вокруг косой черты разрешены пробелы,
                // а кегль с высотой строки обязаны разбираться одним куском:
                // иначе «/ 1 Ahem» уезжало в семейство шрифта целиком.
                let value = join_slash(v);
                let (head, family) = split_font(&value);
                for token in head.split_whitespace() {
                    match token {
                        "italic" | "oblique" => self.italic = Some(true),
                        "bold" | "bolder" => self.font_weight = Some(700),
                        t if t.starts_with(|c: char| c.is_ascii_digit()) => {
                            if let Some((size, lh)) = t.split_once('/') {
                                self.apply_one("font-size", size);
                                self.apply_one("line-height", lh);
                            } else if Len::parse(t).is_some()
                                && !t.chars().all(|c| c.is_ascii_digit())
                            {
                                self.apply_one("font-size", t);
                            } else {
                                self.font_weight = t.parse().ok();
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
                self.word_space_char = match v {
                    "space" => Some(' '),
                    "ideographic-space" => Some('\u{3000}'),
                    _ => None,
                };
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
                        target: 1.0,
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
                                        f.target = n / 100.0;
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
                self.vertical_shift = match v {
                    "super" => Some(-1.0 / 3.0),
                    "sub" => Some(1.0 / 5.0),
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
            "-webkit-line-clamp" | "line-clamp" => self.line_clamp = v.parse().ok(),

            // --- Прочее ------------------------------------------------------
            "pointer-events" => self.pointer_events_none = Some(v == "none"),
            "table-layout" => self.table_fixed = Some(v == "fixed"),

            // --- Фоновая картинка --------------------------------------------
            // Запись бывает и ПОосевой: `repeat space`, `round no-repeat`.
            // Один keyword задаёт обе оси, два — свою каждой.
            "background-repeat" => {
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
                        let mut it = v.split_whitespace();
                        let w = it.next().and_then(Len::parse);
                        let h = it.next().and_then(Len::parse);
                        if w.is_none() && h.is_none() {
                            BgSize::Auto
                        } else {
                            BgSize::Fixed(w, h)
                        }
                    }
                }
            }
            "background-position" => {
                let word = |t: &str| -> Option<Len> {
                    match t {
                        "left" | "top" => Some(Len::Pct(0.0)),
                        "center" => Some(Len::Pct(0.5)),
                        "right" | "bottom" => Some(Len::Pct(1.0)),
                        other => Len::parse(other),
                    }
                };
                let mut it = v.split_whitespace();
                let first = it.next().unwrap_or("");
                let second = it.next();
                // Одно значение задаёт горизонталь, вертикаль тогда по центру.
                self.bg_pos = BgPos {
                    x: word(first),
                    y: second.and_then(word).or(Some(Len::Pct(0.5))),
                };
                if second.is_none() && matches!(first, "top" | "bottom") {
                    self.bg_pos = BgPos {
                        x: Some(Len::Pct(0.5)),
                        y: word(first),
                    };
                }
            }
            "background-attachment" => {
                // `fixed` привязывает фон к окну, а не к элементу; ленты с
                // таким фоном у нас нет, поэтому разбор есть, действия нет.
            }

            // --- Псевдоэлементы и шрифт ---------------------------------------
            "counter-reset" => self.counter_reset = Some(v.to_string()),
            "counter-increment" => self.counter_increment = Some(v.to_string()),
            "content" => {
                self.content = match v {
                    // ПУСТАЯ строка — не то же самое, что `none`: коробка
                    // псевдоэлемента создаётся, просто в ней нет знаков. На
                    // этом стоит целый приём эталонов WPT — `::after` с
                    // `content: ""` и `inset: 0` накрывает красное зелёным
                    // (`overflow-wrap-anywhere-001` и родня).
                    "none" | "normal" => None,
                    other => Some(
                        other
                            .trim_matches(|c| c == '"' || c == '\'')
                            .replace("\\A", "\n")
                            .to_string(),
                    ),
                }
            }
            "font-feature-settings"
            | "font-variant"
            | "font-variant-caps"
            | "font-variant-numeric" => {
                // `font-variant: small-caps` — это возможность шрифта `smcp`;
                // остальные записываются четырёхбуквенным тегом напрямую.
                for token in v.split(',') {
                    let token = token.trim();
                    match token {
                        "small-caps" => self.font_features.push(("smcp".into(), 1)),
                        "all-small-caps" => self.font_features.push(("c2sc".into(), 1)),
                        "oldstyle-nums" => self.font_features.push(("onum".into(), 1)),
                        "lining-nums" => self.font_features.push(("lnum".into(), 1)),
                        "tabular-nums" => self.font_features.push(("tnum".into(), 1)),
                        "proportional-nums" => self.font_features.push(("pnum".into(), 1)),
                        "slashed-zero" => self.font_features.push(("zero".into(), 1)),
                        "normal" | "none" => {}
                        raw => {
                            let mut parts = raw.split_whitespace();
                            let tag = parts.next().unwrap_or("").trim_matches('"');
                            if tag.len() == 4 {
                                let on = match parts.next() {
                                    Some("off") | Some("0") => 0,
                                    Some(n) => n.parse().unwrap_or(1),
                                    None => 1,
                                };
                                self.font_features.push((tag.to_string(), on));
                            }
                        }
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
            "column-count" => self.column_count = v.parse().ok(),
            "column-width" => self.column_width = Len::parse(v),
            "columns" => {
                // `columns: <ширина> <число>` в любом порядке.
                for token in v.split_whitespace() {
                    match token.parse::<u16>() {
                        Ok(n) => self.column_count = Some(n),
                        Err(_) => self.column_width = Len::parse(token),
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
            | "animation-direction" => {
                let mut a = self.animation.clone().unwrap_or(AnimSpec {
                    name: String::new(),
                    seconds: 0.0,
                    infinite: false,
                    alternate: false,
                });
                for token in v.split_whitespace() {
                    if let Some(sec) = token.strip_suffix("ms").and_then(|n| n.parse::<f32>().ok())
                    {
                        a.seconds = sec / 1000.0;
                    } else if let Some(sec) =
                        token.strip_suffix('s').and_then(|n| n.parse::<f32>().ok())
                    {
                        a.seconds = sec;
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
                self.animation = (!a.name.is_empty()).then_some(a);
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
                        // `drop-shadow` и цветовые матрицы — не наш случай.
                        _ => {}
                    }
                }
                self.filter = Some(f);
            }

            // --- Преобразования -----------------------------------------------
            "transform" => {
                let mut t = self.transform.unwrap_or_default();
                for call in v.split(')') {
                    let Some((name, arg)) = call.split_once('(') else {
                        continue;
                    };
                    let (name, arg) = (name.trim(), arg.trim());
                    let nums: Vec<f32> = arg
                        .split(',')
                        .filter_map(|n| {
                            n.trim()
                                .trim_end_matches("deg")
                                .trim_end_matches("rad")
                                .trim_end_matches("px")
                                .trim_end_matches('%')
                                .parse::<f32>()
                                .ok()
                        })
                        .collect();
                    let first = nums.first().copied().unwrap_or(0.0);
                    // Угол в градусах — умолчание CSS; радианы помечены явно.
                    let angle = if arg.contains("rad") {
                        first
                    } else {
                        first.to_radians()
                    };
                    match name {
                        "rotate" | "rotateZ" => t.rotate_rad += angle,
                        "scale" => {
                            t.scale.0 *= first;
                            t.scale.1 *= nums.get(1).copied().unwrap_or(first);
                        }
                        "skew" => {
                            t.skew_rad.0 += angle;
                            let second = nums.get(1).copied().unwrap_or(0.0);
                            t.skew_rad.1 += if arg.contains("rad") {
                                second
                            } else {
                                second.to_radians()
                            };
                        }
                        "skewX" => t.skew_rad.0 += angle,
                        "skewY" => t.skew_rad.1 += angle,
                        "scaleX" => t.scale.0 *= first,
                        "scaleY" => t.scale.1 *= first,
                        // Поворот вокруг оси экрана БЕЗ перспективы — это
                        // прямая проекция на плоскость, то есть сжатие поперёк
                        // оси ровно на косинус угла (css-transforms-2 §11):
                        // `rotateX(60deg)` даёт половину высоты. Перспективы у
                        // нас нет, и приближением это не является — при
                        // `perspective: none` так считает и браузер.
                        "rotateX" => t.scale.1 *= angle.cos(),
                        "rotateY" => t.scale.0 *= angle.cos(),
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
                                t.rotate_rad += a * z;
                                t.scale.1 *= 1.0 - x.abs() * (1.0 - a.cos());
                                t.scale.0 *= 1.0 - y.abs() * (1.0 - a.cos());
                            }
                        }
                        // Третья ось без перспективы ничего не меняет: смещение
                        // по ней не видно, а масштаб по ней не на что влиять.
                        "translateZ" | "perspective" => {}
                        "scaleZ" => {}
                        "scale3d" => {
                            t.scale.0 *= first;
                            t.scale.1 *= nums.get(1).copied().unwrap_or(1.0);
                        }
                        "translate3d" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            for (i, dest) in [&mut t.translate.0, &mut t.translate.1]
                                .into_iter()
                                .enumerate()
                            {
                                if let Some(raw) = parts.get(i) {
                                    *dest += raw
                                        .trim_end_matches("px")
                                        .parse::<f32>()
                                        .unwrap_or(0.0);
                                }
                            }
                        }
                        // Проценты в сдвиге считаются от СВОЕГО размера —
                        // на этом стоит типовое центрирование
                        // `translate(-50%, -50%)`. Раньше процент срезался как
                        // единица, и элемент уезжал на 50 точек.
                        "translate" | "translateX" | "translateY" => {
                            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
                            let mut axis = |i: usize, x: bool| {
                                let Some(raw) = parts.get(i) else { return };
                                let value = raw
                                    .trim_end_matches("px")
                                    .trim_end_matches('%')
                                    .parse::<f32>()
                                    .unwrap_or(0.0);
                                let dest = if raw.ends_with('%') {
                                    if x {
                                        &mut t.translate_pct.0
                                    } else {
                                        &mut t.translate_pct.1
                                    }
                                } else if x {
                                    &mut t.translate.0
                                } else {
                                    &mut t.translate.1
                                };
                                *dest += value / if raw.ends_with('%') { 100.0 } else { 1.0 };
                            };
                            match name {
                                "translateX" => axis(0, true),
                                "translateY" => axis(0, false),
                                _ => {
                                    axis(0, true);
                                    axis(1, false);
                                }
                            }
                        }
                        // Скос выразить нечем: матрица GPUI хранит поворот и
                        // масштаб, но не сдвиг осей.
                        _ => {}
                    }
                }
                self.transform = Some(t);
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
                let mut px_it = v.split_whitespace();
                let (a, b) = (px_it.next().unwrap_or(""), px_it.next().unwrap_or("center"));
                self.transform_origin_px = (px_axis(a), px_axis(b));
                let mut it = v.split_whitespace();
                let first = it.next().unwrap_or("center");
                let second = it.next().unwrap_or("center");
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
            "clear" => self.clear = Some(v != "none"),
            "text-orientation" => self.upright = Some(v == "upright"),
            "writing-mode" => {
                // `sideways-*` отличается от `vertical-*` только поворотом
                // глифов, а направление потока у них общее.
                let vertical = v.starts_with("vertical") || v.starts_with("sideways");
                self.vertical = Some(vertical);
                self.vertical_rl = Some(vertical && v.ends_with("rl"));
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
                // `paint` и `strict` обрезают содержимое по коробке — это
                // ровно то, что делает скрытое переполнение.
                self.contain_paint = Some(v.contains("paint") || v.contains("strict"));
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
            "user-select" | "-webkit-user-select" => self.no_select = Some(matches!(v, "none")),
            "clip-path" | "mask" | "mask-image" => {
                // Круг и эллипс — это скруглённый прямоугольник с радиусом в
                // половину стороны; `inset(… round R)` — он же с заданным
                // радиусом. Многоугольник прямоугольной маской не выразить —
                // его гасит по форме сборка буфера группы.
                let v = v.trim();
                if let Some(rest) = v.strip_prefix("polygon(") {
                    let points: Vec<(Len, Len)> = rest
                        .trim_end_matches(')')
                        .split(',')
                        .filter_map(|pair| {
                            let mut it = pair.split_whitespace();
                            let x = Len::parse(it.next()?)?;
                            let y = Len::parse(it.next()?)?;
                            Some((x, y))
                        })
                        .take(8)
                        .collect();
                    if points.len() >= 3 {
                        self.clip_polygon = Some(points);
                    }
                } else if v.starts_with("circle(") || v.starts_with("ellipse(") {
                    self.clip_round = Some(0.5);
                } else if let Some(rest) = v.strip_prefix("inset(") {
                    let inner = rest.trim_end_matches(')');
                    let radius = inner
                        .split("round")
                        .nth(1)
                        .and_then(|r| Len::parse(r.trim()))
                        .and_then(|l| match l {
                            Len::Px(v) => Some(v),
                            Len::Pct(p) => Some(p),
                            Len::Em(k) => Some(k * 16.0),
                            Len::Ch(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).0),
                            Len::Ic(k) => Some(k * crate::metrics::ic_px("", 16.0)),
                            Len::Ex(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).1),
                            Len::Vw(_) | Len::Vh(_) => None,
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
                    Some(n) => n.parse().ok().map(|k: f32| BorderImageSlice::Pct(k / 100.0)),
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
                        return n.parse().ok().map(|k: f32| BorderImageWidth::Pct(k / 100.0));
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
        let mut width = None;
        let mut color = None;
        let mut visible_style = false;
        // Скобки не разрываем: `border: 1px solid rgba(0, 0, 0, .2)`.
        for token in split_outside_parens(v) {
            let token = token.as_str();
            if token == "none" || token == "hidden" {
                width = Some(Len::Px(0.0));
                self.set_visible(side, false);
            } else if border_style(token) {
                visible_style = true;
                self.set_visible(side, true);
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
        match (color, side) {
            (Some(c), None) => {
                self.border_color = Some(c);
                self.border_colors = [Some(c); 4];
            }
            (Some(c), Some(i)) => self.border_colors[i] = Some(c),
            (None, _) => {}
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
fn border_style(v: &str) -> bool {
    // Значения CSS нечувствительны к регистру: `border: 1px SOLID red` — та же
    // рамка. К нижнему регистру приводится только ИМЯ свойства.
    matches!(
        v.to_ascii_lowercase().as_str(),
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset"
    )
}

/// Толщина рамки словом: `thin`, `medium`, `thick` (css-backgrounds-3 §4.1).
fn line_width(v: &str) -> Option<Len> {
    match v.to_ascii_lowercase().as_str() {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v),
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
        // `first baseline` — обычное выравнивание по базовой линии; `last`
        // раскладке под нами неизвестно, и подмена первой сдвинула бы элемент
        // не туда, поэтому поле очищается.
        "baseline" | "first" => Some(Align::Baseline),
        "last" => None,
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
        "start" | "left" => Some(Justify::WmStart),
        "end" | "right" => Some(Justify::WmEnd),
        "space-between" => Some(Justify::Between),
        "space-around" => Some(Justify::Around),
        "space-evenly" => Some(Justify::Evenly),
        "stretch" => Some(Justify::Stretch),
        _ => None,
    }
}

/// Одна грань размещения: `3`, `span 2`, `auto`.
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

/// `url(...)` из значения фона; кавычки внутри необязательны.
///
/// Имя записи регистронезависимо (§3.3), поэтому `URL(` и `Url(` — та же
/// запись; экранирование в нём разбор объявления уже снял. Конец ищется
/// с учётом кавычек и экранирования: закрывающая скобка внутри строки записи
/// не закрывает (§4.3.6). И искать `url(` внутри строки нельзя вовсе —
/// `content: "url(x)"` записью не является.
fn parse_url(v: &str) -> Option<String> {
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
                let inner = v[at + 4..end.saturating_sub(1).max(at + 4)].trim();
                let inner = match inner.chars().next() {
                    Some(q @ ('"' | '\'')) if inner.ends_with(q) && inner.len() > 1 => {
                        &inner[1..inner.len() - 1]
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
    Auto,
    MinContent,
    MaxContent,
}

/// Дорожка целиком: одиночная либо пара граней `minmax(a, b)`.
///
/// Обе грани нужны по-настоящему: `minmax(120px, 1fr)` — это «не уже 120, а
/// дальше забирай остаток». Сведение к одной грани меняет ширину колонки.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackSize {
    Single(Track),
    MinMax(Track, Track),
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
}

/// Размер повторяемой дорожки в `repeat(auto-fill | auto-fit, …)`.
///
/// Берётся либо нижняя граница `minmax(N, …)`, либо сама дорожка, если она
/// задана точкой: `repeat(auto-fill, 100px)` — три колонки в трёхстах точках.
fn auto_fill_min(v: &str) -> Option<f32> {
    let px_of = |t: &str| match Len::parse(t.trim()) {
        Some(Len::Px(px)) => Some(px),
        _ => None,
    };
    if let Some(rest) = v.split("minmax(").nth(1)
        && let Some(first) = rest.split(',').next()
        && let Some(px) = px_of(first)
    {
        return Some(px);
    }
    let rest = v.split("repeat(").nth(1)?;
    let inner = &rest[..rest.rfind(')')?];
    px_of(inner.split(',').nth(1)?)
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
        && let Some(first) = rest.split(',').next()
        && let Some(k) = pct_of(first)
    {
        return Some(k);
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
        // `fit-content(N)` — дорожка не шире N и не шире содержимого.
        if let Some(inner) = t
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
            && let Some(Len::Px(px)) = Len::parse(inner.trim())
        {
            return Some(Track::Px(px));
        }
        if let Some(fr) = t.strip_suffix("fr") {
            return fr.trim().parse().ok().map(Track::Fr);
        }
        match Len::parse(t) {
            Some(Len::Px(px)) => Some(Track::Px(px)),
            Some(Len::Pct(p)) => Some(Track::Pct(p)),
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
            let count: usize = count.trim().parse().ok()?;
            let unit: Vec<TrackSize> = tokenize_tracks(rest)
                .iter()
                .filter_map(|t| one(t))
                .collect();
            if unit.is_empty() {
                return None;
            }
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
            if let Some(n) = t.strip_suffix('%').and_then(|n| n.trim().parse::<f32>().ok()) {
                any_pct = true;
                raw.push((colour, Some(n / 100.0)));
                raw_px.push((colour, None));
            } else if let Some(Len::Px(v)) = Len::parse(t) {
                // Позиция в точках: долей не выразить, длина оси известна
                // только при отрисовке. Хранится своим списком.
                raw.push((colour, None));
                raw_px.push((colour, Some(v)));
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
    let stops: Vec<(Color, f32)> = raw
        .iter()
        .enumerate()
        .map(|(i, (c, pos))| (*c, pos.unwrap_or(i as f32 / last as f32)))
        .collect();
    // Точечные стопы пригодны к отрисовке, только когда позиции есть у ВСЕХ:
    // смешение точек с долями требует длины оси уже при разборе.
    let stops_px: Vec<(Color, f32)> = if !any_pct && raw_px.iter().all(|(_, p)| p.is_some()) {
        raw_px.iter().map(|(c, p)| (*c, p.unwrap_or(0.0))).collect()
    } else {
        vec![]
    };
    Some(Gradient {
        angle_deg: angle,
        radial,
        circle,
        from: stops[0].0,
        to: stops[last].0,
        stops,
        stops_px,
    })
}
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
