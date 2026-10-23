//! Картинка с авто-размерами: естественный размер, пределы и соотношение.

use super::used_size::{Picture, PictureSizing};
use crate::defaults::{DEFAULT_REPLACED_HEIGHT_PX, DEFAULT_REPLACED_WIDTH_PX};
use crate::layout::replaced::limits::css2_replaced_limits;
use crate::style::values::value::Len;

impl PictureSizing<'_> {
    pub(super) fn auto_picture(&self) -> Picture {
        let style = self.style;
        let (sub_w, sub_h) = self.sub;
        let (max_w, max_h) = (self.max_w, self.max_h);
        let both_auto = matches!(style.width, None | Some(Len::Auto))
            && matches!(style.height, None | Some(Len::Auto));
        if both_auto && let Some(side) = self.natural {
            // Авто-размер замещаемого считается ЗДЕСЬ, а не отдаётся
            // загрузчику картинок: тот работает асинхронно, и в первом
            // кадре коробка выходила нулевой высоты (box-sizing-007 —
            // страница вовсе без квадратов). Недостающая сторона
            // достраивается соотношением, резерв — 300×150
            // (CSS 2.1 §10.3.2/§10.6.2, css-images-3 §default-sizing).
            let (w0, h0) = match (side.w, side.h, side.ratio) {
                (Some(w), Some(h), _) => (w, h),
                (Some(w), None, Some(r)) => (w, w / r),
                (None, Some(h), Some(r)) => (h * r, h),
                (Some(w), None, None) => (w, DEFAULT_REPLACED_HEIGHT_PX),
                (None, Some(h), None) => (DEFAULT_REPLACED_WIDTH_PX, h),
                (None, None, Some(r)) => {
                    // §10.3.2, последний пункт: при обеих `auto` и одном
                    // лишь соотношении ширина берётся из содержащего
                    // блока, а не из резерва. Резерв остаётся, когда
                    // ширину взять неоткуда (`ratio-2.svg` в
                    // `visudet/replaced-elements-*`: мы рисовали 300×150,
                    // эталон — 200×100 по `div { width: 200px }`).
                    let w = self.cb_width.filter(|v| *v > 0.0).unwrap_or_else(|| {
                        (DEFAULT_REPLACED_HEIGHT_PX * r).min(DEFAULT_REPLACED_WIDTH_PX)
                    });
                    (w, w / r)
                }
                // Не разрешать ДОЛЮ собственного размера рисунка
                // (`<svg width="100%">` и SVG вовсе без атрибутов — у
                // него подразумевается `100%`) от содержащего блока без
                // различения ДВУХ СЛУЧАЕВ: у `<img>` доля значит «своего
                // размера нет» и работает умолчальный размер 300×150
                // (css-images-3 §5.2), а у `<object>` SVG — это
                // вложенный ДОКУМЕНТ, и его `100%` считается от коробки
                // объекта. Без различения вся семья `replaced-elements-*`
                // рушится ради `replaced-intrinsic-002`.
                (None, None, None) => (DEFAULT_REPLACED_WIDTH_PX, DEFAULT_REPLACED_HEIGHT_PX),
            };
            if w0 > 0.0 && h0 > 0.0 {
                // §10.4: пределы — по таблице (`css2_replaced_limits`).
                let min_w = self.clamp(style.min_width, sub_w);
                let min_h = self.clamp(style.min_height, sub_h);
                // Без СОБСТВЕННОГО соотношения стороны независимы: потолок
                // высоты режет только высоту, и ширина остаётся своей
                // (§10.4, таблица «no intrinsic ratio»). Прежде общий
                // множитель ужимал и её — картинка без соотношения под
                // `max-height: 20px` выходила у́же в пятнадцать раз
                // (`replaced-elements-max-height-20`, снимки `no-ratio` и
                // `height-25-no-ratio`).
                let без_соотношения =
                    side.ratio.is_none() && !(side.w.is_some() && side.h.is_some());
                let (rw, rh) = if без_соотношения {
                    (self.limit(w0, min_w, max_w), self.limit(h0, min_h, max_h))
                } else {
                    css2_replaced_limits(w0, h0, min_w, max_w, min_h, max_h)
                };
                return Picture::Used {
                    raster: (rw, rh),
                    size: (rw, rh),
                };
            }
        } else if (max_w.is_some() || max_h.is_some())
            && let Some(side) = self.natural
            && let (Some(w0), Some(h0)) = (side.w, side.h)
            && w0 > 0.0
            && h0 > 0.0
        {
            let mut scale = 1.0f32;
            if let Some(m) = max_w {
                scale = scale.min(m / w0);
            }
            if let Some(m) = max_h {
                scale = scale.min(m / h0);
            }
            if scale < 1.0 {
                return Picture::Used {
                    raster: (w0 * scale, h0 * scale),
                    size: (w0 * scale, h0 * scale),
                };
            }
        }
        Picture::Natural
    }
}
