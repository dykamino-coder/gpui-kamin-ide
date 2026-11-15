//! KaminIDE patch: повреждение кадра — где новая сцена рисует иначе, чем та,
//! что окно рисовало прошлым кадром.
//!
//! Кадр режется на клетки `TILE`×`TILE` px. Для каждой клетки сцена хранит
//! последовательность примитивов, которые могут в неё писать, в порядке
//! растра (`BatchIterator`: по `(order, kind)`, внутри вида — по списку).
//! Клетка не повреждена, если обе последовательности равны поэлементно
//! (значения примитивов без `order`; метка группы равна, только если равны
//! сами группы, рекурсивно). Тогда все её пиксели те же, что в прошлом кадре:
//! растр клетки — функция этой последовательности. Сравнение точное, без
//! хешей. Так же поступают Chromium (`DamageTracker` по слоям) и WebRender
//! (`update_dirty_rects` по клеткам picture cache), только у них единица
//! сравнения — слой или плитка картинки, а не примитив кадра.
//!
//! Подложки `backdrop-filter` читают соседние пиксели кадра, поэтому кадр с
//! ними считается неизвестным (`SceneDamage::Unknown`): рендерер рисует всё.

use super::{PaintGroup, PaintSurface, Path, Primitive, Scene, TransformationMatrix};
use crate::{Bounds, ScaledPixels};
use collections::FxHashMap;

/// Сторона клетки повреждения, device px.
pub const DAMAGE_TILE: u32 = 64;

/// Повреждение кадра относительно кадра `base` того же окна.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[doc(hidden)]
pub enum SceneDamage {
    /// Сравнить нельзя: рисовать весь кадр.
    #[default]
    Unknown,
    /// Все пиксели те же, что у кадра `base`.
    Unchanged,
    /// Пиксели вне прямоугольника (device px, полуоткрытый) те же.
    Rect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

impl SceneDamage {
    /// Объединение двух повреждений одной цели.
    pub fn union(self, other: Self) -> Self {
        use SceneDamage::*;
        match (self, other) {
            (Unknown, _) | (_, Unknown) => Unknown,
            (Unchanged, d) | (d, Unchanged) => d,
            (
                Rect {
                    x,
                    y,
                    width,
                    height,
                },
                Rect {
                    x: x2,
                    y: y2,
                    width: w2,
                    height: h2,
                },
            ) => {
                let (l, t) = (x.min(x2), y.min(y2));
                let (r, b) = ((x + width).max(x2 + w2), (y + height).max(y2 + h2));
                Rect {
                    x: l,
                    y: t,
                    width: r - l,
                    height: b - t,
                }
            }
        }
    }
}

/// Клетки кадра со списками примитивов (CSR): строится раз на кадр и
/// служит прошлой стороной сравнения в следующем кадре.
#[derive(Default)]
pub(crate) struct DamageIndex {
    /// Размер кадра, device px; `None` — индекс не построен.
    size: Option<(u32, u32)>,
    cols: u32,
    /// `starts[t]..starts[t + 1]` — записи клетки `t` в `entries`.
    starts: Vec<u32>,
    /// Вид (3 старших бита) и номер примитива в его списке.
    entries: Vec<u32>,
    /// Временные: запись и диапазон клеток каждого примитива.
    spans: Vec<(u32, [u16; 4])>,
    /// В кадре есть подложка `backdrop-filter`.
    backdrop: bool,
    /// Повреждение этого кадра и номера кадров (`Window::frame_serial`).
    pub(crate) damage: SceneDamage,
    pub(crate) id: u64,
    pub(crate) base: u64,
}

const KIND_SHIFT: u32 = 29;

/// Примитивы сцены в порядке растра: тот же выбор, что у `BatchIterator`
/// (наименьший `(order, kind)`, внутри вида — порядок списка).
fn paint_order(scene: &Scene, mut each: impl FnMut(u32)) {
    let lens = [
        scene.shadows.len(),
        scene.quads.len(),
        scene.paths.len(),
        scene.underlines.len(),
        scene.monochrome_sprites.len(),
        scene.subpixel_sprites.len(),
        scene.polychrome_sprites.len(),
        scene.surfaces.len(),
    ];
    let order = |kind: usize, i: usize| -> u32 {
        match kind {
            0 => scene.shadows[i].order,
            1 => scene.quads[i].order,
            2 => scene.paths[i].order,
            3 => scene.underlines[i].order,
            4 => scene.monochrome_sprites[i].order,
            5 => scene.subpixel_sprites[i].order,
            6 => scene.polychrome_sprites[i].order,
            _ => scene.surfaces[i].order,
        }
    };
    let mut next = [0usize; 8];
    loop {
        let mut best: Option<(u32, usize)> = None;
        for kind in 0..8 {
            if next[kind] < lens[kind] {
                let key = order(kind, next[kind]);
                if best.is_none_or(|(o, _)| key < o) {
                    best = Some((key, kind));
                }
            }
        }
        let Some((_, kind)) = best else {
            return;
        };
        each(((kind as u32) << KIND_SHIFT) | next[kind] as u32);
        next[kind] += 1;
    }
}

/// Прямоугольник `x0, y0, x1, y1` (device px), куда примитив может писать.
type Area = [f32; 4];

fn area_of(b: Bounds<ScaledPixels>, mask: Bounds<ScaledPixels>, margin: f32) -> Area {
    let x0 = (b.origin.x.0 - margin).max(mask.origin.x.0);
    let y0 = (b.origin.y.0 - margin).max(mask.origin.y.0);
    let x1 = (b.origin.x.0 + b.size.width.0 + margin).min(mask.origin.x.0 + mask.size.width.0);
    let y1 = (b.origin.y.0 + b.size.height.0 + margin).min(mask.origin.y.0 + mask.size.height.0);
    // Сглаживание краёв и округление растра: пиксель с каждой стороны.
    [x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0]
}

const FULL: Area = [
    f32::NEG_INFINITY,
    f32::NEG_INFINITY,
    f32::INFINITY,
    f32::INFINITY,
];

/// Место примитива с преобразованием на экране; маска обрезает его, только
/// когда преобразования нет (иначе область берётся без маски — с запасом).
fn placed(scene: &Scene, kind: u32, i: usize) -> Area {
    let (primitive, mask, transformed): (Primitive, _, _) = match kind {
        1 => {
            let q = &scene.quads[i];
            (
                (*q).into(),
                q.content_mask.bounds,
                q.transformation != TransformationMatrix::unit(),
            )
        }
        4 => {
            let s = &scene.monochrome_sprites[i];
            (
                (*s).into(),
                s.content_mask.bounds,
                s.transformation != TransformationMatrix::unit(),
            )
        }
        5 => {
            let s = &scene.subpixel_sprites[i];
            (
                (*s).into(),
                s.content_mask.bounds,
                s.transformation != TransformationMatrix::unit(),
            )
        }
        _ => {
            let s = &scene.polychrome_sprites[i];
            (
                (*s).into(),
                s.content_mask.bounds,
                s.transformation != TransformationMatrix::unit(),
            )
        }
    };
    let b = super::placed_bounds(&primitive);
    if transformed {
        area_of(b, b, 1.0)
    } else {
        area_of(b, mask, 0.0)
    }
}

/// Где может писать примитив `entry` кадровой сцены; `None` — подложка.
fn entry_area(scene: &Scene, entry: u32) -> Option<Area> {
    let (kind, i) = (
        entry >> KIND_SHIFT,
        (entry & ((1 << KIND_SHIFT) - 1)) as usize,
    );
    Some(match kind {
        0 => {
            let s = &scene.shadows[i];
            // Тень рисуется в коробке, расширенной на три радиуса размытия.
            area_of(
                s.bounds,
                s.content_mask.bounds,
                3.0 * s.blur_radius.0.max(0.0),
            )
        }
        1 | 4..=6 => placed(scene, kind, i),
        2 => {
            let p = &scene.paths[i];
            area_of(p.bounds, p.content_mask.bounds, 0.0)
        }
        3 => {
            let u = &scene.underlines[i];
            area_of(u.bounds, u.content_mask.bounds, u.thickness.0.max(0.0))
        }
        _ => {
            let s = &scene.surfaces[i];
            if s.group == 0 {
                return None;
            }
            match scene.groups.get(s.group as usize - 1) {
                Some(g) if g.projection.is_none() => area_of(
                    s.bounds,
                    s.content_mask.bounds,
                    3.0 * g.blur_radius.max(0.0) + 2.0,
                ),
                _ => FULL,
            }
        }
    })
}

impl DamageIndex {
    fn clear(&mut self) {
        self.size = None;
        self.starts.clear();
        self.entries.clear();
        self.spans.clear();
        self.backdrop = false;
        self.damage = SceneDamage::Unknown;
        self.id = 0;
        self.base = 0;
    }

    /// Разложить примитивы кадровой сцены по клеткам кадра `size`.
    fn build(&mut self, scene: &Scene, size: (u32, u32)) {
        let (w, h) = (size.0.max(1), size.1.max(1));
        let cols = w.div_ceil(DAMAGE_TILE).min(u16::MAX as u32);
        let rows = h.div_ceil(DAMAGE_TILE).min(u16::MAX as u32);
        let mut spans = std::mem::take(&mut self.spans);
        spans.clear();
        let mut backdrop = !scene.hoisted_backdrops.is_empty();
        let tile = DAMAGE_TILE as f32;
        paint_order(scene, |entry| {
            let Some(a) = entry_area(scene, entry) else {
                backdrop = true;
                return;
            };
            let a = if a.iter().any(|v| v.is_nan()) {
                FULL
            } else {
                a
            };
            if a[2] <= 0.0 || a[3] <= 0.0 || a[0] >= w as f32 || a[1] >= h as f32 {
                return;
            }
            if a[0] >= a[2] || a[1] >= a[3] {
                return;
            }
            let c0 = (a[0] / tile).floor().clamp(0.0, (cols - 1) as f32) as u16;
            let r0 = (a[1] / tile).floor().clamp(0.0, (rows - 1) as f32) as u16;
            let c1 = ((a[2] / tile).ceil().clamp(1.0, cols as f32)) as u16;
            let r1 = ((a[3] / tile).ceil().clamp(1.0, rows as f32)) as u16;
            spans.push((entry, [c0, r0, c1.max(c0 + 1), r1.max(r0 + 1)]));
        });
        let tiles = (cols * rows) as usize;
        self.starts.clear();
        self.starts.resize(tiles + 1, 0);
        for (_, [c0, r0, c1, r1]) in &spans {
            for r in *r0 as u32..*r1 as u32 {
                for c in *c0 as u32..*c1 as u32 {
                    self.starts[(r * cols + c) as usize + 1] += 1;
                }
            }
        }
        for t in 0..tiles {
            self.starts[t + 1] += self.starts[t];
        }
        self.entries.clear();
        self.entries.resize(self.starts[tiles] as usize, 0);
        let mut fill: Vec<u32> = self.starts[..tiles].to_vec();
        for (entry, [c0, r0, c1, r1]) in &spans {
            for r in *r0 as u32..*r1 as u32 {
                for c in *c0 as u32..*c1 as u32 {
                    let t = (r * cols + c) as usize;
                    self.entries[fill[t] as usize] = *entry;
                    fill[t] += 1;
                }
            }
        }
        self.spans = spans;
        self.cols = cols;
        self.size = Some((w, h));
        self.backdrop = backdrop;
    }

    fn tile(&self, t: usize) -> &[u32] {
        &self.entries[self.starts[t] as usize..self.starts[t + 1] as usize]
    }
}

/// Точное сравнение примитивов двух сцен без `order` (и без номера пути).
struct Compare<'a> {
    new: &'a Scene,
    old: &'a Scene,
    groups: FxHashMap<(u32, u32), bool>,
}

fn same_path(a: &Path<ScaledPixels>, b: &Path<ScaledPixels>) -> bool {
    a.bounds == b.bounds
        && a.content_mask == b.content_mask
        && a.color == b.color
        && a.color_mapping == b.color_mapping
        && a.vertices.len() == b.vertices.len()
        && a.vertices.iter().zip(&b.vertices).all(|(x, y)| {
            x.xy_position == y.xy_position
                && x.st_position == y.st_position
                && x.content_mask == y.content_mask
        })
}

fn bits<const N: usize>(v: Option<[f32; N]>) -> Option<[u32; N]> {
    v.map(|a| a.map(f32::to_bits))
}

impl Compare<'_> {
    /// Примитив `a` сцены `sa` (новой стороны) рисует то же, что `b` сцены `sb`.
    fn same(&mut self, sa: &Scene, a: u32, sb: &Scene, b: u32) -> bool {
        if a >> KIND_SHIFT != b >> KIND_SHIFT {
            return false;
        }
        let mask = (1 << KIND_SHIFT) - 1;
        let (i, j) = ((a & mask) as usize, (b & mask) as usize);
        macro_rules! eq {
            ($list:ident) => {{
                let mut x = sa.$list[i];
                x.order = sb.$list[j].order;
                x == sb.$list[j]
            }};
        }
        match a >> KIND_SHIFT {
            0 => eq!(shadows),
            1 => eq!(quads),
            2 => same_path(&sa.paths[i], &sb.paths[j]),
            3 => eq!(underlines),
            4 => eq!(monochrome_sprites),
            5 => eq!(subpixel_sprites),
            6 => eq!(polychrome_sprites),
            _ => self.same_surface(&sa.surfaces[i], &sb.surfaces[j]),
        }
    }

    fn same_surface(&mut self, a: &PaintSurface, b: &PaintSurface) -> bool {
        #[cfg(target_os = "macos")]
        {
            let _ = (a, b);
            false
        }
        #[cfg(not(target_os = "macos"))]
        {
            a.bounds == b.bounds
                && a.content_mask == b.content_mask
                && a.corner_radii == b.corner_radii
                && a.blur_radius.to_bits() == b.blur_radius.to_bits()
                && a.opacity.to_bits() == b.opacity.to_bits()
                && bits(a.color_matrix) == bits(b.color_matrix)
                && a.fragment == b.fragment
                && (a.group == 0) == (b.group == 0)
                && (a.group == 0 || self.same_group(a.group - 1, b.group - 1))
        }
    }

    /// Группа `a` нового кадра рисует ту же картинку, что группа `b` прошлого.
    fn same_group(&mut self, a: u32, b: u32) -> bool {
        if let Some(&known) = self.groups.get(&(a, b)) {
            return known;
        }
        let (new, old) = (self.new, self.old);
        let same = match (new.groups.get(a as usize), old.groups.get(b as usize)) {
            (Some(x), Some(y)) => self.same_group_value(x, y),
            _ => false,
        };
        self.groups.insert((a, b), same);
        same
    }

    fn same_group_value(&mut self, a: &PaintGroup, b: &PaintGroup) -> bool {
        a.bounds == b.bounds
            && a.blur_radius.to_bits() == b.blur_radius.to_bits()
            && a.blend == b.blend
            && a.polygon == b.polygon
            && a.polygon_inverse == b.polygon_inverse
            && match (&a.mask, &b.mask) {
                (None, None) => true,
                (Some(x), Some(y)) => x.id == y.id,
                _ => false,
            }
            && a.mask_bounds == b.mask_bounds
            && a.mask_once == b.mask_once
            && bits(a.mask_clip) == bits(b.mask_clip)
            && bits(a.projection) == bits(b.projection)
            && a.fragments == b.fragments
            && self.same_scene(&a.scene, &b.scene)
    }

    /// Сцены группы рисуют одно и то же: равные примитивы в равном порядке.
    fn same_scene(&mut self, a: &Scene, b: &Scene) -> bool {
        if a.shadows.len() != b.shadows.len()
            || a.quads.len() != b.quads.len()
            || a.paths.len() != b.paths.len()
            || a.underlines.len() != b.underlines.len()
            || a.monochrome_sprites.len() != b.monochrome_sprites.len()
            || a.subpixel_sprites.len() != b.subpixel_sprites.len()
            || a.polychrome_sprites.len() != b.polychrome_sprites.len()
            || a.surfaces.len() != b.surfaces.len()
            || !a.hoisted_backdrops.is_empty()
            || !b.hoisted_backdrops.is_empty()
        {
            return false;
        }
        let (mut sa, mut sb) = (Vec::new(), Vec::new());
        paint_order(a, |e| sa.push(e));
        paint_order(b, |e| sb.push(e));
        sa.iter().zip(&sb).all(|(&x, &y)| self.same(a, x, b, y))
    }
}

impl Scene {
    /// KaminIDE patch: повреждение этого кадра (`size` device px, номер
    /// `id`) относительно `previous` — кадра `previous.damage.id` того же окна.
    pub(crate) fn note_damage(&mut self, previous: &Scene, id: u64, size: (u32, u32)) {
        let mut index = std::mem::take(&mut *self.damage);
        index.build(self, size);
        index.id = id;
        index.base = previous.damage.id;
        index.damage = self.damage_against(&index, previous);
        *self.damage = index;
    }

    fn damage_against(&self, index: &DamageIndex, previous: &Scene) -> SceneDamage {
        let old = &*previous.damage;
        if index.backdrop || old.backdrop || old.size != index.size || old.size.is_none() {
            return SceneDamage::Unknown;
        }
        let (w, h) = index.size.unwrap_or_default();
        let mut cmp = Compare {
            new: self,
            old: previous,
            groups: FxHashMap::default(),
        };
        let cols = index.cols as usize;
        let tiles = index.starts.len() - 1;
        let mut damage = SceneDamage::Unchanged;
        for t in 0..tiles {
            let (a, b) = (index.tile(t), old.tile(t));
            let same = a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(&x, &y)| cmp.same(self, x, previous, y));
            if !same {
                let (c, r) = ((t % cols) as u32, (t / cols) as u32);
                let (x, y) = (c * DAMAGE_TILE, r * DAMAGE_TILE);
                damage = damage.union(SceneDamage::Rect {
                    x,
                    y,
                    width: DAMAGE_TILE.min(w - x),
                    height: DAMAGE_TILE.min(h - y),
                });
            }
        }
        damage
    }

    /// KaminIDE patch: повреждение кадра, номер кадра, с которым сравнивали,
    /// номер этого кадра (`Window::frame_serial`; 0 — кадр не сравнивался)
    /// и размер кадра в device px, для которого считались клетки.
    #[doc(hidden)]
    pub fn frame_damage(&self) -> (SceneDamage, u64, u64, (u32, u32)) {
        (
            self.damage.damage,
            self.damage.base,
            self.damage.id,
            self.damage.size.unwrap_or_default(),
        )
    }

    /// KaminIDE patch: группы, чья картинка нужна кадру, рисуемому только в
    /// прямоугольнике `x, y, width, height`: метка в кадре пересекает его, или
    /// группа вложена в нужную.
    #[doc(hidden)]
    pub fn groups_needed(&self, rect: [u32; 4]) -> Vec<bool> {
        let mut needed = vec![false; self.groups.len()];
        let r = [
            rect[0] as f32,
            rect[1] as f32,
            (rect[0] + rect[2]) as f32,
            (rect[1] + rect[3]) as f32,
        ];
        for (i, s) in self.surfaces.iter().enumerate() {
            if s.group == 0 {
                continue;
            }
            let entry = (7 << KIND_SHIFT) | i as u32;
            let a = entry_area(self, entry).unwrap_or(FULL);
            let hit = a.iter().any(|v| v.is_nan())
                || (a[0] < r[2] && a[2] > r[0] && a[1] < r[3] && a[3] > r[1]);
            if hit && let Some(n) = needed.get_mut(s.group as usize - 1) {
                *n = true;
            }
        }
        // Объемлющая группа стоит в списке позже вложенной.
        for g in (0..self.groups.len()).rev() {
            if !needed[g] {
                continue;
            }
            for s in &self.groups[g].scene.surfaces {
                if s.group > 0
                    && let Some(n) = needed.get_mut(s.group as usize - 1)
                {
                    *n = true;
                }
            }
        }
        needed
    }

    pub(crate) fn clear_damage(&mut self) {
        self.damage.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Background, ContentMask, Hsla, Quad, blue, bounds, point, red, size};

    fn quad(x: f32, y: f32, color: Hsla) -> Quad {
        let b = bounds(
            point(ScaledPixels(x), ScaledPixels(y)),
            size(ScaledPixels(10.0), ScaledPixels(10.0)),
        );
        Quad {
            bounds: b,
            content_mask: ContentMask {
                bounds: bounds(
                    point(ScaledPixels(0.0), ScaledPixels(0.0)),
                    size(ScaledPixels(1000.0), ScaledPixels(1000.0)),
                ),
            },
            background: Background::from(color),
            ..Default::default()
        }
    }

    fn scene(quads: &[Quad]) -> Scene {
        let mut s = Scene::default();
        for (i, q) in quads.iter().enumerate() {
            let mut q = *q;
            q.order = i as u32 + 1;
            s.quads.push(q);
        }
        s
    }

    fn damage(old: &[Quad], new: &[Quad]) -> SceneDamage {
        let mut a = scene(old);
        a.note_damage(&Scene::default(), 1, (256, 256));
        let mut b = scene(new);
        b.note_damage(&a, 2, (256, 256));
        b.frame_damage().0
    }

    #[test]
    fn equal_scenes_are_unchanged_even_with_other_orders() {
        let red = red();
        let mut s = scene(&[quad(5.0, 5.0, red)]);
        s.note_damage(&Scene::default(), 1, (256, 256));
        let mut t = scene(&[quad(5.0, 5.0, red)]);
        t.quads[0].order = 77;
        t.note_damage(&s, 2, (256, 256));
        assert_eq!(t.frame_damage(), (SceneDamage::Unchanged, 1, 2, (256, 256)));
    }

    #[test]
    fn a_changed_primitive_damages_only_its_tiles() {
        let (red, blue) = (red(), blue());
        let old = [quad(5.0, 5.0, red), quad(200.0, 200.0, red)];
        let new = [quad(5.0, 5.0, red), quad(200.0, 200.0, blue)];
        assert_eq!(
            damage(&old, &new),
            SceneDamage::Rect {
                x: 192,
                y: 192,
                width: 64,
                height: 64
            }
        );
    }

    #[test]
    fn swapped_paint_order_is_damage() {
        let (red, blue) = (red(), blue());
        let old = [quad(5.0, 5.0, red), quad(8.0, 8.0, blue)];
        let new = [quad(8.0, 8.0, blue), quad(5.0, 5.0, red)];
        assert_ne!(damage(&old, &new), SceneDamage::Unchanged);
    }

    #[test]
    fn size_change_and_first_frame_are_unknown() {
        let q = [quad(5.0, 5.0, red())];
        let mut a = scene(&q);
        a.note_damage(&Scene::default(), 1, (256, 256));
        assert_eq!(a.frame_damage().0, SceneDamage::Unknown);
        let mut b = scene(&q);
        b.note_damage(&a, 2, (257, 256));
        assert_eq!(b.frame_damage().0, SceneDamage::Unknown);
    }

    #[test]
    fn union_covers_both_rectangles() {
        let a = SceneDamage::Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };
        let b = SceneDamage::Rect {
            x: 128,
            y: 64,
            width: 64,
            height: 64,
        };
        assert_eq!(
            a.union(b),
            SceneDamage::Rect {
                x: 0,
                y: 0,
                width: 192,
                height: 128
            }
        );
        assert_eq!(SceneDamage::Unchanged.union(a), a);
        assert_eq!(SceneDamage::Unknown.union(a), SceneDamage::Unknown);
    }
}
