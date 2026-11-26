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
use crate::{Bounds, ScaledPixels, point, size};
use collections::FxHashMap;

mod clipped;
mod group_match;
pub(crate) mod shift;
mod trace;
pub use shift::{FrameShift, SHIFT_KIND_NAMES, SHIFT_KINDS, SHIFT_RECTS, ShiftStats};

/// KaminIDE patch: the CPU time a frame comparison (`plan_shift`) may take,
/// in microseconds, set by the renderer: 0 never compares (a hardware raster
/// of the window costs less than reading its primitives), `u32::MAX` (the
/// default) has no limit.
static SHIFT_BUDGET_US: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

/// KaminIDE patch: set the frame comparison budget (`SHIFT_BUDGET_US`).
pub fn set_shift_budget_us(us: u32) {
    SHIFT_BUDGET_US.store(us, std::sync::atomic::Ordering::Relaxed);
}

fn shift_budget_us() -> u32 {
    SHIFT_BUDGET_US.load(std::sync::atomic::Ordering::Relaxed)
}

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
    /// Группа прошлого кадра, рисующая ту же картинку, что группа `g`, плюс один
    /// (0 — нет), `group_match`.
    group_base: Vec<u32>,
    pub(crate) id: u64,
    pub(crate) base: u64,
    /// KaminIDE patch: this frame as the last one moved (`shift`), when the
    /// move leaves less to raster than the damage.
    pub(crate) shift: Option<FrameShift>,
    /// Why a frame offered a scroll step was not drawn as a move (DevTools `why`).
    pub(crate) shift_miss: Option<&'static str>,
    /// KaminIDE patch: frames since a moved comparison last ran over its
    /// budget (0: it did not), carried from frame to frame.
    pub(crate) over_budget: u32,
    /// What the comparison of this frame with the last found (DevTools `why`).
    pub(crate) shift_stats: Option<ShiftStats>,
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

/// KaminIDE patch: `order` of the primitive `entry` (`KIND_SHIFT` encoding).
fn entry_order(scene: &Scene, entry: u32) -> u32 {
    let i = (entry & ((1 << KIND_SHIFT) - 1)) as usize;
    match entry >> KIND_SHIFT {
        0 => scene.shadows[i].order,
        1 => scene.quads[i].order,
        2 => scene.paths[i].order,
        3 => scene.underlines[i].order,
        4 => scene.monochrome_sprites[i].order,
        5 => scene.subpixel_sprites[i].order,
        6 => scene.polychrome_sprites[i].order,
        _ => scene.surfaces[i].order,
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
            let Some(g) = scene.groups.get(s.group as usize - 1) else {
                return Some(FULL);
            };
            let grow = 3.0 * g.blur_radius.max(0.0) + 2.0;
            let Some(m) = g.projection else {
                return Some(area_of(s.bounds, s.content_mask.bounds, grow));
            };
            // A plane's mark lies under its 3D context's mask: it writes
            // only where its ink lands (`projected`), 2 px more for the
            // shader's rounding of the map.
            let (o, z) = (g.bounds.origin, g.bounds.size);
            let ink = [o.x.0 - grow, o.y.0 - grow, o.x.0 + z.width.0 + grow, o.y.0 + z.height.0 + grow];
            match super::homography_image(&m, ink) {
                Some(f) => {
                    let image = Bounds {
                        origin: point(ScaledPixels(f[0]), ScaledPixels(f[1])),
                        size: size(ScaledPixels(f[2] - f[0]), ScaledPixels(f[3] - f[1])),
                    };
                    area_of(image, s.content_mask.bounds, 2.0)
                }
                None => FULL,
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
        self.group_base.clear();
        self.id = 0;
        self.base = 0;
        self.shift = None;
        self.shift_miss = None;
        self.shift_stats = None;
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

/// Two surfaces draw alike given the same image (group or backdrop).
pub(super) fn same_surface_values(a: &PaintSurface, b: &PaintSurface) -> bool {
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
    }
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
            1 => if sa.quads[i].is_analytic_gradient() || sb.quads[j].is_analytic_gradient() {
                sa.same_gradient_quad(&sa.quads[i], sb, &sb.quads[j])
            } else { eq!(quads) || clipped::same_visible(&sa.quads[i], &sb.quads[j]) },
            2 => same_path(&sa.paths[i], &sb.paths[j]),
            3 => eq!(underlines),
            4 => eq!(monochrome_sprites),
            5 => eq!(subpixel_sprites),
            6 => eq!(polychrome_sprites),
            _ => self.same_surface(&sa.surfaces[i], &sb.surfaces[j]),
        }
    }

    fn same_surface(&mut self, a: &PaintSurface, b: &PaintSurface) -> bool {
        same_surface_values(a, b)
            && (a.group == 0) == (b.group == 0)
            && (a.group == 0 || self.same_group(a.group - 1, b.group - 1))
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
                // A RenderImage is immutable and its id is unique per image
                // (`RenderImage::new`): an equal id is equal mask bytes.
                (Some(x), Some(y)) => x.id == y.id,
                _ => false,
            }
            && a.mask_bounds == b.mask_bounds
            && a.mask_once == b.mask_once
            && bits(a.mask_clip) == bits(b.mask_clip)
            && bits(a.projection) == bits(b.projection)
            && a.fragments == b.fragments
            && if std::rc::Rc::ptr_eq(&a.scene, &b.scene) && a.scene.book.layers.is_empty() {
                // One shared content: equal unless a group it labels changed.
                a.scene.surfaces.iter().filter(|s| s.group > 0).all(|s| self.same_group(s.group - 1, s.group - 1))
            } else {
                self.same_scene(&a.scene, &b.scene)
            }
    }

    /// Сцены группы рисуют одно и то же: равные примитивы в равном порядке.
    /// Сцена с удерживаемым слоем (`scene::layer`, попавшим в группу повтором
    /// диапазона) не равна никакой: слой меняется на месте, его примитивов
    /// нет в списках сцены, и равные списки не значат равную картинку.
    fn same_scene(&mut self, a: &Scene, b: &Scene) -> bool {
        if !a.book.layers.is_empty() || !b.book.layers.is_empty() {
            return false;
        }
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
    /// `id`) относительно `previous` — кадра `previous.book.index.id` того же окна.
    /// `pub` + `doc(hidden)`: тесты рендерера (`gpui_windows`) строят цепочку кадров.
    #[doc(hidden)]
    pub fn note_frame_damage(&mut self, previous: &Scene, id: u64, size: (u32, u32)) {
        self.note_frame_damage_moved(previous, id, size, None);
    }

    /// KaminIDE patch: `note_frame_damage` of a frame whose scroll nodes moved
    /// the most by `moved` device pixels since `previous`: when the damage is
    /// larger than what the last frame moved by it leaves to raster, the frame
    /// is that move (`frame_shift`) and its damage the moved bounds.
    #[doc(hidden)]
    pub fn note_frame_damage_moved(&mut self, previous: &Scene, id: u64, size: (u32, u32), moved: Option<Result<(i32, i32), &'static str>>) {
        let mut index = std::mem::take(&mut self.book.index);
        index.build(self, size);
        index.id = id;
        index.base = previous.book.index.id;
        // A group of the last frame that paints the same image hands this one
        // its buffer (the renderer's group cache); groups are matched by
        // content, not by index. The layer damage of a frame with groups and
        // the in-place comparison both rest on the match, so it runs first.
        index.group_base.clear();
        if index.base != 0 {
            let mut cmp = Compare {
                new: self,
                old: previous,
                groups: FxHashMap::default(),
            };
            group_match::match_groups(&mut cmp, &mut index.group_base);
        }
        index.damage = self.damage_against(&index, previous);
        let comparable = previous.book.index.size == Some(size) && index.base != 0;
        // A frame whose scroll nodes stayed but whose damage is unknown (a
        // retained layer recorded anew) is compared with the last in place.
        let moved = match moved {
            None if comparable && index.damage == SceneDamage::Unknown => Some(Ok((0, 0))),
            moved => moved,
        };
        index.shift_miss = match moved {
            None => None,
            Some(Err(why)) => Some(why),
            Some(_) if !comparable => Some("not_compared"),
            Some(_) if shift_budget_us() == 0 => Some("hardware_raster"),
            // A moved comparison that ran over its budget most likely does
            // again on the same page: it is tried again every 8th frame. In
            // place, layers recorded again unchanged keep it cheap.
            Some(Ok(d)) if d != (0, 0) && !previous.book.index.over_budget.is_multiple_of(8) => Some("over_budget_last"),
            Some(Ok(d)) => match self.timed_plan_shift(
                previous,
                size,
                d,
                &index.group_base,
                index.shift_stats.insert(ShiftStats::default()),
            ) {
                Err(why) => Some(why),
                Ok(shift) => {
                    let damaged = match index.damage {
                        SceneDamage::Unchanged => 0,
                        SceneDamage::Rect { width, height, .. } => width as u64 * height as u64,
                        SceneDamage::Unknown => u64::MAX,
                    };
                    if shift.in_place() && shift.redraw.is_empty() {
                        index.damage = SceneDamage::Unchanged;
                        None
                    } else if shift.redraw_area() < damaged {
                        let [x, y, width, height] = shift.bounds();
                        index.damage = SceneDamage::Rect { x, y, width, height };
                        index.shift = Some(shift);
                        None
                    } else {
                        Some("damage_smaller")
                    }
                }
            },
        };
        index.over_budget = match (moved, index.shift_miss) {
            (Some(Ok(d)), Some("over_budget")) if d != (0, 0) => 1,
            (Some(Ok(d)), Some("over_budget_last")) if d != (0, 0) => previous.book.index.over_budget + 1,
            (Some(Ok(d)), _) if d != (0, 0) => 0,
            _ => previous.book.index.over_budget,
        };
        trace::log_damage(self, &index, previous);
        self.book.index = index;
    }

    /// KaminIDE patch: группа кадра `frame_damage().1` (с которым сравнивали
    /// этот), что рисует ту же картинку, что группа `g` этого кадра.
    #[doc(hidden)]
    pub fn group_base(&self, g: usize) -> Option<usize> {
        match self.book.index.group_base.get(g).copied().unwrap_or(0) {
            0 => None,
            o => Some(o as usize - 1),
        }
    }

    fn damage_against(&self, index: &DamageIndex, previous: &Scene) -> SceneDamage {
        let old = &previous.book.index;
        if index.backdrop || old.backdrop || old.size != index.size || old.size.is_none() {
            return SceneDamage::Unknown;
        }
        let (w, h) = index.size.unwrap_or_default();
        // Retained layers (`scene::layer`) draw primitives the tiles do not
        // hold; their frame is compared by the layer bookkeeping
        // (`Scene::damage`, set by `Scene::note_damage` before this), which
        // knows the rectangles written into them since the last frame.
        if !self.book.layers.is_empty() || !previous.book.layers.is_empty() {
            // With effect groups the layer damage holds when every group
            // paints the image of the last frame's group of the same index:
            // a mark then shows the same image wherever no layer was written.
            let kept = self.groups.len() == previous.groups.len()
                && index.group_base.len() == self.groups.len()
                && index.group_base.iter().enumerate().all(|(g, &o)| o as usize == g + 1);
            let damage = self.damage().or(self.book.grouped_damage.filter(|_| kept));
            return match damage {
                Some(None) => SceneDamage::Unchanged,
                Some(Some(r)) => {
                    let x0 = r.origin.x.0.max(0.).floor() as u32;
                    let y0 = r.origin.y.0.max(0.).floor() as u32;
                    let x1 = ((r.origin.x.0 + r.size.width.0).ceil().max(0.) as u32).min(w);
                    let y1 = ((r.origin.y.0 + r.size.height.0).ceil().max(0.) as u32).min(h);
                    if x1 <= x0 || y1 <= y0 {
                        SceneDamage::Unchanged
                    } else {
                        SceneDamage::Rect {
                            x: x0,
                            y: y0,
                            width: x1 - x0,
                            height: y1 - y0,
                        }
                    }
                }
                None => SceneDamage::Unknown,
            };
        }
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
            // KaminIDE patch: a tile whose lists pair up entry by entry changes
            // only under the entries that differ (old and new place): every
            // other pixel of it sees the same primitives in the same order.
            // Lists of other lengths damage the whole tile.
            let mut changed: Option<Area> = None;
            let mut whole = a.len() != b.len();
            for (&x, &y) in a.iter().zip(b) {
                if whole || cmp.same(self, x, previous, y) {
                    continue;
                }
                match (entry_area(self, x), entry_area(previous, y)) {
                    (Some(p), Some(q)) if p.iter().chain(&q).all(|v| !v.is_nan()) => {
                        let u = changed.unwrap_or(p);
                        changed = Some([u[0].min(p[0]).min(q[0]), u[1].min(p[1]).min(q[1]),
                            u[2].max(p[2]).max(q[2]), u[3].max(p[3]).max(q[3])]);
                    }
                    _ => whole = true,
                }
            }
            if whole || changed.is_some() {
                let (c, r) = ((t % cols) as u32, (t / cols) as u32);
                let (x, y) = (c * DAMAGE_TILE, r * DAMAGE_TILE);
                let (x1, y1) = (x + DAMAGE_TILE.min(w - x), y + DAMAGE_TILE.min(h - y));
                let a = changed.filter(|_| !whole).unwrap_or(FULL);
                let clamp = |v: f32, lo: u32, hi: u32| (v.max(lo as f32).min(hi as f32)) as u32;
                let (ax, ay) = (clamp(a[0].floor(), x, x1), clamp(a[1].floor(), y, y1));
                let (bx, by) = (clamp(a[2].ceil(), x, x1), clamp(a[3].ceil(), y, y1));
                if bx > ax && by > ay {
                    damage = damage.union(SceneDamage::Rect {
                        x: ax,
                        y: ay,
                        width: bx - ax,
                        height: by - ay,
                    });
                }
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
            self.book.index.damage,
            self.book.index.base,
            self.book.index.id,
            self.book.index.size.unwrap_or_default(),
        )
    }

    /// KaminIDE patch: this frame as the frame `frame_damage().1` moved and
    /// then redrawn within rectangles (`shift`), when it was planned so.
    #[doc(hidden)]
    pub fn frame_shift(&self) -> Option<&FrameShift> {
        self.book.index.shift.as_ref()
    }

    /// KaminIDE patch: why a frame offered a scroll step is not a move
    /// (`no_step`, `backdrop`, `layer_unknown`, `last_layer_unknown`,
    /// `redraw_whole`, `damage_smaller`, `not_compared`), for DevTools `why`.
    #[doc(hidden)]
    pub fn frame_shift_miss(&self) -> Option<&'static str> {
        self.book.index.shift_miss
    }

    /// KaminIDE patch: what the comparison with the last frame found, when it ran.
    #[doc(hidden)]
    pub fn frame_shift_stats(&self) -> Option<ShiftStats> {
        self.book.index.shift_stats
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

    pub(crate) fn clear_damage_index(&mut self) {
        self.book.index.clear();
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
        a.note_frame_damage(&Scene::default(), 1, (256, 256));
        let mut b = scene(new);
        b.note_frame_damage(&a, 2, (256, 256));
        b.frame_damage().0
    }

    #[test]
    fn equal_scenes_are_unchanged_even_with_other_orders() {
        let red = red();
        let mut s = scene(&[quad(5.0, 5.0, red)]);
        s.note_frame_damage(&Scene::default(), 1, (256, 256));
        let mut t = scene(&[quad(5.0, 5.0, red)]);
        t.quads[0].order = 77;
        t.note_frame_damage(&s, 2, (256, 256));
        assert_eq!(t.frame_damage(), (SceneDamage::Unchanged, 1, 2, (256, 256)));
    }

    #[test]
    /// Entry areas carry the one-pixel antialiasing margin of the damage index.
    fn a_changed_primitive_damages_only_its_area() {
        let (red, blue) = (red(), blue());
        let old = [quad(5.0, 5.0, red), quad(200.0, 200.0, red)];
        let new = [quad(5.0, 5.0, red), quad(200.0, 200.0, blue)];
        assert_eq!(
            damage(&old, &new),
            SceneDamage::Rect {
                x: 199,
                y: 199,
                width: 12,
                height: 12
            }
        );
        // A moved primitive damages its old and new place.
        let moved = [quad(5.0, 5.0, red), quad(220.0, 210.0, red)];
        assert_eq!(
            damage(&old, &moved),
            SceneDamage::Rect {
                x: 199,
                y: 199,
                width: 32,
                height: 22
            }
        );
    }

    #[test]
    fn an_added_primitive_damages_its_whole_tiles() {
        let red = red();
        let old = [quad(5.0, 5.0, red)];
        let new = [quad(5.0, 5.0, red), quad(20.0, 20.0, red)];
        assert_eq!(
            damage(&old, &new),
            SceneDamage::Rect {
                x: 0,
                y: 0,
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
        a.note_frame_damage(&Scene::default(), 1, (256, 256));
        assert_eq!(a.frame_damage().0, SceneDamage::Unknown);
        let mut b = scene(&q);
        b.note_frame_damage(&a, 2, (257, 256));
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
