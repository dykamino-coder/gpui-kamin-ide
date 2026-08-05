//! Счётчики пути кадра: где он теряется между Chromium и экраном.
//!
//! Без них «зависла текстура» не отличить от «кадр не пришёл»: и то, и другое
//! выглядит как застывшая панель. Раз в секунду строка в лог — только если
//! что-то происходило.

use std::sync::atomic::{AtomicU32, Ordering};

/// Кадры от Chromium (`on_accelerated_paint`).
static FRAMES: AtomicU32 = AtomicU32::new(0);
/// Отрисовки кадра в окно (наш `paint_external_texture`).
static PAINTS: AtomicU32 = AtomicU32::new(0);
/// Заказы перерисовки, отданные gpui.
static REFRESH: AtomicU32 = AtomicU32::new(0);
/// Кадр не удалось забрать: производитель держал общую текстуру.
static BUSY: AtomicU32 = AtomicU32::new(0);
/// Кадр выброшен: номер буфера указывал на текстуру другого размера.
static MISMATCH: AtomicU32 = AtomicU32::new(0);
/// Кадры интерфейса: сколько раз gpui перерисовал окно.
static DRAWS: AtomicU32 = AtomicU32::new(0);
/// Сумма и максимум времени построения кадра интерфейса, мкс.
static DRAW_US: AtomicU32 = AtomicU32::new(0);
static DRAW_MAX: AtomicU32 = AtomicU32::new(0);
/// Время добычи текстур страниц (открыть + копия + атлас), мкс.
static TEX_US: AtomicU32 = AtomicU32::new(0);
/// Строк дерева файлов в последнем кадре.
static ROWS: AtomicU32 = AtomicU32::new(0);
/// Время подготовки состояния кадра (`frame_ctx`), мкс.
static CTX_US: AtomicU32 = AtomicU32::new(0);
/// Отрисовка обрезанным кадром: Chromium ещё не догнал размер панели.
static CROPPED: AtomicU32 = AtomicU32::new(0);
/// Кадр отрисован С МАСШТАБИРОВАНИЕМ (пиксели пересэмплены) — цель довести
/// до нуля: «никогда не должно быть недорастянувшейся текстуры».
static STRETCHED: AtomicU32 = AtomicU32::new(0);
/// Кадр отрисован пиксель-в-пиксель. Инвариант: exact + stretched == paints.
static EXACT: AtomicU32 = AtomicU32::new(0);

/// Смерти renderer-процессов за секунду: видно в общей строке, чтобы «панель
/// застыла» не приходилось искать по чужим логам.
static RENDERER_DIED: AtomicU32 = AtomicU32::new(0);

pub(crate) fn renderer_died() {
    RENDERER_DIED.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn stretched() {
    STRETCHED.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn exact() {
    EXACT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn frame() {
    FRAMES.fetch_add(1, Ordering::Relaxed);
}
static SW_PAINTS: AtomicU32 = AtomicU32::new(0);

// #76, ввод 1.5с на слабом RDP: разложить sw-путь по цифрам. Копия кадра
// (frames::put), покрытие dirty-областей (CEF говорит, ЧТО изменилось — мы
// пока перезаливаем всё) и интервал прихода кадров от Chromium.
static SW_PUT_US: AtomicU32 = AtomicU32::new(0);
static SW_DIRTY_PCT_SUM: AtomicU32 = AtomicU32::new(0);
static SW_GAP_MS_SUM: AtomicU32 = AtomicU32::new(0);
static SW_LAST_PAINT_MS: AtomicU32 = AtomicU32::new(0);

pub(crate) fn sw_put(dur_us: u32, dirty_pct: u32, now_ms: u32) {
    SW_PUT_US.fetch_add(dur_us, Ordering::Relaxed);
    SW_DIRTY_PCT_SUM.fetch_add(dirty_pct, Ordering::Relaxed);
    let prev = SW_LAST_PAINT_MS.swap(now_ms, Ordering::Relaxed);
    if prev != 0 && now_ms > prev {
        SW_GAP_MS_SUM.fetch_add((now_ms - prev).min(10_000), Ordering::Relaxed);
    }
}

/// Отрисовка software-кадра (RDP-режим) — видно в строке диагностики.
pub(crate) fn sw_paint() {
    SW_PAINTS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn paint() {
    PAINTS.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn refresh() {
    REFRESH.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn busy() {
    BUSY.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn mismatch() {
    MISMATCH.fetch_add(1, Ordering::Relaxed);
}
pub fn drawn() {
    DRAWS.fetch_add(1, Ordering::Relaxed);
}

/// Сколько строк дерева построено в кадре.
pub fn rows_built(n: u32) {
    ROWS.store(n, Ordering::Relaxed);
}

/// Сколько заняла добыча текстуры страницы.
pub(crate) fn tex_took(us: u32) {
    TEX_US.fetch_add(us, Ordering::Relaxed);
}

/// Сколько заняла подготовка состояния кадра.
pub fn ctx_took(us: u32) {
    CTX_US.fetch_add(us, Ordering::Relaxed);
}

/// Сколько заняло построение кадра интерфейса (зовётся из `RootView::render`).
pub fn draw_took(us: u32) {
    DRAW_US.fetch_add(us, Ordering::Relaxed);
    DRAW_MAX.fetch_max(us, Ordering::Relaxed);
}
pub(crate) fn cropped() {
    CROPPED.fetch_add(1, Ordering::Relaxed);
}

/// Начало текущей секунды отсчёта.
static SINCE: std::sync::LazyLock<std::sync::Mutex<std::time::Instant>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::time::Instant::now()));

/// Напечатать сводку, если прошла секунда. Зовётся из насоса.
pub(crate) fn report() {
    {
        let Ok(mut since) = SINCE.lock() else { return };
        if since.elapsed().as_millis() < 1000 {
            return;
        }
        *since = std::time::Instant::now();
    }
    let f = FRAMES.swap(0, Ordering::Relaxed);
    let p = PAINTS.swap(0, Ordering::Relaxed);
    let r = REFRESH.swap(0, Ordering::Relaxed);
    let b = BUSY.swap(0, Ordering::Relaxed);
    let m = MISMATCH.swap(0, Ordering::Relaxed);
    let c = CROPPED.swap(0, Ordering::Relaxed);
    let d = DRAWS.swap(0, Ordering::Relaxed);
    let us = DRAW_US.swap(0, Ordering::Relaxed);
    let mx = DRAW_MAX.swap(0, Ordering::Relaxed);
    let avg = us.checked_div(d).unwrap_or(0);
    let ctx = CTX_US.swap(0, Ordering::Relaxed);
    let tex = TEX_US.swap(0, Ordering::Relaxed) / 1000;
    let rows = ROWS.load(Ordering::Relaxed);
    // Стоимость кадра ОКНА — из счётчиков gpui (patch `frame_perf`).
    let gd = gpui::frame_perf::DRAW_US.swap(0, Ordering::Relaxed) / 1000;
    let gp = gpui::frame_perf::PRESENT_US.swap(0, Ordering::Relaxed) / 1000;
    let gf = gpui::frame_perf::FRAMES.swap(0, Ordering::Relaxed);
    let g_pre = gpui::frame_perf::PREPAINT_US.swap(0, Ordering::Relaxed) / 1000;
    let g_hit = gpui::frame_perf::HITTEST_US.swap(0, Ordering::Relaxed) / 1000;
    let g_pnt = gpui::frame_perf::PAINT_US.swap(0, Ordering::Relaxed) / 1000;
    let g_nodes = gpui::frame_perf::LAYOUT_NODES.swap(0, Ordering::Relaxed);
    let g_runs = gpui::frame_perf::LAYOUT_RUNS.swap(0, Ordering::Relaxed);
    let g_meas = gpui::frame_perf::LAYOUT_MEASURES.swap(0, Ordering::Relaxed);
    let per = g_nodes.checked_div(gf).unwrap_or(0);
    let per_meas = g_meas.checked_div(gf).unwrap_or(0);
    let g_taffy = gpui::frame_perf::TAFFY_US.swap(0, Ordering::Relaxed) / 1000;
    let g_measus = gpui::frame_perf::MEASURE_US.swap(0, Ordering::Relaxed) / 1000;
    let g_reused = gpui::frame_perf::REUSED_VIEWS.swap(0, Ordering::Relaxed);
    // #76, sw-путь: средняя цена копии кадра, покрытие dirty, интервал кадров.
    let sw_put = SW_PUT_US.swap(0, Ordering::Relaxed);
    let sw_dirty = SW_DIRTY_PCT_SUM.swap(0, Ordering::Relaxed);
    let sw_gap = SW_GAP_MS_SUM.swap(0, Ordering::Relaxed);
    if f > 0 && sw_put > 0 {
        emit_line(format!(
            "[sw] кадров {f}: копия в среднем {} мкс, dirty в среднем {}%, интервал {} мс",
            sw_put / f,
            sw_dirty / f,
            sw_gap / f.max(1)
        ));
    }
    // #76: попадания/промахи кэша шейпинга строк.
    let g_shaped = gpui::frame_perf::SHAPED_LINES.swap(0, Ordering::Relaxed);
    let g_cachedl = gpui::frame_perf::CACHED_LINES.swap(0, Ordering::Relaxed);
    let g_shape_ms = gpui::frame_perf::SHAPE_US.swap(0, Ordering::Relaxed) / 1000;
    let ctx_avg = ctx.checked_div(d).unwrap_or(0);
    let stretched = STRETCHED.swap(0, Ordering::Relaxed);
    let exact = EXACT.swap(0, Ordering::Relaxed);
    let died = RENDERER_DIED.swap(0, Ordering::Relaxed);
    if died > 0 {
        emit_line(format!(
            "[cef] УМЕРЛО renderer-процессов за секунду: {died} (подробности в crash.log)"
        ));
    }
    emit_line(format!(
        "[cef] за секунду: кадров {f}, кадров окна {d}, отрисовок {p}, заказов {r}, занято {b}, не тот размер {m}, обрезано {c}, растянуто {stretched}, точно {exact}, кадр окна в среднем {avg} мкс, худший {mx} мкс, из них состояние {ctx_avg} мкс; текстуры {tex} мс, строк дерева {rows}; окно сложило {gf} кадров: сцена {gd} мс, презент {gp} мс (раскладка {g_pre} мс, попадания {g_hit} мс, рисование {g_pnt} мс); узлов на кадр {per}, замеров на кадр {per_meas}, проходов {g_runs}; taffy {g_taffy} мс, замеры {g_measus} мс; переиграно вью {g_reused}; шейпинг: промахов {g_shaped} ({g_shape_ms} мс), из кэша {g_cachedl}"
    ));
    // #76: топ self-time prepaint по типам элементов (KAMIN_PREPAINT_PROF=1).
    let top = gpui::prepaint_prof::take_top(6);
    if !top.is_empty() {
        let line: Vec<String> = top
            .iter()
            .map(|(name, us, cnt)| format!("{name} {} мс/{cnt}", us / 1000))
            .collect();
        emit_line(format!("[prepaint] {}", line.join(", ")));
    }
}

/// Диаг-строка: в stdout (dev, стенды) И в файл `cache/diag.log` — packaged
/// GUI-сборка без консоли иначе теряет телеметрию, а именно с прод-машины
/// юзера нужны цифры (#76: «задержка неприятная», слабый RDP-комп). Ротация:
/// перерос лимит — начали файл заново.
fn emit_line(line: String) {
    println!("{line}");
    use std::io::Write as _;
    const ROTATE_BYTES: u64 = 5 * 1024 * 1024;
    static FILE: std::sync::LazyLock<std::sync::Mutex<Option<std::fs::File>>> =
        std::sync::LazyLock::new(|| {
            let path = crate::host::paths::data_dirs().1.join("diag.log");
            let rotate = std::fs::metadata(&path)
                .map(|m| m.len() > ROTATE_BYTES)
                .unwrap_or(false);
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(!rotate)
                .truncate(rotate)
                .write(true)
                .open(path)
                .ok();
            // Версия и старт сессии — иначе по присланному логу не отличить,
            // какая сборка реально запущена (инцидент «не вижу изменений»).
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "[boot] KaminIDE {}", env!("CARGO_PKG_VERSION"));
            }
            std::sync::Mutex::new(file)
        });
    if let Ok(mut guard) = FILE.lock()
        && let Some(f) = guard.as_mut()
    {
        let _ = writeln!(f, "{line}");
    }
}
