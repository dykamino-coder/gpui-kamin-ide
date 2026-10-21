//! Отрисовка строки по словам (`paint_justified`): остаток строки раздаётся
//! пробелам, слова набираются по отдельности и ставятся каждое на своё
//! место — лигатуры и вязь внутри слова не рвутся. Здесь собирается контекст
//! `JustifiedPaint`: логическое смещение байта с раздачей, видимые куски
//! строки (UAX #9 L2), ось зеркала rtl-строки и места ltr-прогонов в ней.

mod gaps;
mod logical_placement;
mod visual_placement;
mod word_paint;
mod word_preparation;

use super::{Edge, Seg, Word};
use crate::text::paragraph::*;
use gpui::{App, Bounds, Pixels, Window};

#[derive(Clone, Copy)]
pub(crate) struct JustifiedPaint<'a> {
    pub(crate) range: &'a std::ops::Range<usize>,
    pub(crate) segs: &'a [Seg],
    pub(crate) words: &'a [Word],
    pub(crate) ltr_place: &'a [(usize, usize, bool, Pixels)],
    pub(crate) logical_run: &'a [(usize, Pixels)],
    pub(crate) step: Pixels,
    pub(crate) from: Pixels,
    pub(crate) mirror: Pixels,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) dx: Pixels,
    pub(crate) y: Pixels,
    pub(crate) pad: (f32, f32),
    pub(crate) logical_at: &'a dyn Fn(usize) -> Pixels,
    pub(crate) placed: &'a dyn Fn(usize, usize) -> Option<(Pixels, bool)>,
}

impl Paragraph {
    /// Выключка по ширине: остаток строки раздаётся её пробелам.
    ///
    /// Слова набираются по отдельности и ставятся каждое на своё место —
    /// иначе растянуть промежутки нечем: набор отдаёт готовую строку одним
    /// куском. Внутри слова набор остаётся сплошным, поэтому лигатуры и вязь
    /// не рвутся.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_justified(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: Pixels,
        line_width: Pixels,
        bounds: Bounds<Pixels>,
        y: Pixels,
        pad: (f32, f32),
        dx: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (words, ltr_runs, step, absorbed, from) = self.prepare_words(range, segs, free);
        // Ось зеркала правой строки — её СОБСТВЕННЫЙ правый край, а не край
        // коробки: прижим уже учтён в `dx`, и вычитать его из ширины коробки
        // значит ошибиться на `free − 2·dx` (`bidi-box-model-013`: 380 точек).
        // Проверено: зажимать ось шириной коробки
        // (`line_width.min(bounds.size.width)`), чтобы переполняющая строка в
        // rtl росла ВЛЕВО, как у блоков, ничего не меняет — `bounds` здесь
        // коробка САМОГО абзаца, уже растянутая по содержимому. Корень выше:
        // абзац не зажат `max-width` родителя (`absolute-non-replaced-width-
        // 021/022/023`), а не ось зеркала.
        let mirror = bounds.origin.x + dx + line_width + free;
        // UAX#9 L2 разворачивает прогоны уровня ≥1, а зеркало строки
        // переворачивает ВСЕ слова разом: латинский прогон внутри правого
        // абзаца выходил задом наперёд. Прогоны левого уровня выкладываем
        // заново — в своём порядке и на своём месте (выключенный путь
        // `BidiInfo` не считал вовсе).
        //
        // В семье `bidi-box-model-*` левый прогон чаще всего ОДНО слово, а
        // одиночное слово зеркало кладёт верно и без разбора: остаток той
        // семьи держат распорки полей, а не порядок слов.
        // Не распространять пословный разворот на ЛЕВЫЕ строки (правый прогон
        // внутри левого абзаца): прогоны там однознаковые (`CSS2/bidi-005..009`),
        // и разворот нужен ПОГЛИФНЫЙ. Поглифный вариант (прогоны UAX#9 L2 левой
        // строки в видимом порядке, слово правого уровня набором
        // `shape_line_rtl`, в правой строке — тоже справа налево и без трекинга
        // хвоста, накопительный счёт пробелов у разрезанного трекингом слова)
        // сам по себе тоже не годится: распорки полей `<span>` с RLO остаются
        // на логическом месте (`bidi-011`), а `letter-spacing-bidi-003` держит
        // флоат, уходящий строкой ниже. Вернуть можно только вместе с
        // распорками, переставляемыми по прогонам.
        // Logical offset of a byte from the line start, with the justification
        // added by the separators before it (same count as `words`).
        let logical_at = |pos: usize| -> Pixels {
            let seps = self.justify_opps(range, pos).saturating_sub(absorbed);
            (self.x_at(segs, pos, Edge::Start) - from) + step * seps as f32
        };
        // Visual pieces of the line, left to right: each level run in visual
        // order (UAX #9 L2), cut at box edge spacers, with its logical extent.
        let ltr_place = self.place_visual(&ltr_runs, &logical_at);
        // Visual offset (from the line start) of the logical piece `a..b`: a
        // right-to-left piece mirrors its contents within its own extent.
        let placed = |a: usize, b: usize| -> Option<(Pixels, bool)> {
            let &(s, e, rtl, start) = ltr_place.iter().find(|p| p.0 <= a && a < p.1)?;
            Some(if rtl {
                (start + (logical_at(e) - logical_at(b.min(e))), true)
            } else {
                (start + (logical_at(a) - logical_at(s)), false)
            })
        };
        let logical_run =
            self.place_logical(range, segs, &words, &ltr_place, step, from, mirror, window);
        self.paint_words(
            JustifiedPaint {
                range,
                segs,
                words: &words,
                ltr_place: &ltr_place,
                logical_run: &logical_run,
                step,
                from,
                mirror,
                bounds,
                dx,
                y,
                pad,
                logical_at: &logical_at,
                placed: &placed,
            },
            window,
            cx,
        );
    }
}
