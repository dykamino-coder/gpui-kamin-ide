//! Child kinds and margin channels for measured float placement.

use crate::band_flow::{Build, Edge};

/// Чем ребёнок хоста участвует в полосах.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Флоат: сторона `-1`/`1` и `clear` из стиля; `shrink` — ширина
    /// `auto`, то есть shrink-to-fit §10.3.5.
    Float {
        side: i8,
        clear: Option<i8>,
        shrink: bool,
        /// Буквица (`initial-letter`, шаг F11): исключение у начала строки
        /// потока, а не флоат (`FloatBands::add_initial_letter`).
        letter: bool,
    },
    /// Коробка, флоаты НЕ перекрывающая (§9.5, последний абзац): свой
    /// контекст, таблица, атом известного размера. `table` — коробка не уже
    /// своего min-content (CSS 2.1 §17.5.2: ширина таблицы не меньше
    /// минимальной ширины ячеек), блок же со своим контекстом заполняет окно
    /// и при переполнении содержимым (§10.3.3).
    Piece { table: bool, rtl: bool },
    /// Распорка: только высота margin-box, ничего не рисует.
    Strut(f32),
    /// Блок обычного потока (свой контекст НЕ заводит): коробка идёт во всю
    /// ширину поверх флоатов, а строки в ней сужаются (§9.5: «the current
    /// and subsequent line boxes created next to the float are shortened»).
    /// Канал — `FloatBands::shapes` от верха коробки (шаг F4).
    Flow,
    /// Блок обычного потока, внутри которого есть флоаты или коробки своего
    /// контекста (шаг F7): его дети раскладываются по ОБЩИМ полосам
    /// контекста со стенками его содержимого (Servo
    /// `ContainingBlockPositionInfo`, `flow/float.rs:44-64`, и
    /// `replace_containing_block_position_info`, `:971-977`; Blink — одно
    /// `ExclusionSpace` на БФК). Сама коробка рисуется без детей высотой из
    /// плана; детей несёт `Kid::nest`.
    Nest,
}

/// Содержимое `Kind::Nest`.
pub struct Nest {
    pub kids: Vec<Kid>,
    /// Рамка плюс отступ: верх, право, низ, лево.
    pub inset: [Edge; 4],
    /// Заданная высота содержимого в точках; `None` — из детей в потоке
    /// (§10.6.3: флоаты в высоту обычного блока не входят).
    pub height: Option<f32>,
    /// Заданная ширина содержимого в точках; `None` — на всю ширину
    /// содержащего блока (§10.3.3). `width: 0` — законный содержащий блок
    /// для флоатов (`letter-spacing-206`: `.squash {width: 0}` с дюжиной
    /// плавающих абзацев внутри).
    pub width: Option<f32>,
}

pub struct Kid {
    pub kind: Kind,
    /// `clear` куска или блока потока (шаг F6): верх его border-box не выше
    /// низа флоатов названной стороны (§9.5.2). У флоата `clear` живёт в
    /// `Kind::Float`.
    pub clear: Option<i8>,
    /// Поля: верх, право, низ, лево.
    pub margin: [Edge; 4],
    pub build: Build,
    /// Дети `Kind::Nest`.
    pub nest: Option<Nest>,
    /// Анонимный строчный прогон (CSS 2.1 §9.2.1.1): своей коробки у него
    /// нет, и сдвинуть его целиком — то же, что сдвинуть его строки.
    pub anon: bool,
    /// Первое слово анонимного прогона (`render.rs` `band_kids`): по его
    /// min-content решается, влезает ли первая строка в окно (§9.5).
    pub head: Option<Build>,
    /// Строчное содержимое ПЕРЕД флоатом в той же строке (`render.rs`
    /// `wrap_floats`, «Hello<float>Kitty»): у флоата посреди строки — оно
    /// одно, анонимным прогоном. По его ширине план решает, остаётся ли
    /// флоат на этой строке (правило 6 §9.5.1) или уходит под неё.
    pub lead: Option<Build>,
    /// Набранное ДО последнего `<br>` перед флоатом: строка флоата
    /// начинается под ним (`Kid::lead` — только хвост после `<br>`).
    pub lead_base: Option<Build>,
    /// The preceding collapsed margin is not part of the float's own margin.
    pub margin_offset: f32,
    /// Whether this containing block permits block-start margin collapse.
    pub start_open: bool,
}
