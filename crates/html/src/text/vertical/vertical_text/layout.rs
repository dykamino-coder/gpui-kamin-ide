//! Vertical text measurement and physical layout claims.

use super::VT_INLINE_MAX;
use super::VerticalText;
use crate::layout::writing_mode::orthogonal_measure;
use crate::text::vertical::vertical_line_baseline;
use gpui::{App, LayoutId, Window};

impl VerticalText {
    pub(crate) fn request_layout_impl(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        // The atomic subtree is built once and keeps LayoutIds in this engine.
        // A parent measure callback cannot move it to a nested engine: cached
        // child LayoutIds would refer to the original tree. See measurements.
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
        VT_INLINE_MAX.with(|c| {
            if let Some(v) = c.get() {
                c.set(Some(v.max(f32::from(self.natural.width))));
            }
        });
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
                .and_then(|k| {
                    self.values
                        .as_ref()
                        .and_then(|v| v.vcx().doc.vertical_measures.get(k))
                })
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
        let id = window.request_layout(style, [], cx);
        self.layout_id = Some(id);
        id
    }
}
