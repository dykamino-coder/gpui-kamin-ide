//! Краска абзаца только там, где она видна (Blink `CullRect`,
//! `core/paint/cull_rect.h`: фрагмент, чьи чернила не пересекают
//! прямоугольник отсечения, не рисуется). Каждый примитив абзаца сцена GPUI
//! всё равно отбрасывает по маске содержимого (`Scene::insert_primitive`), а
//! набор строк, разбивка на слова и снимок для выделения стоят дороже всего
//! кадра видимой части: страница из тысяч строк рисовала их все.
//!
//! Абзац пропускается, только когда его чернила ограничены по блочной оси:
//! строки лежат подряд от верха коробки (без сдвигов кусков, своей
//! `line-height` кусков, атомов, знаков акцента, руби, подчёркиваний и
//! фрагментации), а глиф выходит за строку не дальше запаса из кегля и высоты
//! строки (полулидинг бывает отрицательным) с полями и рамкой фона прогона.
//! Только в корневом окружении краски (`Window::retained_root_environment`):
//! там примитив кладётся в координатах окна и маска окончательна, а запись
//! краски для повтора на другом месте (сегменты фрагментации) невозможна.
//! Открытым стокам чернил (`layout::reuse`) отдаётся граница пропущенных
//! чернил, и запись блока не теряет ни одного пикселя (`Window::offer_culled_ink`).

use crate::text::paragraph::*;
use gpui::{Bounds, GlobalElementId, Hitbox, Pixels, Window, point, px, size};

/// Ширина отдаваемых стокам чернил: строка без переноса уходит вбок сколь
/// угодно далеко, поэтому граница — вся строчная ось.
const ANY_INLINE: f32 = 1.0e7;

impl Paragraph {
    /// Краска абзаца в коробке `bounds` не положила бы ни одного примитива
    /// внутрь маски содержимого: она пропущена, а граница её чернил отдана
    /// стокам. `false` — абзац рисуется как обычно.
    pub(super) fn culled(
        &self,
        id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        hitbox: &Option<Hitbox>,
        window: &mut Window,
    ) -> bool {
        if !self.ink_bounded() || !window.retained_root_environment() {
            return false;
        }
        let (top, bottom) = self.ink_band(bounds);
        let mask = window.content_mask().bounds;
        if !outside((top, bottom), mask) {
            return false;
        }
        // Слежение за выделением (`track_selection`) вне экрана инертно,
        // только пока выделение пусто и не тянется: иначе краска рисуется.
        if let (Some(global), Some(hitbox)) = (id, hitbox)
            && (hitbox.is_hovered(window) || !crate::interactive::selection::idle(global, window))
        {
            return false;
        }
        window.offer_culled_ink(Bounds {
            origin: point(px(-ANY_INLINE), px(top)),
            size: size(px(2.0 * ANY_INLINE), px(bottom - top)),
        });
        true
    }

    /// Чернила абзаца лежат в полосе строк от верха коробки (см. модуль).
    fn ink_bounded(&self) -> bool {
        !self.vertical
            && !self.paint_context.is_rotated()
            && self.selection_vertical.is_none()
            && self.vertical_inline.is_none()
            && !self.ruby_rotated
            && self.ruby_layers.is_none()
            && !self.lines_reversed
            && self.fragment.is_none()
            && self.fit.is_none()
            && self.atoms.is_empty()
            && self.atom_boxes.is_empty()
            && self.overlays.is_empty()
            && self.decor_spans.is_empty()
            && self.emph_spans.is_empty()
            && self.box_spans.is_empty()
            && self.shift_spans.is_empty()
            && self.lh_spans.is_empty()
            && self.rel_spans.is_empty()
            && self.edge_spans.is_empty()
            && self
                .runs
                .iter()
                .all(|r| r.underline.is_none() && r.strikethrough.is_none())
    }

    /// Полоса чернил по блочной оси (окно, точки): строки подряд от верха
    /// коробки с запасом на выход глифов и фона прогона за строку.
    fn ink_band(&self, bounds: Bounds<Pixels>) -> (f32, f32) {
        let lh = f32::from(self.line_height).abs();
        let lines: f32 = self
            .lines
            .iter()
            .map(|l| lh + f32::from(l.clearance).abs())
            .sum();
        let font = self
            .runs
            .iter()
            .filter_map(|r| r.font_size)
            .map(f32::from)
            .fold(f32::from(self.font_size).abs(), f32::max);
        let backdrop = self
            .runs
            .iter()
            .map(|r| {
                let pad = r.background_pad.iter().map(|p| f32::from(*p).abs());
                let border = r
                    .background_border
                    .iter()
                    .flat_map(|(_, w)| w.iter().map(|p| f32::from(*p).abs()));
                pad.fold(0.0, f32::max) + border.fold(0.0, f32::max)
            })
            .fold(0.0, f32::max);
        band(
            f32::from(bounds.origin.y),
            f32::from(bounds.size.height),
            lines,
            slack(font, lh, backdrop),
        )
    }
}

/// Запас на выход чернил строки за её коробку: глиф выходит за кегль
/// (надстрочные знаки) и за строку при отрицательном полулидинге, фон
/// прогона — на свои поля и рамку; два пикселя — на сдвиг набора и снэп.
fn slack(font: f32, line_height: f32, backdrop: f32) -> f32 {
    2.0 * (font + line_height) + backdrop + 2.0
}

/// Полоса `[верх, низ]` чернил коробки с верхом `top` и высотой `height`,
/// чьи строки занимают `lines`, с запасом `slack` с обеих сторон.
fn band(top: f32, height: f32, lines: f32, slack: f32) -> (f32, f32) {
    (top - slack, top + height.max(lines) + slack)
}

/// Полоса чернил целиком выше или ниже маски.
fn outside((top, bottom): (f32, f32), mask: Bounds<Pixels>) -> bool {
    bottom < f32::from(mask.origin.y) || top > f32::from(mask.origin.y + mask.size.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        }
    }

    #[test]
    fn band_takes_the_taller_of_box_and_lines_with_slack_on_both_sides() {
        assert_eq!(band(100.0, 20.0, 54.0, 10.0), (90.0, 164.0));
        assert_eq!(band(100.0, 80.0, 54.0, 10.0), (90.0, 190.0));
    }

    #[test]
    fn only_a_band_wholly_above_or_below_the_mask_is_outside() {
        assert!(outside((601.0, 700.0), mask()));
        assert!(outside((-90.0, -1.0), mask()));
        // Touching or overlapping either edge still paints.
        assert!(!outside((600.0, 700.0), mask()));
        assert!(!outside((-90.0, 0.0), mask()));
        assert!(!outside((-90.0, 900.0), mask()));
    }

    #[test]
    fn slack_covers_glyph_overhang_and_run_backgrounds() {
        // 16px text on 20px lines: a glyph may leave its line by more than a
        // line, a padded run background by its padding and border too.
        assert_eq!(slack(16.0, 20.0, 0.0), 74.0);
        assert_eq!(slack(16.0, 20.0, 9.0), 83.0);
    }
}
