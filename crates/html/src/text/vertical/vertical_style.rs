//! Configuration for rotated text sizing and writing-mode projection.
use crate::interact::VerticalText;
use gpui::{AnyElement, Pixels};

impl VerticalText {
    pub fn new(child: AnyElement) -> Self {
        VerticalText {
            child: Some(child),
            natural: gpui::Size::default(),
            fit_limit: None,
            inline_constraint: None,
            inline_keyword: None,
            claim_cap: None,
            key: None,
            ccw: false,
            col_min: false,
            lr: false,
            first_line: None,
        }
    }

    /// Строки поданы снизу вверх (`vertical-lr`): первая — левая колонка.
    pub fn lines_left_first(mut self, on: bool) -> Self {
        self.lr = on;
        self
    }

    /// Метрики первой строки — для базовой линии по оси x (см. `first_line`).
    pub fn first_line(
        mut self,
        font: gpui::Font,
        size: Pixels,
        line_height: Option<Pixels>,
        central: bool,
    ) -> Self {
        self.first_line = Some((font, size, line_height, central));
        self
    }

    /// Поворот против часовой стрелки (`sideways-lr`).
    pub fn counter_clockwise(mut self, on: bool) -> Self {
        self.ccw = on;
        self
    }

    /// Включить двухкадровый замер: заявка ширины уточняется фактом
    /// прошлого кадра (перенос строк меняет число колонок).
    pub fn keyed(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    /// Заявить и высоту — потолком родителя, только при ПОЛНОМ зажиме
    /// (строка длиннее потолка): короче потолка коробка прижимается к
    /// содержимому сама, а заявка ломала поток соседей.
    pub fn claiming_height(mut self, cap: Pixels) -> Self {
        self.claim_cap = Some(cap);
        self
    }

    /// Заявить высоту коробки, когда строка не длиннее предела: переносу
    /// такая заявка не мешает (переносить нечего), а замер становится
    /// честным для гибких родителей.
    pub fn fit_within(mut self, limit: Pixels) -> Self {
        self.fit_limit = Some(limit);
        self
    }

    /// Мерить содержимое по МИНИМАЛЬНОМУ вдоль строки (см. поле `col_min`).
    pub(crate) fn inline_constraint(
        mut self,
        constraint: crate::computed::orthogonal::InlineConstraint,
    ) -> Self {
        self.inline_constraint = Some(constraint);
        self
    }

    pub fn column_min(mut self, on: bool) -> Self {
        self.col_min = on;
        self
    }

    pub(crate) fn inline_keyword(
        mut self,
        keyword: Option<crate::computed::orthogonal::InlineKeyword>,
    ) -> Self {
        self.inline_keyword = keyword;
        self
    }
}
