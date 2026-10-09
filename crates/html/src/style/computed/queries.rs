//! Запросы к стилю: border-shape, тени, содержимое (contain), рамки, слои фона.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

impl Computed {
    /// Активный кламп строк: стандартный `line-clamp` всегда, а
    /// `-webkit-line-clamp` — только в паре с `-webkit-box` по вертикали.
    pub fn clamp_lines(&self) -> Option<u32> {
        let legacy_ok = self.webkit_box == Some(true) && self.webkit_box_vertical == Some(true);
        self.line_clamp
            .filter(|_| self.clamp_legacy != Some(true) || legacy_ok)
    }

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
            .or(if o.style == Some(2) { self.accent_color } else { None })
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
            && (matches!(self.overflow_x, Some(Overflow::Hidden) | Some(Overflow::Clip))
                || matches!(self.overflow_y, Some(Overflow::Hidden) | Some(Overflow::Clip)))
    }

    /// Тени `box-shadow` с решённым цветом: без своего цвета — цвет текста
    /// (css-backgrounds-3 §box-shadow, `currentColor`; метка — отрицательная
    /// альфа, как у `apply::shadow_colour`). `inset` — внутренние.
    pub fn resolved_shadows(&self, inset: bool) -> Vec<(Shadow, Color)> {
        let list = if inset { &self.inset_shadows } else { &self.shadows };
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
                (sh.clone(), colour)
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
        let radii = [self.radius.tl, self.radius.tr, self.radius.br, self.radius.bl];
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

    /// Слои фона СВЕРХУ ВНИЗ, когда их больше одного: каждый — копия стиля с
    /// одним слоем (картинка, размер, положение, повтор, область) и без
    /// цвета фона — цвет лежит под всеми слоями и красится коробкой
    /// (css-backgrounds-3 §3.1, §2.1: значения списков, которых меньше
    /// слоёв, повторяются по кругу). Градиент слоя уходит в растровую плитку
    /// (`bg_image` с сырой записью), чтобы все слои шли одним путём и в
    /// своём порядке. `None` — слой один.
    /// `background-clip` of the background COLOR: css-backgrounds-3 §3.2,
    /// «the background color is clipped according to the background-clip
    /// value associated with the bottom-most background image layer». The
    /// number of layers comes from `background-image` (§2.1); a shorter
    /// `background-clip` list repeats, a longer one is truncated
    /// (`background-color-clip`: two `none` layers, clip list
    /// `border-box, content-box, border-box` → `content-box`).
    pub(crate) fn color_clip(&self) -> Option<BgClip> {
        let Some((_, clips)) = self.bg_lists.iter().find(|(k, _)| k == "background-clip") else {
            return self.bg_clip;
        };
        let Some((_, images)) = self.bg_lists.iter().find(|(k, _)| k == "background-image") else {
            return self.bg_clip;
        };
        let n = background_layers(images).len();
        let clips = background_layers(clips);
        if n < 2 || clips.is_empty() {
            return self.bg_clip;
        }
        let mut probe = Computed::default();
        probe.apply_one("background-clip", clips[(n - 1) % clips.len()]);
        probe.bg_clip
    }

    pub(crate) fn bg_layers(&self) -> Option<Vec<Computed>> {
        let short = self.bg_lists.iter().find(|(k, _)| k == "background").map(|(_, v)| v.clone());
        let image = self.bg_lists.iter().find(|(k, _)| k == "background-image").map(|(_, v)| v.clone());
        let images: Vec<String> = match (&image, &short) {
            (Some(v), _) | (None, Some(v)) => background_layers(v).into_iter().map(str::to_string).collect(),
            _ => return None,
        };
        if images.len() < 2 {
            return None;
        }
        // Длины слоёв в единицах шрифта (`1ch 0 / 4ch 1ch`): слой разбирается
        // заново из сырой записи уже ПОСЛЕ `resolve_em`, и `ch` в положении и
        // размере оставался нерешённым — слой выходил нулевым
        // (`hanging-whitespace-001..004`). Решаем по своему кеглю.
        let font_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::text::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let (ch, ex) = crate::text::metrics::ch_ex_px(&family, font_px);
        let px_of = |v: &str| -> String {
            if has_font_units(v) {
                font_lengths_to_px(v, font_px, 16.0, ex, ch)
            } else {
                v.to_string()
            }
        };
        let mut out = vec![];
        for (i, _) in images.iter().enumerate() {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            if let Some(v) = &short {
                c.bg_image = None;
                c.gradient = None;
                c.gradient_raw = None;
                c.bg_size = BgSize::Auto;
                c.bg_pos = BgPos::default();
                c.bg_repeat = None;
                c.bg_origin = None;
                let layers = background_layers(v);
                c.apply_one("background", &px_of(layers[i % layers.len()]));
                c.background = None;
            }
            for (k, v) in self.bg_lists.iter().filter(|(k, _)| k != "background") {
                let layers = background_layers(v);
                c.apply_one(k, &px_of(layers[i % layers.len()]));
            }
            c.bg_lists.clear();
            // Градиент слоя — плиткой: источником идёт сама функция
            // градиента из записи слоя (в сокращении рядом с ней размер,
            // положение и повтор).
            if c.bg_image.is_none()
                && c.gradient.is_some()
                && let Some(r) = images.get(i)
                && let Some(at) = r.find("gradient(")
            {
                let start = r[..at].rfind(|ch: char| ch.is_whitespace() || ch == ',').map_or(0, |p| p + 1);
                let mut depth = 0i32;
                let mut end = r.len();
                for (j, ch) in r[at..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = at + j + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                c.bg_image = Some(r[start..end].to_string());
            }
            c.gradient = None;
            c.gradient_raw = None;
            out.push(c);
        }
        Some(out)
    }
}
