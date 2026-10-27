//! Фаза `request_layout` элемента `VerticalText`: содержимое меряется собственным корнем
//! (по максимальному или минимальному содержимому, либо ортогональной мерой), ширина
//! заявляется высотой содержимого (факт прошлого кадра сильнее свободного замера), высота —
//! длиной строки при потолке/пределе, базовая по оси x — через `vertical_line_baseline`;
//! длина самой длинной строки отдаётся в `InlineMeasure`.

use super::VerticalText;
use crate::layout::writing_mode::orthogonal_measure;
use crate::text::vertical::vertical_line_baseline;
use gpui::{App, LayoutId, Window};

impl VerticalText {
    pub(crate) fn request_layout_impl(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        // Собственный корень раскладки: размер текста нужен ЗДЕСЬ, чтобы
        // отдать его родителю перевёрнутым. Атомарное поддерево строится один
        // раз и держит `LayoutId` этого движка; замер по запросу родителя
        // (`request_measured_layout`) не годится: повёрнутый блок сам живёт
        // внутри чужого замера, движок там недоступен, а перенос поддерева во
        // вложенный движок оставил бы в детях `LayoutId` исходного дерева
        // (см. `measurements`). Ограничение доводится на подготовке.
        //
        // Ячейка вертикальной таблицы меряется по МИНИМАЛЬНОМУ содержимому
        // вдоль строки: её вклад в дорожку колонки — это min-content
        // («the outer min-content width of each cell that spans the column»,
        // css-tables-3 §computing-column-measures), а не длина всей строки.
        // Обёртка до поворота стоит при этом БЕЗ жёсткой ширины
        // (`col_min` в `render/paragraph/vertical.rs`), поэтому запрос
        // доходит до самого абзаца: `Paragraph` на
        // `AvailableSpace::MinContent` отвечает `probe.min_content(window)`.
        // Дальше `natural.width` — инлайн-мера колонки (её заявит высотой
        // `fit_within`), а `natural.height` — число строк при этой мере, то
        // есть блочная толщина ряда.
        let space = gpui::size(
            if self.col_min {
                gpui::AvailableSpace::MinContent
            } else {
                gpui::AvailableSpace::MaxContent
            },
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = if let Some(constraint) = self.inline_constraint {
            orthogonal_measure::measure(
                &mut self.child,
                constraint,
                self.inline_keyword,
                window,
                cx,
            )
        } else {
            self.child.layout_as_root_unrounded(space, window, cx)
        };
        // Внутренний строчный размер повёрнутого текста — длина его самой
        // длинной строки (`natural.width` горизонтального абзаца до
        // поворота). Наружу высотой он не заявляется (см. ниже), и пробе
        // флоата хоста полос (`band_flow::intrinsic`, письмо вертикальное)
        // его взять неоткуда — сборщик кладёт его сюда, когда открыт.
        if let Some(measure) = &self.inline_measure {
            measure.report(f32::from(self.natural.width));
        }
        let mut style = gpui::Style {
            flex_shrink: 0.0,
            ..Default::default()
        };
        // The turned line box is block-axis content of a vertical container
        // laid out by a flex-row adapter: lines overflow the block-end edge,
        // they are never compressed (CSS Writing Modes 4 §7.1). A shrunk
        // frame turned the line about the box's right edge, so an inline-block
        // narrower than its line painted the line flush right while its
        // exported baseline counted from the left (grid-self-baseline-001-ref).
        // Ordinary orthogonal blocks claim the independently measured inline size;
        // other contexts let their existing parent-sizing contract decide height.
        // Факт прошлого кадра сильнее свободного замера: перенос строк при
        // решённой длине меняет число колонок, а свободный замер его не
        // видит (text-combine-upright-line-breaking-rules-001).
        let claim = if self.inline_keyword.is_some() {
            self.natural.height
        } else {
            self.key
                .and_then(|k| self.measures.as_ref().and_then(|m| m.get(k)))
                .unwrap_or(self.natural.height)
        };
        style.size.width = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(claim),
        ));
        vertical_line_baseline::apply(
            &mut style,
            &mut self.child,
            self.natural,
            self.ccw,
            self.lr,
            self.first_line.as_ref(),
            window,
            cx,
        );
        if self.inline_constraint.is_some() {
            style.size.height = self.natural.width.into();
        } else if let Some(cap) = self.claim_cap
            && self.natural.width >= cap
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(cap),
            ));
        } else if let Some(limit) = self.fit_limit
            // Без допуска +1 на привязку к пикселю (`natural.width <= limit + 1`
            // с заявкой `min(natural, limit)`): обёртка до поворота стоит
            // ЖЁСТКОЙ шириной в предел, и короткой строке нужна её длина, а не
            // предел — `sizing-orthog-v{lr,rl}-in-htb-008/009/011/020/021/023/024`
            // (эталон — абсолютная вертикальная коробка с `height: auto`,
            // схлопывается так же) ломаются. Вернуть можно только вместе с
            // замером длины строки и правкой абсолютного эталона.
            && self.natural.width <= limit
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(self.natural.width),
            ));
        }
        let id = window.request_layout(style, [], cx);
        self.layout_id = Some(id);
        id
    }
}
