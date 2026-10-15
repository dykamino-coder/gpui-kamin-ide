#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)] // Not useful, GPUI makes heavy use of callbacks
#![allow(clippy::collapsible_else_if)] // False positives in platform specific code
#![allow(unused_mut)] // False positives in platform specific code

extern crate self as gpui;
#[macro_use]
mod action;
mod app;

mod arena;
mod asset_cache;
mod assets;
mod bounds_tree;
mod color;
/// The default colors used by GPUI.
pub mod colors;
/// KaminIDE patch (INC-2026-0001, диагностика): КТО делает окно грязным —
/// место в исходниках вызова notify/refresh/on_next_frame (`#[track_caller]`).
/// Без этого 60 кадров/с без кадров CEF, ввода и известных анимаций в логе
/// не к чему привязать.
pub mod dirty_sources {
    use std::collections::HashMap;
    use std::panic::Location;
    use std::sync::{LazyLock, Mutex};

    type Key = (&'static str, u32, &'static str);
    static SOURCES: LazyLock<Mutex<HashMap<Key, u32>>> = LazyLock::new(Default::default);

    /// Учесть один заказ перерисовки из `loc`.
    pub fn note(kind: &'static str, loc: &'static Location<'static>) {
        if let Ok(mut map) = SOURCES.lock() {
            *map.entry((loc.file(), loc.line(), kind)).or_default() += 1;
        }
    }

    /// Топ-`n` мест за период; счётчики обнуляются.
    pub fn take_top(n: usize) -> Vec<(String, u32)> {
        let Ok(mut map) = SOURCES.lock() else {
            return Vec::new();
        };
        let mut top: Vec<(String, u32)> = map
            .drain()
            .map(|((file, line, kind), count)| {
                let parts: Vec<&str> = file.split(['/', '\\']).collect();
                let short = parts[parts.len().saturating_sub(3)..].join("/");
                (format!("{kind}@{short}:{line}"), count)
            })
            .collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top.truncate(n);
        top
    }
}

/// KaminIDE patch (INC-2026-0001, диагностика): цена кадра по видам пачек —
/// число примитивов, площадь в пикселях и GPU-время (пишет рендерер
/// платформы, читает секундная сводка приложения).
pub mod batch_cost {
    use std::sync::Mutex;

    /// Виды пачек — в порядке `PrimitiveBatch`, последняя — буферы групп.
    pub const KINDS: [&str; 9] = [
        "shadows", "quads", "paths", "underlines", "mono", "subpixel", "poly", "surfaces", "groups",
    ];

    #[derive(Default, Clone, Copy)]
    struct Kind {
        batches: u64,
        prims: u64,
        area_px: f64,
        gpu_ns: f64,
    }

    #[derive(Default)]
    struct Totals {
        frames: u64,
        timed_frames: u64,
        frame_gpu_ns: f64,
        kinds: [Kind; 9],
    }

    static TOTALS: Mutex<Option<Totals>> = Mutex::new(None);

    fn with(f: impl FnOnce(&mut Totals)) {
        if let Ok(mut guard) = TOTALS.lock() {
            f(guard.get_or_insert_with(Totals::default));
        }
    }

    /// Начат кадр окна.
    pub fn frame() {
        with(|t| t.frames += 1);
    }

    /// Пачка на CPU-стороне: сколько примитивов и какой площади.
    pub fn count(kind: usize, prims: usize, area_px: f64) {
        with(|t| {
            let k = &mut t.kinds[kind];
            k.batches += 1;
            k.prims += prims as u64;
            k.area_px += area_px;
        });
    }

    /// Готовый GPU-замер кадра: всё время и время каждой пачки.
    pub fn timed(frame_ns: f64, kinds: &[(usize, f64)]) {
        with(|t| {
            t.timed_frames += 1;
            t.frame_gpu_ns += frame_ns;
            for (kind, ns) in kinds {
                t.kinds[*kind].gpu_ns += ns;
            }
        });
    }

    /// Крупнейшие по площади квады последнего кадра: что именно закрашивает.
    static DETAIL: Mutex<String> = Mutex::new(String::new());

    /// Описать крупнейшие квады сцены (зовёт рендерер раз на кадр).
    pub fn quad_detail(quads: &[crate::Quad]) {
        let mut top: Vec<(f32, String)> = quads
            .iter()
            .map(|q| {
                let i = q.bounds.intersect(&q.content_mask.bounds);
                let (w, h) = (i.size.width.0.max(0.0), i.size.height.0.max(0.0));
                let c = &q.corner_radii;
                let r = [c.top_left, c.top_right, c.bottom_left, c.bottom_right]
                    .iter()
                    .fold(0.0f32, |m, v| m.max(v.0));
                let e = &q.border_widths;
                let b = [e.top, e.right, e.bottom, e.left]
                    .iter()
                    .fold(0.0f32, |m, v| m.max(v.0));
                let tf = q.transformation != crate::TransformationMatrix::unit();
                (
                    w * h,
                    format!(
                        "{w:.0}x{h:.0}@{:.0},{:.0} bg={:?} a={:.2} r={r:.0} b={b:.0}{}",
                        i.origin.x.0,
                        i.origin.y.0,
                        q.background.tag,
                        q.background.solid.a,
                        if tf { " tf" } else { "" }
                    ),
                )
            })
            .collect();
        top.sort_by(|a, b| b.0.total_cmp(&a.0));
        let line = top.iter().take(10).map(|(_, d)| d.as_str()).collect::<Vec<_>>().join(" | ");
        if let Ok(mut detail) = DETAIL.lock() {
            *detail = line;
        }
    }

    /// Строка для `diag.log` за период; счётчики обнуляются.
    pub fn take_line() -> Option<String> {
        let t = TOTALS.lock().ok()?.take()?;
        if t.frames == 0 {
            return None;
        }
        let per = |v: f64| v / t.frames as f64;
        let timed = t.timed_frames.max(1) as f64;
        let kinds = KINDS
            .iter()
            .zip(t.kinds.iter())
            .filter(|(_, k)| k.batches > 0)
            .map(|(name, k)| {
                let gpu = if t.timed_frames > 0 {
                    format!(" gpu_us={:.0}", k.gpu_ns / timed / 1000.0)
                } else {
                    String::new()
                };
                format!(
                    "{name}: n={:.0} kpx={:.0}{gpu}",
                    per(k.prims as f64),
                    per(k.area_px) / 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let detail = DETAIL.lock().map(|d| d.clone()).unwrap_or_default();
        let frame_gpu = if t.timed_frames > 0 {
            format!(" timed={} frame_gpu_us={:.0}", t.timed_frames, t.frame_gpu_ns / timed / 1000.0)
        } else {
            String::new()
        };
        Some(format!(
            "[gpu] frames={}{frame_gpu} per frame: {kinds}; top quads: {detail}",
            t.frames
        ))
    }
}

/// KaminIDE patch: счётчики стоимости кадра. Приложение печатает их в свою
/// секундную сводку — без них «фриз» не отличить от «кадр дорогой»
/// (`plan/101-cef.md`, Ф6).
pub mod frame_perf {
    use std::sync::atomic::AtomicU32;

    /// Время layout+prepaint+paint (построение сцены), мкс.
    pub static DRAW_US: AtomicU32 = AtomicU32::new(0);
    /// Время презента (отправка сцены на ГПУ и Commit), мкс.
    pub static PRESENT_US: AtomicU32 = AtomicU32::new(0);
    /// Сколько кадров окна сложилось за период.
    pub static FRAMES: AtomicU32 = AtomicU32::new(0);
    /// Время фазы prepaint (раскладка), мкс.
    pub static PREPAINT_US: AtomicU32 = AtomicU32::new(0);
    /// Время построения карты попаданий мыши, мкс.
    pub static HITTEST_US: AtomicU32 = AtomicU32::new(0);
    /// Время фазы paint (сбор примитивов), мкс.
    pub static PAINT_US: AtomicU32 = AtomicU32::new(0);
    /// Узлов раскладки заведено за период.
    pub static LAYOUT_NODES: AtomicU32 = AtomicU32::new(0);
    /// Вызовов `compute_layout` (полных проходов раскладки) за период.
    pub static LAYOUT_RUNS: AtomicU32 = AtomicU32::new(0);
    /// Замеров содержимого (текст, чужие элементы) за период.
    pub static LAYOUT_MEASURES: AtomicU32 = AtomicU32::new(0);
    /// Время внутри самого taffy (`compute_layout`), мкс.
    pub static TAFFY_US: AtomicU32 = AtomicU32::new(0);
    /// Время внутри замеров содержимого, мкс.
    pub static MEASURE_US: AtomicU32 = AtomicU32::new(0);
    /// Кэшированных вью переиграно (не считалось заново) за период.
    pub static REUSED_VIEWS: AtomicU32 = AtomicU32::new(0);
    // KaminIDE patch (#76): промахи/попадания line-layout-кэша — диагноз
    // «пере-шейпинг текста каждый кадр» (prepaint 65-70мкс/узел).
    /// Строк ЗАШЕЙПЛЕНО заново (промах кэша) за период.
    pub static SHAPED_LINES: AtomicU32 = AtomicU32::new(0);
    /// Строк взято из кэша (текущий или прошлый кадр) за период.
    pub static CACHED_LINES: AtomicU32 = AtomicU32::new(0);
    /// Время шейпинга (промахов), мкс.
    pub static SHAPE_US: AtomicU32 = AtomicU32::new(0);
}
// KaminIDE patch (#76): наружу для диаг-строки шелла.
pub use element::prepaint_prof;
#[cfg(feature = "profiler")]
mod debug_overlay;
mod element;
mod elements;
mod executor;
mod platform_scheduler;
pub(crate) use platform_scheduler::PlatformScheduler;
mod geometry;
mod gestures;
mod global;
mod input;
mod inspector;
mod interactive;
mod key_dispatch;
mod keymap;
mod path_builder;
mod platform;
pub mod prelude;
/// Profiling utilities for task, frame, and thread performance tracking.
pub mod profiler;
#[cfg(any(
    test,
    target_os = "windows",
    target_os = "linux",
    target_family = "wasm",
    feature = "test-support",
    feature = "bench-support"
))]
#[expect(missing_docs)]
pub mod queue;
/// A seeded, mutable element tree for benchmarks and tests of GPUI rendering.
#[cfg(any(test, feature = "bench-support"))]
pub mod randomized_element_tree;
mod scene;
#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
mod seeds;
mod shared_uri;
mod spring;
mod style;
mod styled;
mod subscription;
mod svg_renderer;
mod tab_stop;
/// KaminIDE patch: GPU readback of drawn frames (WPT runner).
pub mod frame_capture;
mod taffy;
#[cfg(any(test, feature = "test-support"))]
pub mod test;
mod text_system;
mod util;
mod view;
mod window;

#[cfg(any(test, feature = "test-support"))]
pub use proptest;
#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
pub use seeds::calculate_seeds;

#[cfg(doc)]
pub mod _accessibility;
#[cfg(doc)]
pub mod _ownership_and_data_flow;

/// Do not touch, here be dragons for use by gpui_macros and such.
#[doc(hidden)]
pub mod private {
    pub use anyhow;
    pub use inventory;
    pub use rand;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}

mod seal {
    /// A mechanism for restricting implementations of a trait to only those in GPUI.
    /// See: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
    pub trait Sealed {}
}

pub use accesskit;
pub use accesskit::Action as AccessibleAction;
pub use accesskit::{Orientation, Role, Toggled};
pub use action::*;
pub use anyhow::Result;
pub use app::*;
pub(crate) use arena::*;
pub use asset_cache::*;
pub use assets::*;
pub use color::*;
pub use ctor::ctor;
#[cfg(feature = "profiler")]
pub use debug_overlay::*;
pub use element::*;
pub use elements::*;
pub use executor::*;
pub use geometry::*;
pub use gestures::*;
pub use global::*;
pub use gpui_macros::{
    AppContext, IntoElement, Render, VisualContext, bench, property_test, register_action, test,
};
pub use spring::*;

/// Defines a Criterion benchmark group for benchmarks annotated with [`gpui::bench`].
///
/// This mirrors `criterion::criterion_group!`, but the group measures with the
/// `gpui::BenchMeasurement` configured by `BENCH_MEASUREMENT` (see
/// `BenchMeasurement::from_env`). By default Criterion analyzes wall time
/// while retired instructions, cycles, IPC, context switches, and the other
/// counters this machine supports are printed per iteration in the GPUI bench
/// report. `BENCH_MEASUREMENT=instructions` makes Criterion analyze
/// process-wide instructions instead, `foreground-instructions` the benchmark
/// thread's alone, and `wall-time` disables all counters. A
/// `config = ...` expression may set any other Criterion option; its
/// measurement is replaced. To measure with something else, call
/// `criterion::criterion_group!` directly with
/// `config = criterion::Criterion::default().with_measurement(gpui::BenchMeasurement::new(...))`.
///
/// [`gpui::bench`]: crate::bench
#[macro_export]
macro_rules! bench_group {
    (name = $name:ident; config = $config:expr; targets = $($target:path),+ $(,)?) => {
        criterion::criterion_group! {
            name = $name;
            config = ($config).with_measurement($crate::BenchMeasurement::from_env_or_exit());
            targets = $($target),+
        }
    };
    ($name:ident, $($target:path),+ $(,)?) => {
        $crate::bench_group! {
            name = $name;
            config = criterion::Criterion::default();
            targets = $($target),+
        }
    };
}

/// Defines the entry point for GPUI Criterion benchmark groups.
///
/// This mirrors `criterion::criterion_main!` so GPUI benchmark files can keep the
/// same shape as ordinary Criterion benchmarks. It also installs
/// [`CountingAllocator`] as the binary's global allocator, so reports include
/// heap allocations per iteration. It wraps the system allocator by default;
/// `allocator = Path;` wraps another unit-struct allocator instead, e.g. to
/// compare allocators, which pay the same small counting cost:
///
/// ```ignore
/// gpui::bench_main!(allocator = mimalloc::MiMalloc; benches);
/// ```
///
/// `allocator = none;` installs no global allocator, leaving Rust's default and
/// dropping the allocation metrics from reports.
#[macro_export]
macro_rules! bench_main {
    (allocator = none; $($groups:tt)*) => {
        criterion::criterion_main!($($groups)*);
    };
    (allocator = $allocator:path; $($groups:tt)*) => {
        #[global_allocator]
        static GPUI_BENCH_ALLOCATOR: $crate::CountingAllocator<$allocator> =
            $crate::CountingAllocator::new($allocator);

        criterion::criterion_main!($($groups)*);
    };
    ($($groups:tt)*) => {
        $crate::bench_main!(allocator = ::std::alloc::System; $($groups)*);
    };
}
pub use gpui_shared_string::*;
pub use gpui_util::arc_cow::ArcCow;
pub use http_client;
pub use input::*;
pub use inspector::*;
pub use interactive::*;
use key_dispatch::*;
pub use keymap::*;
pub use path_builder::*;
pub use platform::*;
pub use profiler::*;
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
pub use queue::{PriorityQueueReceiver, PriorityQueueSender};
pub use refineable::*;
pub use scene::*;
pub use shared_uri::*;
use std::{any::Any, future::Future};
pub use style::*;
pub use styled::*;
pub use subscription::*;
pub use svg_renderer::*;
// KaminIDE patch: растеризация SVG из строки — нужна крейту разметки (функции
// модуля публичны и попадают в glob выше); FontStretch и замер — для html.
pub use text_system::FontStretch;
pub(crate) use tab_stop::*;
use taffy::TaffyLayoutEngine;
pub use taffy::{AvailableSpace, LayoutId, LayoutMeasurement, MeasuredContent};
#[cfg(any(test, feature = "test-support"))]
pub use test::*;
pub use text_system::*;
pub use util::{FutureExt, Timeout};
pub use view::*;
pub use window::*;

#[cfg(not(target_family = "wasm"))]
pub use pollster::block_on;

/// The context trait, allows the different contexts in GPUI to be used
/// interchangeably for certain operations.
pub trait AppContext {
    /// Create a new entity in the app context.
    #[expect(
        clippy::wrong_self_convention,
        reason = "`App::new` is an ubiquitous function for creating entities"
    )]
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>;

    /// Reserve a slot for a entity to be inserted later.
    /// The returned [Reservation] allows you to obtain the [EntityId] for the future entity.
    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T>;

    /// Insert a new entity in the app context based on a [Reservation] previously obtained from [`reserve_entity`].
    ///
    /// [`reserve_entity`]: Self::reserve_entity
    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T>;

    /// Update a entity in the app context.
    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static;

    /// Update a entity in the app context.
    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static;

    /// Read a entity from the app context.
    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static;

    /// Update a window for the given handle.
    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T;

    /// Run `f` against the entity's *current* window — the most recently
    /// rendered window that referenced the entity. Returns `None` if the
    /// entity has no current window or that window is unavailable. See
    /// [`App::with_window`] for the underlying lookup.
    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R>;

    /// Read a window off of the application context.
    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static;

    /// Spawn a future on a background thread
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static;

    /// Read a global from this app context
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global;
}

/// Returned by [Context::reserve_entity] to later be passed to [Context::insert_entity].
/// Allows you to obtain the [EntityId] for a entity before it is created.
pub struct Reservation<T>(pub(crate) Slot<T>);

impl<T: 'static> Reservation<T> {
    /// Returns the [EntityId] that will be associated with the entity once it is inserted.
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }
}

/// This trait is used for the different visual contexts in GPUI that
/// require a window to be present.
pub trait VisualContext: AppContext {
    /// The result type for window operations.
    type Result<T>;

    /// Returns the handle of the window associated with this context.
    fn window_handle(&self) -> AnyWindowHandle;

    /// Update a view with the given callback
    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Self::Result<R>;

    /// Create a new entity, with access to `Window`.
    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// Replace the root view of a window with a new view.
    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Self::Result<Entity<V>>
    where
        V: 'static + Render;

    /// Focus a entity in the window, if it implements the [`Focusable`] trait.
    fn focus<V>(&mut self, entity: &Entity<V>) -> Self::Result<()>
    where
        V: Focusable;
}

/// A trait for tying together the types of a GPUI entity and the events it can
/// emit.
pub trait EventEmitter<E: Any>: 'static {}

/// A helper trait for auto-implementing certain methods on contexts that
/// can be used interchangeably.
pub trait BorrowAppContext {
    /// Set a global value on the context.
    fn set_global<T: Global>(&mut self, global: T);
    /// Updates the global state of the given type.
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global;
    /// Updates the global state of the given type, creating a default if it didn't exist before.
    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default;
}

impl<C> BorrowAppContext for C
where
    C: std::borrow::BorrowMut<App>,
{
    fn set_global<G: Global>(&mut self, global: G) {
        self.borrow_mut().set_global(global)
    }

    #[track_caller]
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global,
    {
        let mut global = self.borrow_mut().lease_global::<G>();
        let result = f(&mut global, self);
        self.borrow_mut().end_global_lease(global);
        result
    }

    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default,
    {
        self.borrow_mut().default_global::<G>();
        self.update_global(f)
    }
}

/// Information about the GPU GPUI is running on.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GpuSpecs {
    /// Whether the GPU is really a fake (like `llvmpipe`) running on the CPU.
    pub is_software_emulated: bool,
    /// The name of the device, as reported by Vulkan.
    pub device_name: String,
    /// The name of the driver, as reported by Vulkan.
    pub driver_name: String,
    /// Further information about the driver, as reported by Vulkan.
    pub driver_info: String,
}
