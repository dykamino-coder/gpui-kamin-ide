//! Запросы border-shape к Computed: обводка формы, выносы геометрических коробок, контур, обрезка.

use super::*;

impl Computed {
    /// Обводка `border-shape` по «relevant side» (css-borders-4
    /// §border-shape-relevant-side): первая сторона в порядке block-start,
    /// inline-start, block-end, inline-end со стилем не `none`, иначе
    /// block-start; берутся её толщина и цвет (Blink
    /// `RelevantSideForBorderShape`). Физические индексы t/r/b/l = 0..3;
    /// без цвета — цвет текста, без него чёрный (`currentColor`).
    pub fn border_shape_stroke(&self) -> (f32, Color) {
        let vertical = self.vertical == Some(true);
        let rl = self.vertical_rl == Some(true);
        let rtl = self.rtl == Some(true);
        let (block_start, block_end) = match (vertical, rl) {
            (false, _) => (0usize, 2usize),
            (true, true) => (1, 3),
            (true, false) => (3, 1),
        };
        let (inline_start, inline_end) = match (vertical, rtl) {
            (false, false) => (3usize, 1usize),
            (false, true) => (1, 3),
            (true, false) => (0, 2),
            (true, true) => (2, 0),
        };
        let order = [block_start, inline_start, block_end, inline_end];
        let side = order
            .iter()
            .copied()
            .find(|i| self.border_visible[*i] == Some(true))
            .unwrap_or(block_start);
        let w = self.borders();
        let width = match [w.top, w.right, w.bottom, w.left][side] {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let colour = self.border_colors[side]
            .or(self.border_color)
            .or(self.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        (width, colour)
    }

    /// Края опорной коробки `<geometry-box>` относительно border-box, t/r/b/l,
    /// наружу положительные: margin-box шире на поля, padding-box уже на
    /// рамку, content-box — на рамку и отбивку, half-border-box — на половину
    /// рамки (Blink `GeometryBoxUtils::ReferenceBoxBorderBoxOutsets`).
    pub fn geometry_outsets(&self, kind: u8) -> [f32; 4] {
        let px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = self.borders();
        let bw = [px(b.top), px(b.right), px(b.bottom), px(b.left)];
        let pd = [
            px(self.padding.top),
            px(self.padding.right),
            px(self.padding.bottom),
            px(self.padding.left),
        ];
        let mg = [
            px(self.margin.top),
            px(self.margin.right),
            px(self.margin.bottom),
            px(self.margin.left),
        ];
        let mut out = [0.0f32; 4];
        for i in 0..4 {
            out[i] = match kind {
                1 => mg[i],
                2 => -bw[i],
                3 => -(bw[i] + pd[i]),
                4 => -bw[i] / 2.0,
                _ => 0.0,
            };
        }
        out
    }

    /// Вынос слоёв `border-shape` за border-box (t/r/b/l): половина обводки
    /// наружу у одной фигуры, опорная коробка шире border-box (margin-box) и
    /// запас под митры прямолинейного контура — Blink держит предел митры 1e10
    /// у polygon, шип длиной w/sin(θ/2); 5w покрывает углы от ~23°
    /// (border-shape-polygon-miter-limit: шип ~80 px при w=20).
    pub fn border_shape_ext(&self) -> [f32; 4] {
        let Some(bs) = &self.border_shape else {
            return [0.0; 4];
        };
        let out = self.geometry_outsets(bs.outer_box);
        let stroke = if bs.inner.is_some() {
            0.0
        } else {
            self.border_shape_stroke().0
        };
        let spike = if stroke > 0.0 && crate::paint::background::shape_is_linear(&bs.outer) {
            stroke * 5.0
        } else {
            0.0
        };
        let mut ext = out.map(|o| o.max(0.0) + stroke / 2.0 + spike);
        // Тени повторяют фигуру (css-borders-4 §border-shape-shadow-interaction)
        // и рисуются растром на той же области (`background::
        // border_shape_shadow_svg`): наружная уходит за border-box на разлёт,
        // смещение и хвост размытия (3σ = 1.5·blur); у внутренней хвост
        // размытия тоже нужен — область фильтра обрезает бросающий
        // прямоугольник, и без запаса край холста просвечивал бы.
        for sh in &self.shadows {
            let tail = sh.spread.max(0.0) + sh.blur.max(0.0) * 1.5 + 1.0;
            ext[0] = ext[0].max(tail - sh.y);
            ext[1] = ext[1].max(tail + sh.x);
            ext[2] = ext[2].max(tail + sh.y);
            ext[3] = ext[3].max(tail - sh.x);
        }
        for sh in &self.inset_shadows {
            let tail = sh.blur.max(0.0) * 1.5 + 1.0;
            for e in &mut ext {
                *e = e.max(tail);
            }
        }
        // Контур `outline` повторяет фигуру (слой над группой,
        // `background::border_shape_outline_svg`) — вынос на сдвиг и толщину.
        if let Some((w, off, _)) = self.shaped_outline() {
            let reach = (off + w).max(0.0) + 1.0;
            for e in &mut ext {
                *e = e.max(reach);
            }
        }
        ext
    }

    /// Контур `outline` коробки с `border-shape`, который рисуется по
    /// фигуре: (толщина, сдвиг, цвет). Только сплошной/`auto`/`double`
    /// (css-ui-4; Blink `BorderShapePainter::PaintOutline` остальные стили
    /// отдаёт обычному контуру) и видимый. Толщина без значения — `medium`
    /// (3 px), цвет без своего — `accent-color` при `auto`, иначе цвет текста,
    /// иначе чёрный (как у `render::decorations`); `outline-offset: inset` —
    /// минус толщина. Шрифтовые единицы — своим кеглем.
    pub fn shaped_outline(&self) -> Option<(f32, f32, Color)> {
        let o = self.outline.as_ref()?;
        self.border_shape.as_ref()?;
        if !matches!(o.style, Some(1) | Some(2) | Some(OUTLINE_DOUBLE)) {
            return None;
        }
        let em = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em,
            _ => 0.0,
        };
        let w = match o.width {
            None => 3.0,
            other => px_of(other),
        };
        if w <= 0.0 {
            return None;
        }
        let off = if o.inset { -w } else { px_of(o.offset) };
        let colour = o
            .color
            .or(if o.style == Some(2) {
                self.accent_color
            } else {
                None
            })
            .or(self.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        Some((w, off, colour))
    }

    /// Переполнение коробки с `border-shape` режется внутренним контуром
    /// фигуры (css-borders-4 §border-shape-overflow-interaction: «The inner
    /// border-shape clips the overflow content of the element»): маска
    /// группы берёт внутренний контур, а кольцо рамки ложится НАД буфером
    /// (`Grouped::over`). `scroll`/`auto` идут лентой прокрутки мимо группы.
    pub fn border_shape_clips(&self) -> bool {
        self.border_shape.is_some()
            && (matches!(
                self.overflow_x,
                Some(Overflow::Hidden) | Some(Overflow::Clip)
            ) || matches!(
                self.overflow_y,
                Some(Overflow::Hidden) | Some(Overflow::Clip)
            ))
    }
}
