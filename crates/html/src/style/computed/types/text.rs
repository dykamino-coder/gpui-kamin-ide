//! Типы текста: text-transform, text-fit, hanging-punctuation, ruby, text-align.

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
