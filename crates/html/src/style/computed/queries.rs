//! Запросы к стилю: border-shape, тени, содержимое (contain), рамки, слои фона.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod backgrounds;
mod border_shape;

impl Computed {
    /// Активный кламп строк: стандартный `line-clamp` всегда, а
    /// `-webkit-line-clamp` — только в паре с `-webkit-box` по вертикали.
    pub fn clamp_lines(&self) -> Option<u32> {
        let legacy_ok = self.webkit_box == Some(true) && self.webkit_box_vertical == Some(true);
        self.line_clamp
            .filter(|_| self.clamp_legacy != Some(true) || legacy_ok)
    }

    /// Тени `box-shadow` с решённым цветом: без своего цвета — цвет текста
    /// (css-backgrounds-3 §box-shadow, `currentColor`; метка — отрицательная
    /// альфа, как у `apply::shadow_colour`). `inset` — внутренние.
    pub fn resolved_shadows(&self, inset: bool) -> Vec<(Shadow, Color)> {
        let list = if inset {
            &self.inset_shadows
        } else {
            &self.shadows
        };
        list.iter()
            .map(|sh| {
                let colour = if sh.color.a < 0.0 {
                    self.color.unwrap_or(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    })
                } else {
                    sh.color
                };
                (*sh, colour)
            })
            .collect()
    }

    /// Есть ли угол с формой, отличной от круглой, при ненулевом радиусе
    /// (css-borders-4 §corner-shaping: «if border-radius is 0, corner-shape
    /// won't have any effect»). Такой угол уходит растровой маской контура,
    /// а рамка красится кольцом по контуру (`render::decorations`).
    pub fn corner_shaped(&self) -> bool {
        let Some(k) = self.corner_shape else {
            return false;
        };
        let radii = [
            self.radius.tl,
            self.radius.tr,
            self.radius.br,
            self.radius.bl,
        ];
        k.iter().zip(radii).any(|(k, r)| {
            let shaped = (*k - 1.0).abs() > 1e-3;
            let has_radius = match r {
                Some(Len::Px(v)) => v > 0.0,
                Some(Len::Pct(p)) => p > 0.0,
                _ => false,
            };
            shaped && has_radius
        })
    }

    /// Обособление размера не действует на таблицу: её размер задают
    /// дорожки, а не «содержимое как таковое» (css-contain-2 §size
    /// containment; Blink `layout_table.h` — таблица не годится под него).
    pub(crate) fn size_containment_applies(&self) -> bool {
        !matches!(
            self.display,
            Some(Display::Table) | Some(Display::InlineTable)
        )
    }

    /// Повтор «сколько влезет» по оси РЯДОВ, если он задан и годен к
    /// передаче раскладке (есть размер дорожки).
    pub fn grid_rows_repeat(&self) -> Option<AutoRepeat> {
        let r = self.auto_repeat_rows?;
        (r.track.is_some() || r.track_pct.is_some()).then_some(r)
    }

    /// Обособлена ли СТРОЧНАЯ ось: `contain: size` держит обе, `inline-size`
    /// только её (css-contain-2 §containment-types).
    pub fn contains_inline_size(&self) -> bool {
        self.size_containment_applies()
            && (self.contain_size == Some(true) || self.contain_inline_size == Some(true))
    }

    /// Обособлена ли БЛОЧНАЯ ось.
    pub fn contains_block_size(&self) -> bool {
        self.size_containment_applies() && self.contain_size == Some(true)
    }

    /// То же по ФИЗИЧЕСКИМ осям: при вертикальном письме строчная ось идёт
    /// сверху вниз, и обособление меняется местами.
    pub fn contains_width(&self) -> bool {
        if self.vertical == Some(true) {
            self.contains_block_size()
        } else {
            self.contains_inline_size()
        }
    }

    /// Обособлена ли высота (физическая ось).
    pub fn contains_height(&self) -> bool {
        if self.vertical == Some(true) {
            self.contains_inline_size()
        } else {
            self.contains_block_size()
        }
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
}
