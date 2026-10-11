//! Layout for vertical_text; split out to keep the owning module within 250 lines.

use super::VerticalText;
use super::{VT_INLINE_MAX, VT_MEASURED};
use crate::layout::writing_mode::orthogonal_measure;
use crate::text::vertical::vertical_line_baseline;
use gpui::{App, GlobalElementId, InspectorElementId, LayoutId, Window};

impl VerticalText {
    pub(crate) fn request_layout_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // Собственный корень раскладки: размер текста нужен ЗДЕСЬ, чтобы
        // отдать его родителю перевёрнутым.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: замер по запросу родителя
        // (`request_measured_layout`), чтобы ограничение доходило до
        // содержимого и вертикальный текст переносился. Не работает: на этом
        // шаге раскладочный движок недоступен — повёрнутый блок сам живёт
        // внутри чужого замера, и вызов падает на `layout_engine.unwrap()`
        // (vendor/gpui/src/window.rs). Ограничение придётся доводить другим
        // путём — например, осью потока в самом стиле.
        //
        // Ячейка вертикальной таблицы меряется по МИНИМАЛЬНОМУ содержимому
        // вдоль строки: её вклад в дорожку колонки — это min-content
        // («the outer min-content width of each cell that spans the column»,
        // css-tables-3 §computing-column-measures), а не длина всей строки.
        // Обёртка до поворота стоит при этом БЕЗ жёсткой ширины
        // (`render.rs`, `col_min`), поэтому запрос доходит до самого абзаца:
        // `Paragraph` на `AvailableSpace::MinContent` отвечает
        // `probe.min_content(window)` (`lines.rs`). Дальше `natural.width` —
        // инлайн-мера колонки (её заявит высотой `fit_within`), а
        // `natural.height` — число строк при этой мере, то есть блочная
        // толщина ряда.
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
                self.child.as_mut().unwrap(),
                constraint,
                self.inline_keyword,
                window,
                cx,
            )
        } else {
            self.child
                .as_mut()
                .unwrap()
                .layout_as_root_unrounded(space, window, cx)
        };
        // Внутренний строчный размер повёрнутого текста — длина его самой
        // длинной строки (`natural.width` горизонтального абзаца до
        // поворота). Наружу высотой он не заявляется (см. ниже), и пробе
        // флоата хоста полос (`band_flow::intrinsic`, письмо вертикальное)
        // его взять неоткуда — сборщик кладёт его сюда, когда открыт.
        VT_INLINE_MAX.with(|c| {
            if let Some(v) = c.get() {
                c.set(Some(v.max(f32::from(self.natural.width))));
            }
        });
        let mut style = gpui::Style::default();
        // Ordinary orthogonal blocks claim the independently measured inline size;
        // other contexts let their existing parent-sizing contract decide height.
        // Факт прошлого кадра сильнее свободного замера: перенос строк при
        // решённой длине меняет число колонок, а свободный замер его не
        // видит (text-combine-upright-line-breaking-rules-001).
        let claim = if self.inline_keyword.is_some() {
            self.natural.height
        } else {
            self.key
                .and_then(|k| VT_MEASURED.with(|c| c.borrow().get(&k).copied()))
                .unwrap_or(self.natural.height)
        };
        style.size.width = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(claim),
        ));
        vertical_line_baseline::apply(
            &mut style,
            self.child.as_mut().unwrap(),
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
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (02.10): допуск +1 на привязку к пикселю
            // (`natural.width <= limit + 1`, заявка `min(natural, limit)`).
            // Обёртка до поворота стоит ЖЁСТКОЙ шириной в предел, и при пределе
            // 394 (рамка 3) замер возвращает 394.4 — строгое `<=` заявку
            // отклоняет, коробка схлопывается в рамку. С допуском:
            // `sizing-orthog-v{lr,rl}-in-htb-007/010/019/022` 0.68-0.83 → 0.00
            // (+8), но `-008/009/011/020/021/023/024` (обе стороны) 0.00-0.20 →
            // 0.55-0.75 (−14): у них эталон — абсолютная вертикальная коробка с
            // `height: auto`, и она схлопывается так же; короткой строке нужна
            // её длина, а жёсткая ширина даёт всегда предел. Возвращать только
            // вместе с замером длины строки и правкой абсолютного эталона.
            && self.natural.width <= limit
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(self.natural.width),
            ));
        }
        (window.request_layout(style, [], cx), ())
    }
}
